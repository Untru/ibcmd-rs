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
mod config_rows;
mod dcs;
mod fetch;
mod form_body;
mod forms;
mod metadata;
mod refs;
mod role_rights;
mod selected;
mod source_assets;
mod timing;

use command_interface::*;
use config_rows::*;
use dcs::*;
use fetch::*;
use form_body::*;
use forms::*;
use metadata::*;
use refs::*;
use role_rights::*;
use selected::*;
use source_assets::*;

pub(crate) use form_body::{extract_form_body_xml, unpack_form_body_module_text};

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
    object_refs: &'a BTreeMap<String, String>,
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
    let type_index = if extract_metadata_xml || needs_source_layout_refs {
        build_metadata_type_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let refs_for_standalone =
        extract_metadata_xml || needs_standalone_refs || needs_source_layout_refs;
    let form_refs = if refs_for_standalone {
        build_form_source_reference_index_from_texts(&metadata_texts)
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
    let body_owners = if extract_metadata_xml {
        build_body_owner_source_index_from_texts(&metadata_texts, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    let configuration_module_groups = configuration_module_groups(&file_names_owned);
    ensure_unique_source_asset_paths(&source_assets)?;

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
        object_refs: &object_refs,
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
    output_dir: &Path,
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
        selected_configuration_source_asset_index_needs(&file_names);
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
    let type_index = if (extract_metadata_xml || needs_source_layout_refs)
        && (source_reference_needs.type_index || build_selected_local_refs)
    {
        build_metadata_type_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_type_index_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let form_refs = if (extract_metadata_xml
        && (source_reference_needs.form_refs
            || source_reference_needs.object_refs
            || build_selected_local_refs))
        || needs_standalone_refs
    {
        build_form_source_reference_index_from_texts(&index_metadata_texts)
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
    let body_owners = if extract_metadata_xml
        && (source_reference_needs.body_owners || build_selected_local_refs)
    {
        build_body_owner_source_index_from_texts(&index_metadata_texts, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    timings.prepare_body_owners_ms += elapsed_ms(index_part_started);
    let configuration_module_groups = configuration_module_groups(&file_names);
    ensure_unique_source_asset_paths(&source_assets)?;
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
        object_refs: &object_refs,
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
            metadata_text_row_from_blob(&row.file_name, &bytes)
        })
        .collect()
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
                context.functional_option_refs,
                context.form_refs,
                context.template_refs,
                context.subsystem_refs,
                context.source_version,
            )
        } else {
            extract_metadata_source_xml_with_refs(
                &bytes,
                file_name,
                context.type_index,
                context.object_refs,
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

struct MoxelSpreadsheet {
    column_count: usize,
    column_sets: Vec<MoxelColumnSet>,
    column_formats: Vec<MoxelFormat>,
    extra_formats: BTreeMap<usize, MoxelFormat>,
    default_format_width: Option<usize>,
    default_format: MoxelFormat,
    formats: Vec<MoxelFormat>,
    rows: Vec<MoxelRow>,
    vertical_groups: Vec<MoxelVerticalGroup>,
    merges: Vec<MoxelMerge>,
    horizontal_unmerges: Vec<MoxelMerge>,
    vertical_unmerges: Vec<MoxelMerge>,
    named_items: Vec<MoxelNamedItem>,
    areas: Vec<MoxelArea>,
    print_area: Option<MoxelArea>,
    print_settings: Option<MoxelPrintSettings>,
    lines: Vec<MoxelLine>,
    fonts: Vec<MoxelFont>,
    drawings: Vec<MoxelDrawing>,
    pictures: Vec<MoxelPicture>,
    empty_headers_footers: bool,
    header_footer_format_index: Option<usize>,
    default_format_index: Option<usize>,
    height: usize,
}

#[derive(Clone)]
struct MoxelRow {
    index: usize,
    index_to: Option<usize>,
    format_index: usize,
    source_format_index: Option<usize>,
    columns_id: Option<String>,
    cells: Vec<MoxelCell>,
}

struct MoxelColumnSet {
    id: Option<String>,
    default_format_index: Option<usize>,
    source_default_format_index: Option<usize>,
    size: usize,
    columns: Vec<MoxelColumn>,
}

struct MoxelColumn {
    index: i32,
    format_index: usize,
    source_format_index: Option<usize>,
}

#[derive(Clone)]
struct MoxelCell {
    column_index: usize,
    format_index: usize,
    source_format_index: Option<usize>,
    text: Option<String>,
    parameter: Option<String>,
    detail_parameter: Option<String>,
    empty_text: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct MoxelLocalizedValue {
    lang: String,
    content: String,
}

#[derive(Clone)]
enum MoxelNamedItem {
    Cells(MoxelArea),
    Drawing { name: String, drawing_id: usize },
}

#[derive(Clone)]
struct MoxelArea {
    name: String,
    area_type: &'static str,
    begin_row: i32,
    end_row: i32,
    begin_column: i32,
    end_column: i32,
    columns_id: Option<String>,
}

struct MoxelVerticalGroup {
    begin_row: usize,
    end_row: usize,
    level: usize,
}

#[derive(Clone)]
struct MoxelMerge {
    row: i32,
    column: i32,
    height: i32,
    width: i32,
    columns_id: Option<String>,
}

struct MoxelFont {
    ref_name: Option<String>,
    face_name: Option<String>,
    height: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    kind: &'static str,
    scale: Option<usize>,
}

struct MoxelLine {
    style: &'static str,
    line_type: &'static str,
    width: usize,
}

struct MoxelDrawing {
    id: usize,
    format_index: usize,
    begin_row: i32,
    begin_row_offset: i32,
    end_row: i32,
    end_row_offset: i32,
    begin_column: i32,
    begin_column_offset: i32,
    end_column: i32,
    end_column_offset: i32,
    auto_size: bool,
    picture_size: &'static str,
    z_order: usize,
    picture_index: usize,
}

struct MoxelPicture {
    index: usize,
    ref_name: Option<String>,
    payload: Option<String>,
}

#[derive(Clone, Default)]
struct MoxelPrintSettings {
    page_orientation: Option<&'static str>,
    scale: Option<usize>,
    collate: Option<bool>,
    copies: Option<usize>,
    per_page: Option<usize>,
    top_margin: Option<usize>,
    left_margin: Option<usize>,
    bottom_margin: Option<usize>,
    right_margin: Option<usize>,
    header_size: Option<usize>,
    footer_size: Option<usize>,
    fit_to_page: Option<bool>,
    black_and_white: Option<bool>,
    printer_name: Option<String>,
    paper: Option<usize>,
    paper_source: Option<usize>,
    page_width: Option<usize>,
    page_height: Option<usize>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct MoxelFormat {
    font: Option<usize>,
    border: Option<usize>,
    left_border: Option<usize>,
    top_border: Option<usize>,
    right_border: Option<usize>,
    bottom_border: Option<usize>,
    height: Option<i32>,
    border_color: Option<String>,
    width: Option<usize>,
    width_weight_factor: Option<usize>,
    horizontal_alignment: Option<&'static str>,
    vertical_alignment: Option<&'static str>,
    back_color: Option<String>,
    pattern: Option<&'static str>,
    text_color: Option<String>,
    text_placement: Option<&'static str>,
    text_orientation: Option<usize>,
    fill_type: Option<&'static str>,
    number_format_present: bool,
    number_format: Vec<MoxelLocalizedValue>,
    edit_format_present: bool,
    edit_format: Vec<MoxelLocalizedValue>,
    drawing_border: Option<usize>,
    by_selected_columns: Option<bool>,
    details_use: Option<&'static str>,
    hyper_link: Option<bool>,
    protection: Option<bool>,
    hidden: Option<bool>,
    indent: Option<usize>,
    auto_indent: Option<usize>,
    mask: Option<&'static str>,
    pic_index: Option<usize>,
    picture_size_mode: Option<&'static str>,
    pic_horizontal_alignment: Option<&'static str>,
    pic_vertical_alignment: Option<&'static str>,
    text_position: Option<&'static str>,
}

impl MoxelFormat {
    fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.border.is_none()
            && self.left_border.is_none()
            && self.top_border.is_none()
            && self.right_border.is_none()
            && self.bottom_border.is_none()
            && self.height.is_none()
            && self.border_color.is_none()
            && self.width.is_none()
            && self.width_weight_factor.is_none()
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.back_color.is_none()
            && self.pattern.is_none()
            && self.text_color.is_none()
            && self.text_placement.is_none()
            && self.text_orientation.is_none()
            && self.fill_type.is_none()
            && !self.number_format_present
            && self.number_format.is_empty()
            && !self.edit_format_present
            && self.edit_format.is_empty()
            && self.drawing_border.is_none()
            && self.by_selected_columns.is_none()
            && self.details_use.is_none()
            && self.hyper_link.is_none()
            && self.protection.is_none()
            && self.hidden.is_none()
            && self.indent.is_none()
            && self.auto_indent.is_none()
            && self.mask.is_none()
            && self.pic_index.is_none()
            && self.picture_size_mode.is_none()
            && self.pic_horizontal_alignment.is_none()
            && self.pic_vertical_alignment.is_none()
            && self.text_position.is_none()
    }
}

fn normalize_moxel_default_match_format(mut format: MoxelFormat) -> MoxelFormat {
    if format.font == Some(0) {
        format.font = None;
    }
    format
}

fn resolve_existing_moxel_default_format_index(
    column_formats: &[MoxelFormat],
    formats: &[MoxelFormat],
    default_format: &MoxelFormat,
    default_format_width: Option<usize>,
) -> Option<(usize, bool)> {
    let all_formats = column_formats
        .iter()
        .chain(formats.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut target = default_format.clone();
    if target.width.is_none() {
        target.width = default_format_width;
    }
    if target.is_empty() {
        return None;
    }
    let preferred_target_exact = if default_format.is_empty() && default_format_width.is_some() {
        Some(MoxelFormat {
            font: Some(0),
            width: default_format_width,
            ..MoxelFormat::default()
        })
    } else {
        None
    };
    let target_exact = target.clone();
    let target_normalized = normalize_moxel_default_match_format(target);
    let last_exact_match = |target: &MoxelFormat| {
        all_formats
            .iter()
            .enumerate()
            .filter_map(|(index, format)| (format == target).then_some(index + 1))
            .last()
    };
    preferred_target_exact
        .as_ref()
        .and_then(|target| last_exact_match(target).map(|index| (index, true)))
        .or_else(|| last_exact_match(&target_exact).map(|index| (index, false)))
        .or_else(|| {
            all_formats
                .iter()
                .enumerate()
                .filter_map(|(index, format)| {
                    (normalize_moxel_default_match_format(format.clone()) == target_normalized)
                        .then_some((index + 1, false))
                })
                .last()
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

fn parse_business_process_flowchart_blob(bytes: &[u8]) -> Option<BusinessProcessFlowchart> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    parse_business_process_flowchart_text(text.trim_start_matches('\u{feff}'))
}

fn parse_business_process_flowchart_text(text: &str) -> Option<BusinessProcessFlowchart> {
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
            z_order.to_string(),
        )?);
    }

    Some(BusinessProcessFlowchart { items })
}

fn parse_flowchart_item(
    code: &str,
    body: &str,
    names: &BTreeMap<String, String>,
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
    let uuid = if matches!(code, "2" | "3" | "4" | "5" | "7" | "8" | "9") {
        head.get(2).map(|value| value.trim().to_string())
    } else {
        None
    }
    .filter(|value| is_uuid_text(value));
    let base_fields = if matches!(code, "2" | "3" | "4" | "5" | "7" | "8" | "9") {
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
        "Start" | "Activity" | "Condition" | "Completion" | "Processing" | "Split" | "Join"
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

pub(crate) fn extract_moxel_spreadsheet_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    if !inflated.starts_with(b"MOXCEL") {
        return None;
    }
    let text = String::from_utf8(inflated).ok()?;
    let body_start = text.find("{8,")?;
    let spreadsheet = parse_moxel_spreadsheet_text(&text[body_start..], object_refs)?;
    Some(format_moxel_spreadsheet_xml(&spreadsheet))
}

fn parse_moxel_spreadsheet_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelSpreadsheet> {
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "8" {
        return None;
    }
    let declared_column_count = fields.get(2)?.trim().parse::<usize>().ok()? + 1;
    let mut rows = parse_moxel_rows(&fields);
    if rows.is_empty() {
        return None;
    }
    let vertical_groups = parse_moxel_vertical_groups(&fields);
    let (merges, horizontal_unmerges, vertical_unmerges) = parse_moxel_merge_regions(&fields);
    let named_items = parse_moxel_named_items(&fields);
    let areas = named_items
        .iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area.clone()),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect::<Vec<_>>();
    let print_area = parse_moxel_print_area(&fields);
    trim_moxel_trailing_empty_rows(
        &mut rows,
        &areas,
        &merges,
        &horizontal_unmerges,
        &vertical_unmerges,
    );
    compact_moxel_empty_row_ranges(&mut rows);
    let (column_sets, row_column_ids, declared_sheet_height) = parse_moxel_column_sets(&fields);
    let fonts = parse_moxel_fonts(&fields);
    let pictures = parse_moxel_pictures(&fields, object_refs);
    let style_refs = parse_moxel_style_refs(&fields, object_refs);
    let mut default_format = parse_moxel_default_format(&fields, object_refs);
    let print_settings = parse_moxel_print_settings(&fields);
    let empty_headers_footers = parse_moxel_empty_headers_footers(&fields);
    let header_footer_format_ref = parse_moxel_uniform_header_footer_format_ref(&fields);
    let observed_column_count = rows
        .iter()
        .flat_map(|row| row.cells.iter().map(|cell| cell.column_index + 1))
        .max()
        .unwrap_or(0);
    let column_count = if observed_column_count > 0 {
        observed_column_count
    } else {
        declared_column_count
    };
    let column_sets = if column_sets.is_empty() {
        default_moxel_column_sets(column_count)
    } else {
        column_sets
    };
    let column_format_slots = moxel_column_format_slots(&column_sets, column_count);
    let source_column_format_refs = moxel_source_column_format_refs(&column_sets);
    let source_column_format_offset = moxel_source_column_format_offset(&column_sets);
    let has_high_source_column_format_refs = column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| column.source_format_index)
        .any(|source_format_index| source_format_index > column_format_slots);
    let needs_sparse_column_set_default_format =
        source_column_format_offset > 0 && header_footer_format_ref.is_some();
    let mut column_sets = column_sets;
    if source_column_format_offset == 0 && column_format_slots == 0 {
        normalize_moxel_zero_column_format_refs(&mut rows);
    }
    let mut default_format_width = parse_moxel_default_format_width(&fields, column_format_slots);
    let has_equal_width_only_format_table =
        parse_moxel_equal_width_only_format_table(&fields, column_count).is_some();
    let sparse_source_format_refs = moxel_uses_sparse_source_format_refs(
        &column_sets,
        column_count,
        &rows,
        &default_format,
        default_format_width,
    );
    if sparse_source_format_refs
        && has_high_source_column_format_refs
        && source_column_format_refs.len() > 1
        && default_format_width.is_some()
        && default_format.border_color.is_none()
        && default_format.is_empty()
        && style_refs.first().and_then(|slot| slot.as_deref()) == Some("style:BorderColor")
    {
        default_format.font = Some(0);
        default_format.border_color = Some("style:BorderColor".to_string());
    }
    let format_offset = if sparse_source_format_refs || has_equal_width_only_format_table {
        0
    } else {
        column_format_slots.saturating_sub(1)
    };
    for row in &mut rows {
        if let Some(columns_id) = row_column_ids.get(&row.index) {
            row.columns_id = Some(columns_id.clone());
        }
        if source_column_format_offset == 0 {
            if row.format_index > 1 {
                row.format_index += format_offset;
            }
            for cell in &mut row.cells {
                if cell.format_index > 0 {
                    cell.format_index += format_offset;
                }
            }
        }
    }
    let height = moxel_spreadsheet_height(
        &rows,
        &merges,
        &horizontal_unmerges,
        &vertical_unmerges,
        &areas,
    )
    .max(declared_sheet_height.unwrap_or(0));
    let drawings = parse_moxel_drawings(&fields);
    let drawing_format_indices = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .collect::<BTreeSet<_>>();
    let number_format_refs = parse_moxel_number_format_refs(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
    );
    if default_format.is_empty() && default_format_width.is_none() {
        if let Some(leading_default_format) =
            parse_moxel_leading_default_format(&fields, &style_refs, &number_format_refs)
        {
            default_format_width = leading_default_format.width;
            default_format = leading_default_format;
        }
    }
    let (column_formats, formats) = parse_moxel_formats(
        &fields,
        column_format_slots,
        sparse_source_format_refs,
        &source_column_format_refs,
        &style_refs,
        &drawing_format_indices,
        &number_format_refs,
    );
    let (column_formats, mut formats) = (column_formats, formats);
    if source_column_format_offset == 0 && column_formats.is_empty() && formats.is_empty() {
        restore_moxel_source_format_refs_without_format_table(&mut rows);
    }
    if source_column_format_offset > 0 {
        if sparse_source_format_refs {
            remap_moxel_column_set_sparse_internal_format_indices(
                &mut column_sets,
                &source_column_format_refs,
                column_formats.len(),
                formats.len(),
            );
            remap_moxel_row_and_cell_sparse_internal_format_indices(
                &mut rows,
                &source_column_format_refs,
                column_formats.len(),
                formats.len(),
            );
        } else if column_formats.len() > source_column_format_refs.len()
            || needs_sparse_column_set_default_format
        {
            let source_output_indices = moxel_source_derived_internal_output_order(
                &column_sets,
                column_formats.len(),
                formats.len(),
            );
            remap_moxel_column_set_internal_format_indices(
                &mut column_sets,
                column_formats.len(),
                formats.len(),
            );
            remap_moxel_row_and_cell_sparse_source_format_indices(
                &mut rows,
                &source_column_format_refs,
                &source_output_indices,
            );
        } else {
            remap_moxel_column_set_output_format_indices(
                &mut column_sets,
                &source_column_format_refs,
            );
            remap_moxel_row_and_cell_output_format_indices(&mut rows, &source_column_format_refs);
        }
    } else if sparse_source_format_refs && !source_column_format_refs.is_empty() {
        remap_moxel_column_set_output_format_indices(&mut column_sets, &source_column_format_refs);
        remap_moxel_row_and_cell_output_format_indices(&mut rows, &source_column_format_refs);
    }
    let extra_formats = BTreeMap::new();
    let header_footer_format_index = if needs_sparse_column_set_default_format {
        resolve_sparse_moxel_column_set_default_format_index(
            &mut column_sets,
            column_formats.len(),
            &formats,
            header_footer_format_ref,
        )
    } else {
        None
    };
    let all_formats = column_formats
        .iter()
        .chain(formats.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut fonts = fonts;
    normalize_moxel_fonts(&mut fonts, &all_formats);
    let has_sparse_column_sets = column_sets
        .iter()
        .any(|column_set| column_set.columns.len() != column_set.size);
    let mut lines = parse_moxel_lines(&fields, &all_formats, has_sparse_column_sets);
    normalize_moxel_single_set_report_header_tail(
        &column_sets,
        &column_formats,
        &style_refs,
        &mut lines,
        &mut formats,
    );
    let drawing_max_format_index = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    let row_cell_max_format_index = rows.iter().fold(
        moxel_column_format_slots(&column_sets, column_count).max(1),
        |max_index, row| {
            let row_max = row.cells.iter().fold(row.format_index, |cell_max, cell| {
                cell_max.max(cell.format_index)
            });
            max_index.max(row_max)
        },
    );
    let max_format_index = row_cell_max_format_index.max(drawing_max_format_index);
    let format_table_fallback = column_formats.len() + formats.len() + 1;
    let mut default_format_index = moxel_default_format_index(
        &column_sets,
        print_settings.as_ref(),
        !default_format.is_empty() || default_format_width.is_some(),
        format_table_fallback.max(max_format_index + 1),
    );
    if default_format_index.is_some_and(|index| index > column_formats.len() + formats.len())
        && let Some((existing_index, exact_font_zero_match)) =
            resolve_existing_moxel_default_format_index(
                &column_formats,
                &formats,
                &default_format,
                default_format_width,
            )
    {
        default_format_index = Some(existing_index);
        if exact_font_zero_match && default_format.is_empty() {
            default_format.font = Some(0);
        }
    }
    if source_column_format_offset > 0
        && default_format.is_empty()
        && default_format_width.is_none()
        && column_formats.len() == source_column_format_refs.len()
        && source_column_format_refs
            .iter()
            .copied()
            .max()
            .is_some_and(|max_source_format_index| {
                max_source_format_index < column_formats.len() + formats.len()
            })
        && let Some(min_source_format_index) = source_column_format_refs.iter().copied().min()
        && min_source_format_index > 1
    {
        default_format_index = Some(column_formats.len() + min_source_format_index);
    }
    if header_footer_format_index.is_some()
        && default_format.is_empty()
        && default_format_width.is_none()
    {
        default_format_index = None;
    }
    if column_sets.len() == 1
        && let Some(shared_format_index) = header_footer_format_index
        && shared_format_index > column_formats.len()
        && let Some(shared_format) =
            moxel_internal_format(&column_formats, &formats, shared_format_index)
    {
        if shared_format.is_empty() {
            if default_format_index.is_none_or(|index| index <= column_formats.len()) {
                default_format_index = Some(shared_format_index);
            }
        } else {
            default_format_index = None;
        }
    }
    if column_sets.len() == 1
        && let Some(shared_format_index) = header_footer_format_index
        && shared_format_index > column_formats.len()
        && let Some(shared_format) =
            moxel_internal_format(&column_formats, &formats, shared_format_index)
        && shared_format.is_empty()
        && default_format_index.is_some_and(|index| index > shared_format_index)
        && let Some(default_set) = column_sets.first_mut()
    {
        default_set.default_format_index = Some(shared_format_index);
    }
    Some(MoxelSpreadsheet {
        column_count,
        column_sets,
        column_formats,
        extra_formats,
        default_format_width,
        default_format,
        formats,
        rows,
        vertical_groups,
        merges,
        horizontal_unmerges,
        vertical_unmerges,
        named_items,
        areas,
        print_area,
        print_settings,
        lines,
        fonts,
        drawings,
        pictures,
        empty_headers_footers,
        header_footer_format_index,
        default_format_index,
        height,
    })
}

fn normalize_moxel_fonts(fonts: &mut Vec<MoxelFont>, formats: &[MoxelFormat]) {
    let Some(max_used_index) = formats.iter().filter_map(|format| format.font).max() else {
        return;
    };
    if max_used_index != fonts.len() || fonts.is_empty() {
        return;
    }
    if fonts.iter().any(|font| font.kind == "StyleItem") {
        return;
    }
    if !fonts
        .last()
        .is_some_and(|font| font.kind == "Absolute" && font.ref_name.is_none())
    {
        return;
    }

    // Some MXL variants reference one implicit TextFont slot that is not present
    // in the raw font table. Native XML places it before the last explicit font.
    fonts.insert(
        fonts.len() - 1,
        MoxelFont {
            ref_name: Some("style:TextFont".to_string()),
            face_name: None,
            height: None,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            kind: "StyleItem",
            scale: None,
        },
    );
}

fn default_moxel_column_sets(column_count: usize) -> Vec<MoxelColumnSet> {
    vec![MoxelColumnSet {
        id: None,
        default_format_index: None,
        source_default_format_index: None,
        size: column_count,
        columns: (0..column_count)
            .map(|index| MoxelColumn {
                index: index as i32,
                format_index: index + 1,
                source_format_index: None,
            })
            .collect(),
    }]
}

fn parse_moxel_column_sets(
    fields: &[&str],
) -> (Vec<MoxelColumnSet>, BTreeMap<usize, String>, Option<usize>) {
    for index in 0..fields.len() {
        let Some(default_set) = parse_moxel_column_set(fields[index]) else {
            continue;
        };
        if default_set.id.is_some() || index + 2 >= fields.len() {
            continue;
        }
        let Some(declared_sheet_height) = fields
            .get(index + 1)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let Some(additional_count) = fields
            .get(index + 2)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if additional_count > 64 || index + 3 + additional_count >= fields.len() {
            continue;
        }

        let mut column_sets = vec![default_set];
        let mut cursor = index + 3;
        for _ in 0..additional_count {
            let Some(column_set) = parse_moxel_column_set(fields[cursor]) else {
                column_sets.clear();
                break;
            };
            if column_set.id.is_none() {
                column_sets.clear();
                break;
            }
            column_sets.push(column_set);
            cursor += 1;
        }
        if column_sets.is_empty() {
            continue;
        }
        normalize_moxel_column_set_format_indices(&mut column_sets);
        let row_column_ids =
            parse_moxel_row_column_set_ids(fields, cursor, &column_sets[1..]).unwrap_or_default();
        return (column_sets, row_column_ids, Some(declared_sheet_height));
    }
    (Vec::new(), BTreeMap::new(), None)
}

fn parse_moxel_vertical_groups(fields: &[&str]) -> Vec<MoxelVerticalGroup> {
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count == 0 || count > 2048 {
            continue;
        }
        let Some(last_group_field) = index.checked_add(count * 2) else {
            continue;
        };
        if last_group_field + 3 >= fields.len() {
            continue;
        }
        let mut groups = Vec::with_capacity(count);
        let mut cursor = index + 1;
        let mut valid = true;
        for _ in 0..count {
            let Some(group) = fields
                .get(cursor)
                .and_then(|field| parse_moxel_vertical_group(field))
            else {
                valid = false;
                break;
            };
            if fields.get(cursor + 1).map(|field| field.trim()) != Some("-1") {
                valid = false;
                break;
            }
            groups.push(group);
            cursor += 2;
        }
        if valid
            && !groups.is_empty()
            && fields.get(cursor).map(|field| field.trim()) == Some("0")
            && fields.get(cursor + 1).map(|field| field.trim()) == Some("0")
            && fields.get(cursor + 2).map(|field| field.trim()) == Some("0")
        {
            return groups;
        }
    }
    Vec::new()
}

fn parse_moxel_vertical_group(text: &str) -> Option<MoxelVerticalGroup> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 6 || fields.get(3).map(|field| field.trim()) != Some("{1,0}") {
        return None;
    }
    Some(MoxelVerticalGroup {
        begin_row: fields.first()?.trim().parse::<usize>().ok()?,
        end_row: fields.get(1)?.trim().parse::<usize>().ok()?,
        level: fields.get(2)?.trim().parse::<usize>().ok()?,
    })
}

fn parse_moxel_column_set(text: &str) -> Option<MoxelColumnSet> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 {
        return None;
    }
    let declared_count = fields.first()?.trim().parse::<usize>().ok()?;
    let raw_default_format_index = fields.get(1)?.trim().parse::<usize>().ok()?;
    let count = fields.get(3)?.trim().parse::<usize>().ok()?;
    if count > 2048 || fields.len() != count * 2 + 4 {
        return None;
    }
    let uuid = parse_uuid_field(fields.get(2)?.trim())?;
    let id = if uuid == "00000000-0000-0000-0000-000000000000" {
        None
    } else {
        Some(uuid)
    };
    let mut columns = Vec::with_capacity(count);
    for column_index in 0..count {
        let index = fields
            .get(column_index * 2 + 4)?
            .trim()
            .parse::<i32>()
            .ok()?;
        let format_index = fields
            .get(column_index * 2 + 5)?
            .trim()
            .parse::<usize>()
            .ok()?;
        columns.push(MoxelColumn {
            index,
            format_index,
            source_format_index: Some(format_index),
        });
    }
    Some(MoxelColumnSet {
        id,
        default_format_index: None,
        source_default_format_index: (raw_default_format_index > 1)
            .then_some(raw_default_format_index),
        size: declared_count,
        columns,
    })
}

fn normalize_moxel_column_set_format_indices(column_sets: &mut [MoxelColumnSet]) {
    let mut normalized = BTreeMap::new();
    for column_set in column_sets.iter_mut() {
        for column in column_set.columns.iter_mut() {
            let source_format_index = column.source_format_index.unwrap_or(column.format_index);
            if source_format_index == 0 {
                column.format_index = 0;
                continue;
            }
            let next_index = normalized.len() + 1;
            column.format_index = *normalized.entry(source_format_index).or_insert(next_index);
        }
    }
}

fn parse_moxel_uniform_header_footer_format_ref(fields: &[&str]) -> Option<usize> {
    fields.windows(6).find_map(|window| {
        let refs = window
            .iter()
            .map(|field| parse_moxel_header_footer_format_ref(field))
            .collect::<Option<Vec<_>>>()?;
        let first = refs.first().copied().flatten()?;
        refs.iter()
            .all(|candidate| *candidate == Some(first))
            .then_some(first)
    })
}

fn parse_moxel_header_footer_format_ref(text: &str) -> Option<Option<usize>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 || fields.first().map(|field| field.trim()) != Some("0") {
        return None;
    }
    let format_index = fields.get(1)?.trim().parse::<usize>().ok()?;
    Some((format_index > 0).then_some(format_index))
}

fn is_sparse_moxel_column_set_default_format(format: &MoxelFormat) -> bool {
    format.font == Some(0)
        && format.width == Some(72)
        && format.height.is_none()
        && format.border.is_none()
        && format.left_border.is_none()
        && format.top_border.is_none()
        && format.right_border.is_none()
        && format.bottom_border.is_none()
        && format.border_color.is_none()
        && format.width_weight_factor.is_none()
        && format.horizontal_alignment.is_none()
        && format.vertical_alignment.is_none()
        && format.back_color.is_none()
        && format.text_color.is_none()
        && format.text_placement.is_none()
        && format.text_orientation.is_none()
        && format.fill_type.is_none()
}

fn resolve_sparse_moxel_column_set_default_format_index(
    column_sets: &mut [MoxelColumnSet],
    column_format_len: usize,
    formats: &[MoxelFormat],
    header_footer_format_ref: Option<usize>,
) -> Option<usize> {
    if column_sets.is_empty() {
        return None;
    }
    let header_footer_format_index = header_footer_format_ref.and_then(|source_format_index| {
        moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            formats.len(),
        )
    });
    if column_sets.len() <= 1 {
        return header_footer_format_index;
    }
    let sparse_default_format_index = formats.iter().enumerate().find_map(|(index, format)| {
        is_sparse_moxel_column_set_default_format(format).then_some(column_format_len + index + 1)
    });
    if let Some(format_index) = sparse_default_format_index {
        if column_sets.len() > 1 {
            for column_set in column_sets.iter_mut().skip(1) {
                column_set.default_format_index = Some(format_index);
            }
        }
        return Some(format_index);
    }

    if let Some(format_index) = header_footer_format_index
        && format_index > column_format_len
    {
        if column_sets.len() > 1 {
            for column_set in column_sets.iter_mut() {
                column_set.default_format_index = Some(format_index);
            }
        }
        return Some(format_index);
    }

    let format_index = header_footer_format_index?;
    for column_set in column_sets.iter_mut().skip(1) {
        column_set.default_format_index = Some(format_index);
    }
    Some(format_index)
}

fn moxel_internal_format<'a>(
    column_formats: &'a [MoxelFormat],
    formats: &'a [MoxelFormat],
    format_index: usize,
) -> Option<&'a MoxelFormat> {
    if format_index == 0 {
        return None;
    }
    if format_index <= column_formats.len() {
        return column_formats.get(format_index - 1);
    }
    formats.get(format_index - column_formats.len() - 1)
}

fn moxel_column_format_slots(column_sets: &[MoxelColumnSet], column_count: usize) -> usize {
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter().map(|column| column.format_index))
        .max()
        .unwrap_or_else(|| {
            if column_sets.is_empty() {
                column_count
            } else {
                0
            }
        })
}

fn moxel_default_format_index(
    column_sets: &[MoxelColumnSet],
    _print_settings: Option<&MoxelPrintSettings>,
    has_default_format: bool,
    fallback: usize,
) -> Option<usize> {
    if has_default_format {
        return Some(fallback);
    }
    if column_sets.len() > 1 {
        return Some(fallback);
    }
    None
}

fn parse_moxel_row_column_set_ids(
    fields: &[&str],
    index: usize,
    additional_sets: &[MoxelColumnSet],
) -> Option<BTreeMap<usize, String>> {
    if additional_sets.is_empty() {
        return Some(BTreeMap::new());
    }
    let count = fields.get(index)?.trim().parse::<usize>().ok()?;
    if count > 4096 || index + count >= fields.len() {
        return None;
    }
    if index + count * 2 < fields.len() {
        let mut row_column_ids = BTreeMap::new();
        let mut pair_mode = true;
        for pair_index in 0..count {
            let row_index = fields[index + 1 + pair_index * 2]
                .trim()
                .parse::<usize>()
                .ok();
            let set_index = fields[index + 2 + pair_index * 2]
                .trim()
                .parse::<usize>()
                .ok();
            let Some(row_index) = row_index else {
                pair_mode = false;
                break;
            };
            let Some(set_index) = set_index else {
                pair_mode = false;
                break;
            };
            let Some(columns_id) = additional_sets
                .get(set_index)
                .and_then(|set| set.id.as_ref())
            else {
                pair_mode = false;
                break;
            };
            row_column_ids.insert(row_index, columns_id.clone());
        }
        if pair_mode {
            return Some(row_column_ids);
        }
    }
    let first_columns_id = additional_sets.first()?.id.as_ref()?;
    let mut row_column_ids = BTreeMap::new();
    for field in &fields[index + 1..=index + count] {
        let row_index = field.trim().parse::<usize>().ok()?;
        row_column_ids.insert(row_index, first_columns_id.clone());
    }
    Some(row_column_ids)
}

fn moxel_spreadsheet_height(
    rows: &[MoxelRow],
    merges: &[MoxelMerge],
    horizontal_unmerges: &[MoxelMerge],
    vertical_unmerges: &[MoxelMerge],
    areas: &[MoxelArea],
) -> usize {
    let row_max = rows
        .iter()
        .filter(|row| row.format_index > 1 || !row.cells.is_empty())
        .map(|row| row.index as i32)
        .max()
        .unwrap_or(0);
    let merge_max = merges
        .iter()
        .chain(horizontal_unmerges.iter())
        .chain(vertical_unmerges.iter())
        .map(|merge| merge.row + merge.height)
        .max()
        .unwrap_or(0);
    let area_max = areas.iter().map(|area| area.end_row).max().unwrap_or(0);
    row_max.max(merge_max).max(area_max).max(0) as usize + 1
}

fn trim_moxel_trailing_empty_rows(
    rows: &mut Vec<MoxelRow>,
    areas: &[MoxelArea],
    merges: &[MoxelMerge],
    horizontal_unmerges: &[MoxelMerge],
    vertical_unmerges: &[MoxelMerge],
) {
    let Some(material_limit) = areas
        .iter()
        .map(|area| area.end_row.max(0) as usize + 1)
        .chain(
            merges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .chain(
            horizontal_unmerges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .chain(
            vertical_unmerges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .max()
    else {
        return;
    };
    let mut last_trimmed_index = None;
    while rows.last().is_some_and(|row| {
        row.index > material_limit && row.format_index <= 1 && row.cells.is_empty()
    }) {
        if let Some(index) = rows.last().map(|row| row.index) {
            last_trimmed_index = Some(last_trimmed_index.unwrap_or(index).max(index));
        }
        rows.pop();
    }
    if let (Some(index_to), Some(row)) = (last_trimmed_index, rows.last_mut()) {
        if row.index == material_limit && row.format_index <= 1 && row.cells.is_empty() {
            row.index_to = Some(index_to);
        }
    }
}

fn compact_moxel_empty_row_ranges(rows: &mut Vec<MoxelRow>) {
    let mut compacted = Vec::with_capacity(rows.len());
    let mut index = 0usize;
    while index < rows.len() {
        let mut row = rows[index].clone();
        if is_moxel_compactable_empty_row(&row) {
            let mut cursor = index + 1;
            while cursor < rows.len()
                && rows[cursor].index == rows[cursor - 1].index + 1
                && is_moxel_compactable_empty_row(&rows[cursor])
            {
                row.index_to = Some(rows[cursor].index);
                cursor += 1;
            }
            compacted.push(row);
            index = cursor;
        } else {
            compacted.push(row);
            index += 1;
        }
    }
    *rows = compacted;
}

fn is_moxel_compactable_empty_row(row: &MoxelRow) -> bool {
    row.format_index <= 1 && row.columns_id.is_none() && row.cells.is_empty()
}

fn parse_moxel_rows(fields: &[&str]) -> Vec<MoxelRow> {
    let mut best_rows = Vec::new();
    for index in 3..fields.len().saturating_sub(3) {
        if fields.get(index).map(|field| field.trim()) != Some("1")
            || fields.get(index + 1).map(|field| field.trim()) != Some("2")
        {
            continue;
        }
        let Some(height) = fields
            .get(index + 2)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if height == 0 || height > 1_000_000 {
            continue;
        }
        let mut rows = Vec::new();
        let mut cursor = index + 3;
        let mut expected_row_index = 0usize;
        while rows.len() < height {
            let Some((row, next_cursor)) = parse_moxel_row_at(fields, cursor, expected_row_index)
            else {
                break;
            };
            if next_cursor <= cursor {
                break;
            }
            rows.push(row);
            expected_row_index += 1;
            cursor = next_cursor;
        }
        if rows.len() > best_rows.len() {
            best_rows = rows;
        }
    }
    if best_rows.is_empty() {
        parse_moxel_rows_by_scanning(fields)
    } else {
        best_rows
    }
}

fn parse_moxel_rows_by_scanning(fields: &[&str]) -> Vec<MoxelRow> {
    let mut best_rows = Vec::new();
    let mut index = 3usize;
    while index < fields.len() {
        let Some((row, next_index)) = parse_moxel_row_start_at_for_scanning(fields, index) else {
            index += 1;
            continue;
        };
        let mut rows = vec![row];
        let mut cursor = next_index;
        while cursor < fields.len() {
            let expected_row_index = rows.last().map(|row| row.index + 1).unwrap_or(0);
            let Some((row, next_cursor)) =
                parse_moxel_row_at_for_scanning(fields, cursor, expected_row_index)
            else {
                break;
            };
            if next_cursor <= cursor {
                break;
            }
            rows.push(row);
            cursor = next_cursor;
        }
        if rows.len() > best_rows.len() {
            best_rows = rows;
        }
        index = next_index.max(index + 1);
    }
    best_rows
}

fn parse_moxel_row_start_at_for_scanning(
    fields: &[&str],
    index: usize,
) -> Option<(MoxelRow, usize)> {
    let expected_row_index = fields.get(index)?.trim().parse::<usize>().ok()?;
    parse_moxel_row_at_for_scanning(fields, index, expected_row_index)
}

fn parse_moxel_row_at(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
) -> Option<(MoxelRow, usize)> {
    if let Some(row) = parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 0,
            format_offset: 1,
            cell_count_offset: 2,
            cells_offset: 3,
            allow_empty: true,
            validate_empty_prefix: false,
        },
    ) {
        return Some(row);
    }
    parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 3,
            format_offset: 4,
            cell_count_offset: 5,
            cells_offset: 6,
            allow_empty: true,
            validate_empty_prefix: true,
        },
    )
}

fn parse_moxel_row_at_for_scanning(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
) -> Option<(MoxelRow, usize)> {
    if let Some(row) = parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 0,
            format_offset: 1,
            cell_count_offset: 2,
            cells_offset: 3,
            allow_empty: true,
            validate_empty_prefix: false,
        },
    ) {
        return Some(row);
    }
    if expected_row_index != 0 {
        return None;
    }
    parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 3,
            format_offset: 4,
            cell_count_offset: 5,
            cells_offset: 6,
            allow_empty: true,
            validate_empty_prefix: true,
        },
    )
}

#[derive(Clone, Copy)]
struct MoxelRowShape {
    row_index_offset: usize,
    format_offset: usize,
    cell_count_offset: usize,
    cells_offset: usize,
    allow_empty: bool,
    validate_empty_prefix: bool,
}

fn parse_moxel_row_shape(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
    shape: MoxelRowShape,
) -> Option<(MoxelRow, usize)> {
    let row_index = fields
        .get(index + shape.row_index_offset)?
        .trim()
        .parse::<usize>()
        .ok()?;
    if row_index != expected_row_index {
        return None;
    }
    let format_index = fields
        .get(index + shape.format_offset)?
        .trim()
        .parse::<usize>()
        .ok()?
        + 1;
    let cell_count = fields
        .get(index + shape.cell_count_offset)?
        .trim()
        .parse::<usize>()
        .ok()?;
    if (!shape.allow_empty && cell_count == 0) || cell_count > 2048 {
        return None;
    }
    if shape.validate_empty_prefix && cell_count == 0 {
        let prefix_left = fields.get(index)?.trim().parse::<usize>().ok()?;
        let prefix_right = fields.get(index + 1)?.trim().parse::<usize>().ok()?;
        if prefix_left == 0 || prefix_right == 0 {
            return None;
        }
    }
    let mut cells = Vec::with_capacity(cell_count);
    let mut cursor = index + shape.cells_offset;
    for _ in 0..cell_count {
        let column_index = fields.get(cursor)?.trim().parse::<usize>().ok()?;
        let cell = parse_moxel_cell(fields.get(cursor + 1)?, column_index)?;
        cells.push(cell);
        cursor += 2;
    }
    Some((
        MoxelRow {
            index: row_index,
            index_to: None,
            format_index,
            source_format_index: Some(format_index),
            columns_id: None,
            cells,
        },
        cursor,
    ))
}

fn parse_moxel_cell(text: &str, column_index: usize) -> Option<MoxelCell> {
    let fields = split_1c_braced_fields(text, 0)?;
    let cell_kind = fields.first()?.trim();
    let format_index = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| if value == 0 { 0 } else { value + 1 })
        .unwrap_or(0);
    let detail_parameter_field = match cell_kind {
        // Native dumps also use these cell kinds for detail-only and note-bearing
        // spreadsheet cells, with detailParameter kept in slot 2.
        "8" | "24" | "56" => Some(2),
        _ => None,
    };
    let detail_parameter = detail_parameter_field
        .and_then(|index| fields.get(index))
        .and_then(|value| parse_1c_string(value));
    let localized_index = detail_parameter_field.map(|index| index + 1).unwrap_or(2);
    let localized = fields
        .get(localized_index)
        .and_then(|value| parse_moxel_localized_cell_value(value));
    let empty_text = matches!(localized, Some(None));
    let localized = localized.flatten();
    let text = localized
        .as_ref()
        .filter(|value| !value.lang.is_empty())
        .map(|value| value.content.clone());
    let parameter = localized
        .as_ref()
        .filter(|value| value.lang.is_empty())
        .map(|value| value.content.clone());
    Some(MoxelCell {
        column_index,
        format_index,
        source_format_index: if format_index == 0 {
            None
        } else {
            Some(format_index)
        },
        text,
        parameter,
        detail_parameter,
        empty_text,
    })
}

fn parse_moxel_localized_cell_value(text: &str) -> Option<Option<MoxelLocalizedValue>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 {
        return Some(None);
    }
    let pair = split_1c_braced_fields(fields.iter().skip(2).take(count).next()?, 0)?;
    let lang = parse_1c_string(pair.first()?)?;
    let content = parse_1c_string(pair.get(1)?)?;
    Some(Some(MoxelLocalizedValue { lang, content }))
}

fn parse_moxel_areas(fields: &[&str]) -> Vec<MoxelArea> {
    parse_moxel_named_items(fields)
        .into_iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect()
}

fn parse_moxel_named_items(fields: &[&str]) -> Vec<MoxelNamedItem> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_named_item_list(field))
        .next()
        .unwrap_or_default()
}

fn parse_moxel_print_area(fields: &[&str]) -> Option<MoxelArea> {
    fields.iter().find_map(|field| {
        let bounds = split_1c_braced_fields(field, 0)?;
        if bounds.len() != 6 {
            return None;
        }
        parse_moxel_bounds_area(&bounds, String::new())
    })
}

fn parse_moxel_fonts(fields: &[&str]) -> Vec<MoxelFont> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_font(field))
        .collect()
}

fn parse_moxel_font(text: &str) -> Option<MoxelFont> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    match fields.get(1)?.trim() {
        "0" if fields.len() >= 19 => {
            let height_raw = fields.get(3)?.trim().parse::<usize>().ok()?;
            let weight = fields.get(7)?.trim().parse::<usize>().ok()?;
            Some(MoxelFont {
                ref_name: None,
                face_name: Some(parse_1c_string(fields.get(16)?)?),
                height: Some(format_moxel_font_height(height_raw)),
                bold: weight >= 700,
                italic: fields.get(8)?.trim() != "0",
                underline: fields.get(9)?.trim() != "0",
                strikeout: fields.get(10)?.trim() != "0",
                kind: "Absolute",
                scale: Some(fields.get(18)?.trim().parse::<usize>().ok()?),
            })
        }
        "2" if fields.len() >= 10 => {
            let raw_fields = split_1c_braced_fields(fields.get(3)?, 0)?;
            let (ref_name, face_name) = match raw_fields.first()?.trim() {
                "-20" => (
                    "style:TextFont",
                    fields.get(8).and_then(|field| parse_1c_string(field)),
                ),
                "-31" => ("style:NormalTextFont", None),
                "-32" => ("style:LargeTextFont", None),
                _ => return None,
            };
            let weight = fields.get(4)?.trim().parse::<usize>().ok()?;
            Some(MoxelFont {
                ref_name: Some(ref_name.to_string()),
                face_name,
                height: None,
                bold: weight >= 700,
                italic: fields.get(5)?.trim() != "0",
                underline: fields.get(6)?.trim() != "0",
                strikeout: fields.get(7)?.trim() != "0",
                kind: "StyleItem",
                scale: None,
            })
        }
        "1" if fields.len() >= 6 => {
            let (height, weight, italic, underline, strikeout, scale) = if fields.len() >= 11 {
                (
                    fields
                        .get(4)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .map(format_moxel_font_height),
                    fields
                        .get(5)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .unwrap_or(400),
                    fields
                        .get(6)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(7)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(8)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(10)
                        .and_then(|field| field.trim().parse::<usize>().ok()),
                )
            } else if fields.len() >= 8 {
                (
                    fields
                        .get(4)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .map(format_moxel_font_height),
                    fields
                        .get(5)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .unwrap_or(400),
                    false,
                    false,
                    false,
                    None,
                )
            } else {
                (None, 400, false, false, false, None)
            };
            Some(MoxelFont {
                ref_name: Some("sys:DefaultGUIFont".to_string()),
                face_name: None,
                height,
                bold: weight >= 700,
                italic,
                underline,
                strikeout,
                kind: "WindowsFont",
                scale,
            })
        }
        _ => None,
    }
}

fn format_moxel_font_height(raw_height: usize) -> String {
    if raw_height % 10 == 0 {
        (raw_height / 10).to_string()
    } else {
        format!("{}.{}", raw_height / 10, raw_height % 10)
    }
}

fn parse_moxel_lines(
    fields: &[&str],
    formats: &[MoxelFormat],
    shift_default_line_styles: bool,
) -> Vec<MoxelLine> {
    let used_indexes = moxel_used_line_indexes(formats);
    if used_indexes.is_empty() {
        return Vec::new();
    }
    let uses_drawing_line = formats.iter().any(|format| format.drawing_border.is_some());
    let mut lines = fields
        .iter()
        .filter_map(|field| parse_moxel_line(field))
        .collect::<Vec<_>>();
    if lines.len() > 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
    {
        lines.truncate(3);
    }
    let expected_line_slots =
        expected_moxel_line_slots(&used_indexes, uses_drawing_line, shift_default_line_styles);
    if expected_line_slots > 0
        && lines.len() > expected_line_slots
        && !(lines.len() == 3
            && lines.first().is_some_and(|line| line.style == "None")
            && lines.get(1).is_some_and(|line| line.style == "Solid")
            && lines.get(2).is_some_and(|line| line.style == "Dotted"))
    {
        lines.truncate(expected_line_slots);
    }
    if lines.len() == 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && used_indexes.len() == 4
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
        && used_indexes.contains(&3)
    {
        return vec![
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 3,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
        ];
    }
    if uses_drawing_line
        && lines.len() >= 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
        && used_indexes.len() == 4
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
        && used_indexes.contains(&3)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                width: 1,
            },
        ];
    }
    if uses_drawing_line
        && lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
    {
        return vec![
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                width: 1,
            },
        ];
    }
    if lines.len() >= 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
        && shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && shift_default_line_styles
        && used_indexes.len() == 3
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 3,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && !shift_default_line_styles
        && used_indexes.len() == 3
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 0,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Dotted",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && used_indexes.len() == 1
        && used_indexes.contains(&0)
    {
        return vec![MoxelLine {
            style: "Solid",
            line_type: "v8ui:SpreadsheetDocumentCellLineType",
            width: 1,
        }];
    }
    if !lines.is_empty() {
        return lines;
    }
    vec![MoxelLine {
        style: "Solid",
        line_type: "v8ui:SpreadsheetDocumentCellLineType",
        width: 1,
    }]
}

fn expected_moxel_line_slots(
    used_indexes: &BTreeSet<usize>,
    uses_drawing_line: bool,
    shift_default_line_styles: bool,
) -> usize {
    let mut expected = used_indexes
        .iter()
        .next_back()
        .copied()
        .map(|index| index + 1)
        .unwrap_or(0);
    if used_indexes.len() == 1 && used_indexes.contains(&0) {
        expected = expected.max(2);
    }
    if shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        expected = expected.max(3);
    }
    if uses_drawing_line {
        expected = expected.max(3);
    }
    expected
}

fn moxel_used_line_indexes(formats: &[MoxelFormat]) -> BTreeSet<usize> {
    let mut indexes = BTreeSet::new();
    for format in formats {
        for value in [
            format.border,
            format.left_border,
            format.top_border,
            format.right_border,
            format.bottom_border,
            format.drawing_border,
        ] {
            if let Some(index) = value {
                indexes.insert(index);
            }
        }
    }
    indexes
}

fn parse_moxel_pictures(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<MoxelPicture> {
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count == 0 || count > 512 || index + count >= fields.len() {
            continue;
        }
        let mut pictures = Vec::with_capacity(count);
        for (picture_index, field) in fields[index + 1..=index + count].iter().enumerate() {
            let Some(mut picture) = parse_moxel_picture(field, object_refs) else {
                pictures.clear();
                break;
            };
            picture.index = picture_index;
            pictures.push(picture);
        }
        if pictures.len() == count {
            return pictures;
        }
    }
    Vec::new()
}

fn parse_moxel_picture(text: &str, object_refs: &BTreeMap<String, String>) -> Option<MoxelPicture> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let ref_name = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|picture_ref| {
            match picture_ref.first().map(|field| field.trim()) {
                Some("-13") => return Some("v8ui:Print".to_string()),
                Some("-6") => return Some("v8ui:InputFieldCalculator".to_string()),
                _ => {}
            }
            if picture_ref.first().map(|field| field.trim()) != Some("0") {
                return None;
            }
            let uuid = parse_uuid_field(picture_ref.get(1)?.trim())?;
            match uuid.as_str() {
                STD_PICTURE_INFORMATION_UUID => return Some("v8ui:Information".to_string()),
                STD_PICTURE_SAVE_FILE_UUID => return Some("v8ui:SaveFile".to_string()),
                _ => {}
            }
            object_refs
                .get(&uuid)
                .and_then(|reference| reference.strip_prefix("CommonPicture."))
                .map(|name| format!("v8ui:{name}"))
        });
    let payload = fields
        .iter()
        .find_map(|field| extract_base64_payload(field))
        .map(normalize_moxel_picture_payload);
    Some(MoxelPicture {
        index: fields.get(1)?.trim().parse::<usize>().ok()?,
        ref_name,
        payload,
    })
}

fn normalize_moxel_picture_payload(payload: &str) -> String {
    payload
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn parse_moxel_drawings(fields: &[&str]) -> Vec<MoxelDrawing> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_drawing(field))
        .collect()
}

fn parse_moxel_drawing(text: &str) -> Option<MoxelDrawing> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 14 || fields.get(1)?.trim() != "5" {
        return None;
    }
    let format_fields = split_1c_braced_fields(fields.first()?, 0)?;
    if format_fields.len() != 2 || format_fields.first()?.trim() != "0" {
        return None;
    }
    let begin_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    let begin_column_offset = fields.get(4)?.trim().parse::<i32>().ok()?;
    let begin_row_offset = fields.get(5)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(6)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(7)?.trim().parse::<i32>().ok()?;
    let end_column_offset = fields.get(8)?.trim().parse::<i32>().ok()?;
    let end_row_offset = fields.get(9)?.trim().parse::<i32>().ok()?;
    if begin_column < 0
        || begin_row < 0
        || end_column < begin_column
        || end_row < begin_row
        || begin_column_offset < 0
        || begin_row_offset < 0
        || end_column_offset < 0
        || end_row_offset < 0
    {
        return None;
    }
    let picture_index = fields.get(11)?.trim().parse::<usize>().ok()?;
    let picture_size = match fields.get(12)?.trim().parse::<usize>().ok()? {
        0 => "RealSize",
        1 => "Stretch",
        2 => "Proportionally",
        4 => "AutoSize",
        _ => return None,
    };
    let id = fields.get(10)?.trim().parse::<usize>().ok()?;
    Some(MoxelDrawing {
        id,
        format_index: format_fields.get(1)?.trim().parse::<usize>().ok()?,
        begin_row,
        begin_row_offset,
        end_row,
        end_row_offset,
        begin_column,
        begin_column_offset,
        end_column,
        end_column_offset,
        auto_size: fields.get(13)?.trim() != "0",
        picture_size,
        z_order: picture_index,
        picture_index,
    })
}

fn parse_moxel_default_format_width(fields: &[&str], column_count: usize) -> Option<usize> {
    if let Some((table_index, slots)) =
        parse_moxel_equal_width_only_format_table(fields, column_count)
        && slots.iter().any(|slot| slot.is_none())
        && let Some(width) = fields[..table_index]
            .iter()
            .rev()
            .find_map(|field| parse_moxel_column_width(field))
    {
        return Some(width);
    }
    let widths = fields
        .iter()
        .filter_map(|field| parse_moxel_column_width(field))
        .collect::<Vec<_>>();
    if widths.len() <= column_count {
        return fields
            .iter()
            .find_map(|field| parse_moxel_extended_default_format_width(field))
            .or_else(|| {
                fields
                    .iter()
                    .take(8)
                    .find_map(|field| parse_moxel_leading_default_format_width_129(field))
            });
    }
    widths.first().copied().or_else(|| {
        fields
            .iter()
            .take(8)
            .find_map(|field| parse_moxel_leading_default_format_width_129(field))
    })
}

fn parse_moxel_extended_default_format_width(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 4
        || fields.first()?.trim() != "161"
        || fields.get(1)?.trim() != "0"
        || fields.get(2)?.trim() != "0"
    {
        return None;
    }
    fields.get(3)?.trim().parse::<usize>().ok()
}

fn parse_moxel_leading_default_format_width_129(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "129" || fields.get(1)?.trim() != "0" {
        return None;
    }
    fields.get(2)?.trim().parse::<usize>().ok()
}

fn parse_moxel_default_format(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> MoxelFormat {
    fields
        .iter()
        .filter_map(|field| parse_moxel_default_format_field(field, object_refs))
        .next()
        .unwrap_or_default()
}

fn parse_moxel_leading_default_format(
    fields: &[&str],
    style_refs: &[Option<String>],
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<MoxelFormat> {
    fields
        .iter()
        .take(8)
        .filter_map(|field| parse_moxel_format(field, style_refs, number_format_refs))
        .find(|format| !format.is_empty())
}

fn parse_moxel_default_format_field(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelFormat> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3
        || fields.first().map(|field| field.trim()) != Some("1")
        || fields.get(1).map(|field| field.trim()) != Some("0")
    {
        return None;
    }
    let border_color = fields
        .get(2)
        .and_then(|field| parse_moxel_style_ref_slot(field, object_refs))
        .flatten()?;
    Some(MoxelFormat {
        border_color: Some(border_color),
        ..MoxelFormat::default()
    })
}

fn parse_moxel_print_settings(fields: &[&str]) -> Option<MoxelPrintSettings> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_print_settings_field(field))
        .next()
}

fn parse_moxel_print_settings_field(text: &str) -> Option<MoxelPrintSettings> {
    let mut fields = split_1c_braced_fields(text, 0)?;
    if fields.len() == 1 && fields.first()?.trim_start().starts_with('{') {
        fields = split_1c_braced_fields(fields.first()?, 0)?;
    }
    if fields.len() < 4 || fields.first().map(|field| field.trim()) != Some("0") {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 18 || fields.len() != count * 2 + 2 {
        return None;
    }
    let mut settings = MoxelPrintSettings::default();
    let mut seen_keys = BTreeSet::new();
    for pair in fields[2..].chunks_exact(2) {
        let key = pair.first()?.trim().parse::<usize>().ok()?;
        if key > 17 || !seen_keys.insert(key) {
            return None;
        }
        let value = parse_moxel_print_settings_value(pair.get(1)?)?;
        match key {
            0 => settings.paper = value.as_usize(),
            1 => settings.page_orientation = value.as_usize().and_then(moxel_page_orientation),
            2 => settings.scale = value.as_usize(),
            3 => settings.collate = value.as_bool(),
            4 => settings.copies = value.as_usize(),
            5 => settings.per_page = value.as_usize(),
            6 => settings.top_margin = value.as_usize(),
            7 => settings.left_margin = value.as_usize(),
            8 => settings.bottom_margin = value.as_usize(),
            9 => settings.right_margin = value.as_usize(),
            10 => settings.header_size = value.as_usize(),
            11 => settings.footer_size = value.as_usize(),
            12 => settings.fit_to_page = value.as_bool(),
            13 => settings.black_and_white = value.as_bool(),
            14 => settings.printer_name = value.into_string(),
            15 => settings.paper_source = value.as_usize(),
            16 => settings.page_width = value.as_usize(),
            17 => settings.page_height = value.as_usize(),
            _ => return None,
        }
    }
    Some(settings)
}

enum MoxelPrintSettingsValue {
    Number(usize),
    Text(String),
}

impl MoxelPrintSettingsValue {
    fn as_usize(&self) -> Option<usize> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        self.as_usize().map(|value| value != 0)
    }

    fn into_string(self) -> Option<String> {
        match self {
            Self::Number(_) => None,
            Self::Text(value) => Some(value),
        }
    }
}

fn parse_moxel_print_settings_value(text: &str) -> Option<MoxelPrintSettingsValue> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 {
        return None;
    }
    match fields.first()?.trim().trim_matches('"') {
        "N" => fields
            .get(1)?
            .trim()
            .parse::<usize>()
            .ok()
            .map(MoxelPrintSettingsValue::Number),
        "S" => Some(MoxelPrintSettingsValue::Text(
            unquote_moxel_string(fields.get(1)?.trim()).unwrap_or_else(|| fields[1].to_string()),
        )),
        _ => None,
    }
}

fn unquote_moxel_string(value: &str) -> Option<String> {
    let value = value.trim();
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\"\"", "\""))
}

fn parse_moxel_formats(
    fields: &[&str],
    column_count: usize,
    sparse_source_format_refs: bool,
    source_column_format_refs: &[usize],
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    let all_formats = parse_moxel_format_table(
        fields,
        column_count,
        style_refs,
        drawing_format_indices,
        number_format_refs,
    );
    if let Some(formats) = all_formats {
        if sparse_source_format_refs && !source_column_format_refs.is_empty() {
            return split_moxel_formats_by_source_refs(formats, source_column_format_refs);
        }
        if prefers_moxel_leading_source_column_formats(&formats, source_column_format_refs) {
            return split_moxel_formats_by_source_refs(formats, source_column_format_refs);
        }
        return split_moxel_formats_for_output(
            formats,
            column_count,
            sparse_source_format_refs,
            drawing_format_indices,
        );
    }
    if let Some((_, slots)) = parse_moxel_equal_width_only_format_table(fields, column_count) {
        let formats = slots
            .into_iter()
            .map(|width| MoxelFormat {
                width,
                ..MoxelFormat::default()
            })
            .collect::<Vec<_>>();
        return split_moxel_formats_for_output(
            formats,
            column_count,
            sparse_source_format_refs,
            drawing_format_indices,
        );
    }
    (Vec::new(), Vec::new())
}

fn parse_moxel_format_table(
    fields: &[&str],
    column_count: usize,
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<Vec<MoxelFormat>> {
    for index in 0..fields.len() {
        if let Some(formats) = parse_moxel_nested_format_table(
            fields[index],
            column_count,
            style_refs,
            drawing_format_indices,
            number_format_refs,
        ) {
            return Some(formats);
        }
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count <= column_count || count > 2048 || index + count >= fields.len() {
            continue;
        }
        let mut formats = Vec::with_capacity(count);
        for (format_offset, field) in fields[index + 1..=index + count].iter().enumerate() {
            let Some(mut format) = parse_moxel_format(field, style_refs, number_format_refs) else {
                formats.clear();
                break;
            };
            if drawing_format_indices.contains(&(format_offset + 1)) {
                normalize_moxel_drawing_format(&mut format);
            }
            formats.push(format);
        }
        if formats.len() == count {
            return Some(formats);
        }
    }
    None
}

fn parse_moxel_equal_width_only_format_table(
    fields: &[&str],
    column_count: usize,
) -> Option<(usize, Vec<Option<usize>>)> {
    if column_count == 0 {
        return None;
    }
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count != column_count || index + count >= fields.len() {
            continue;
        }
        let mut saw_width = false;
        let mut slots = Vec::with_capacity(count);
        let mut valid = true;
        for field in &fields[index + 1..=index + count] {
            let trimmed = field.trim();
            if trimmed == "{0}" {
                slots.push(None);
                continue;
            }
            let Some(width) = parse_moxel_column_width(trimmed) else {
                valid = false;
                break;
            };
            saw_width = true;
            slots.push(Some(width));
        }
        if valid && saw_width {
            return Some((index, slots));
        }
    }
    None
}

fn parse_moxel_nested_format_table(
    text: &str,
    column_count: usize,
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<Vec<MoxelFormat>> {
    let nested = split_1c_braced_fields(text, 0)?;
    let count = nested.first()?.trim().parse::<usize>().ok()?;
    if count <= column_count || count > 2048 || nested.len() != count + 1 {
        return None;
    }
    let mut formats = Vec::with_capacity(count);
    for (format_offset, field) in nested.iter().skip(1).enumerate() {
        let Some(mut format) = parse_moxel_format(field, style_refs, number_format_refs) else {
            return None;
        };
        if drawing_format_indices.contains(&(format_offset + 1)) {
            normalize_moxel_drawing_format(&mut format);
        }
        formats.push(format);
    }
    Some(formats)
}

fn normalize_moxel_drawing_format(format: &mut MoxelFormat) {
    format.drawing_border = format.left_border;
    format.left_border = None;
    if format.back_color.is_none() {
        match format.border_color.as_deref() {
            Some("style:ToolTipBackColor") => {
                format.back_color = Some("style:FormBackColor".to_string());
                format.border_color = None;
            }
            Some(
                "style:FormBackColor" | "style:FieldBackColor" | "style:FieldSelectionBackColor",
            ) => {
                format.back_color = format.border_color.take();
            }
            _ => {}
        }
    }
    if format.back_color.as_deref() == Some("style:ToolTipBackColor") {
        format.back_color = Some("style:FormBackColor".to_string());
    }
}

fn normalize_moxel_single_set_report_header_tail(
    column_sets: &[MoxelColumnSet],
    column_formats: &[MoxelFormat],
    style_refs: &[Option<String>],
    lines: &mut [MoxelLine],
    formats: &mut [MoxelFormat],
) {
    if column_sets.len() != 1
        || column_formats.len() != 8
        || style_refs.get(2).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
        || style_refs.get(3).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
        || style_refs.get(4).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
    {
        return;
    }
    if let Some(line) = lines.get_mut(1)
        && line.style == "Dotted"
        && line.width == 1
    {
        line.style = "Solid";
        line.width = 2;
    }
    for (offset, format) in formats.iter_mut().enumerate() {
        let format_index = column_formats.len() + offset + 1;
        if format.back_color.as_deref() == Some("style:ReportHeaderBackColor")
            && format.border_color.is_none()
            && format.text_placement == Some("Wrap")
            && format_index >= 48
        {
            format.back_color = Some("#F4ECC5".to_string());
        }
    }
}

fn split_moxel_formats_by_source_refs(
    formats: Vec<MoxelFormat>,
    source_column_format_refs: &[usize],
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    let mut selected_refs = BTreeSet::new();
    let mut column_formats = Vec::new();
    for source_format_index in source_column_format_refs {
        if *source_format_index == 0
            || *source_format_index > formats.len()
            || !selected_refs.insert(*source_format_index)
        {
            continue;
        }
        column_formats.push(formats[*source_format_index - 1].clone());
    }
    let formats = formats
        .into_iter()
        .enumerate()
        .filter_map(|(index, format)| {
            let source_format_index = index + 1;
            if selected_refs.contains(&source_format_index) {
                None
            } else {
                Some(format)
            }
        })
        .collect::<Vec<_>>();
    (column_formats, formats)
}

fn prefers_moxel_leading_source_column_formats(
    formats: &[MoxelFormat],
    source_column_format_refs: &[usize],
) -> bool {
    if source_column_format_refs.is_empty() || source_column_format_refs.len() >= formats.len() {
        return false;
    }
    if !source_column_format_refs
        .iter()
        .enumerate()
        .all(|(index, source_format_index)| *source_format_index == index + 1)
    {
        return false;
    }
    if !source_column_format_refs.iter().all(|source_format_index| {
        formats
            .get(source_format_index - 1)
            .is_some_and(is_moxel_width_only_format)
    }) {
        return false;
    }
    formats
        .iter()
        .skip(source_column_format_refs.len())
        .any(|format| !is_moxel_width_only_format(format))
}

fn is_moxel_width_only_format(format: &MoxelFormat) -> bool {
    format.width.is_some()
        && format.height.is_none()
        && format.font.is_none()
        && format.border.is_none()
        && format.left_border.is_none()
        && format.top_border.is_none()
        && format.right_border.is_none()
        && format.bottom_border.is_none()
        && format.drawing_border.is_none()
        && format.border_color.is_none()
        && format.horizontal_alignment.is_none()
        && format.vertical_alignment.is_none()
        && format.text_color.is_none()
        && format.back_color.is_none()
        && format.pattern.is_none()
        && format.text_placement.is_none()
        && format.text_orientation.is_none()
        && format.fill_type.is_none()
        && !format.number_format_present
        && format.number_format.is_empty()
        && !format.edit_format_present
        && format.edit_format.is_empty()
        && format.hyper_link.is_none()
        && format.protection.is_none()
        && format.hidden.is_none()
        && format.indent.is_none()
        && format.auto_indent.is_none()
        && format.mask.is_none()
        && format.pic_index.is_none()
        && format.picture_size_mode.is_none()
        && format.pic_horizontal_alignment.is_none()
        && format.pic_vertical_alignment.is_none()
        && format.text_position.is_none()
        && format.details_use.is_none()
        && format.by_selected_columns.is_none()
}

fn split_moxel_formats_for_output(
    mut formats: Vec<MoxelFormat>,
    column_count: usize,
    sparse_source_format_refs: bool,
    drawing_format_indices: &BTreeSet<usize>,
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    if sparse_source_format_refs {
        let trailing_drawing_count = (1..=formats.len())
            .rev()
            .take_while(|format_index| drawing_format_indices.contains(format_index))
            .count();
        let column_start = formats
            .len()
            .saturating_sub(trailing_drawing_count + column_count);
        let column_end = formats.len().saturating_sub(trailing_drawing_count);
        let trailing_formats = formats.split_off(column_end);
        let column_formats = formats.split_off(column_start);
        formats.extend(trailing_formats);
        return (column_formats, formats);
    }
    let trailing_drawing_count = (1..=formats.len())
        .rev()
        .take_while(|format_index| drawing_format_indices.contains(format_index))
        .count();
    let column_start = formats
        .len()
        .saturating_sub(trailing_drawing_count + column_count);
    let column_end = formats.len().saturating_sub(trailing_drawing_count);
    let trailing_formats = formats.split_off(column_end);
    let column_formats = formats.split_off(column_start);
    formats.extend(trailing_formats);
    (column_formats, formats)
}

fn parse_moxel_number_format_refs(
    fields: &[&str],
    column_count: usize,
    style_refs: &[Option<String>],
    _drawing_format_indices: &BTreeSet<usize>,
) -> Vec<Vec<MoxelLocalizedValue>> {
    let mut required_count = 0usize;
    let mut start = 0usize;
    for index in 0..fields.len() {
        if let Some(nested) = split_1c_braced_fields(fields[index], 0) {
            let Some(count) = nested
                .first()
                .and_then(|field| field.trim().parse::<usize>().ok())
            else {
                continue;
            };
            if count > column_count
                && count <= 2048
                && nested.len() == count + 1
                && nested
                    .iter()
                    .skip(1)
                    .all(|field| parse_moxel_format(field, style_refs, &[]).is_some())
            {
                required_count = nested
                    .iter()
                    .skip(1)
                    .map(|field| parse_moxel_format_localized_value_required_count(field))
                    .max()
                    .unwrap_or(0);
                start = index + 1;
                break;
            }
        }
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count <= column_count || count > 2048 || index + count >= fields.len() {
            continue;
        }
        let format_fields = &fields[index + 1..=index + count];
        if format_fields
            .iter()
            .all(|field| parse_moxel_format(field, style_refs, &[]).is_some())
        {
            required_count = format_fields
                .iter()
                .map(|field| parse_moxel_format_localized_value_required_count(field))
                .max()
                .unwrap_or(0);
            start = index + count + 1;
            break;
        }
    }
    if required_count == 0 {
        return Vec::new();
    }
    for index in start..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count < required_count || count > 1024 || index + count >= fields.len() {
            continue;
        }
        let formats = fields[index + 1..=index + count]
            .iter()
            .map(|field| parse_moxel_localized_values(field))
            .collect::<Option<Vec<_>>>();
        if let Some(formats) = formats {
            return formats;
        }
    }
    Vec::new()
}

fn spreadsheet_number_format_hint_from_text(raw_text: &str) -> Option<SpreadsheetNumberFormatHint> {
    let body_start = raw_text.find('{')?;
    let body = raw_text[body_start..].trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(body, 0)?;
    if fields.first()?.trim() != "8" {
        return None;
    }
    let declared_column_count = fields.get(2)?.trim().parse::<usize>().ok()? + 1;
    let rows = parse_moxel_rows(&fields);
    if rows.is_empty() {
        return None;
    }
    let (column_sets, _, _) = parse_moxel_column_sets(&fields);
    let style_refs = parse_moxel_style_refs(&fields, &BTreeMap::new());
    let default_format = parse_moxel_default_format(&fields, &BTreeMap::new());
    let observed_column_count = rows
        .iter()
        .flat_map(|row| row.cells.iter().map(|cell| cell.column_index + 1))
        .max()
        .unwrap_or(0);
    let column_count = if observed_column_count > 0 {
        observed_column_count
    } else {
        declared_column_count
    };
    let default_format_width = parse_moxel_default_format_width(
        &fields,
        moxel_column_format_slots(&column_sets, declared_column_count),
    );
    let column_sets = if column_sets.is_empty() {
        default_moxel_column_sets(column_count)
    } else {
        column_sets
    };
    let drawings = parse_moxel_drawings(&fields);
    let drawing_format_indices = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .collect::<BTreeSet<_>>();
    let column_format_slots = moxel_column_format_slots(&column_sets, column_count);
    let _sparse_source_format_refs = moxel_uses_sparse_source_format_refs(
        &column_sets,
        column_count,
        &rows,
        &default_format,
        default_format_width,
    );
    let number_format_refs = parse_moxel_number_format_refs(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
    );
    let slots = number_format_refs
        .iter()
        .map(|slot| {
            slot.iter()
                .map(|value| LocalizedString {
                    lang: value.lang.clone(),
                    content: value.content.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for index in 0..fields.len() {
        if let Some(nested) = split_1c_braced_fields(fields[index], 0) {
            let Some(count) = nested
                .first()
                .and_then(|field| field.trim().parse::<usize>().ok())
            else {
                continue;
            };
            if count > column_count
                && count <= 2048
                && nested.len() == count + 1
                && nested.iter().skip(1).all(|field| {
                    parse_moxel_format(field, &style_refs, &number_format_refs).is_some()
                })
            {
                return Some(SpreadsheetNumberFormatHint {
                    slots,
                    format_slot_indices: nested
                        .iter()
                        .skip(1)
                        .map(|field| parse_moxel_format_number_format_index(field))
                        .collect(),
                });
            }
        }
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count <= column_count || count > 2048 || index + count >= fields.len() {
            continue;
        }
        let format_fields = &fields[index + 1..=index + count];
        if format_fields
            .iter()
            .all(|field| parse_moxel_format(field, &style_refs, &number_format_refs).is_some())
        {
            return Some(SpreadsheetNumberFormatHint {
                slots,
                format_slot_indices: format_fields
                    .iter()
                    .map(|field| parse_moxel_format_number_format_index(field))
                    .collect(),
            });
        }
    }
    None
}

pub(crate) fn spreadsheet_number_format_hint_from_blob(
    blob: &[u8],
) -> Option<SpreadsheetNumberFormatHint> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let raw_text = String::from_utf8(inflated).ok()?;
    spreadsheet_number_format_hint_from_text(&raw_text)
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct DebugMoxelSpreadsheetSummary {
    pub column_count: usize,
    pub column_format_slots: usize,
    pub source_column_format_offset: usize,
    pub default_format_index: Option<usize>,
    pub column_formats_len: usize,
    pub formats_len: usize,
    pub number_format_indices: Vec<usize>,
    pub first_rows: Vec<String>,
    pub first_columns: Vec<String>,
}

#[cfg(test)]
pub(crate) fn debug_moxel_spreadsheet_summary_from_blob(
    blob: &[u8],
) -> Option<DebugMoxelSpreadsheetSummary> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let raw_text = String::from_utf8(inflated).ok()?;
    let body_start = raw_text.find('{')?;
    let body = raw_text[body_start..].trim_start_matches('\u{feff}');
    let spreadsheet = parse_moxel_spreadsheet_text(body, &BTreeMap::new())?;
    let first_rows = spreadsheet
        .rows
        .iter()
        .take(6)
        .map(|row| {
            let first_cells = row
                .cells
                .iter()
                .take(4)
                .map(|cell| format!("c{}:f{}", cell.column_index, cell.format_index))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "r{}:f{}:{}",
                row.index,
                row.format_index,
                if first_cells.is_empty() {
                    "<empty>".to_string()
                } else {
                    first_cells
                }
            )
        })
        .collect::<Vec<_>>();
    let first_columns = spreadsheet
        .column_sets
        .iter()
        .flat_map(|set| set.columns.iter())
        .take(8)
        .map(|column| {
            format!(
                "c{}:{}->{}",
                column.index,
                column.format_index,
                column.source_format_index.unwrap_or(column.format_index)
            )
        })
        .collect::<Vec<_>>();
    let format_count = spreadsheet
        .default_format_index
        .unwrap_or(0)
        .max(spreadsheet.column_formats.len() + spreadsheet.formats.len())
        .max(1);
    let number_format_indices = (1..=format_count)
        .filter(|format_index| {
            let format = moxel_format_for_index(&spreadsheet, *format_index);
            !format.number_format.is_empty() || !format.edit_format.is_empty()
        })
        .collect::<Vec<_>>();
    Some(DebugMoxelSpreadsheetSummary {
        column_count: spreadsheet.column_count,
        column_format_slots: moxel_column_format_slots(
            &spreadsheet.column_sets,
            spreadsheet.column_count,
        ),
        source_column_format_offset: moxel_source_column_format_offset(&spreadsheet.column_sets),
        default_format_index: spreadsheet.default_format_index,
        column_formats_len: spreadsheet.column_formats.len(),
        formats_len: spreadsheet.formats.len(),
        number_format_indices,
        first_rows,
        first_columns,
    })
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct DebugMoxelNumberFormatUsage {
    pub slots: Vec<String>,
    pub format_slot_indices: Vec<Option<usize>>,
}

#[cfg(test)]
pub(crate) fn debug_moxel_number_format_usage(
    raw_text: &str,
) -> Option<DebugMoxelNumberFormatUsage> {
    let hint = spreadsheet_number_format_hint_from_text(raw_text)?;
    Some(DebugMoxelNumberFormatUsage {
        slots: hint
            .slots
            .iter()
            .map(|slot| {
                if slot.is_empty() {
                    "<empty>".to_string()
                } else {
                    slot.iter()
                        .map(|value| format!("{}={}", value.lang, value.content))
                        .collect::<Vec<_>>()
                        .join("|")
                }
            })
            .collect(),
        format_slot_indices: hint.format_slot_indices,
    })
}

fn parse_moxel_format_number_format_index(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    parse_moxel_format_usize(&values, 24)
}

fn parse_moxel_format_edit_format_index(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    parse_moxel_format_usize(&values, 32)
}

fn parse_moxel_format_localized_value_required_count(text: &str) -> usize {
    [
        parse_moxel_format_number_format_index(text),
        parse_moxel_format_edit_format_index(text),
    ]
    .into_iter()
    .flatten()
    .max()
    .map(|index| index + 1)
    .unwrap_or(0)
}

fn parse_moxel_localized_values(text: &str) -> Option<Vec<MoxelLocalizedValue>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != count + 2 {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|field| {
            let pair = split_1c_braced_fields(field, 0)?;
            if pair.len() != 2 {
                return None;
            }
            Some(MoxelLocalizedValue {
                lang: parse_1c_string(pair.first()?)?,
                content: parse_1c_string(pair.get(1)?)?,
            })
        })
        .collect()
}

fn parse_moxel_format(
    text: &str,
    style_refs: &[Option<String>],
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<MoxelFormat> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    let left_border = parse_moxel_format_usize(&values, 1);
    let top_border = parse_moxel_format_usize(&values, 2);
    let right_border = parse_moxel_format_usize(&values, 3);
    let bottom_border = parse_moxel_format_usize(&values, 4);
    let border = match (left_border, top_border, right_border, bottom_border) {
        (Some(left), Some(top), Some(right), Some(bottom))
            if left == top && top == right && right == bottom =>
        {
            Some(left)
        }
        _ => None,
    };
    let mut format = MoxelFormat {
        font: parse_moxel_format_usize(&values, 0),
        border,
        left_border: if border.is_some() { None } else { left_border },
        top_border: if border.is_some() { None } else { top_border },
        right_border: if border.is_some() { None } else { right_border },
        bottom_border: if border.is_some() {
            None
        } else {
            bottom_border
        },
        height: parse_moxel_format_i32(&values, 6),
        border_color: parse_moxel_format_style_ref(&values, 5, style_refs),
        width: parse_moxel_format_usize(&values, 7),
        width_weight_factor: parse_moxel_format_usize(&values, 41),
        horizontal_alignment: parse_moxel_format_usize(&values, 8)
            .and_then(moxel_horizontal_alignment),
        vertical_alignment: parse_moxel_format_usize(&values, 9).and_then(moxel_vertical_alignment),
        back_color: parse_moxel_format_style_ref(&values, 11, style_refs),
        pattern: parse_moxel_format_usize(&values, 12).and_then(moxel_format_pattern),
        text_color: parse_moxel_format_style_ref(&values, 10, style_refs),
        text_placement: parse_moxel_format_usize(&values, 14).and_then(moxel_text_placement),
        text_orientation: parse_moxel_format_usize(&values, 13),
        fill_type: parse_moxel_format_usize(&values, 15).and_then(moxel_fill_type),
        number_format_present: values[24].is_some(),
        number_format: parse_moxel_format_usize(&values, 24)
            .and_then(|index| number_format_refs.get(index))
            .cloned()
            .unwrap_or_default(),
        edit_format_present: values[32].is_some(),
        edit_format: parse_moxel_format_usize(&values, 32)
            .and_then(|index| number_format_refs.get(index))
            .cloned()
            .unwrap_or_default(),
        drawing_border: None,
        by_selected_columns: parse_moxel_format_usize(&values, 20)
            .and_then(moxel_by_selected_columns),
        details_use: parse_moxel_format_usize(&values, 19).and_then(moxel_details_use),
        hyper_link: parse_moxel_format_usize(&values, 26).and_then(moxel_hyper_link),
        protection: parse_moxel_format_usize(&values, 16).and_then(moxel_protection),
        hidden: parse_moxel_format_usize(&values, 17).and_then(moxel_hidden),
        indent: parse_moxel_format_usize(&values, 30),
        auto_indent: parse_moxel_format_usize(&values, 31),
        mask: parse_moxel_format_usize(&values, 34).and_then(moxel_mask),
        pic_index: parse_moxel_format_usize(&values, 35),
        pic_horizontal_alignment: parse_moxel_format_usize(&values, 36)
            .and_then(moxel_picture_horizontal_alignment),
        pic_vertical_alignment: parse_moxel_format_usize(&values, 37)
            .and_then(moxel_picture_vertical_alignment),
        picture_size_mode: parse_moxel_format_usize(&values, 38).and_then(moxel_picture_size_mode),
        text_position: parse_moxel_format_usize(&values, 39).and_then(moxel_text_position),
    };
    if format.pattern.is_none()
        && format.back_color.is_some()
        && format.border_color.is_some()
        && matches!(format.text_placement, Some("Auto"))
    {
        format.pattern = Some("Solid");
    }
    Some(format)
}

fn moxel_format_values<'a>(flags: u64, fields: &[&'a str]) -> Option<[Option<&'a str>; 64]> {
    let mut values = [None; 64];
    if flags == 0 {
        return (fields.len() == 1).then_some(values);
    }
    let mut field_index = 1usize;
    for (bit, value) in values.iter_mut().enumerate() {
        if flags & (1u64 << bit) == 0 {
            continue;
        }
        let field = *fields.get(field_index)?;
        if moxel_format_bit_is_supported(bit) {
            *value = Some(field);
        }
        field_index += 1;
    }
    (field_index == fields.len()).then_some(values)
}

fn moxel_format_bit_is_supported(bit: usize) -> bool {
    matches!(
        bit,
        0 | 1
            | 2
            | 3
            | 4
            | 5
            | 6
            | 7
            | 8
            | 9
            | 10
            | 11
            | 13
            | 14
            | 15
            | 16
            | 17
            | 19
            | 20
            | 24
            | 26
            | 30
            | 31
            | 32
            | 34
            | 35
            | 36
            | 37
            | 38
            | 39
            | 41
    )
}

fn parse_moxel_format_usize(values: &[Option<&str>; 64], bit: usize) -> Option<usize> {
    values
        .get(bit)
        .and_then(|value| value.and_then(|value| value.trim().parse::<usize>().ok()))
}

fn parse_moxel_format_i32(values: &[Option<&str>; 64], bit: usize) -> Option<i32> {
    values
        .get(bit)
        .and_then(|value| value.and_then(|value| value.trim().parse::<i32>().ok()))
}

fn parse_moxel_format_style_ref(
    values: &[Option<&str>; 64],
    bit: usize,
    style_refs: &[Option<String>],
) -> Option<String> {
    let raw_index = parse_moxel_format_usize(values, bit)?;
    let style_ref_index = remap_moxel_format_style_ref_index(style_refs, raw_index);
    style_refs
        .get(style_ref_index)
        .cloned()
        .flatten()
        .and_then(|style_ref| resolve_moxel_format_style_ref(&style_ref, bit))
        .or_else(|| resolve_moxel_compact_style_ref_index(raw_index, bit))
}

fn remap_moxel_format_style_ref_index(style_refs: &[Option<String>], raw_index: usize) -> usize {
    if raw_index > 0
        && style_refs.len() >= 5
        && style_refs[0].as_deref() == Some("moxel:f527:1:1")
        && style_refs[1].as_deref() == Some("moxel:f527:1:2")
        && style_refs[2].as_deref() == Some("moxel:f527:1:3")
        && style_refs[3].as_deref() == Some("style:FormBackColor")
        && style_refs[4].as_deref() == Some("style:FormTextColor")
    {
        return raw_index + 3;
    }
    raw_index
}

fn resolve_moxel_format_style_ref(style_ref: &str, bit: usize) -> Option<String> {
    if let Some((family, kind)) = parse_moxel_f527_style_ref(style_ref) {
        return match (bit, family, kind) {
            (11, "0", "1") | (5, "0", "1") => Some("style:ToolTipBackColor".to_string()),
            (10, "0", "1") => Some("style:ToolTipTextColor".to_string()),
            (11, "1", "1") | (5, "1", "1") => Some("style:FormBackColor".to_string()),
            (10, "1", "1") => Some("style:FormTextColor".to_string()),
            (11, "1", "2") | (5, "1", "2") => Some("style:FieldBackColor".to_string()),
            (10, "1", "2") => Some("style:FieldTextColor".to_string()),
            (11, "1", "3") | (10, "1", "3") | (5, "1", "3") => {
                Some("style:FieldSelectionBackColor".to_string())
            }
            _ => None,
        };
    }
    Some(style_ref.to_string())
}

fn resolve_moxel_compact_style_ref_index(raw_index: usize, bit: usize) -> Option<String> {
    match (bit, raw_index) {
        (11 | 5, 0) => Some("style:ToolTipBackColor".to_string()),
        (10, 0) => Some("style:ToolTipTextColor".to_string()),
        (11 | 5, 1) => Some("style:FormBackColor".to_string()),
        (10, 1) => Some("style:FormTextColor".to_string()),
        (11 | 5, 2) => Some("style:FieldBackColor".to_string()),
        (10, 2) => Some("style:FieldTextColor".to_string()),
        _ => None,
    }
}

fn parse_moxel_f527_style_ref(style_ref: &str) -> Option<(&str, &str)> {
    let suffix = style_ref.strip_prefix("moxel:f527:")?;
    let (family, kind) = suffix.split_once(':')?;
    Some((family, kind))
}

fn parse_moxel_style_refs(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let mut style_refs = Vec::new();
    let mut index = 0usize;
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
    };
    while index < fields.len() {
        if normalize(fields[index]) == "{1,3,{3,3,{-28}}}" {
            index += 1;
            continue;
        }
        let field = fields[index];
        if let Some(style_ref) = parse_moxel_style_ref_slot(field, object_refs) {
            style_refs.push(style_ref);
            index += 1;
            continue;
        }
        if let Some(overrides) = parse_moxel_indexed_style_ref_overrides(field, object_refs) {
            for (slot_index, style_ref) in overrides {
                if style_refs.len() <= slot_index {
                    style_refs.resize(slot_index + 1, None);
                }
                style_refs[slot_index] = style_ref;
            }
            index += 1;
            continue;
        }
        let wrapped_style_refs = parse_moxel_wrapped_style_refs(field, object_refs);
        if !wrapped_style_refs.is_empty() {
            style_refs.extend(wrapped_style_refs);
            index += 1;
            continue;
        }
        style_refs.extend(parse_moxel_embedded_style_refs(field, object_refs));
        index += 1;
    }
    if style_refs.len() >= 5
        && style_refs.first().is_some_and(Option::is_none)
        && style_refs.get(1).is_some_and(Option::is_none)
        && style_refs.get(2).and_then(|slot| slot.as_deref()) == Some("style:ReportLineColor")
        && style_refs.get(4).and_then(|slot| slot.as_deref()) == Some("auto")
    {
        style_refs[1] = Some("style:FormTextColor".to_string());
    }
    style_refs
}

fn parse_moxel_indexed_style_ref_overrides(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<(usize, Option<String>)>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 5 || fields.first()?.trim() != "3" || fields.get(1)?.trim() != "2" {
        return None;
    }
    let mut overrides = Vec::new();
    let mut cursor = 3usize;
    while cursor + 1 < fields.len() {
        let slot_index = fields.get(cursor)?.trim().parse::<usize>().ok()?;
        let style_ref = parse_moxel_style_ref_slot(fields.get(cursor + 1)?, object_refs)?;
        overrides.push((slot_index, style_ref));
        cursor += 2;
    }
    (!overrides.is_empty()).then_some(overrides)
}

fn parse_moxel_wrapped_style_refs(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    if fields.len() < 3 || fields.first().map(|field| field.trim()) != Some("1") {
        return Vec::new();
    }
    let mut refs = Vec::new();
    for field in fields.iter().skip(2) {
        if let Some(style_ref) = parse_moxel_style_ref_slot(field, object_refs) {
            refs.push(style_ref);
            continue;
        }
        let nested = parse_moxel_embedded_style_refs(field, object_refs);
        if nested.is_empty() {
            return Vec::new();
        }
        refs.extend(nested);
    }
    refs
}

fn parse_moxel_empty_headers_footers(fields: &[&str]) -> bool {
    fields.windows(6).any(|window| {
        window
            .iter()
            .all(|field| parse_moxel_empty_header_footer(field))
    })
}

fn parse_moxel_empty_header_footer(text: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return false;
    };
    if fields.len() != 5 || fields.first().map(|field| field.trim()) != Some("16") {
        return false;
    }
    if fields.get(1).map(|field| field.trim()) != Some("0")
        || fields.get(3).map(|field| field.trim()) != Some("1")
    {
        return false;
    }
    let Some(text_fields) = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return false;
    };
    let Some(format_fields) = fields
        .get(4)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return false;
    };
    text_fields.len() == 2
        && text_fields.first().map(|field| field.trim()) == Some("1")
        && text_fields.get(1).map(|field| field.trim()) == Some("0")
        && format_fields.len() == 3
        && format_fields.first().map(|field| field.trim()) == Some("1")
        && format_fields.get(2).map(|field| field.trim()) == Some("1")
        && format_fields.get(1).and_then(|field| {
            let nested = split_1c_braced_fields(field, 0)?;
            Some(
                nested.len() == 2
                    && nested.first().map(|value| value.trim()) == Some("1")
                    && nested.get(1).map(|value| value.trim()) == Some("0"),
            )
        }) == Some(true)
}

fn parse_moxel_style_ref_slot(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    match fields.get(1)?.trim() {
        "3" => match payload.first()?.trim() {
            "-1" => Some(Some("style:FormBackColor".to_string())),
            "-3" => Some(Some("style:FormTextColor".to_string())),
            "-10" => Some(Some("style:FieldBackColor".to_string())),
            "-11" => Some(Some("style:FieldTextColor".to_string())),
            "-13" => Some(Some("style:FieldTextColor".to_string())),
            "-14" => Some(Some("style:FieldSelectionBackColor".to_string())),
            "-16" => Some(Some("style:SpecialTextColor".to_string())),
            "-17" => Some(Some("style:NegativeTextColor".to_string())),
            "-21" => Some(Some("style:FieldSelectionBackColor".to_string())),
            "-23" => Some(Some("style:ToolTipBackColor".to_string())),
            "-24" => Some(Some("style:ToolTipTextColor".to_string())),
            "-7" => Some(Some("style:ButtonBackColor".to_string())),
            "-15" => Some(Some("style:ButtonTextColor".to_string())),
            "-22" => Some(Some("style:BorderColor".to_string())),
            "-25" => Some(Some("style:ReportHeaderBackColor".to_string())),
            "-26" => Some(Some("style:ReportGroup1BackColor".to_string())),
            "-27" => Some(Some("style:ReportGroup2BackColor".to_string())),
            "-28" => Some(Some("style:ReportLineColor".to_string())),
            "-34" => Some(Some("style:ButtonBorderColor".to_string())),
            "-35" => Some(Some("style:TableHeaderBackColor".to_string())),
            "-36" => Some(Some("style:TableHeaderTextColor".to_string())),
            "-37" => Some(Some("style:TableFooterBackColor".to_string())),
            "-38" => Some(Some("style:TableFooterTextColor".to_string())),
            "-42" => Some(Some("style:NavigationColor".to_string())),
            "-43" => Some(Some("style:AuxiliaryNavigationColor".to_string())),
            "-44" => Some(Some("style:ActivityColor".to_string())),
            "0" => {
                let uuid = parse_uuid_field(payload.get(1)?.trim())?;
                Some(moxel_style_ref_for_uuid(&uuid, object_refs))
            }
            _ => None,
        },
        "4" => match payload.first()?.trim() {
            "0" => Some(Some("auto".to_string())),
            _ => None,
        },
        "2" => payload
            .first()
            .and_then(|value| parse_moxel_web_color(value.trim()))
            .map(Some),
        "0" => payload
            .first()
            .and_then(|value| parse_moxel_style_color(value.trim()))
            .map(Some),
        _ => None,
    }
}

fn parse_moxel_embedded_style_refs(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    if fields.len() < 3
        || fields.get(1).map(|field| field.trim()) != Some("1")
        || !matches!(fields.first().map(|field| field.trim()), Some("3"))
    {
        return Vec::new();
    }
    let container_kind = fields.first().map(|field| field.trim());
    if fields
        .get(2)
        .and_then(|field| parse_moxel_embedded_style_ref(field, container_kind, object_refs))
        .is_none()
    {
        return Vec::new();
    }
    let mut refs = fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_moxel_embedded_style_ref(field, container_kind, object_refs))
        .collect::<Vec<_>>();
    if moxel_uses_sparse_f527_embedded_slots(&fields, &refs) {
        refs = vec![
            refs[0].clone(),
            None,
            refs[1].clone(),
            None,
            refs[2].clone(),
        ];
    }
    refs
}

fn moxel_uses_sparse_f527_embedded_slots(fields: &[&str], refs: &[Option<String>]) -> bool {
    let sparse_wrapper = fields.len() == 10
        && fields[3].trim() == "0"
        && fields[4].trim() == "1"
        && fields[6].trim() == "0"
        && fields[7].trim() == "1"
        && fields[9].trim() == "0";
    if !sparse_wrapper || refs.len() != 3 {
        return false;
    }
    matches!(
        (refs[0].as_deref(), refs[1].as_deref(), refs[2].as_deref(),),
        (
            Some("moxel:f527:0:1"),
            Some("moxel:f527:1:3"),
            Some("moxel:f527:1:1"),
        )
    )
}

fn parse_moxel_embedded_style_ref(
    text: &str,
    container_kind: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 8 || fields.first()?.trim() != "4" || fields.get(1)?.trim() != "0" {
        return None;
    }
    let uuid = parse_uuid_field(fields.get(6)?.trim())?;
    Some(moxel_embedded_style_ref_for_uuid(
        &uuid,
        container_kind,
        fields.get(3).map(|field| field.trim()),
        fields.get(4).map(|field| field.trim()),
        object_refs,
    ))
}

fn moxel_style_ref_for_uuid(uuid: &str, object_refs: &BTreeMap<String, String>) -> Option<String> {
    match uuid {
        "f527dc88-1d39-40b3-bcbb-d98b690ead68" => Some("style:FormBackColor".to_string()),
        _ => object_refs
            .get(uuid)
            .and_then(|reference| reference.strip_prefix("StyleItem."))
            .map(|name| format!("style:{name}")),
    }
}

fn moxel_embedded_style_ref_for_uuid(
    uuid: &str,
    container_kind: Option<&str>,
    family: Option<&str>,
    kind: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    match (uuid, container_kind, family, kind) {
        ("f527dc88-1d39-40b3-bcbb-d98b690ead68", _, Some(family), Some(kind)) => {
            Some(format!("moxel:f527:{family}:{kind}"))
        }
        _ => moxel_style_ref_for_uuid(uuid, object_refs),
    }
}

fn parse_moxel_web_color(value: &str) -> Option<String> {
    let name = match value.parse::<u32>().ok()? {
        8 => "Black",
        10 => "Blue",
        20 => "Cream",
        21 => "Crimson",
        23 => "DarkBlue",
        27 | 31 => "DarkGreen",
        33 => "DarkRed",
        37 => "DarkSlateGray",
        44 => "FireBrick",
        45 => "FloralWhite",
        46 => "ForestGreen",
        48 => "Gainsboro",
        52 => "Gray",
        53 => "Green",
        55 => "HoneyDew",
        64 => "LemonChiffon",
        67 => "LightCyan",
        68 => "LightGoldenRod",
        69 => "LightGoldenRodYellow",
        71 => "LightGray",
        72 => "LightPink",
        79 => "LightYellow",
        84 => "Maroon",
        97 => "MintCream",
        98 => "MistyRose",
        108 => "PaleGoldenrod",
        119 => "Red",
        120 => "RosyBrown",
        121 => "RoyalBlue",
        128 => "Silver",
        130 => "SlateBlue",
        134 => "SteelBlue",
        140 => "Violet",
        141 => "VioletRed",
        144 => "WhiteSmoke",
        145 => "Yellow",
        _ => return None,
    };
    Some(format!("d3p1:{name}"))
}

fn parse_moxel_style_color(value: &str) -> Option<String> {
    match value.parse::<u32>().ok()? {
        12971252 => Some("style:ReportHeaderBackColor".to_string()),
        8765644 => Some("style:ReportLineColor".to_string()),
        _ => parse_moxel_direct_color(value),
    }
}

fn parse_moxel_direct_color(value: &str) -> Option<String> {
    let color = value.parse::<u32>().ok()?;
    let red = color & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = (color >> 16) & 0xff;
    Some(format!("#{red:02X}{green:02X}{blue:02X}"))
}

fn moxel_horizontal_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        2 => Some("Right"),
        6 => Some("Center"),
        7 => Some("Right"),
        _ => None,
    }
}

fn moxel_vertical_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Top"),
        4 | 24 => Some("Center"),
        8 | 48 => Some("Bottom"),
        _ => None,
    }
}

fn moxel_text_placement(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Auto"),
        1 => Some("Cut"),
        2 => Some("Block"),
        3 => Some("Wrap"),
        _ => None,
    }
}

fn moxel_format_pattern(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("WithoutPattern"),
        1 => Some("Solid"),
        _ => None,
    }
}

fn moxel_page_orientation(value: usize) -> Option<&'static str> {
    match value {
        1 => Some("Portrait"),
        2 => Some("Landscape"),
        _ => None,
    }
}

fn moxel_fill_type(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Text"),
        1 => Some("Parameter"),
        2 => Some("Template"),
        _ => None,
    }
}

fn moxel_details_use(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Cell"),
        1 => Some("Row"),
        2 => Some("WithoutProcessing"),
        _ => None,
    }
}

fn moxel_by_selected_columns(value: usize) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn moxel_mask(value: usize) -> Option<&'static str> {
    match value {
        0 => Some(""),
        _ => None,
    }
}

fn moxel_protection(value: usize) -> Option<bool> {
    match value {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

fn moxel_hidden(value: usize) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn moxel_hyper_link(value: usize) -> Option<bool> {
    match value {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

fn moxel_picture_size_mode(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("RealSize"),
        2 => Some("Proportionally"),
        4 => Some("AutoSize"),
        7 => Some("ByFontSize"),
        _ => None,
    }
}

fn moxel_picture_horizontal_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        2 => Some("Right"),
        5 => Some("Auto"),
        6 => Some("Center"),
        _ => None,
    }
}

fn moxel_picture_vertical_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Top"),
        8 => Some("Bottom"),
        24 => Some("Center"),
        _ => None,
    }
}

fn moxel_text_position(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        1 => Some("Right"),
        5 => Some("Auto"),
        _ => None,
    }
}

fn parse_moxel_column_width(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 || fields.first()?.trim() != "128" {
        return None;
    }
    fields.get(1)?.trim().parse::<usize>().ok()
}

fn parse_moxel_line(text: &str) -> Option<MoxelLine> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" || fields.get(1)?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    let style = match payload.first()?.trim() {
        "-1" => "None",
        "-3" => "Solid",
        "-10" => "Dotted",
        "-11" => "Dotted",
        _ => return None,
    };
    Some(MoxelLine {
        style,
        line_type: "v8ui:SpreadsheetDocumentCellLineType",
        width: 1,
    })
}

fn parse_moxel_merge_regions(
    fields: &[&str],
) -> (Vec<MoxelMerge>, Vec<MoxelMerge>, Vec<MoxelMerge>) {
    let mut merges = Vec::new();
    let mut horizontal_unmerges = Vec::new();
    let mut vertical_unmerges = Vec::new();
    for (field_merges, field_horizontal_unmerges, field_vertical_unmerges) in fields
        .iter()
        .filter_map(|field| parse_moxel_merge_region_list(field))
    {
        merges.extend(field_merges);
        horizontal_unmerges.extend(field_horizontal_unmerges);
        vertical_unmerges.extend(field_vertical_unmerges);
    }
    normalize_moxel_merge_order(&mut merges);
    (merges, horizontal_unmerges, vertical_unmerges)
}

fn normalize_moxel_merge_order(merges: &mut Vec<MoxelMerge>) {
    if merges.len() < 2 {
        return;
    }
    let mut ordered = Vec::with_capacity(merges.len());
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row >= 0 && merge.column >= 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row < 0 && merge.column >= 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row >= 0 && merge.column < 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row < 0 && merge.column < 0)
            .cloned(),
    );
    if ordered.len() == merges.len() {
        *merges = ordered;
    }
}

fn parse_moxel_merge_region_list(
    text: &str,
) -> Option<(Vec<MoxelMerge>, Vec<MoxelMerge>, Vec<MoxelMerge>)> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 4096 || fields.len() != count + 1 {
        return None;
    }
    let mut merges = Vec::with_capacity(count);
    let mut horizontal_unmerges = Vec::new();
    let mut vertical_unmerges = Vec::new();
    for field in fields.iter().skip(1) {
        let (merge, kind) = parse_moxel_merge_region(field)?;
        match kind {
            0 => merges.push(merge),
            1 => horizontal_unmerges.push(merge),
            2 => vertical_unmerges.push(merge),
            _ => return None,
        }
    }
    Some((merges, horizontal_unmerges, vertical_unmerges))
}

fn parse_moxel_merge_region(text: &str) -> Option<(MoxelMerge, usize)> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 {
        return None;
    }
    let begin_column = fields.first()?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(1)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    if end_row < begin_row || end_column < begin_column {
        return None;
    }
    let kind = fields
        .get(4)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let columns_id = fields
        .get(5)
        .and_then(|field| parse_non_zero_uuid(field.trim()));
    Some((
        MoxelMerge {
            row: begin_row,
            column: begin_column,
            height: end_row - begin_row,
            width: end_column - begin_column,
            columns_id,
        },
        kind,
    ))
}

fn parse_moxel_area_list(text: &str) -> Option<Vec<MoxelArea>> {
    let items = parse_moxel_named_item_list(text)?;
    let areas = items
        .into_iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect::<Vec<_>>();
    (!areas.is_empty()).then_some(areas)
}

fn parse_moxel_named_item_list(text: &str) -> Option<Vec<MoxelNamedItem>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 512 || fields.len() != count * 2 + 1 {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let name = parse_1c_string(fields.get(index * 2 + 1)?)?;
        if let Some(item) = parse_moxel_named_item(fields.get(index * 2 + 2)?, name) {
            items.push(item);
        }
    }
    (!items.is_empty()).then_some(items)
}

fn parse_moxel_named_item(text: &str, name: String) -> Option<MoxelNamedItem> {
    let fields = split_1c_braced_fields(text, 0)?;
    match fields.first()?.trim() {
        "1" => {
            let bounds = split_1c_braced_fields(fields.get(1)?, 0)?;
            parse_moxel_bounds_area(&bounds, name).map(MoxelNamedItem::Cells)
        }
        "2" => Some(MoxelNamedItem::Drawing {
            name,
            drawing_id: fields.get(1)?.trim().parse::<usize>().ok()?,
        }),
        _ => None,
    }
}

fn parse_moxel_area(text: &str, name: String) -> Option<MoxelArea> {
    match parse_moxel_named_item(text, name)? {
        MoxelNamedItem::Cells(area) => Some(area),
        MoxelNamedItem::Drawing { .. } => None,
    }
}

fn parse_moxel_bounds_area(bounds: &[&str], name: String) -> Option<MoxelArea> {
    let area_type = match bounds.first()?.trim() {
        "1" => "Rows",
        "2" => "Columns",
        "3" => "Rectangle",
        _ => return None,
    };
    Some(MoxelArea {
        name,
        area_type,
        begin_column: bounds.get(1)?.trim().parse::<i32>().ok()?,
        begin_row: bounds.get(2)?.trim().parse::<i32>().ok()?,
        end_column: bounds.get(3)?.trim().parse::<i32>().ok()?,
        end_row: bounds.get(4)?.trim().parse::<i32>().ok()?,
        columns_id: bounds
            .get(5)
            .and_then(|value| parse_non_zero_uuid(value.trim())),
    })
}

fn format_moxel_spreadsheet_xml(spreadsheet: &MoxelSpreadsheet) -> String {
    let output_format_indices = moxel_output_format_indices(spreadsheet);
    let output_format_index_map = moxel_output_format_index_map(&output_format_indices);
    let emit_first_row_format_index =
        moxel_column_format_slots(&spreadsheet.column_sets, spreadsheet.column_count) == 0;
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<languageSettings>\r\n\
\t\t<currentLanguage>ru</currentLanguage>\r\n\
\t\t<defaultLanguage>ru</defaultLanguage>\r\n\
\t\t<languageInfo>\r\n\
\t\t\t<id>ru</id>\r\n\
\t\t\t<code>Русский</code>\r\n\
\t\t\t<description>Русский</description>\r\n\
\t\t</languageInfo>\r\n\
\t</languageSettings>\r\n",
    );
    for column_set in &spreadsheet.column_sets {
        push_moxel_columns_xml(&mut xml, column_set, &output_format_index_map);
    }
    for row in &spreadsheet.rows {
        push_moxel_row_xml(
            &mut xml,
            row,
            &output_format_index_map,
            emit_first_row_format_index,
        );
    }
    for drawing in &spreadsheet.drawings {
        push_moxel_drawing_xml(&mut xml, drawing, &output_format_index_map);
    }
    if let Some(header_footer_format_index) = spreadsheet.header_footer_format_index {
        let header_footer_format_index = output_format_index_map
            .get(&header_footer_format_index)
            .copied()
            .unwrap_or(header_footer_format_index);
        push_moxel_header_footer_format_refs_xml(&mut xml, header_footer_format_index);
    } else if spreadsheet.empty_headers_footers {
        push_moxel_empty_headers_footers_xml(&mut xml);
    }
    xml.push_str("\t<templateMode>true</templateMode>\r\n");
    if let Some(default_format_index) = spreadsheet.default_format_index {
        let default_format_index = output_format_index_map
            .get(&default_format_index)
            .copied()
            .unwrap_or(default_format_index);
        xml.push_str(&format!(
            "\t<defaultFormatIndex>{default_format_index}</defaultFormatIndex>\r\n"
        ));
    }
    xml.push_str(&format!("\t<height>{}</height>\r\n", spreadsheet.height));
    if !spreadsheet.vertical_groups.is_empty() {
        let vg_levels = spreadsheet
            .vertical_groups
            .iter()
            .map(|group| group.level + 1)
            .max()
            .unwrap_or(0);
        if vg_levels > 0 {
            xml.push_str(&format!("\t<vgLevels>{vg_levels}</vgLevels>\r\n"));
        }
    }
    xml.push_str(&format!("\t<vgRows>{}</vgRows>\r\n", spreadsheet.height));
    for group in &spreadsheet.vertical_groups {
        push_moxel_vertical_group_xml(&mut xml, group);
    }
    for merge in &spreadsheet.merges {
        push_moxel_merge_xml(&mut xml, merge);
    }
    for vertical_unmerge in &spreadsheet.vertical_unmerges {
        push_moxel_vertical_unmerge_xml(&mut xml, vertical_unmerge);
    }
    for horizontal_unmerge in &spreadsheet.horizontal_unmerges {
        push_moxel_horizontal_unmerge_xml(&mut xml, horizontal_unmerge);
    }
    for named_item in &spreadsheet.named_items {
        push_moxel_named_item_xml(&mut xml, named_item);
    }
    if let Some(print_area) = &spreadsheet.print_area {
        push_moxel_print_area_xml(&mut xml, print_area);
    }
    if let Some(print_settings) = &spreadsheet.print_settings
        && !print_settings.is_default_margins_only()
    {
        push_moxel_print_settings_xml(&mut xml, print_settings);
    }
    for line in &spreadsheet.lines {
        push_moxel_line_xml(&mut xml, line);
    }
    for font in &spreadsheet.fonts {
        push_moxel_font_xml(&mut xml, font);
    }
    for format_index in output_format_indices {
        push_moxel_format_xml(&mut xml, spreadsheet, format_index);
    }
    for picture in &spreadsheet.pictures {
        push_moxel_picture_xml(&mut xml, picture);
    }
    xml.push_str("</document>");
    xml
}

fn moxel_output_format_count(spreadsheet: &MoxelSpreadsheet) -> usize {
    let max_column_format_index = spreadsheet
        .column_sets
        .iter()
        .flat_map(|column_set| {
            column_set
                .columns
                .iter()
                .map(|column| column.format_index)
                .chain(column_set.default_format_index)
        })
        .max()
        .unwrap_or(0);
    let max_row_or_cell_format_index = spreadsheet.rows.iter().fold(0usize, |max_index, row| {
        let row_max = row.cells.iter().fold(row.format_index, |cell_max, cell| {
            cell_max.max(cell.format_index)
        });
        max_index.max(row_max)
    });
    let max_drawing_format_index = spreadsheet
        .drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    spreadsheet
        .default_format_index
        .unwrap_or(0)
        .max(spreadsheet.header_footer_format_index.unwrap_or(0))
        .max(spreadsheet.extra_formats.keys().copied().max().unwrap_or(0))
        .max(spreadsheet.column_formats.len() + spreadsheet.formats.len())
        .max(max_column_format_index)
        .max(max_row_or_cell_format_index)
        .max(max_drawing_format_index)
        .max(1)
}

fn moxel_sparse_default_column_set_insertion_point(
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
) -> Option<usize> {
    if !spreadsheet
        .column_sets
        .iter()
        .skip(1)
        .any(|column_set| column_set.default_format_index == Some(format_index))
    {
        return None;
    }
    let default_set = spreadsheet.column_sets.first()?;
    let mut seen = BTreeSet::new();
    Some(
        default_set
            .columns
            .iter()
            .filter(|column| seen.insert(column.format_index))
            .count(),
    )
}

fn moxel_sparse_source_output_order(spreadsheet: &MoxelSpreadsheet) -> Option<Vec<usize>> {
    let shared_default_format_index = spreadsheet.header_footer_format_index?;
    let selected_count = spreadsheet.column_formats.len();
    if selected_count == 0 {
        return None;
    }
    if spreadsheet.column_sets.len() == 1
        && shared_default_format_index > selected_count
        && spreadsheet
            .formats
            .get(shared_default_format_index - selected_count - 1)
            .is_some_and(MoxelFormat::is_empty)
        && spreadsheet
            .default_format_index
            .is_some_and(|index| index > shared_default_format_index)
    {
        let format_count = moxel_output_format_count(spreadsheet);
        let mut ordered = Vec::with_capacity(format_count);
        ordered.push(shared_default_format_index);
        for format_index in 1..=selected_count {
            ordered.push(format_index);
        }
        for format_index in (selected_count + 1)..=format_count {
            if format_index != shared_default_format_index {
                ordered.push(format_index);
            }
        }
        return Some(ordered);
    }
    if shared_default_format_index > selected_count
        && spreadsheet
            .column_sets
            .iter()
            .all(|column_set| column_set.default_format_index == Some(shared_default_format_index))
    {
        let format_count = moxel_output_format_count(spreadsheet);
        let mut ordered = Vec::with_capacity(format_count);
        ordered.push(shared_default_format_index);
        for format_index in 1..=selected_count {
            ordered.push(format_index);
        }
        for format_index in (selected_count + 1)..=format_count {
            if format_index != shared_default_format_index {
                ordered.push(format_index);
            }
        }
        return Some(ordered);
    }
    if spreadsheet.default_format_index.is_some() {
        return None;
    }
    if spreadsheet.column_sets.len() <= 1
        || !spreadsheet
            .column_sets
            .iter()
            .skip(1)
            .all(|column_set| column_set.default_format_index == Some(shared_default_format_index))
    {
        return None;
    }
    let default_set_selected_count = spreadsheet
        .column_sets
        .first()?
        .columns
        .iter()
        .map(|column| column.format_index)
        .collect::<BTreeSet<_>>()
        .len();
    let format_count = moxel_output_format_count(spreadsheet);
    let mut ordered = Vec::with_capacity(format_count);
    for format_index in 1..=default_set_selected_count.min(selected_count) {
        ordered.push(format_index);
    }
    if shared_default_format_index > 0 && shared_default_format_index <= format_count {
        ordered.push(shared_default_format_index);
    }
    for format_index in (default_set_selected_count + 1)..=selected_count {
        ordered.push(format_index);
    }
    for format_index in (selected_count + 1)..=format_count {
        if format_index != shared_default_format_index {
            ordered.push(format_index);
        }
    }
    Some(ordered)
}

fn moxel_output_format_indices(spreadsheet: &MoxelSpreadsheet) -> Vec<usize> {
    let format_count = moxel_output_format_count(spreadsheet);
    if let Some(ordered) = moxel_sparse_source_output_order(spreadsheet) {
        return ordered;
    }
    if moxel_source_column_format_offset(&spreadsheet.column_sets) > 0 {
        let source_column_format_refs = moxel_source_column_format_refs(&spreadsheet.column_sets);
        if spreadsheet.column_formats.len() > source_column_format_refs.len() {
            let mut ordered = moxel_source_derived_internal_output_order(
                &spreadsheet.column_sets,
                spreadsheet.column_formats.len(),
                spreadsheet.formats.len(),
            );
            if spreadsheet.default_format_index.is_none()
                && let Some(extra_format_index) = spreadsheet.header_footer_format_index
                && let Some(insert_at) =
                    moxel_sparse_default_column_set_insertion_point(spreadsheet, extra_format_index)
            {
                if let Some(existing_pos) = ordered
                    .iter()
                    .position(|format_index| *format_index == extra_format_index)
                {
                    let format_index = ordered.remove(existing_pos);
                    ordered.insert(insert_at.min(ordered.len()), format_index);
                } else {
                    ordered.insert(insert_at.min(ordered.len()), extra_format_index);
                }
            }
            let mut seen_internal = ordered.iter().copied().collect::<BTreeSet<_>>();
            let mut push_internal = |format_index: usize| {
                if format_index > 0
                    && format_index <= format_count
                    && seen_internal.insert(format_index)
                {
                    ordered.push(format_index);
                }
            };
            for format_index in 1..=format_count {
                push_internal(format_index);
            }

            return ordered;
        }
        return (1..=format_count).collect();
    }
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::with_capacity(format_count);

    let mut push = |format_index: usize| {
        if format_index > 0 && format_index <= format_count && seen.insert(format_index) {
            ordered.push(format_index);
        }
    };

    let prioritize_shared_sparse_defaults = spreadsheet.default_format_index.is_none();
    for column_set in &spreadsheet.column_sets {
        if prioritize_shared_sparse_defaults
            && let Some(default_format_index) = column_set.default_format_index
        {
            push(default_format_index);
        }
        for column in &column_set.columns {
            push(column.format_index);
        }
    }
    for row in &spreadsheet.rows {
        push(row.format_index);
        for cell in &row.cells {
            push(cell.format_index);
        }
    }
    for drawing in &spreadsheet.drawings {
        push(drawing.format_index);
    }
    if prioritize_shared_sparse_defaults
        && let Some(header_footer_format_index) = spreadsheet.header_footer_format_index
    {
        push(header_footer_format_index);
    }
    if prioritize_shared_sparse_defaults
        && let Some(default_format_index) = spreadsheet.default_format_index
    {
        push(default_format_index);
    }
    for format_index in 1..=format_count {
        push(format_index);
    }

    ordered
}

fn moxel_output_format_index_map(output_indices: &[usize]) -> BTreeMap<usize, usize> {
    output_indices
        .iter()
        .enumerate()
        .map(|(new_index, old_index)| (*old_index, new_index + 1))
        .collect()
}

fn push_moxel_columns_xml(
    xml: &mut String,
    column_set: &MoxelColumnSet,
    output_format_index_map: &BTreeMap<usize, usize>,
) {
    xml.push_str("\t<columns>\r\n");
    if let Some(id) = &column_set.id {
        xml.push_str(&format!("\t\t<id>{}</id>\r\n", escape_xml_text(id)));
    }
    if let Some(default_format_index) = column_set.default_format_index {
        let default_format_index = output_format_index_map
            .get(&default_format_index)
            .copied()
            .unwrap_or(default_format_index);
        xml.push_str(&format!(
            "\t\t<formatIndex>{default_format_index}</formatIndex>\r\n"
        ));
    }
    xml.push_str(&format!("\t\t<size>{}</size>\r\n", column_set.size));
    for column in &column_set.columns {
        let column_index = column.index;
        let format_index = output_format_index_map
            .get(&column.format_index)
            .copied()
            .unwrap_or(column.format_index);
        xml.push_str(&format!(
            "\t\t<columnsItem>\r\n\
\t\t\t<index>{column_index}</index>\r\n\
\t\t\t<column>\r\n\
\t\t\t\t<formatIndex>{format_index}</formatIndex>\r\n\
\t\t\t</column>\r\n\
\t\t</columnsItem>\r\n"
        ));
    }
    xml.push_str("\t</columns>\r\n");
}

fn moxel_source_column_format_offset(column_sets: &[MoxelColumnSet]) -> usize {
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| {
            column
                .source_format_index
                .and_then(|source| source.checked_sub(column.format_index))
        })
        .next()
        .unwrap_or(0)
}

fn moxel_source_column_format_refs(column_sets: &[MoxelColumnSet]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for source_format_index in column_sets
        .iter()
        .filter_map(|column_set| column_set.source_default_format_index)
    {
        if source_format_index > 0 && seen.insert(source_format_index) {
            ordered.push(source_format_index);
        }
    }
    for column in column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
    {
        let source_format_index = column.source_format_index.unwrap_or(column.format_index);
        if source_format_index > 0 && seen.insert(source_format_index) {
            ordered.push(source_format_index);
        }
    }
    ordered
}

fn remap_moxel_column_set_output_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_column_format_refs: &[usize],
) {
    if source_column_format_refs.is_empty() {
        return;
    }
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(position) = source_column_format_refs
                .iter()
                .position(|candidate| *candidate == source_format_index)
        {
            column_set.default_format_index = Some(position + 1);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let source_format_index = column.source_format_index.unwrap_or(column.format_index);
        if let Some(position) = source_column_format_refs
            .iter()
            .position(|candidate| *candidate == source_format_index)
        {
            column.format_index = position + 1;
        }
    }
}

fn remap_moxel_row_or_cell_source_format_index(
    format_index: usize,
    source_column_format_refs: &[usize],
    is_row: bool,
) -> usize {
    if source_column_format_refs.is_empty() {
        return format_index;
    }
    if is_row {
        if format_index <= 1 {
            return format_index;
        }
    } else if format_index == 0 {
        return format_index;
    }
    let source_slot = format_index.saturating_sub(1);
    if let Some(position) = source_column_format_refs
        .iter()
        .position(|source_format_index| *source_format_index == source_slot)
    {
        return position + 1;
    }
    let removed_before = source_column_format_refs
        .iter()
        .filter(|source_format_index| **source_format_index < source_slot)
        .count();
    source_slot + source_column_format_refs.len() - removed_before
}

fn moxel_internal_format_index_for_source_index(
    source_format_index: usize,
    column_format_len: usize,
    format_len: usize,
) -> Option<usize> {
    if source_format_index == 0 {
        return None;
    }
    let total_source_formats = column_format_len + format_len;
    if source_format_index > total_source_formats {
        return None;
    }
    let column_source_start = total_source_formats
        .saturating_sub(column_format_len)
        .saturating_add(1);
    if source_format_index >= column_source_start {
        return Some(source_format_index - column_source_start + 1);
    }
    Some(column_format_len + source_format_index)
}

fn moxel_internal_format_index_for_sparse_source_index(
    source_format_index: usize,
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
) -> Option<usize> {
    if source_format_index == 0 {
        return None;
    }
    let total_source_formats = column_format_len + format_len;
    if source_format_index > total_source_formats {
        return None;
    }
    if let Some(position) = source_column_format_refs
        .iter()
        .position(|candidate| *candidate == source_format_index)
    {
        return Some(position + 1);
    }
    let removed_before = source_column_format_refs
        .iter()
        .filter(|candidate| **candidate < source_format_index)
        .count();
    Some(source_column_format_refs.len() + source_format_index - removed_before)
}

fn moxel_source_derived_internal_output_order(
    column_sets: &[MoxelColumnSet],
    column_format_len: usize,
    format_len: usize,
) -> Vec<usize> {
    let total_source_formats = column_format_len + format_len;
    let mut seen_sources = BTreeSet::new();
    let mut seen_internal = BTreeSet::new();
    let mut ordered = Vec::with_capacity(total_source_formats.max(1));

    let mut push_source = |source_format_index: usize| {
        if source_format_index == 0
            || source_format_index > total_source_formats
            || !seen_sources.insert(source_format_index)
        {
            return;
        }
        if let Some(format_index) = moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            format_len,
        ) && seen_internal.insert(format_index)
        {
            ordered.push(format_index);
        }
    };

    for column in column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
    {
        push_source(column.source_format_index.unwrap_or(column.format_index));
    }
    for source_format_index in 1..=total_source_formats {
        push_source(source_format_index);
    }

    ordered
}

fn remap_moxel_column_set_internal_format_indices(
    column_sets: &mut [MoxelColumnSet],
    column_format_len: usize,
    format_len: usize,
) {
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(format_index) = moxel_internal_format_index_for_source_index(
                source_format_index,
                column_format_len,
                format_len,
            )
        {
            column_set.default_format_index = Some(format_index);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let Some(source_format_index) = column.source_format_index else {
            continue;
        };
        if let Some(format_index) = moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            format_len,
        ) {
            column.format_index = format_index;
        }
    }
}

fn remap_moxel_column_set_sparse_internal_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
) {
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
                source_format_index,
                source_column_format_refs,
                column_format_len,
                format_len,
            )
        {
            column_set.default_format_index = Some(format_index);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let Some(source_format_index) = column.source_format_index else {
            continue;
        };
        if let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
            source_format_index,
            source_column_format_refs,
            column_format_len,
            format_len,
        ) {
            column.format_index = format_index;
        }
    }
}

fn remap_moxel_row_and_cell_sparse_source_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
    output_indices: &[usize],
) {
    let output_to_internal = output_indices
        .iter()
        .enumerate()
        .map(|(index, internal)| (index + 1, *internal))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            let output_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                true,
            );
            if let Some(format_index) = output_to_internal.get(&output_index).copied() {
                row.format_index = format_index;
            }
        }
        for cell in &mut row.cells {
            let Some(source_format_index) = cell.source_format_index else {
                continue;
            };
            let output_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                false,
            );
            if let Some(format_index) = output_to_internal.get(&output_index).copied() {
                cell.format_index = format_index;
            }
        }
    }
}

fn remap_moxel_row_and_cell_sparse_internal_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            if source_format_index <= 1 {
                row.format_index = source_format_index;
            } else if let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
                source_format_index - 1,
                source_column_format_refs,
                column_format_len,
                format_len,
            ) {
                row.format_index = format_index;
            }
        }
        for cell in &mut row.cells {
            let Some(source_format_index) = cell.source_format_index else {
                continue;
            };
            if source_format_index == 0 {
                cell.format_index = 0;
            } else if let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
                source_format_index - 1,
                source_column_format_refs,
                column_format_len,
                format_len,
            ) {
                cell.format_index = format_index;
            }
        }
    }
}

fn remap_moxel_row_and_cell_output_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            row.format_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                true,
            );
        }
        for cell in &mut row.cells {
            let Some(source_format_index) = cell.source_format_index else {
                continue;
            };
            cell.format_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                false,
            );
        }
    }
}

fn normalize_moxel_zero_column_format_refs(rows: &mut [MoxelRow]) {
    for row in rows {
        if row.format_index > 0 {
            row.format_index -= 1;
        }
        row.source_format_index = Some(row.format_index);
        for cell in &mut row.cells {
            if cell.format_index > 0 {
                cell.format_index -= 1;
            }
            cell.source_format_index = if cell.format_index == 0 {
                None
            } else {
                Some(cell.format_index)
            };
        }
    }
}

fn restore_moxel_source_format_refs_without_format_table(rows: &mut [MoxelRow]) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            row.format_index = source_format_index;
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                cell.format_index = source_format_index;
            }
        }
    }
}

fn moxel_uses_sparse_source_format_refs(
    column_sets: &[MoxelColumnSet],
    column_count: usize,
    _rows: &[MoxelRow],
    _default_format: &MoxelFormat,
    _default_format_width: Option<usize>,
) -> bool {
    let column_format_slots = moxel_column_format_slots(column_sets, column_count);
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| column.source_format_index)
        .any(|source_format_index| source_format_index > column_format_slots)
}

fn push_moxel_empty_headers_footers_xml(xml: &mut String) {
    for tag in [
        "leftHeader",
        "centerHeader",
        "rightHeader",
        "leftFooter",
        "centerFooter",
        "rightFooter",
    ] {
        xml.push_str(&format!(
            "\t<{tag}>\r\n\t\t<f>0</f>\r\n\t\t<tfl/>\r\n\t</{tag}>\r\n"
        ));
    }
}

fn push_moxel_header_footer_format_refs_xml(xml: &mut String, format_index: usize) {
    for tag in [
        "leftHeader",
        "centerHeader",
        "rightHeader",
        "leftFooter",
        "centerFooter",
        "rightFooter",
    ] {
        xml.push_str(&format!(
            "\t<{tag}>\r\n\t\t<f>{format_index}</f>\r\n\t</{tag}>\r\n"
        ));
    }
}

fn push_moxel_print_settings_xml(xml: &mut String, settings: &MoxelPrintSettings) {
    xml.push_str("\t<printSettings>\r\n");
    push_moxel_format_text(xml, "pageOrientation", settings.page_orientation);
    push_moxel_format_usize(xml, "scale", settings.scale);
    push_moxel_format_bool(xml, "collate", settings.collate);
    push_moxel_format_usize(xml, "copies", settings.copies);
    push_moxel_format_usize(xml, "perPage", settings.per_page);
    push_moxel_format_usize(xml, "topMargin", settings.top_margin);
    push_moxel_format_usize(xml, "leftMargin", settings.left_margin);
    push_moxel_format_usize(xml, "bottomMargin", settings.bottom_margin);
    push_moxel_format_usize(xml, "rightMargin", settings.right_margin);
    push_moxel_format_usize(xml, "headerSize", settings.header_size);
    push_moxel_format_usize(xml, "footerSize", settings.footer_size);
    push_moxel_format_bool(xml, "fitToPage", settings.fit_to_page);
    push_moxel_format_bool(xml, "blackAndWhite", settings.black_and_white);
    push_moxel_format_text(xml, "printerName", settings.printer_name.as_deref());
    push_moxel_format_usize(xml, "paper", settings.paper);
    push_moxel_format_usize(xml, "paperSource", settings.paper_source);
    push_moxel_format_usize(xml, "pageWidth", settings.page_width);
    push_moxel_format_usize(xml, "pageHeight", settings.page_height);
    xml.push_str("\t</printSettings>\r\n");
}

impl MoxelPrintSettings {
    fn is_default_margins_only(&self) -> bool {
        self.page_orientation.is_none()
            && self.scale.is_none()
            && self.collate.is_none()
            && self.copies.is_none()
            && self.per_page.is_none()
            && self.top_margin == Some(1000)
            && self.left_margin == Some(1000)
            && self.bottom_margin == Some(1000)
            && self.right_margin == Some(1000)
            && self.header_size == Some(1000)
            && self.footer_size == Some(1000)
            && self.fit_to_page.is_none()
            && self.black_and_white.is_none()
            && self.printer_name.is_none()
            && self.paper.is_none()
            && self.paper_source.is_none()
            && self.page_width.is_none()
            && self.page_height.is_none()
    }
}

fn push_moxel_format_xml(xml: &mut String, spreadsheet: &MoxelSpreadsheet, format_index: usize) {
    let format = moxel_format_for_index(spreadsheet, format_index);
    if format.is_empty() {
        xml.push_str("\t<format/>\r\n");
        return;
    };
    xml.push_str("\t<format>\r\n");
    push_moxel_format_usize(xml, "font", format.font);
    push_moxel_format_usize(xml, "border", format.border);
    if format.border.is_none() {
        push_moxel_format_usize(xml, "leftBorder", format.left_border);
        push_moxel_format_usize(xml, "topBorder", format.top_border);
        push_moxel_format_usize(xml, "rightBorder", format.right_border);
        push_moxel_format_usize(xml, "bottomBorder", format.bottom_border);
    }
    push_moxel_format_i32(xml, "height", format.height);
    push_moxel_format_text(xml, "borderColor", format.border_color.as_deref());
    push_moxel_format_usize(xml, "width", format.width);
    push_moxel_format_usize(xml, "widthWeightFactor", format.width_weight_factor);
    push_moxel_format_usize(xml, "drawingBorder", format.drawing_border);
    push_moxel_format_text(xml, "horizontalAlignment", format.horizontal_alignment);
    push_moxel_format_text(xml, "verticalAlignment", format.vertical_alignment);
    push_moxel_format_text(xml, "textColor", format.text_color.as_deref());
    push_moxel_format_text(xml, "backColor", format.back_color.as_deref());
    push_moxel_format_text(xml, "pattern", format.pattern);
    push_moxel_format_text(xml, "textPlacement", format.text_placement);
    push_moxel_format_usize(xml, "textOrientation", format.text_orientation);
    push_moxel_format_text(xml, "fillType", format.fill_type);
    push_moxel_localized_values_xml(
        xml,
        "format",
        &format.number_format,
        format.number_format_present,
    );
    push_moxel_localized_values_xml(
        xml,
        "editFormat",
        &format.edit_format,
        format.edit_format_present,
    );
    if let Some(by_selected_columns) = format.by_selected_columns {
        xml.push_str(&format!(
            "\t\t<bySelectedColumns>{by_selected_columns}</bySelectedColumns>\r\n"
        ));
    }
    push_moxel_format_text(xml, "detailsUse", format.details_use);
    if let Some(hyper_link) = format.hyper_link {
        xml.push_str(&format!("\t\t<hyperLink>{hyper_link}</hyperLink>\r\n"));
    }
    if let Some(protection) = format.protection {
        xml.push_str(&format!("\t\t<protection>{protection}</protection>\r\n"));
    }
    if let Some(hidden) = format.hidden {
        xml.push_str(&format!("\t\t<hidden>{hidden}</hidden>\r\n"));
    }
    push_moxel_format_usize(xml, "indent", format.indent);
    push_moxel_format_usize(xml, "autoIndent", format.auto_indent);
    if let Some(mask) = format.mask {
        if mask.is_empty() {
            xml.push_str("\t\t<mask/>\r\n");
        } else {
            xml.push_str(&format!(
                "\t\t<mask>{}</mask>\r\n",
                escape_xml_element_text(mask)
            ));
        }
    }
    push_moxel_format_usize(xml, "picIndex", format.pic_index);
    push_moxel_format_text(xml, "pictureSizeMode", format.picture_size_mode);
    push_moxel_format_text(
        xml,
        "picHorizontalAlignment",
        format.pic_horizontal_alignment,
    );
    push_moxel_format_text(xml, "picVerticalAlignment", format.pic_vertical_alignment);
    push_moxel_format_text(xml, "textPosition", format.text_position);
    xml.push_str("\t</format>\r\n");
}

fn push_moxel_localized_values_xml(
    xml: &mut String,
    tag: &str,
    values: &[MoxelLocalizedValue],
    present: bool,
) {
    if values.is_empty() && !present {
        return;
    }
    if values.is_empty() {
        xml.push_str(&format!("\t\t<{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!("\t\t<{tag}>\r\n"));
    for value in values {
        xml.push_str("\t\t\t<v8:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(&value.lang)
        ));
        xml.push_str(&format!(
            "\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&value.content)
        ));
        xml.push_str("\t\t\t</v8:item>\r\n");
    }
    xml.push_str(&format!("\t\t</{tag}>\r\n"));
}

fn moxel_format_for_index(spreadsheet: &MoxelSpreadsheet, format_index: usize) -> MoxelFormat {
    let column_format_slots = spreadsheet
        .column_formats
        .len()
        .max(moxel_column_format_slots(
            &spreadsheet.column_sets,
            spreadsheet.column_count,
        ));
    if let Some(format) = spreadsheet
        .column_formats
        .get(format_index.saturating_sub(1))
        .cloned()
    {
        return format;
    }
    if let Some(format) = spreadsheet.extra_formats.get(&format_index).cloned() {
        return format;
    }
    if spreadsheet.default_format_index == Some(format_index) {
        if spreadsheet.column_sets.len() == 1
            && spreadsheet.header_footer_format_index == Some(format_index)
            && format_index > column_format_slots
            && let Some(format) = spreadsheet
                .formats
                .get(format_index - column_format_slots - 1)
                .cloned()
        {
            return format;
        }
        let mut format = spreadsheet.default_format.clone();
        if format.width.is_none() {
            format.width = spreadsheet.default_format_width;
        }
        if !format.is_empty() {
            return format;
        }
        return MoxelFormat {
            width: spreadsheet.default_format_width,
            ..MoxelFormat::default()
        };
    }
    if format_index <= column_format_slots {
        return MoxelFormat::default();
    }
    spreadsheet
        .formats
        .get(format_index - column_format_slots - 1)
        .cloned()
        .unwrap_or_default()
}

fn push_moxel_format_usize(xml: &mut String, tag: &str, value: Option<usize>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{value}</{tag}>\r\n"));
    }
}

fn push_moxel_format_i32(xml: &mut String, tag: &str, value: Option<i32>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{value}</{tag}>\r\n"));
    }
}

fn push_moxel_format_bool(xml: &mut String, tag: &str, value: Option<bool>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{}</{tag}>\r\n", xml_bool(value)));
    }
}

fn push_moxel_format_text(xml: &mut String, tag: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "\t\t<{tag}>{}</{tag}>\r\n",
            escape_xml_element_text(value)
        ));
    }
}

fn push_moxel_picture_xml(xml: &mut String, picture: &MoxelPicture) {
    xml.push_str("\t<picture>\r\n");
    xml.push_str(&format!("\t\t<index>{}</index>\r\n", picture.index));
    if let Some(payload) = &picture.payload {
        xml.push_str(&format!(
            "\t\t<picture t=\"false\">{}</picture>\r\n",
            escape_xml_text(payload)
        ));
    } else if let Some(ref_name) = &picture.ref_name {
        xml.push_str(&format!(
            "\t\t<picture t=\"false\" ref=\"{}\"/>\r\n",
            escape_xml_text(ref_name)
        ));
    } else {
        xml.push_str("\t\t<picture/>\r\n");
    }
    xml.push_str("\t</picture>\r\n");
}

fn push_moxel_drawing_xml(
    xml: &mut String,
    drawing: &MoxelDrawing,
    output_format_index_map: &BTreeMap<usize, usize>,
) {
    xml.push_str("\t<drawing>\r\n");
    xml.push_str("\t\t<drawingType>Picture</drawingType>\r\n");
    xml.push_str(&format!("\t\t<id>{}</id>\r\n", drawing.id));
    let format_index = output_format_index_map
        .get(&drawing.format_index)
        .copied()
        .unwrap_or(drawing.format_index);
    xml.push_str(&format!(
        "\t\t<formatIndex>{}</formatIndex>\r\n",
        format_index
    ));
    xml.push_str(&format!(
        "\t\t<beginRow>{}</beginRow>\r\n",
        drawing.begin_row
    ));
    xml.push_str(&format!(
        "\t\t<beginRowOffset>{}</beginRowOffset>\r\n",
        drawing.begin_row_offset
    ));
    xml.push_str(&format!("\t\t<endRow>{}</endRow>\r\n", drawing.end_row));
    xml.push_str(&format!(
        "\t\t<endRowOffset>{}</endRowOffset>\r\n",
        drawing.end_row_offset
    ));
    xml.push_str(&format!(
        "\t\t<beginColumn>{}</beginColumn>\r\n",
        drawing.begin_column
    ));
    xml.push_str(&format!(
        "\t\t<beginColumnOffset>{}</beginColumnOffset>\r\n",
        drawing.begin_column_offset
    ));
    xml.push_str(&format!(
        "\t\t<endColumn>{}</endColumn>\r\n",
        drawing.end_column
    ));
    xml.push_str(&format!(
        "\t\t<endColumnOffset>{}</endColumnOffset>\r\n",
        drawing.end_column_offset
    ));
    xml.push_str(&format!(
        "\t\t<autoSize>{}</autoSize>\r\n",
        xml_bool(drawing.auto_size)
    ));
    xml.push_str(&format!(
        "\t\t<pictureSize>{}</pictureSize>\r\n",
        drawing.picture_size
    ));
    xml.push_str(&format!("\t\t<zOrder>{}</zOrder>\r\n", drawing.z_order));
    xml.push_str(&format!(
        "\t\t<pictureIndex>{}</pictureIndex>\r\n",
        drawing.picture_index
    ));
    xml.push_str("\t</drawing>\r\n");
}

fn push_moxel_merge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<merge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</merge>\r\n");
}

fn push_moxel_vertical_group_xml(xml: &mut String, group: &MoxelVerticalGroup) {
    xml.push_str("\t<vg>\r\n");
    xml.push_str(&format!("\t\t<b>{}</b>\r\n", group.begin_row));
    if group.end_row != group.begin_row {
        xml.push_str(&format!("\t\t<e>{}</e>\r\n", group.end_row));
    }
    xml.push_str("\t</vg>\r\n");
}

fn push_moxel_vertical_unmerge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<verticalUnmerge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</verticalUnmerge>\r\n");
}

fn push_moxel_horizontal_unmerge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<horizontalUnmerge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</horizontalUnmerge>\r\n");
}

fn push_moxel_merge_body_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str(&format!("\t\t<r>{}</r>\r\n", merge.row));
    xml.push_str(&format!("\t\t<c>{}</c>\r\n", merge.column));
    if merge.height > 0 {
        xml.push_str(&format!("\t\t<h>{}</h>\r\n", merge.height));
    }
    if merge.width > 0 {
        xml.push_str(&format!("\t\t<w>{}</w>\r\n", merge.width));
    }
    if let Some(columns_id) = &merge.columns_id {
        xml.push_str(&format!("\t\t<columnsID>{columns_id}</columnsID>\r\n"));
    }
}

fn push_moxel_line_xml(xml: &mut String, line: &MoxelLine) {
    xml.push_str(&format!(
        "\t<line width=\"{}\" gap=\"false\">\r\n",
        line.width
    ));
    xml.push_str(&format!(
        "\t\t<v8ui:style xsi:type=\"{}\">{}</v8ui:style>\r\n",
        line.line_type, line.style
    ));
    xml.push_str("\t</line>\r\n");
}

fn push_moxel_font_xml(xml: &mut String, font: &MoxelFont) {
    xml.push_str("\t<font");
    if let Some(ref_name) = &font.ref_name {
        if ref_name.starts_with("sys:") {
            xml.push_str(" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"");
        }
        xml.push_str(&format!(" ref=\"{}\"", escape_xml_text(ref_name)));
    }
    if let Some(face_name) = &font.face_name {
        xml.push_str(&format!(" faceName=\"{}\"", escape_xml_text(face_name)));
    }
    if let Some(height) = &font.height {
        xml.push_str(&format!(" height=\"{}\"", escape_xml_text(height)));
    }
    if font.kind == "WindowsFont" {
        xml.push_str(&format!(" bold=\"{}\"", font.bold));
        if font.italic {
            xml.push_str(" italic=\"true\"");
        }
        if font.underline {
            xml.push_str(" underline=\"true\"");
        }
        if font.strikeout {
            xml.push_str(" strikeout=\"true\"");
        }
        xml.push_str(" kind=\"WindowsFont\"");
    } else if font.kind == "StyleItem"
        && !font.bold
        && !font.italic
        && !font.underline
        && !font.strikeout
        && font.scale.is_none()
    {
        xml.push_str(" kind=\"StyleItem\"");
    } else {
        xml.push_str(&format!(
            " bold=\"{}\" italic=\"{}\" underline=\"{}\" strikeout=\"{}\" kind=\"{}\"",
            font.bold, font.italic, font.underline, font.strikeout, font.kind
        ));
        if let Some(scale) = font.scale {
            xml.push_str(&format!(" scale=\"{scale}\""));
        }
    }
    xml.push_str("/>\r\n");
}

fn push_moxel_named_item_xml(xml: &mut String, named_item: &MoxelNamedItem) {
    match named_item {
        MoxelNamedItem::Cells(area) => push_moxel_area_xml(xml, area),
        MoxelNamedItem::Drawing { name, drawing_id } => {
            xml.push_str("\t<namedItem xsi:type=\"NamedItemDrawing\">\r\n");
            xml.push_str(&format!(
                "\t\t<name>{}</name>\r\n",
                escape_xml_element_text(name)
            ));
            xml.push_str(&format!("\t\t<drawingID>{drawing_id}</drawingID>\r\n"));
            xml.push_str("\t</namedItem>\r\n");
        }
    }
}

fn push_moxel_area_xml(xml: &mut String, area: &MoxelArea) {
    xml.push_str("\t<namedItem xsi:type=\"NamedItemCells\">\r\n");
    xml.push_str(&format!(
        "\t\t<name>{}</name>\r\n",
        escape_xml_element_text(&area.name)
    ));
    xml.push_str("\t\t<area>\r\n");
    xml.push_str(&format!("\t\t\t<type>{}</type>\r\n", area.area_type));
    xml.push_str(&format!(
        "\t\t\t<beginRow>{}</beginRow>\r\n",
        area.begin_row
    ));
    xml.push_str(&format!("\t\t\t<endRow>{}</endRow>\r\n", area.end_row));
    xml.push_str(&format!(
        "\t\t\t<beginColumn>{}</beginColumn>\r\n",
        area.begin_column
    ));
    xml.push_str(&format!(
        "\t\t\t<endColumn>{}</endColumn>\r\n",
        area.end_column
    ));
    if let Some(columns_id) = &area.columns_id {
        xml.push_str(&format!(
            "\t\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    xml.push_str("\t\t</area>\r\n");
    xml.push_str("\t</namedItem>\r\n");
}

fn push_moxel_print_area_xml(xml: &mut String, area: &MoxelArea) {
    xml.push_str("\t<printArea>\r\n");
    xml.push_str(&format!("\t\t<type>{}</type>\r\n", area.area_type));
    xml.push_str(&format!("\t\t<beginRow>{}</beginRow>\r\n", area.begin_row));
    xml.push_str(&format!("\t\t<endRow>{}</endRow>\r\n", area.end_row));
    xml.push_str(&format!(
        "\t\t<beginColumn>{}</beginColumn>\r\n",
        area.begin_column
    ));
    xml.push_str(&format!(
        "\t\t<endColumn>{}</endColumn>\r\n",
        area.end_column
    ));
    if let Some(columns_id) = &area.columns_id {
        xml.push_str(&format!(
            "\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    xml.push_str("\t</printArea>\r\n");
}

fn push_moxel_row_xml(
    xml: &mut String,
    row: &MoxelRow,
    output_format_index_map: &BTreeMap<usize, usize>,
    emit_first_format_index: bool,
) {
    xml.push_str(&format!(
        "\t<rowsItem>\r\n\t\t<index>{}</index>\r\n",
        row.index
    ));
    if let Some(index_to) = row.index_to {
        xml.push_str(&format!("\t\t<indexTo>{index_to}</indexTo>\r\n"));
    }
    xml.push_str("\t\t<row>\r\n");
    let format_index = output_format_index_map
        .get(&row.format_index)
        .copied()
        .unwrap_or(row.format_index);
    if let Some(columns_id) = &row.columns_id {
        xml.push_str(&format!(
            "\t\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    let explicit_source_format_collapsed_to_one = format_index == 1
        && row
            .source_format_index
            .is_some_and(|source_format_index| source_format_index > 1);
    let leading_shared_default_shifted_row_format = format_index == 2
        && row.format_index == 1
        && row.source_format_index == Some(1)
        && output_format_index_map.get(&1).copied() == Some(2);
    if format_index > 1 && !leading_shared_default_shifted_row_format
        || (emit_first_format_index && format_index == 1)
        || explicit_source_format_collapsed_to_one
    {
        xml.push_str(&format!(
            "\t\t\t<formatIndex>{format_index}</formatIndex>\r\n"
        ));
    }
    if row.cells.is_empty() {
        xml.push_str("\t\t\t<empty>true</empty>\r\n");
        xml.push_str("\t\t</row>\r\n\t</rowsItem>\r\n");
        return;
    }
    let mut expected_column = 0usize;
    for cell in &row.cells {
        xml.push_str("\t\t\t<c>\r\n");
        if cell.column_index != expected_column {
            xml.push_str(&format!("\t\t\t\t<i>{}</i>\r\n", cell.column_index));
        }
        xml.push_str("\t\t\t\t<c>\r\n");
        let cell_format_index = if cell.format_index == 0 {
            0
        } else {
            output_format_index_map
                .get(&cell.format_index)
                .copied()
                .unwrap_or(cell.format_index)
        };
        xml.push_str(&format!("\t\t\t\t\t<f>{cell_format_index}</f>\r\n"));
        if let Some(text) = &cell.text {
            xml.push_str("\t\t\t\t\t<tl>\r\n");
            xml.push_str("\t\t\t\t\t\t<v8:item>\r\n");
            xml.push_str("\t\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(text)
            ));
            xml.push_str("\t\t\t\t\t\t</v8:item>\r\n");
            xml.push_str("\t\t\t\t\t</tl>\r\n");
        } else if cell.empty_text {
            xml.push_str("\t\t\t\t\t<tl/>\r\n");
        }
        if let Some(parameter) = &cell.parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<parameter>{}</parameter>\r\n",
                escape_xml_element_text(parameter)
            ));
        }
        if let Some(detail_parameter) = &cell.detail_parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<detailParameter>{}</detailParameter>\r\n",
                escape_xml_element_text(detail_parameter)
            ));
        }
        xml.push_str("\t\t\t\t</c>\r\n");
        xml.push_str("\t\t\t</c>\r\n");
        expected_column = cell.column_index + 1;
    }
    xml.push_str("\t\t</row>\r\n\t</rowsItem>\r\n");
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
    let form_refs = build_form_source_reference_index_from_texts(metadata_texts);
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
    for command in nested_command_headers_from_text(text, owner_uuid) {
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

fn contains_any_metadata_header_between(text: &str, start: usize, end: usize) -> bool {
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
        if is_uuid_text(uuid) && is_metadata_header_marker(text, uuid_end) {
            return true;
        }
    }
    false
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
        ("AccumulationRegister", "1")
        | ("AccountingRegister", "1")
        | ("CalculationRegister", "1")
        | ("InformationRegister", "1") => Some("RecordSetModule.bsl"),
        ("AccumulationRegister", "2")
        | ("AccountingRegister", "2")
        | ("CalculationRegister", "2")
        | ("InformationRegister", "2") => Some("ManagerModule.bsl"),
        ("DocumentJournal", "1") => Some("ManagerModule.bsl"),
        ("Task", "6") => Some("ObjectModule.bsl"),
        ("Task", "7") => Some("ManagerModule.bsl"),
        ("BusinessProcess", "6") => Some("ObjectModule.bsl"),
        ("BusinessProcess", "8") => Some("ManagerModule.bsl"),
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

struct SubsystemProperties {
    include_help_in_contents: bool,
    include_in_command_interface: bool,
    use_one_command: bool,
    child_subsystems: Vec<String>,
}

struct ExchangePlanProperties {
    this_node: Option<String>,
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    child_objects: Vec<MetadataChildObject>,
}

struct RegisterProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    register_type: Option<&'static str>,
    include_help_in_contents: Option<bool>,
    data_lock_control_mode: Option<&'static str>,
    full_text_search: Option<&'static str>,
    default_record_form: Option<String>,
    default_list_form: Option<String>,
    auxiliary_record_form: Option<String>,
    auxiliary_list_form: Option<String>,
    standard_attributes: Vec<RegisterStandardAttribute>,
    child_objects: Vec<MetadataChildObject>,
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
    quick_choice: Option<&'static str>,
    create_on_input: Option<&'static str>,
    choice_form: Option<MetadataChoiceForm>,
    link_by_type_empty: bool,
    choice_history_on_input: Option<&'static str>,
    use_mode: Option<&'static str>,
    indexing: Option<&'static str>,
    full_text_search: Option<&'static str>,
    data_history: Option<&'static str>,
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
    Boolean(bool),
    DesignTimeRef(String),
    FixedArray(Vec<String>),
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
}

struct BusinessProcessProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
}

struct TaskProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
}

struct SettingsStorageProperties {
    generated_types: Vec<GeneratedTypeEntry>,
}

struct EnumProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
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
    command_parameter_types: Vec<String>,
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
    compatibility_mode: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationRootReference {
    value: Option<String>,
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
}

#[allow(dead_code)]
fn build_metadata_type_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_type_index_from_texts(&metadata_texts)
}

fn build_metadata_type_index_from_texts(rows: &[MetadataTextRow]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        let entries = parse_generated_type_entries_from_text(row)
            .or_else(|| parse_generated_type_entries_from_source_xml_text(&row.text));
        let Some(entries) = entries else { continue };
        for (type_id, reference) in entries {
            index.insert(type_id.to_ascii_lowercase(), reference);
        }
    }
    index
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
    let text = row.text.as_str();
    let object_code = row.object_code?;
    let header = row.header.as_ref()?;
    let root_fields = split_1c_braced_fields(text, 0)?;
    let object_text = *root_fields.get(1)?;
    let fields = split_1c_braced_fields(object_text, 0)?;
    let mut entries = Vec::new();
    let header_index = metadata_header_field_index(&fields, &row.file_name);

    if object_code == 0 {
        push_indexed_generated_type(&mut entries, &fields, 1, "DefinedType", &header.name);
    }
    if object_code == 16 {
        push_indexed_generated_type(&mut entries, &fields, 2, "ConstantManager", &header.name);
        push_indexed_generated_type(
            &mut entries,
            &fields,
            4,
            "ConstantValueManager",
            &header.name,
        );
    }
    if object_code == 30 {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            3,
            "BusinessProcessObject",
            &header.name,
        );
        push_indexed_generated_type(&mut entries, &fields, 5, "BusinessProcessRef", &header.name);
        push_indexed_generated_type(
            &mut entries,
            &fields,
            11,
            "BusinessProcessManager",
            &header.name,
        );
    }
    if object_code == 37 {
        push_indexed_generated_type(&mut entries, &fields, 1, "ExchangePlanObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "ExchangePlanRef", &header.name);
        push_indexed_generated_type(
            &mut entries,
            &fields,
            9,
            "ExchangePlanManager",
            &header.name,
        );
    }
    if object_code == 40 {
        push_indexed_generated_type(&mut entries, &fields, 1, "DocumentObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "DocumentRef", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 26, "DocumentManager", &header.name);
    }
    if object_code == 17 {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            1,
            "DataProcessorObject",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            7,
            "DataProcessorManager",
            &header.name,
        );
    }
    if object_code == 19 {
        push_indexed_generated_type(&mut entries, &fields, 12, "ReportManager", &header.name);
    }
    if object_code == 2 && header_index == Some(1) && field_starts_with(fields.get(1), "{0,") {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            2,
            "SettingsStorageManager",
            &header.name,
        );
    }
    if object_code == 57 {
        push_indexed_generated_type(&mut entries, &fields, 1, "CatalogObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "CatalogRef", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 5, "CatalogSelection", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 7, "CatalogList", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 34, "CatalogManager", &header.name);
    }

    if object_code == 20 && header_index == Some(5) {
        if let Some(type_id) = fields.get(1).copied().and_then(parse_uuid_field) {
            entries.push((type_id, format!("cfg:EnumRef.{}", header.name)));
        }
    }
    if object_code == 21 {
        push_indexed_register_generated_type_entries(
            &mut entries,
            &fields,
            1,
            "CalculationRegister",
            &header.name,
        );
    }
    if object_code == 22 && header_index != Some(1) && field_is_unsigned_integer(fields.get(1)) {
        push_indexed_register_generated_type_entries(
            &mut entries,
            &fields,
            2,
            "AccountingRegister",
            &header.name,
        );
    }
    if object_code == 28 {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            9,
            "AccumulationRegisterRecordSet",
            &header.name,
        );
    }
    if object_code == 33 && fields.get(1).copied().and_then(parse_uuid_field).is_some() {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            1,
            "InformationRegisterRecord",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            3,
            "InformationRegisterManager",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            5,
            "InformationRegisterSelection",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            7,
            "InformationRegisterList",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            9,
            "InformationRegisterRecordSet",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            11,
            "InformationRegisterRecordKey",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            13,
            "InformationRegisterRecordManager",
            &header.name,
        );
    }
    if object_code == 33 && header_index == Some(1) {
        push_indexed_generated_type(&mut entries, &fields, 3, "TaskObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 5, "TaskRef", &header.name);
    }
    if object_code == 34 {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            1,
            "ChartOfCharacteristicTypesObject",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            3,
            "ChartOfCharacteristicTypesRef",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            5,
            "ChartOfCharacteristicTypesSelection",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            7,
            "ChartOfCharacteristicTypesList",
            &header.name,
        );
        push_indexed_generated_type(&mut entries, &fields, 9, "Characteristic", &header.name);
        push_indexed_generated_type(
            &mut entries,
            &fields,
            11,
            "ChartOfCharacteristicTypesManager",
            &header.name,
        );
    }
    if object_code == 32 {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            1,
            "ChartOfAccountsObject",
            &header.name,
        );
        push_indexed_generated_type(&mut entries, &fields, 3, "ChartOfAccountsRef", &header.name);
        push_indexed_generated_type(
            &mut entries,
            &fields,
            5,
            "ChartOfAccountsSelection",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            7,
            "ChartOfAccountsList",
            &header.name,
        );
        push_indexed_generated_type(
            &mut entries,
            &fields,
            9,
            "ChartOfAccountsManager",
            &header.name,
        );
    }

    Some(entries)
}

fn parse_generated_type_entries_from_source_xml_text(text: &str) -> Option<Vec<(String, String)>> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if !text.starts_with('<') || !text.contains("GeneratedType") {
        return None;
    }

    let mut reader = NsReader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut entries = Vec::new();
    let mut current_name = None::<String>;
    let mut in_generated_type = false;
    let mut in_type_id = false;

    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (_, local) = reader.resolve_element(event.name());
                let local = local.as_ref();
                if local == b"GeneratedType" {
                    current_name = xml_attribute_value_ns(&reader, &event, "name")?;
                    in_generated_type = current_name.is_some();
                } else if in_generated_type && local == b"TypeId" {
                    in_type_id = true;
                }
            }
            Event::Empty(event) => {
                let (_, local) = reader.resolve_element(event.name());
                if local.as_ref() == b"GeneratedType" {
                    current_name = None;
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
                        entries.push((type_id.to_string(), format!("cfg:{name}")));
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
    entries: &mut Vec<(String, String)>,
    fields: &[&str],
    index: usize,
    generated_type: &str,
    name: &str,
) {
    if let Some(type_id) = fields.get(index).copied().and_then(parse_uuid_field) {
        entries.push((type_id, format!("cfg:{generated_type}.{name}")));
    }
}

fn push_indexed_register_generated_type_entries(
    entries: &mut Vec<(String, String)>,
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
    if uuid.contains('.') {
        return None;
    }
    let row = metadata_text_row_from_blob(uuid, blob)?;
    extract_metadata_source_xml_from_text_row(
        &row,
        type_index,
        object_refs,
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
    if let Some(xml) = extract_configuration_source_xml(text, uuid, object_refs, source_version) {
        return Some(ExtractedMetadataSourceXml {
            relative_path: PathBuf::from("Configuration.xml"),
            xml: xml.into_bytes(),
        });
    }
    let object_code = row.object_code?;
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
    if is_form_metadata_text(text, uuid) {
        let header = row.header.as_ref()?;
        let form_ref = form_refs.get(uuid);
        let relative_path = form_ref
            .map(|form_ref| form_ref.relative_path.clone())
            .unwrap_or_else(|| {
                PathBuf::from("CommonForms")
                    .join(sanitize_source_path_segment(&header.name))
                    .with_extension("xml")
            });
        let kind = form_ref
            .map(|form_ref| form_ref.kind)
            .unwrap_or("CommonForm");
        let xml = format_form_source_xml(kind, &header, source_version).into_bytes();
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
    let nested_commands =
        if metadata_kind_can_own_commands(kind) && !matches!(kind, "Report" | "DataProcessor") {
            nested_command_headers_from_text(text, uuid)
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
            form_refs,
            template_refs,
        )?;
        format_document_source_xml(&header, &document, source_version).into_bytes()
    } else if kind == "BusinessProcess" {
        let business_process = parse_business_process_properties_from_text(text, uuid)?;
        format_business_process_source_xml(&header, &business_process, source_version).into_bytes()
    } else if kind == "Task" {
        let task = parse_task_properties_from_text(text, uuid)?;
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
        let subsystem = parse_subsystem_properties_from_text(text, uuid, subsystem_refs)?;
        format_subsystem_source_xml(&header, &subsystem, source_version).into_bytes()
    } else if kind == "ExchangePlan" {
        let exchange_plan = parse_exchange_plan_properties_from_text(text, uuid, type_index)?;
        format_exchange_plan_source_xml(&header, &exchange_plan, source_version).into_bytes()
    } else if metadata_kind_uses_register_resources(kind) {
        let register =
            parse_register_properties_from_text(kind, text, uuid, type_index, form_refs)?;
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
    } else if kind == "HTTPService" {
        let service = parse_http_service_properties_from_text(text, uuid)?;
        format_http_service_source_xml(&header, &service, source_version).into_bytes()
    } else if kind == "CommonAttribute" {
        let common_attribute =
            parse_common_attribute_properties_from_text(text, uuid, type_index, object_refs)?;
        format_common_attribute_source_xml(&header, &common_attribute, source_version).into_bytes()
    } else if kind == "FilterCriterion" {
        format_filter_criterion_source_xml(&header, source_version).into_bytes()
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
    if !nested_commands.is_empty() || !owned_form_template_child_objects.is_empty() {
        let mut xml_text = String::from_utf8(xml).ok()?;
        insert_metadata_child_objects_xml(&mut xml_text, kind, &owned_form_template_child_objects);
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

fn parse_subsystem_properties_from_text(
    text: &str,
    uuid: &str,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<SubsystemProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("22")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    let second_scalar = fields
        .get(3)
        .and_then(|field| parse_1c_bool_field(Some(*field)));
    let include_help_in_contents = if second_scalar.is_some() {
        parse_1c_bool_field(fields.get(2).copied()).unwrap_or(false)
    } else {
        false
    };
    let include_in_command_interface = second_scalar
        .unwrap_or_else(|| parse_1c_bool_field(fields.get(2).copied()).unwrap_or(true));
    let use_one_command = fields
        .get(4)
        .and_then(|field| parse_1c_bool_field(Some(*field)))
        .unwrap_or(false);
    Some(SubsystemProperties {
        include_help_in_contents,
        include_in_command_interface,
        use_one_command,
        child_subsystems: parse_subsystem_child_references(&fields, uuid, subsystem_refs),
    })
}

fn parse_subsystem_child_references(
    fields: &[&str],
    uuid: &str,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut children = Vec::new();
    for field in fields.iter().skip(3) {
        for child_uuid in uuid_like_values_in_text_order(field) {
            if child_uuid == uuid || !seen.insert(child_uuid.clone()) {
                continue;
            }
            let Some(subsystem_ref) = subsystem_refs.get(&child_uuid) else {
                continue;
            };
            if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
                children.push(reference);
            }
        }
    }
    children
}

fn parse_exchange_plan_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<ExchangePlanProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("37") {
        return None;
    }
    let header_index = metadata_header_field_index(&fields, uuid)?;

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("ExchangePlanObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("ExchangePlanRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        5,
        6,
        &format!("ExchangePlanSelection.{}", header.name),
        "Selection",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("ExchangePlanList.{}", header.name),
        "List",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        9,
        10,
        &format!("ExchangePlanManager.{}", header.name),
        "Manager",
    );

    let this_node = parse_exchange_plan_this_node(&fields, header_index);
    let use_standard_commands =
        parse_1c_bool_field(fields.get(header_index + 1).copied()).unwrap_or(true);

    Some(ExchangePlanProperties {
        this_node,
        generated_types,
        use_standard_commands,
        child_objects: parse_exchange_plan_child_objects(text, uuid, type_index),
    })
}

fn parse_exchange_plan_this_node(fields: &[&str], header_index: usize) -> Option<String> {
    if header_index <= 11 {
        return None;
    }
    header_index
        .checked_sub(1)
        .and_then(|index| fields.get(index).copied())
        .and_then(parse_non_zero_uuid)
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

fn parse_register_properties_from_text(
    kind: &str,
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<RegisterProperties> {
    if !metadata_kind_uses_register_resources(kind) {
        return None;
    }
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    let mut generated_types = Vec::new();
    let register_start_index = match kind {
        "AccountingRegister" => Some(2),
        "AccumulationRegister" | "CalculationRegister" => Some(1),
        _ => None,
    };
    if let Some(start_index) = register_start_index {
        push_register_generated_type_entries(
            &mut generated_types,
            &fields,
            start_index,
            kind,
            &header.name,
        );
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
    let use_standard_commands = parse_register_use_standard_commands(kind, &fields, uuid);
    let register_type = parse_register_type(kind, &fields, uuid);
    let include_help_in_contents = parse_register_include_help_in_contents(kind);
    let data_lock_control_mode = parse_register_data_lock_control_mode(kind, &fields, uuid);
    let full_text_search = parse_register_full_text_search(kind, &fields, uuid);
    let form_refs = parse_register_form_refs(kind, &fields, uuid, form_refs);
    let standard_attributes = register_standard_attributes(kind, register_type);
    let child_objects = nested_headers_with_offsets_from_text(text, uuid, |_| true)
        .into_iter()
        .filter_map(|(header, marker_start)| {
            let tag = register_child_object_tag(kind, text, marker_start)?;
            let value_types =
                parse_metadata_child_value_types(text, marker_start, &header.uuid, type_index);
            let properties = parse_metadata_child_properties(
                kind,
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
        .collect();
    Some(RegisterProperties {
        generated_types,
        use_standard_commands,
        register_type,
        include_help_in_contents,
        data_lock_control_mode,
        full_text_search,
        default_record_form: form_refs.0,
        default_list_form: form_refs.1,
        auxiliary_record_form: form_refs.2,
        auxiliary_list_form: form_refs.3,
        standard_attributes,
        child_objects,
    })
}

fn parse_register_form_refs(
    kind: &str,
    fields: &[&str],
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return (None, None, None, None);
    };
    match kind {
        "InformationRegister" => (
            parse_catalog_form_ref(fields.get(header_index + 1).copied(), form_refs),
            parse_catalog_form_ref(fields.get(header_index + 2).copied(), form_refs),
            None,
            None,
        ),
        "AccumulationRegister" => (
            None,
            parse_catalog_form_ref(fields.get(header_index + 2).copied(), form_refs),
            None,
            parse_catalog_form_ref(fields.get(header_index + 3).copied(), form_refs),
        ),
        _ => (None, None, None, None),
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
    register_type: Option<&'static str>,
) -> Vec<RegisterStandardAttribute> {
    let mut attributes = Vec::new();
    if kind == "AccumulationRegister" && register_type == Some("Balance") {
        attributes.push(RegisterStandardAttribute {
            name: "RecordType",
            fill_checking: "DontCheck",
        });
    }
    if matches!(kind, "AccumulationRegister" | "InformationRegister") {
        attributes.extend([
            RegisterStandardAttribute {
                name: "Active",
                fill_checking: "DontCheck",
            },
            RegisterStandardAttribute {
                name: "LineNumber",
                fill_checking: "DontCheck",
            },
            RegisterStandardAttribute {
                name: "Recorder",
                fill_checking: "DontCheck",
            },
            RegisterStandardAttribute {
                name: "Period",
                fill_checking: "ShowError",
            },
        ]);
    }
    attributes
}

fn parse_register_data_lock_control_mode(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> Option<&'static str> {
    let header_index = metadata_header_field_index(fields, uuid)?;
    let field_offset = match kind {
        "AccumulationRegister" | "InformationRegister" => 5,
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

fn parse_register_full_text_search(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> Option<&'static str> {
    if kind != "AccumulationRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    match fields.get(header_index + 6).map(|field| field.trim()) {
        Some("0") => Some("DontUse"),
        Some("1") => Some("Use"),
        _ => None,
    }
}

fn parse_register_use_standard_commands(kind: &str, fields: &[&str], uuid: &str) -> bool {
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return true;
    };
    if kind == "InformationRegister"
        && let Some(value) = fields
            .get(header_index + 6)
            .and_then(|field| parse_1c_bool_field(Some(*field)))
    {
        return value;
    }
    fields
        .get(header_index + 1)
        .and_then(|field| parse_1c_bool_field(Some(*field)))
        .unwrap_or(true)
}

fn parse_register_include_help_in_contents(kind: &str) -> Option<bool> {
    match kind {
        "AccumulationRegister" => Some(false),
        _ => None,
    }
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
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<MetadataChildObject> {
    let mut roots = Vec::<MetadataChildObject>::new();
    let mut tabular_section_indexes = BTreeMap::<String, usize>::new();
    let mut pending_by_tabular_section = BTreeMap::<String, Vec<MetadataChildObject>>::new();

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
        let properties = if tag == "Attribute" {
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

    if owner_kind == "DataProcessor" {
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
        choice_history_on_input: fields
            .get(header_index + 21)
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim())),
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
        quick_choice,
        create_on_input,
        choice_form: parse_metadata_child_choice_form(fields.get(12).copied(), object_refs),
        link_by_type_empty: metadata_child_collection_is_empty(fields.get(17).copied()),
        choice_history_on_input: fields
            .get(18)
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim()))
            .or(Some("Auto")),
        use_mode: None,
        indexing: None,
        full_text_search: None,
        data_history: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
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
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<CatalogProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("57") {
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
            type_index,
            object_refs,
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
            type_index,
            object_refs,
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
        default_list_form: parse_catalog_form_ref(
            fields.get(header_index + 15).copied(),
            form_refs,
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
            type_index,
            object_refs,
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
    })
}

fn parse_task_properties_from_text(text: &str, uuid: &str) -> Option<TaskProperties> {
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
            type_index,
            object_refs,
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
    if let Some(form_uuids) = owned_form_uuid_values_in_text_order(text) {
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
    let mut details = BTreeMap::new();
    if let Some(field) = field {
        parse_catalog_standard_attribute_details_from_field(field, &mut details);
    }
    details
}

fn parse_catalog_standard_attribute_details_from_field(
    field: &str,
    details: &mut BTreeMap<&'static str, CatalogStandardAttributeDetails>,
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
            .and_then(|code| catalog_standard_attribute_name(code.trim()))
        else {
            continue;
        };
        let detail = parse_catalog_standard_attribute_detail(fields[index + 2]);
        if !detail.tooltip.is_empty() || !detail.synonym.is_empty() {
            details.insert(name, detail);
        }
    }

    for nested in fields {
        if nested.starts_with('{') {
            parse_catalog_standard_attribute_details_from_field(nested, details);
        }
    }
}

fn parse_catalog_standard_attribute_detail(field: &str) -> CatalogStandardAttributeDetails {
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
        "-3" => Some("Description"),
        "-2" => Some("Code"),
        _ => None,
    }
}

fn parse_1c_bool_field(value: Option<&str>) -> Option<bool> {
    parse_1c_bool_flag(value?.trim())
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
    Some(DocumentStandardAttributes { number_type })
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
        _ => Some(MetadataChoiceParameterValue::FixedArray(value_refs)),
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
}

fn parse_http_service_url_templates_from_text(
    text: &str,
    owner_uuid: &str,
) -> Vec<HttpServiceUrlTemplateProperties> {
    let candidates = http_service_child_candidates_from_text(text, owner_uuid);
    let method_candidates = candidates
        .iter()
        .filter_map(|candidate| {
            let http_method = candidate.strings.first()?;
            let handler = candidate.strings.get(1)?;
            is_http_service_method_name(http_method).then(|| HttpServiceChildCandidate {
                start: candidate.start,
                end: candidate.end,
                header: candidate.header.clone(),
                strings: vec![http_method.clone(), handler.clone()],
            })
        })
        .collect::<Vec<_>>();

    let mut seen = BTreeSet::new();
    let template_candidates = candidates
        .into_iter()
        .filter_map(|candidate| {
            let template = candidate.strings.first()?.clone();
            if is_http_service_method_name(&template)
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
        let strings = match header_index {
            2 => fields
                .get(1)
                .and_then(|field| parse_1c_quoted_string(field.trim()))
                .map(|template| vec![template])
                .unwrap_or_default(),
            3 if is_http_service_method_name(&header.name) => fields
                .get(1)
                .and_then(|field| parse_1c_quoted_string(field.trim()))
                .map(|handler| vec![header.name.clone(), handler])
                .unwrap_or_default(),
            _ => fields
                .iter()
                .skip(header_index + 1)
                .filter_map(|field| parse_1c_quoted_string(field.trim()))
                .collect::<Vec<_>>(),
        };
        if strings.is_empty() {
            continue;
        }
        candidates.push(HttpServiceChildCandidate {
            start,
            end,
            header,
            strings,
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

fn is_http_service_method_name(value: &str) -> bool {
    matches!(
        value,
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
    )
}

fn is_http_service_url_template_text(value: &str) -> bool {
    value.starts_with('/') || value.contains('{') || value == "*"
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
    let command_parameter_types =
        parse_common_command_parameter_type_names(fields.get(8)?, type_index);
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

fn parse_common_command_parameter_type_names(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Vec<String> {
    parse_metadata_type_pattern(value, type_index)
        .unwrap_or_default()
        .iter()
        .map(metadata_type_xml_name)
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
    push_optional_simple_property_xml(
        &mut insert,
        "CompatibilityMode",
        properties.compatibility_mode.as_deref(),
    );
    insert_metadata_properties_xml(&mut xml, &insert);
    xml
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
        xml.insert_str(
            offset,
            &format!(
                "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n\
\t\t\t<IncludeInCommandInterface>{}</IncludeInCommandInterface>\r\n\
\t\t\t<UseOneCommand>{}</UseOneCommand>\r\n",
                xml_bool(subsystem.include_help_in_contents),
                xml_bool(subsystem.include_in_command_interface),
                xml_bool(subsystem.use_one_command)
            ),
        );
    }
    if !subsystem.child_subsystems.is_empty()
        && let Some(offset) = xml.find("\t</Subsystem>")
    {
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
    let internal_info = format_exchange_plan_internal_info_xml(exchange_plan);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        xml.insert_str(
            index,
            &format!(
                "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
                xml_bool(exchange_plan.use_standard_commands)
            ),
        );
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

fn format_exchange_plan_internal_info_xml(exchange_plan: &ExchangePlanProperties) -> String {
    if exchange_plan.this_node.is_none() && exchange_plan.generated_types.is_empty() {
        return String::new();
    }
    let mut xml = "\t\t<InternalInfo>\r\n".to_string();
    if let Some(this_node) = &exchange_plan.this_node {
        xml.push_str(&format!(
            "\t\t\t<xr:ThisNode>{}</xr:ThisNode>\r\n",
            escape_xml_text(this_node)
        ));
    }
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

fn format_register_source_xml(
    kind: &str,
    header: &MetadataHeader,
    register: &RegisterProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml(kind, header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&register.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(register.use_standard_commands)
        );
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
    if kind == "InformationRegister"
        && let Some(index) = xml.find("\t\t</Properties>")
    {
        let mut form_refs_xml = String::new();
        push_optional_text_element(
            &mut form_refs_xml,
            "\t\t\t",
            "DefaultRecordForm",
            register.default_record_form.as_deref(),
        );
        push_optional_text_element(
            &mut form_refs_xml,
            "\t\t\t",
            "DefaultListForm",
            register.default_list_form.as_deref(),
        );
        push_optional_text_element(
            &mut form_refs_xml,
            "\t\t\t",
            "AuxiliaryRecordForm",
            register.auxiliary_record_form.as_deref(),
        );
        push_optional_text_element(
            &mut form_refs_xml,
            "\t\t\t",
            "AuxiliaryListForm",
            register.auxiliary_list_form.as_deref(),
        );
        xml.insert_str(index, &form_refs_xml);
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
        for child in &document.child_metadata_objects {
            push_metadata_child_object_xml(&mut child_objects, child);
        }
        for form in &document.child_forms {
            child_objects.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
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
        xml.insert_str(
            index,
            &format!(
                "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
                xml_bool(business_process.use_standard_commands)
            ),
        );
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
        xml.insert_str(
            index,
            &format!(
                "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
                xml_bool(task.use_standard_commands)
            ),
        );
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
    push_enum_standard_attributes_xml(&mut xml);
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
            "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n\
\t\t\t\t\t<xr:LinkByType/>\r\n\
\t\t\t\t\t<xr:FillChecking>{}</xr:FillChecking>\r\n\
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
            escape_xml_text(attribute.name),
            attribute.fill_checking,
        ));
    }
    xml.push_str("\t\t\t</StandardAttributes>\r\n");
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
    synonym: Option<&'static str>,
    tooltip: Option<&'static str>,
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
            synonym: None,
            tooltip: None,
            fill_value: DocumentStandardAttributeFillValue::Nil,
        },
        DocumentStandardAttribute {
            name: "Ref",
            fill_checking: "DontCheck",
            synonym: None,
            tooltip: None,
            fill_value: DocumentStandardAttributeFillValue::Nil,
        },
        DocumentStandardAttribute {
            name: "DeletionMark",
            fill_checking: "DontCheck",
            synonym: None,
            tooltip: None,
            fill_value: DocumentStandardAttributeFillValue::BooleanFalse,
        },
        DocumentStandardAttribute {
            name: "Date",
            fill_checking: "ShowError",
            synonym: Some("Дата"),
            tooltip: Some("Дата документа"),
            fill_value: DocumentStandardAttributeFillValue::DateTimeZero,
        },
        DocumentStandardAttribute {
            name: "Number",
            fill_checking: "DontCheck",
            synonym: Some("Номер"),
            tooltip: Some("Номер документа"),
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
    push_document_standard_attribute_localized_xml(xml, "ToolTip", attribute.tooltip);
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
    push_document_standard_attribute_localized_xml(xml, "Synonym", attribute.synonym);
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

fn push_document_standard_attribute_localized_xml(
    xml: &mut String,
    name: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "\t\t\t\t\t<xr:{name}>\r\n\
\t\t\t\t\t\t<v8:item>\r\n\
\t\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n\
\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n\
\t\t\t\t\t\t</v8:item>\r\n\
\t\t\t\t\t</xr:{name}>\r\n",
            escape_xml_element_text(value)
        ));
    } else {
        xml.push_str(&format!("\t\t\t\t\t<xr:{name}/>\r\n"));
    }
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
        push_metadata_child_choice_parameters_xml(xml, indent, choice_parameters);
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
    if properties.link_by_type_empty {
        xml.push_str(&format!("{indent}<LinkByType/>\r\n"));
    }
    if let Some(choice_history_on_input) = properties.choice_history_on_input {
        xml.push_str(&format!(
            "{indent}<ChoiceHistoryOnInput>{choice_history_on_input}</ChoiceHistoryOnInput>\r\n"
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

fn push_metadata_child_choice_parameters_xml(
    xml: &mut String,
    indent: &str,
    parameters: &[MetadataChoiceParameter],
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
        match &parameter.value {
            MetadataChoiceParameterValue::Boolean(value) => {
                xml.push_str(&format!(
                    "{indent}\t\t<app:value xsi:type=\"xs:boolean\">{}</app:value>\r\n",
                    xml_bool(*value)
                ));
            }
            MetadataChoiceParameterValue::DesignTimeRef(value_ref) => {
                xml.push_str(&format!(
                    "{indent}\t\t<app:value xsi:type=\"xr:DesignTimeRef\">{}</app:value>\r\n",
                    escape_xml_element_text(value_ref)
                ));
            }
            MetadataChoiceParameterValue::FixedArray(value_refs) => {
                xml.push_str(&format!(
                    "{indent}\t\t<app:value xsi:type=\"v8:FixedArray\">\r\n"
                ));
                for value_ref in value_refs {
                    xml.push_str(&format!(
                        "{indent}\t\t\t<v8:Value xsi:type=\"xr:DesignTimeRef\">{}</v8:Value>\r\n",
                        escape_xml_element_text(value_ref)
                    ));
                }
                xml.push_str(&format!("{indent}\t\t</app:value>\r\n"));
            }
        }
        xml.push_str(&format!("{indent}\t</app:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</ChoiceParameters>\r\n"));
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

fn format_common_command_parameter_type_xml(xml: &mut String, types: &[String]) {
    format_common_command_parameter_type_xml_with_indent(xml, types, "\t\t\t");
}

fn format_common_command_parameter_type_xml_with_indent(
    xml: &mut String,
    types: &[String],
    indent: &str,
) {
    if types.is_empty() {
        xml.push_str(&format!("{indent}<CommandParameterType/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<CommandParameterType>\r\n"));
    for value_type in types {
        let tag_name = if value_type.starts_with("cfg:DefinedType.") {
            "TypeSet"
        } else {
            "Type"
        };
        xml.push_str(&format!(
            "{indent}\t<v8:{tag_name}>{}</v8:{tag_name}>\r\n",
            escape_xml_text(value_type)
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
        _ => "Type",
    }
}

fn metadata_type_xml_name(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Boolean => "xs:boolean".to_string(),
        ConstantValueType::String { .. } => "xs:string".to_string(),
        ConstantValueType::Number { .. } => "xs:decimal".to_string(),
        ConstantValueType::DateTime { .. } => "xs:dateTime".to_string(),
        ConstantValueType::Reference { reference, .. } => reference.clone(),
    }
}

fn metadata_type_xml_namespace_attr(value_type: &ConstantValueType) -> &'static str {
    match value_type {
        ConstantValueType::Reference { reference } if reference.starts_with("mxl:") => {
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

const MODULE_BODY_SUFFIXES: &[&str] = &["0", "1", "2", "3", "5", "6", "7", "8", "15", "16"];

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
