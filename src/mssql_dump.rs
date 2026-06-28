use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;
use serde::Serialize;

use crate::cli::MssqlDumpConfigArgs;
use crate::module_blob::{parse_form_body_blob, unpack_module_blob_text};
use crate::parallel;

const STD_PICTURE_INFORMATION_UUID: &str = "4b54770b-d069-4c0e-9b17-5cc2a01134d9";
const STD_PICTURE_SAVE_FILE_UUID: &str = "818ab7d0-4654-4542-bd5e-fd9d1352b5a1";

#[derive(Debug, Serialize)]
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
}

#[derive(Debug, Serialize)]
pub struct MssqlDumpedTableReport {
    pub table: String,
    pub rows: usize,
    pub binary_bytes: usize,
    pub inflated_rows: usize,
    pub module_text_rows: usize,
    pub metadata_xml_rows: usize,
    pub source_asset_rows: usize,
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
    binary_path: String,
    inflated_path: Option<String>,
    module_text_path: Option<String>,
    metadata_xml_path: Option<String>,
    source_asset_path: Option<String>,
}

#[derive(Debug)]
struct ConfigRow {
    file_name: String,
    part_no: i32,
    data_size: i64,
    binary_hex: String,
}

#[derive(Debug)]
struct ConfigChunkRow {
    file_name: String,
    part_no: i32,
    data_size: i64,
    chunk_index: i32,
    binary_hex: String,
}

pub fn dump_config(args: &MssqlDumpConfigArgs) -> Result<MssqlDumpConfigReport> {
    prepare_output_dir(&args.output_dir, args.overwrite)?;

    let mut table_names = vec!["Config"];
    if args.include_config_save {
        table_names.push("ConfigSave");
    }

    let mut reports = Vec::new();
    let mut manifest_tables = Vec::new();
    let selected_file_names = expand_selected_file_names(&args.file_names);
    for table in table_names {
        let rows = fetch_rows(
            &args.sqlcmd,
            &args.server,
            &args.database,
            table,
            &selected_file_names,
        )?;
        let dumped = dump_table_rows(
            &args.output_dir,
            table,
            rows,
            args.inflate,
            args.extract_module_text,
            args.extract_metadata_xml,
        )?;
        reports.push(MssqlDumpedTableReport {
            table: table.to_string(),
            rows: dumped.rows.len(),
            binary_bytes: dumped.binary_bytes,
            inflated_rows: dumped.inflated_rows,
            module_text_rows: dumped.module_text_rows,
            metadata_xml_rows: dumped.metadata_xml_rows,
            source_asset_rows: dumped.source_asset_rows,
        });
        manifest_tables.push(MssqlDumpTableManifest {
            table: table.to_string(),
            rows: dumped.rows,
        });
    }

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

    Ok(MssqlDumpConfigReport {
        database: args.database.clone(),
        output_dir: args.output_dir.clone(),
        total_rows: reports.iter().map(|table| table.rows).sum(),
        total_binary_bytes: reports.iter().map(|table| table.binary_bytes).sum(),
        total_inflated_rows: reports.iter().map(|table| table.inflated_rows).sum(),
        total_module_text_rows: reports.iter().map(|table| table.module_text_rows).sum(),
        total_metadata_xml_rows: reports.iter().map(|table| table.metadata_xml_rows).sum(),
        total_source_asset_rows: reports.iter().map(|table| table.source_asset_rows).sum(),
        tables: reports,
    })
}

struct DumpedTable {
    rows: Vec<MssqlDumpRowManifest>,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
    metadata_xml_rows: usize,
    source_asset_rows: usize,
}

struct DumpedRow {
    manifest: MssqlDumpRowManifest,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
    metadata_xml_rows: usize,
    source_asset_rows: usize,
}

struct DumpRowContext<'a> {
    output_dir: &'a Path,
    table: &'a str,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
    module_text_paths: &'a BTreeMap<String, PathBuf>,
    source_assets: &'a BTreeMap<String, SourceAsset>,
    type_index: &'a BTreeMap<String, String>,
    object_refs: &'a BTreeMap<String, String>,
    form_refs: &'a BTreeMap<String, FormSourceReference>,
    template_refs: &'a BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &'a BTreeMap<String, SubsystemSourceReference>,
}

fn dump_table_rows(
    output_dir: &Path,
    table: &str,
    rows: Vec<ConfigRow>,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
) -> Result<DumpedTable> {
    let table_dir = output_dir.join(table);
    fs::create_dir_all(&table_dir)
        .with_context(|| format!("failed to create {}", table_dir.display()))?;
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
    let module_text_paths = if extract_module_text {
        module_body_paths(&rows)
    } else {
        BTreeMap::new()
    };
    let source_assets = source_asset_paths(&rows);
    let type_index = if extract_metadata_xml {
        build_metadata_type_index(&rows)
    } else {
        BTreeMap::new()
    };
    let form_refs = if extract_metadata_xml {
        build_form_source_reference_index(&rows)
    } else {
        BTreeMap::new()
    };
    let template_refs = if extract_metadata_xml {
        build_template_source_reference_index(&rows)
    } else {
        BTreeMap::new()
    };
    let subsystem_refs = if extract_metadata_xml {
        build_subsystem_source_reference_index(&rows)
    } else {
        BTreeMap::new()
    };
    let object_refs = if extract_metadata_xml {
        build_metadata_object_reference_index(&rows)
    } else {
        BTreeMap::new()
    };
    ensure_unique_source_asset_paths(&source_assets)?;

    let context = DumpRowContext {
        output_dir,
        table,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        module_text_paths: &module_text_paths,
        source_assets: &source_assets,
        type_index: &type_index,
        object_refs: &object_refs,
        form_refs: &form_refs,
        template_refs: &template_refs,
        subsystem_refs: &subsystem_refs,
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
    })
}

fn dump_table_row(context: &DumpRowContext<'_>, row: &ConfigRow) -> Result<DumpedRow> {
    let bytes = decode_hex(&row.binary_hex)
        .with_context(|| format!("failed to decode {} row {}", context.table, row.file_name))?;
    if bytes.len() != row.data_size as usize {
        bail!(
            "{} row {} DataSize {} does not match BinaryData length {}",
            context.table,
            row.file_name,
            row.data_size,
            bytes.len()
        );
    }

    let safe_name = safe_storage_file_name(&row.file_name, row.part_no);
    let binary_relative = PathBuf::from(context.table).join(format!("{safe_name}.bin"));
    let binary_path = context.output_dir.join(&binary_relative);
    fs::write(&binary_path, &bytes)
        .with_context(|| format!("failed to write {}", binary_path.display()))?;

    let mut inflated_rows = 0;
    let inflated_relative = if context.inflate {
        match inflate_raw_deflate(&bytes) {
            Ok(inflated) => {
                let relative = PathBuf::from(format!("{}_inflated", context.table))
                    .join(format!("{safe_name}.txt"));
                let path = context.output_dir.join(&relative);
                fs::write(&path, inflated)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                inflated_rows = 1;
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let mut module_text_rows = 0;
    let module_text_relative = if context.extract_module_text {
        let module_text = match unpack_module_blob_text(&bytes) {
            Ok(text) => Some(text),
            Err(_) if context.module_text_paths.contains_key(&row.file_name) => {
                unpack_form_body_module_text(&bytes)
            }
            Err(_) => None,
        };
        match module_text {
            Some(text) => {
                let relative = context
                    .module_text_paths
                    .get(&row.file_name)
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
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            None => None,
        }
    } else {
        None
    };

    let mut metadata_xml_rows = 0;
    let metadata_xml_relative = if context.extract_metadata_xml {
        match extract_metadata_source_xml_with_refs(
            &bytes,
            &row.file_name,
            context.type_index,
            context.object_refs,
            context.form_refs,
            context.template_refs,
            context.subsystem_refs,
        ) {
            Some(extracted) => {
                let path = context.output_dir.join(&extracted.relative_path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&path, extracted.xml)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                metadata_xml_rows = 1;
                Some(extracted.relative_path.to_string_lossy().replace('\\', "/"))
            }
            None => None,
        }
    } else {
        None
    };

    let mut source_asset_rows = 0;
    let source_asset_relative = if metadata_xml_relative.is_none() {
        match context.source_assets.get(&row.file_name) {
            Some(asset) => {
                let relative = write_source_asset(context.output_dir, asset, &bytes)?;
                source_asset_rows = 1;
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            None => None,
        }
    } else {
        None
    };

    Ok(DumpedRow {
        manifest: MssqlDumpRowManifest {
            file_name: row.file_name.clone(),
            part_no: row.part_no,
            data_size: row.data_size,
            binary_bytes: bytes.len(),
            binary_path: binary_relative.to_string_lossy().replace('\\', "/"),
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
    })
}

fn ensure_unique_source_asset_paths(source_assets: &BTreeMap<String, SourceAsset>) -> Result<()> {
    let mut paths = BTreeMap::<String, &str>::new();
    for (file_name, asset) in source_assets {
        let path = asset.primary_path.to_string_lossy().replace('\\', "/");
        if let Some(previous_file_name) = paths.insert(path.clone(), file_name.as_str()) {
            bail!(
                "source asset output path {path} is produced by both {previous_file_name} and {file_name}"
            );
        }
    }
    Ok(())
}

#[derive(Clone)]
enum SourceAssetKind {
    Binary,
    CommandInterface {
        command_refs: BTreeMap<String, String>,
        metadata_refs: BTreeMap<String, MetadataCommandReference>,
    },
    ConfigDumpInfo,
    ExchangePlanContent {
        object_refs: BTreeMap<String, String>,
    },
    BusinessProcessFlowchart,
    ExtPicture,
    Form {
        object_refs: BTreeMap<String, String>,
    },
    Help,
    InflatedBase64OrBinary,
    InflatedBinary,
    MoxelSpreadsheet {
        object_refs: BTreeMap<String, String>,
    },
    PredefinedData {
        xsi_type: &'static str,
        type_index: BTreeMap<String, String>,
    },
    RoleRights {
        object_refs: BTreeMap<String, String>,
        field_refs: BTreeMap<String, String>,
    },
    Schedule,
    StyleBody {
        object_refs: BTreeMap<String, String>,
    },
}

struct SourceAsset {
    primary_path: PathBuf,
    kind: SourceAssetKind,
}

fn source_asset_paths(rows: &[ConfigRow]) -> BTreeMap<String, SourceAsset> {
    let command_refs = build_command_interface_reference_index(rows);
    let metadata_refs = build_metadata_command_reference_index(rows);
    let object_refs = build_metadata_object_reference_index(rows);
    let field_refs = build_metadata_field_reference_index(rows);
    let type_index = build_metadata_type_index(rows);
    let form_refs = build_form_source_reference_index(rows);
    let template_refs = build_template_source_reference_index(rows);
    let subsystem_refs = build_subsystem_source_reference_index(rows);
    let rows_by_file_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in &file_names {
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
    if file_names.contains("versions") {
        paths.insert(
            "versions".to_string(),
            SourceAsset {
                primary_path: PathBuf::from("ConfigDumpInfo.xml"),
                kind: SourceAssetKind::ConfigDumpInfo,
            },
        );
    }
    for (metadata_id, suffixes) in suffixes_by_id {
        if file_names.contains(metadata_id) || !is_configuration_module_group(&suffixes) {
            continue;
        }
        for (suffix, path, kind) in CONFIGURATION_SOURCE_ASSET_SUFFIXES {
            let body_id = format!("{metadata_id}.{suffix}");
            if file_names.contains(body_id.as_str()) {
                paths.insert(
                    body_id,
                    SourceAsset {
                        primary_path: PathBuf::from(path),
                        kind: kind.clone(),
                    },
                );
            }
        }
        for (suffix, path) in [
            ("9", "Ext/MainSectionCommandInterface.xml"),
            ("a", "Ext/CommandInterface.xml"),
        ] {
            let interface_id = format!("{metadata_id}.{suffix}");
            if let Some(row) = rows_by_file_name.get(interface_id.as_str())
                && let Ok(bytes) = decode_hex(&row.binary_hex)
                && parse_command_interface_blob(&bytes, &command_refs, &metadata_refs).is_some()
            {
                paths.insert(
                    interface_id,
                    SourceAsset {
                        primary_path: PathBuf::from(path),
                        kind: SourceAssetKind::CommandInterface {
                            command_refs: command_refs.clone(),
                            metadata_refs: metadata_refs.clone(),
                        },
                    },
                );
            }
        }
    }
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        for (body_id, asset) in source_assets_from_metadata_blob(
            &bytes,
            &row.file_name,
            &file_names,
            &rows_by_file_name,
            &command_refs,
            &metadata_refs,
            &object_refs,
            &field_refs,
            &type_index,
            &subsystem_refs,
        ) {
            paths.insert(body_id, asset);
        }
    }
    paths.extend(form_help_asset_paths(rows, &rows_by_file_name, &form_refs));
    paths.extend(form_body_asset_paths(rows, &file_names, &object_refs));
    paths.extend(template_body_asset_paths(
        &template_refs,
        &file_names,
        &object_refs,
    ));

    paths
}

fn template_body_asset_paths(
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    file_names: &BTreeSet<&str>,
    object_refs: &BTreeMap<String, String>,
) -> BTreeMap<String, SourceAsset> {
    let mut paths = BTreeMap::new();
    for (uuid, template_ref) in template_refs {
        let body_id = format!("{uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let Some((file_name, mut kind)) = template_body_source_asset(template_ref.template_type)
        else {
            continue;
        };
        if matches!(kind, SourceAssetKind::MoxelSpreadsheet { .. }) {
            kind = SourceAssetKind::MoxelSpreadsheet {
                object_refs: object_refs.clone(),
            };
        }
        paths.insert(
            body_id,
            SourceAsset {
                primary_path: template_ref
                    .relative_path
                    .with_extension("")
                    .join("Ext")
                    .join(file_name),
                kind,
            },
        );
    }

    paths
}

fn template_body_source_asset(template_type: &str) -> Option<(&'static str, SourceAssetKind)> {
    match template_type {
        "AddIn" => Some(("Template.bin", SourceAssetKind::InflatedBase64OrBinary)),
        "BinaryData" => Some(("Template.bin", SourceAssetKind::InflatedBase64OrBinary)),
        "DataCompositionAppearanceTemplate" => {
            Some(("Template.xml", SourceAssetKind::InflatedBinary))
        }
        "DataCompositionSchema" => Some(("Template.xml", SourceAssetKind::InflatedBinary)),
        "GraphicalSchema" => Some(("Template.xml", SourceAssetKind::InflatedBinary)),
        "HTMLDocument" => Some(("Template.xml", SourceAssetKind::Help)),
        "TextDocument" => Some(("Template.txt", SourceAssetKind::InflatedBinary)),
        "SpreadsheetDocument" => Some((
            "Template.xml",
            SourceAssetKind::MoxelSpreadsheet {
                object_refs: BTreeMap::new(),
            },
        )),
        _ => None,
    }
}

fn form_body_asset_paths(
    rows: &[ConfigRow],
    file_names: &BTreeSet<&str>,
    object_refs: &BTreeMap<String, String>,
) -> BTreeMap<String, SourceAsset> {
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in build_form_source_reference_index(rows) {
        let body_id = format!("{form_uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let mut form_dir = form_ref.relative_path;
        form_dir.set_extension("");
        paths.insert(
            body_id,
            SourceAsset {
                primary_path: form_dir.join("Ext").join("Form.xml"),
                kind: SourceAssetKind::Form {
                    object_refs: object_refs.clone(),
                },
            },
        );
    }

    paths
}

const CONFIGURATION_SOURCE_ASSET_SUFFIXES: &[(&str, &str, SourceAssetKind)] = &[
    ("2", "Ext/Splash.xml", SourceAssetKind::ExtPicture),
    ("4", "Ext/ParentConfigurations.bin", SourceAssetKind::Binary),
    (
        "8",
        "Ext/HomePageWorkArea.xml",
        SourceAssetKind::InflatedBinary,
    ),
    (
        "10",
        "Ext/MobileClientSignature.bin",
        SourceAssetKind::InflatedBinary,
    ),
    (
        "b",
        "Ext/ClientApplicationInterface.xml",
        SourceAssetKind::InflatedBinary,
    ),
    (
        "c",
        "Ext/MainSectionPicture.xml",
        SourceAssetKind::ExtPicture,
    ),
    (
        "f",
        "Ext/StandaloneConfigurationContent.bin",
        SourceAssetKind::InflatedBinary,
    ),
];

fn source_assets_from_metadata_blob(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Vec<(String, SourceAsset)> {
    source_assets_from_metadata_blob_inner(
        blob,
        uuid,
        file_names,
        rows_by_file_name,
        command_refs,
        metadata_refs,
        object_refs,
        field_refs,
        type_index,
        subsystem_refs,
    )
    .unwrap_or_default()
}

fn source_assets_from_metadata_blob_inner(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<Vec<(String, SourceAsset)>> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let object_code = parse_metadata_object_code(text)?;
    let (kind, folder) = metadata_source_for_text(object_code, text, uuid)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
    let object_path = if kind == "Subsystem" {
        subsystem_refs
            .get(uuid)
            .map(|subsystem_ref| subsystem_ref.relative_path.with_extension(""))
            .unwrap_or_else(|| {
                PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
            })
    } else {
        PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
    };
    let mut assets = Vec::new();

    if kind == "ExchangePlan" {
        let content_id = format!("{uuid}.1");
        if file_names.contains(content_id.as_str()) {
            assets.push((
                content_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Content.xml"),
                    kind: SourceAssetKind::ExchangePlanContent {
                        object_refs: object_refs.clone(),
                    },
                },
            ));
        }
    }

    if kind == "BusinessProcess" {
        let flowchart_id = format!("{uuid}.7");
        if file_names.contains(flowchart_id.as_str()) {
            assets.push((
                flowchart_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Flowchart.xml"),
                    kind: SourceAssetKind::BusinessProcessFlowchart,
                },
            ));
        }
    }

    if let Some(suffix) = additional_indexes_body_suffix(kind) {
        let additional_indexes_id = format!("{uuid}.{suffix}");
        if file_names.contains(additional_indexes_id.as_str()) {
            assets.push((
                additional_indexes_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("AdditionalIndexes.xml"),
                    kind: SourceAssetKind::InflatedBinary,
                },
            ));
        }
    }

    let body_id = format!("{uuid}.0");
    if file_names.contains(body_id.as_str()) {
        let asset = match kind {
            "CommonPicture" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Picture.xml"),
                kind: SourceAssetKind::ExtPicture,
            }),
            "ScheduledJob" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Schedule.xml"),
                kind: SourceAssetKind::Schedule,
            }),
            "XDTOPackage" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Package.bin"),
                kind: SourceAssetKind::InflatedBinary,
            }),
            "Style" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Style.xml"),
                kind: SourceAssetKind::StyleBody {
                    object_refs: object_refs.clone(),
                },
            }),
            "WSReference" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("WSDefinition.xml"),
                kind: SourceAssetKind::InflatedBinary,
            }),
            "Role"
                if rows_by_file_name
                    .get(body_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .and_then(|bytes| parse_role_rights_blob(&bytes, object_refs, field_refs))
                    .is_some() =>
            {
                Some(SourceAsset {
                    primary_path: object_path.join("Ext").join("Rights.xml"),
                    kind: SourceAssetKind::RoleRights {
                        object_refs: object_refs.clone(),
                        field_refs: field_refs.clone(),
                    },
                })
            }
            _ => None,
        };
        if let Some(asset) = asset {
            assets.push((body_id, asset));
        }
    }

    let command_mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .collect::<BTreeSet<_>>();
    for suffix in ["0", "1"] {
        let body_id = format!("{uuid}.{suffix}");
        if command_mapped_ids.contains(&body_id) {
            continue;
        }
        if let Some(row) = rows_by_file_name.get(body_id.as_str())
            && let Ok(bytes) = decode_hex(&row.binary_hex)
            && parse_command_interface_blob(&bytes, command_refs, metadata_refs).is_some()
        {
            assets.push((
                body_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("CommandInterface.xml"),
                    kind: SourceAssetKind::CommandInterface {
                        command_refs: command_refs.clone(),
                        metadata_refs: metadata_refs.clone(),
                    },
                },
            ));
        }
    }

    let mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .collect::<BTreeSet<_>>();
    let object_row_prefix = format!("{uuid}.");
    let preferred_help_body_id = preferred_help_body_id(kind, uuid);
    for (body_id, body_row) in rows_by_file_name {
        if !body_id.starts_with(&object_row_prefix) || mapped_ids.contains(*body_id) {
            continue;
        }
        if let Ok(help_bytes) = decode_hex(&body_row.binary_hex)
            && parse_help_blob_pages(&help_bytes).is_some()
        {
            if rows_by_file_name.contains_key(preferred_help_body_id.as_str())
                && *body_id != preferred_help_body_id
            {
                continue;
            }
            assets.push((
                (*body_id).to_string(),
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Help.xml"),
                    kind: SourceAssetKind::Help,
                },
            ));
            continue;
        }
        if let Some(xsi_type) = predefined_data_xsi_type(kind)
            && let Ok(predefined_bytes) = decode_hex(&body_row.binary_hex)
            && parse_predefined_data_blob(&predefined_bytes, type_index).is_some()
        {
            assets.push((
                (*body_id).to_string(),
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Predefined.xml"),
                    kind: SourceAssetKind::PredefinedData {
                        xsi_type,
                        type_index: type_index.clone(),
                    },
                },
            ));
        }
    }

    Some(assets)
}

fn additional_indexes_body_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "Document" => Some("3"),
        "AccumulationRegister" => Some("4"),
        _ => None,
    }
}

fn preferred_help_body_id(kind: &str, uuid: &str) -> String {
    let suffix = if matches!(kind, "Form" | "CommonForm") {
        "1"
    } else {
        "5"
    };
    format!("{uuid}.{suffix}")
}
fn write_source_asset(output_dir: &Path, asset: &SourceAsset, bytes: &[u8]) -> Result<PathBuf> {
    match &asset.kind {
        SourceAssetKind::Binary => {
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, bytes)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::ExtPicture => {
            let picture = extract_ext_picture(bytes).with_context(|| {
                format!(
                    "failed to extract picture from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let xml_path = output_dir.join(&asset.primary_path);
            if let Some(parent) = xml_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let picture_dir = output_dir.join(asset.primary_path.with_extension(""));
            fs::create_dir_all(&picture_dir)
                .with_context(|| format!("failed to create {}", picture_dir.display()))?;
            let picture_file_name = ext_picture_file_name(&picture);
            let picture_path = picture_dir.join(picture_file_name);
            fs::write(&picture_path, &picture)
                .with_context(|| format!("failed to write {}", picture_path.display()))?;
            fs::write(&xml_path, format_ext_picture_xml(picture_file_name))
                .with_context(|| format!("failed to write {}", xml_path.display()))?;
        }
        SourceAssetKind::Schedule => {
            let xml = extract_schedule_xml(bytes).with_context(|| {
                format!(
                    "failed to extract schedule from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::StyleBody { object_refs } => {
            let xml = extract_style_body_xml(bytes, object_refs).with_context(|| {
                format!(
                    "failed to extract style body from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::Form { object_refs } => {
            let xml = extract_form_body_xml(bytes, object_refs).with_context(|| {
                format!(
                    "failed to extract form xml from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
            for item_asset in extract_form_item_assets(bytes) {
                let item_path = output_dir
                    .join(asset.primary_path.with_extension(""))
                    .join("Items")
                    .join(sanitize_source_path_segment(&item_asset.item_name))
                    .join(&item_asset.file_name);
                if let Some(parent) = item_path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&item_path, &item_asset.content)
                    .with_context(|| format!("failed to write {}", item_path.display()))?;
            }
        }
        SourceAssetKind::Help => {
            let help = parse_help_blob(bytes).with_context(|| {
                format!(
                    "failed to extract help from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let xml_path = output_dir.join(&asset.primary_path);
            if let Some(parent) = xml_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let help_dir = output_dir.join(asset.primary_path.with_extension(""));
            fs::create_dir_all(&help_dir)
                .with_context(|| format!("failed to create {}", help_dir.display()))?;
            for page in &help.pages {
                let page_path = help_dir.join(&page.file_name);
                fs::write(&page_path, &page.content)
                    .with_context(|| format!("failed to write {}", page_path.display()))?;
            }
            if !help.files.is_empty() {
                let files_dir = help_dir.join("_files");
                fs::create_dir_all(&files_dir)
                    .with_context(|| format!("failed to create {}", files_dir.display()))?;
                for file in &help.files {
                    let file_path = files_dir.join(&file.file_name);
                    fs::write(&file_path, &file.content)
                        .with_context(|| format!("failed to write {}", file_path.display()))?;
                }
            }
            fs::write(&xml_path, format_help_xml(&help.pages))
                .with_context(|| format!("failed to write {}", xml_path.display()))?;
        }
        SourceAssetKind::InflatedBinary => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, inflated)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::InflatedBase64OrBinary => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let content = if let Ok(text) = std::str::from_utf8(&inflated) {
                extract_base64_payload(text)
                    .and_then(decode_base64_mime)
                    .unwrap_or(inflated)
            } else {
                inflated
            };
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::PredefinedData {
            xsi_type,
            type_index,
        } => {
            let items = parse_predefined_data_blob(bytes, type_index).with_context(|| {
                format!(
                    "failed to extract predefined data from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_predefined_data_xml(xsi_type, &items))
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::RoleRights {
            object_refs,
            field_refs,
        } => {
            let rights =
                parse_role_rights_blob(bytes, object_refs, field_refs).with_context(|| {
                    format!(
                        "failed to extract role rights from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_role_rights_xml(&rights))
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::CommandInterface {
            command_refs,
            metadata_refs,
        } => {
            let entries = parse_command_interface_blob(bytes, command_refs, metadata_refs)
                .with_context(|| {
                    format!(
                        "failed to extract command interface from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_command_interface_xml(&entries))
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::ConfigDumpInfo => {
            let _ = parse_config_dump_versions_blob(bytes).with_context(|| {
                format!(
                    "failed to parse config dump versions from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_config_dump_info_xml())
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::ExchangePlanContent { object_refs } => {
            let items =
                parse_exchange_plan_content_blob(bytes, object_refs).with_context(|| {
                    format!(
                        "failed to extract exchange plan content from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_exchange_plan_content_xml(&items))
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::BusinessProcessFlowchart => {
            let flowchart = parse_business_process_flowchart_blob(bytes).with_context(|| {
                format!(
                    "failed to extract business process flowchart from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, format_business_process_flowchart_xml(&flowchart))
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        SourceAssetKind::MoxelSpreadsheet { object_refs } => {
            let xml = extract_moxel_spreadsheet_xml(bytes, object_refs).with_context(|| {
                format!(
                    "failed to extract spreadsheet template from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
        }
    }

    Ok(asset.primary_path.clone())
}

struct HelpPage {
    page: String,
    file_name: String,
    content: Vec<u8>,
}

struct HelpFile {
    file_name: String,
    content: Vec<u8>,
}

struct HelpContent {
    pages: Vec<HelpPage>,
    files: Vec<HelpFile>,
}

struct FormItemAsset {
    item_name: String,
    file_name: String,
    content: Vec<u8>,
}

struct MoxelSpreadsheet {
    column_count: usize,
    column_sets: Vec<MoxelColumnSet>,
    column_formats: Vec<MoxelFormat>,
    default_format_width: Option<usize>,
    default_format: MoxelFormat,
    formats: Vec<MoxelFormat>,
    rows: Vec<MoxelRow>,
    merges: Vec<MoxelMerge>,
    areas: Vec<MoxelArea>,
    print_area: Option<MoxelArea>,
    print_settings: Option<MoxelPrintSettings>,
    lines: Vec<MoxelLine>,
    fonts: Vec<MoxelFont>,
    drawings: Vec<MoxelDrawing>,
    pictures: Vec<MoxelPicture>,
    empty_headers_footers: bool,
    default_format_index: Option<usize>,
    height: usize,
}

#[derive(Clone)]
struct MoxelRow {
    index: usize,
    index_to: Option<usize>,
    format_index: usize,
    columns_id: Option<String>,
    cells: Vec<MoxelCell>,
}

struct MoxelColumnSet {
    id: Option<String>,
    size: usize,
    columns: Vec<MoxelColumn>,
}

struct MoxelColumn {
    index: usize,
    format_index: usize,
}

#[derive(Clone)]
struct MoxelCell {
    column_index: usize,
    format_index: usize,
    text: Option<String>,
    parameter: Option<String>,
    detail_parameter: Option<String>,
    empty_text: bool,
}

struct MoxelLocalizedValue {
    lang: String,
    content: String,
}

struct MoxelArea {
    name: String,
    area_type: &'static str,
    begin_row: i32,
    end_row: i32,
    begin_column: i32,
    end_column: i32,
    columns_id: Option<String>,
}

struct MoxelMerge {
    row: i32,
    column: i32,
    height: i32,
    width: i32,
}

struct MoxelFont {
    ref_name: Option<String>,
    face_name: Option<String>,
    height: Option<usize>,
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

#[derive(Clone, Default)]
struct MoxelFormat {
    font: Option<usize>,
    border: Option<usize>,
    left_border: Option<usize>,
    top_border: Option<usize>,
    right_border: Option<usize>,
    bottom_border: Option<usize>,
    height: Option<usize>,
    border_color: Option<String>,
    width: Option<usize>,
    horizontal_alignment: Option<&'static str>,
    vertical_alignment: Option<&'static str>,
    back_color: Option<String>,
    text_color: Option<String>,
    text_placement: Option<&'static str>,
    fill_type: Option<&'static str>,
    drawing_border: Option<usize>,
    by_selected_columns: Option<bool>,
    details_use: Option<&'static str>,
    hyper_link: Option<bool>,
    protection: Option<bool>,
    indent: Option<usize>,
    auto_indent: Option<usize>,
    mask: Option<&'static str>,
    pic_index: Option<usize>,
    picture_size_mode: Option<&'static str>,
    pic_horizontal_alignment: Option<&'static str>,
    pic_vertical_alignment: Option<&'static str>,
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
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.back_color.is_none()
            && self.text_color.is_none()
            && self.text_placement.is_none()
            && self.fill_type.is_none()
            && self.drawing_border.is_none()
            && self.by_selected_columns.is_none()
            && self.details_use.is_none()
            && self.hyper_link.is_none()
            && self.protection.is_none()
            && self.indent.is_none()
            && self.auto_indent.is_none()
            && self.mask.is_none()
            && self.pic_index.is_none()
            && self.picture_size_mode.is_none()
            && self.pic_horizontal_alignment.is_none()
            && self.pic_vertical_alignment.is_none()
    }
}

struct CommandInterfaceEntry {
    name: String,
    common: bool,
}

struct ExchangePlanContentItem {
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
    properties: FlowchartItemProperties,
    events: Vec<FlowchartEvent>,
}

struct FlowchartItemProperties {
    location: Option<FlowchartLocation>,
    pivot_points: Vec<FlowchartPoint>,
    from: Option<FlowchartConnectionEnd>,
    to: Option<FlowchartConnectionEnd>,
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
struct PredefinedItem {
    id: String,
    name: String,
    code: String,
    description: String,
    value_types: Vec<ConstantValueType>,
    is_folder: bool,
    children: Vec<PredefinedItem>,
}

struct RoleRights {
    set_for_new_objects: bool,
    objects: Vec<RoleObjectRights>,
    restriction_templates: Vec<RoleRestrictionTemplate>,
}

struct RoleObjectRights {
    name: String,
    rights: Vec<RoleRight>,
}

struct RoleRight {
    name: String,
    value: bool,
    restriction_by_condition: Option<RoleRightRestriction>,
}

#[derive(Clone)]
struct RoleRightRestriction {
    field: Option<String>,
    condition: String,
}

struct RoleRestrictionTemplate {
    name: String,
    condition: String,
}

#[derive(Clone)]
struct MetadataCommandReference {
    kind: String,
    name: String,
}

fn build_metadata_command_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, MetadataCommandReference> {
    let mut index = BTreeMap::new();
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some((kind, header, _text)) =
            parse_metadata_command_reference_blob(&bytes, &row.file_name)
        else {
            continue;
        };
        index.insert(
            row.file_name.clone(),
            MetadataCommandReference {
                kind,
                name: header.name,
            },
        );
    }
    index
}

fn build_metadata_object_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        if let Some(name) = parse_configuration_reference_blob(&bytes) {
            index.insert(row.file_name.clone(), format!("Configuration.{name}"));
            continue;
        }
        let Some((kind, header, text)) =
            parse_metadata_command_reference_blob(&bytes, &row.file_name)
        else {
            continue;
        };
        index.insert(row.file_name.clone(), format!("{kind}.{}", header.name));
        for command in nested_command_headers_from_text(&text, &row.file_name) {
            index.insert(
                command.uuid,
                format!("{}.{}.Command.{}", kind, header.name, command.name),
            );
        }
        if kind == "WebService" {
            for operation in nested_web_service_operation_headers_from_text(&text, &row.file_name) {
                index.insert(
                    operation.uuid,
                    format!("WebService.{}.Operation.{}", header.name, operation.name),
                );
            }
        }
    }
    index
}

fn build_metadata_field_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some((_kind, _header, text)) =
            parse_metadata_command_reference_blob(&bytes, &row.file_name)
        else {
            continue;
        };
        for header in nested_metadata_headers_from_text(&text, &row.file_name) {
            index.insert(header.uuid, header.name);
        }
    }
    index
}

fn build_form_source_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, FormSourceReference> {
    let mut forms = Vec::<MetadataHeader>::new();
    let mut owners = Vec::<(String, PathBuf)>::new();

    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        let text = text.trim_start_matches('\u{feff}');
        let Some(object_code) = parse_metadata_object_code(text) else {
            continue;
        };
        if is_form_metadata_text(text, &row.file_name) {
            if let Some(header) = parse_metadata_header_from_text(text, &row.file_name) {
                forms.push(header);
            }
            continue;
        }
        let Some((kind, folder)) = metadata_source_for_text(object_code, text, &row.file_name)
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let Some(header) = parse_metadata_header_from_text(text, &row.file_name) else {
            continue;
        };
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        owners.push((text.to_string(), owner_path));
    }

    let mut index = BTreeMap::new();
    for form in forms {
        let owner_matches = owners
            .iter()
            .filter(|(owner_text, _)| owner_text.contains(&form.uuid))
            .collect::<Vec<_>>();
        let relative_path = if let [(_, owner_path)] = owner_matches.as_slice() {
            owner_path
                .join("Forms")
                .join(sanitize_source_path_segment(&form.name))
                .with_extension("xml")
        } else {
            PathBuf::from("CommonForms")
                .join(sanitize_source_path_segment(&form.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonForms") {
            "CommonForm"
        } else {
            "Form"
        };
        index.insert(
            form.uuid,
            FormSourceReference {
                relative_path,
                kind,
            },
        );
    }

    index
}

fn build_template_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, TemplateSourceReference> {
    let rows_by_file_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut templates = Vec::<MetadataHeader>::new();
    let mut owners = Vec::<(String, PathBuf)>::new();

    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        let text = text.trim_start_matches('\u{feff}');
        let Some(object_code) = parse_metadata_object_code(text) else {
            continue;
        };
        if is_template_metadata_text(text, &row.file_name) {
            if let Some(header) = parse_metadata_header_from_text(text, &row.file_name) {
                templates.push(header);
            }
            continue;
        }
        let Some((kind, folder)) = metadata_source_for_text(object_code, text, &row.file_name)
        else {
            continue;
        };
        if !metadata_kind_can_own_templates(kind) {
            continue;
        }
        let Some(header) = parse_metadata_header_from_text(text, &row.file_name) else {
            continue;
        };
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        owners.push((text.to_string(), owner_path));
    }

    let mut index = BTreeMap::new();
    for template in templates {
        let owner_matches = owners
            .iter()
            .filter(|(owner_text, _)| owner_text.contains(&template.uuid))
            .collect::<Vec<_>>();
        let relative_path = if let [(_, owner_path)] = owner_matches.as_slice() {
            owner_path
                .join("Templates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        } else {
            PathBuf::from("CommonTemplates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonTemplates") {
            "CommonTemplate"
        } else {
            "Template"
        };
        let body_id = format!("{}.0", template.uuid);
        let template_type = template_template_type_from_metadata(&template)
            .or_else(|| {
                rows_by_file_name
                    .get(body_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .and_then(|bytes| infer_template_type_from_body(&bytes))
            })
            .unwrap_or("BinaryData");
        index.insert(
            template.uuid,
            TemplateSourceReference {
                relative_path,
                kind,
                template_type,
            },
        );
    }

    index
}

fn build_subsystem_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, SubsystemSourceReference> {
    let mut subsystems = BTreeMap::<String, (MetadataHeader, String)>::new();

    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        let text = text.trim_start_matches('\u{feff}');
        let Some(object_code) = parse_metadata_object_code(text) else {
            continue;
        };
        let Some((kind, _folder)) = metadata_source_for_text(object_code, text, &row.file_name)
        else {
            continue;
        };
        if kind != "Subsystem" {
            continue;
        }
        let Some(header) = parse_metadata_header_from_text(text, &row.file_name) else {
            continue;
        };
        subsystems.insert(header.uuid.clone(), (header, text.to_string()));
    }

    let mut parent_by_child = BTreeMap::<String, String>::new();
    for child_uuid in subsystems.keys() {
        let owners = subsystems
            .iter()
            .filter(|(owner_uuid, (_, owner_text))| {
                owner_uuid.as_str() != child_uuid.as_str() && owner_text.contains(child_uuid)
            })
            .map(|(owner_uuid, _)| owner_uuid.clone())
            .collect::<Vec<_>>();
        if let [owner_uuid] = owners.as_slice() {
            parent_by_child.insert(child_uuid.clone(), owner_uuid.clone());
        }
    }

    let mut memo = BTreeMap::<String, PathBuf>::new();
    for uuid in subsystems.keys() {
        let mut visiting = BTreeSet::<String>::new();
        let _ = resolve_subsystem_source_path(
            uuid,
            &subsystems,
            &parent_by_child,
            &mut memo,
            &mut visiting,
        );
    }

    memo.into_iter()
        .map(|(uuid, relative_path)| (uuid, SubsystemSourceReference { relative_path }))
        .collect()
}

fn resolve_subsystem_source_path(
    uuid: &str,
    subsystems: &BTreeMap<String, (MetadataHeader, String)>,
    parent_by_child: &BTreeMap<String, String>,
    memo: &mut BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<String>,
) -> Option<PathBuf> {
    if let Some(path) = memo.get(uuid) {
        return Some(path.clone());
    }
    if !visiting.insert(uuid.to_string()) {
        return None;
    }
    let (header, _) = subsystems.get(uuid)?;
    let name = sanitize_source_path_segment(&header.name);
    let relative_path = if let Some(parent_uuid) = parent_by_child.get(uuid) {
        resolve_subsystem_source_path(parent_uuid, subsystems, parent_by_child, memo, visiting)
            .map(|parent_path| {
                parent_path
                    .with_extension("")
                    .join("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
            .unwrap_or_else(|| {
                PathBuf::from("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
    } else {
        PathBuf::from("Subsystems")
            .join(&name)
            .with_extension("xml")
    };
    visiting.remove(uuid);
    memo.insert(uuid.to_string(), relative_path.clone());
    Some(relative_path)
}

fn infer_template_type_from_body(bytes: &[u8]) -> Option<&'static str> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    if inflated.starts_with(b"MOXCEL") {
        return Some("SpreadsheetDocument");
    }
    let Ok(text) = std::str::from_utf8(&inflated) else {
        return Some("BinaryData");
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.starts_with("<?xml") && text.contains("data-composition-system/appearance-template") {
        Some("DataCompositionAppearanceTemplate")
    } else if text.starts_with("<?xml") && text.contains("data-composition-system/schema") {
        Some("DataCompositionSchema")
    } else if text.starts_with("<?xml") && text.contains("8.3/xcf/scheme") {
        Some("GraphicalSchema")
    } else if text.starts_with("<!DOCTYPE")
        || text.starts_with("<html")
        || text.starts_with("<?xml") && text.contains("<html")
    {
        Some("HTMLDocument")
    } else {
        Some("TextDocument")
    }
}

fn template_template_type_from_metadata(header: &MetadataHeader) -> Option<&'static str> {
    template_type_from_code(header.template_type_code?)
}

fn template_type_from_code(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("SpreadsheetDocument"),
        1 => Some("BinaryData"),
        3 => Some("HTMLDocument"),
        4 => Some("TextDocument"),
        6 => Some("DataCompositionSchema"),
        9 => Some("AddIn"),
        _ => None,
    }
}

fn form_help_asset_paths(
    rows: &[ConfigRow],
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> BTreeMap<String, SourceAsset> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in form_refs {
        let row_prefix = format!("{form_uuid}.");
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        for body_id in file_names
            .iter()
            .filter(|file_name| file_name.starts_with(&row_prefix))
        {
            let module_body_id = format!("{form_uuid}.0");
            if *body_id == module_body_id.as_str() {
                continue;
            }
            if let Some(row) = rows_by_file_name.get(*body_id)
                && let Ok(bytes) = decode_hex(&row.binary_hex)
                && parse_help_blob_pages(&bytes).is_some()
            {
                paths.insert(
                    (*body_id).to_string(),
                    SourceAsset {
                        primary_path: form_dir.join("Ext").join("Help.xml"),
                        kind: SourceAssetKind::Help,
                    },
                );
            }
        }
    }
    paths
}

fn parse_configuration_reference_blob(blob: &[u8]) -> Option<String> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let uuid_start = marker_start + marker.len();
    let uuid_end = uuid_start + 36;
    let uuid = text.get(uuid_start..uuid_end)?;
    if !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
        return None;
    }
    parse_metadata_header_from_text(text, uuid).map(|header| header.name)
}

fn extract_configuration_source_xml(text: &str, uuid: &str) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let uuid_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    if uuid_fields.first()?.trim() != uuid {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let header_uuid_start = marker_start + marker.len();
    let header_uuid_end = header_uuid_start + 36;
    let header_uuid = text.get(header_uuid_start..header_uuid_end)?;
    if !is_uuid_text(header_uuid) || !is_metadata_header_marker(text, header_uuid_end) {
        return None;
    }
    let mut header = parse_metadata_header_from_text(text, header_uuid)?;
    header.uuid = uuid.to_string();
    Some(format_metadata_source_xml("Configuration", &header))
}

fn build_command_interface_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some((kind, header, text)) =
            parse_metadata_command_reference_blob(&bytes, &row.file_name)
        else {
            continue;
        };
        if kind == "CommonCommand" {
            index.insert(
                row.file_name.clone(),
                format!("CommonCommand.{}", header.name),
            );
        }
        for command in nested_command_headers_from_text(&text, &row.file_name) {
            index.insert(
                command.uuid,
                format!("{}.{}.Command.{}", kind, header.name, command.name),
            );
        }
    }
    index
}

fn parse_metadata_command_reference_blob(
    blob: &[u8],
    uuid: &str,
) -> Option<(String, MetadataHeader, String)> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let object_code = parse_metadata_object_code(&text)?;
    let (kind, _) = metadata_source_for_text(object_code, &text, uuid)?;
    let header = parse_metadata_header_from_text(&text, uuid)?;
    Some((kind.to_string(), header, text))
}

fn parse_command_interface_blob(
    bytes: &[u8],
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> Option<Vec<CommandInterfaceEntry>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    let count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let mut entries = Vec::with_capacity(count);
    let mut index = 3usize;
    for _ in 0..count {
        let command_ref = split_1c_braced_fields(fields.get(index)?, 0)?;
        index += 1;
        let code = command_ref.first()?.trim();
        if !code.chars().all(|ch| ch.is_ascii_digit()) {
            return None;
        }
        let common = parse_command_interface_common_flag(fields.get(index)?)?;
        index += 1;
        let name = if let Some(uuid) = command_ref.get(1).map(|value| value.trim()) {
            if !is_uuid_text(uuid) {
                return None;
            }
            command_interface_command_name(code, uuid, command_refs, metadata_refs)
        } else {
            code.to_string()
        };
        entries.push(CommandInterfaceEntry { name, common });
    }

    Some(entries)
}

fn parse_command_interface_common_flag(value: &str) -> Option<bool> {
    if value.contains(r#"{"B",1}"#) {
        Some(true)
    } else if value.contains(r#"{"B",0}"#) {
        Some(false)
    } else {
        None
    }
}

fn parse_exchange_plan_content_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<ExchangePlanContentItem>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut items = Vec::with_capacity(count);
    let mut index = 2usize;
    for _ in 0..count {
        let object_id = parse_uuid_field(fields.get(index)?.trim())?;
        let auto_record = exchange_plan_auto_record_xml(fields.get(index + 1)?.trim());
        let metadata = object_refs.get(&object_id)?.clone();
        items.push(ExchangePlanContentItem {
            metadata,
            auto_record,
        });
        index += 2;
    }

    Some(items)
}

fn parse_config_dump_versions_blob(bytes: &[u8]) -> Option<usize> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    fields.get(1)?.trim().parse::<usize>().ok()
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
    for (code, body) in raw_items {
        items.push(parse_flowchart_item(&code, &body, &names)?);
    }

    Some(BusinessProcessFlowchart { items })
}

fn parse_flowchart_item(
    code: &str,
    body: &str,
    names: &BTreeMap<String, String>,
) -> Option<FlowchartItem> {
    let fields = split_1c_braced_fields(body, 0)?;
    let base = parse_flowchart_base(code, body)?;
    let mut properties = FlowchartItemProperties {
        location: None,
        pivot_points: Vec::new(),
        from: None,
        to: None,
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
        _ => return None,
    };

    Some(FlowchartItem {
        tag,
        id: base.id,
        uuid: base.uuid,
        name: base.name,
        title: base.title,
        tab_order: base.tab_order,
        properties,
        events,
    })
}

fn parse_flowchart_base(code: &str, body: &str) -> Option<FlowchartBase> {
    let fields = split_1c_braced_fields(body, 0)?;
    let head = split_1c_braced_fields(fields.first()?, 0)?;
    let uuid = if matches!(code, "2" | "3" | "4" | "5") {
        head.get(2).map(|value| value.trim().to_string())
    } else {
        None
    }
    .filter(|value| is_uuid_text(value));
    let base_fields = if matches!(code, "2" | "3" | "4" | "5") {
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
        "1" => "Auto",
        _ => "Auto",
    }
}

fn command_interface_command_name(
    code: &str,
    uuid: &str,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> String {
    if let Some(name) = command_refs.get(uuid) {
        return name.clone();
    }
    if let Some(metadata) = metadata_refs.get(uuid) {
        if code == "0"
            && let Some(standard) = command_interface_standard_command(&metadata.kind)
        {
            return format!(
                "{}.{}.StandardCommand.{standard}",
                metadata.kind, metadata.name
            );
        }
        if code == "1" {
            return format!("{}.{}.StandardCommand.Create", metadata.kind, metadata.name);
        }
    }

    format!("{code}:{uuid}")
}

fn command_interface_standard_command(kind: &str) -> Option<&'static str> {
    match kind {
        "DataProcessor" | "Report" | "CommonForm" => Some("Open"),
        "AccountingRegister"
        | "AccumulationRegister"
        | "BusinessProcess"
        | "Catalog"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "ChartOfCharacteristicTypes"
        | "Document"
        | "DocumentJournal"
        | "Enum"
        | "ExchangePlan"
        | "InformationRegister"
        | "Task" => Some("OpenList"),
        _ => None,
    }
}

fn parse_role_rights_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
) -> Option<RoleRights> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "10" {
        return None;
    }
    let object_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    let count = object_fields.first()?.trim().parse::<usize>().ok()?;
    if object_fields.len() != count + 1 {
        return None;
    }

    let mut objects = Vec::with_capacity(count);
    for object_field in object_fields.iter().skip(1) {
        let entry = split_1c_braced_fields(object_field, 0)?;
        if entry.len() != 2 {
            return None;
        }

        let object_ref = split_1c_braced_fields(entry[0], 0)?;
        let object_uuid = object_ref.get(1)?.trim();
        if !is_uuid_text(object_uuid) {
            return None;
        }
        let object_name = object_refs.get(object_uuid)?.clone();

        let rights = parse_role_object_rights(entry[1], field_refs)?;
        objects.push(RoleObjectRights {
            name: object_name,
            rights,
        });
    }

    objects.reverse();
    let restriction_templates = parse_role_restriction_templates(fields.get(2)?)?;
    let set_for_new_objects = parse_role_bool_field(fields.get(4)?)?;
    Some(RoleRights {
        set_for_new_objects,
        objects,
        restriction_templates,
    })
}

fn parse_role_object_rights(
    value: &str,
    field_refs: &BTreeMap<String, String>,
) -> Option<Vec<RoleRight>> {
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first()?.trim() {
        "0" if (fields.len() - 1) % 2 == 0 => {
            parse_role_right_pairs(&fields, 1, (fields.len() - 1) / 2, &BTreeMap::new())
        }
        "0" => None,
        "1" => {
            let count = fields.get(1)?.trim().parse::<usize>().ok()?;
            let pairs_start = 2usize;
            let restrictions_count_index = pairs_start.checked_add(count.checked_mul(2)?)?;
            if fields.len() <= restrictions_count_index {
                return None;
            }
            let restrictions = parse_role_right_restrictions(
                fields.get(restrictions_count_index)?.trim(),
                &fields[restrictions_count_index + 1..],
                field_refs,
            )?;
            parse_role_right_pairs(&fields, pairs_start, count, &restrictions)
        }
        _ => None,
    }
}

fn parse_role_right_pairs(
    fields: &[&str],
    start: usize,
    count: usize,
    restrictions: &BTreeMap<String, RoleRightRestriction>,
) -> Option<Vec<RoleRight>> {
    let mut rights = Vec::with_capacity(count);
    for index in 0..count {
        let offset = start.checked_add(index.checked_mul(2)?)?;
        let right_uuid = fields.get(offset)?.trim();
        if !is_uuid_text(right_uuid) {
            return None;
        }
        let value = parse_role_right_value(fields.get(offset + 1)?.trim())?;
        rights.push(RoleRight {
            name: role_right_name(right_uuid)?.to_string(),
            value,
            restriction_by_condition: restrictions.get(right_uuid).cloned(),
        });
    }
    Some(rights)
}

fn parse_role_bool_field(value: &str) -> Option<bool> {
    match value.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_role_right_value(value: &str) -> Option<bool> {
    match value {
        "-1" | "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_role_right_restrictions(
    count_text: &str,
    values: &[&str],
    field_refs: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, RoleRightRestriction>> {
    let count = count_text.parse::<usize>().ok()?;
    let mut restrictions = BTreeMap::new();
    if count == 0 {
        if !values.is_empty() {
            return None;
        }
        return Some(restrictions);
    }
    if values.len() == count {
        for entry in values {
            let pair = split_1c_braced_fields(entry, 0)?;
            if pair.len() != 2 {
                return None;
            }
            let right_uuid = pair.first()?.trim();
            if !is_uuid_text(right_uuid) {
                return None;
            }
            let condition = parse_role_restriction_condition(pair.get(1)?, field_refs)?;
            restrictions.insert(right_uuid.to_string(), condition);
        }
        return Some(restrictions);
    }
    if values.len() != 1 {
        return None;
    }
    let entries = split_1c_braced_fields(values[0], 0)?;
    if entries.len() == count * 2
        && entries
            .first()
            .is_some_and(|entry| is_uuid_text(entry.trim()))
    {
        for entry in entries.chunks(2) {
            let right_uuid = entry.first()?.trim();
            let condition = parse_role_restriction_condition(entry.get(1)?, field_refs)?;
            restrictions.insert(right_uuid.to_string(), condition);
        }
        return Some(restrictions);
    }
    if entries.len() != count {
        return None;
    }
    for entry in entries {
        let pair = split_1c_braced_fields(entry, 0)?;
        if pair.len() != 2 {
            return None;
        }
        let right_uuid = pair.first()?.trim();
        if !is_uuid_text(right_uuid) {
            return None;
        }
        let condition = parse_role_restriction_condition(pair.get(1)?, field_refs)?;
        restrictions.insert(right_uuid.to_string(), condition);
    }
    Some(restrictions)
}

fn parse_role_restriction_condition(
    value: &str,
    field_refs: &BTreeMap<String, String>,
) -> Option<RoleRightRestriction> {
    let wrapper = split_1c_braced_fields(value, 0)?;
    match wrapper.first()?.trim() {
        "1" => parse_role_restriction_condition_body(wrapper.get(1)?),
        "2" => {
            let mut restriction = parse_role_restriction_condition_body(wrapper.get(2)?)?;
            let field = parse_role_restriction_field(wrapper.get(2)?, field_refs)?;
            restriction.field = Some(field);
            Some(restriction)
        }
        _ => None,
    }
}

fn parse_role_restriction_condition_body(value: &str) -> Option<RoleRightRestriction> {
    let condition_fields = split_1c_braced_fields(value, 0)?;
    if condition_fields.first()?.trim() != "1" {
        return None;
    }
    parse_1c_quoted_string_with_len(condition_fields.get(1)?.trim()).map(|(condition, _)| {
        RoleRightRestriction {
            field: None,
            condition,
        }
    })
}

fn parse_role_restriction_field(
    value: &str,
    field_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some((_, name)) = field_refs
        .iter()
        .find(|(uuid, _)| value.contains(uuid.as_str()))
    {
        return Some(name.clone());
    }
    let field_wrapper = split_1c_braced_fields(value, 0)?;
    if field_wrapper.first()?.trim() != "0" {
        return None;
    }
    let field_fields = split_1c_braced_fields(field_wrapper.get(1)?, 0)?;
    if field_fields.first()?.trim() != "1" {
        return None;
    }
    parse_1c_quoted_string_with_len(field_fields.get(1)?.trim()).map(|(value, _)| value)
}

fn parse_role_restriction_templates(value: &str) -> Option<Vec<RoleRestrictionTemplate>> {
    let fields = split_1c_braced_fields(value, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if fields.len() != count + 1 {
        return None;
    }
    let mut templates = Vec::with_capacity(count);
    for field in fields.iter().skip(1) {
        let template = split_1c_braced_fields(field, 0)?;
        if template.len() != 2 {
            return None;
        }
        let name = parse_1c_quoted_string_with_len(template.first()?.trim())?.0;
        let condition = parse_1c_quoted_string_with_len(template.get(1)?.trim())?.0;
        templates.push(RoleRestrictionTemplate { name, condition });
    }
    Some(templates)
}

fn role_right_name(uuid: &str) -> Option<&'static str> {
    ROLE_RIGHT_NAMES
        .iter()
        .find_map(|(right_uuid, name)| (*right_uuid == uuid).then_some(*name))
}

const ROLE_RIGHT_NAMES: &[(&str, &str)] = &[
    ("fd05f656-7a23-43a4-8996-f480a806fb97", "ActiveUsers"),
    ("900e3c92-6e18-4874-846a-b28780b5b54c", "Administration"),
    (
        "f7c6a0bb-bca6-4cd3-9146-832971cd7073",
        "AnalyticsSystemClient",
    ),
    ("07ef4641-f7da-417a-bd75-35c40a17c2f7", "Automation"),
    (
        "399d7390-8d83-4a57-b4d7-c902c15b701f",
        "ConfigurationExtensionsAdministration",
    ),
    ("10b8ce49-ae3d-4a2e-afe7-1e3648bd59f7", "DataAdministration"),
    ("c0028105-4cc1-41ca-aef1-bfbd8fc8f8c4", "Delete"),
    ("b7bab52d-c1b1-4bd8-8276-02db08d42352", "Edit"),
    (
        "8497054a-ffd1-4ca7-bdfe-340b9ddc050a",
        "EditDataHistoryVersionComment",
    ),
    ("1c799cf9-342d-4bf7-9b6f-951a009228ce", "EventLog"),
    ("8fb221e3-0d4f-43f2-ad71-1984cad63375", "ExclusiveMode"),
    ("74fd69fa-368e-4292-956a-65eb2f9877bd", "Execute"),
    ("02119c69-f08a-4142-9426-3725d74b7719", "ExternalConnection"),
    ("499e8968-ca89-43f0-9955-8756058b1b53", "Get"),
    ("b5f861d3-d9c5-45ec-98bf-0ed4d489a351", "InputByString"),
    ("33200740-82b0-4de7-8556-d3fb25ca4328", "Insert"),
    (
        "3b869658-ebc9-49ff-9bb3-e7c59686f538",
        "InteractiveActivate",
    ),
    (
        "b0c0cbfc-f2cc-4b80-8460-5d5d7a599d9d",
        "InteractiveChangeOfPosted",
    ),
    (
        "798cf688-ad74-44fe-a464-236b49e910e0",
        "InteractiveClearDeletionMark",
    ),
    (
        "e7f9daf9-eac2-4ada-9c26-c380858f3589",
        "InteractiveClearDeletionMarkPredefinedData",
    ),
    ("b53db6ed-6e5b-4035-8d24-f10083d646ed", "InteractiveDelete"),
    (
        "fa6dbe86-856a-4ac4-b8ac-bce99f8b8b22",
        "InteractiveDeleteMarked",
    ),
    (
        "65e5f92c-40ff-4130-9652-c0e7612d0609",
        "InteractiveDeleteMarkedPredefinedData",
    ),
    (
        "013a262e-165f-4815-bdae-7a1bed6a68e4",
        "InteractiveDeletePredefinedData",
    ),
    ("fb88c756-91c9-4351-9cdf-e027879886c6", "InteractiveInsert"),
    (
        "7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6",
        "InteractiveOpenExtDataProcessors",
    ),
    (
        "eb29e198-c338-4a20-a253-be6fc3dd44d9",
        "InteractiveOpenExtReports",
    ),
    ("5d167fcc-b11f-403a-9a37-1eda64c19df1", "InteractivePosting"),
    (
        "21b4742a-d335-4234-bf0f-a3074a0e31ac",
        "InteractivePostingRegular",
    ),
    (
        "d76b72ba-5388-4b7f-af64-1b351f63a1e1",
        "InteractiveSetDeletionMark",
    ),
    (
        "408c56c0-e210-4e2e-8e82-610050a08a39",
        "InteractiveSetDeletionMarkPredefinedData",
    ),
    (
        "4d0d77ec-8511-430d-bd77-8407f27bc8f4",
        "InteractiveUndoPosting",
    ),
    ("5e664189-f0ee-439c-bdc5-eb81cca41ddf", "InteractiveExecute"),
    (
        "b9b44b51-3ac9-47cd-8b5a-df51afdcceb0",
        "MainWindowModeEmbeddedWorkplace",
    ),
    (
        "818fc6c3-4691-44e3-a80c-e8d424730ead",
        "MainWindowModeFullscreenWorkplace",
    ),
    (
        "155a0b35-4343-4047-989b-d385373b063e",
        "MainWindowModeKiosk",
    ),
    (
        "d066966a-ff6a-4a41-bd68-6191cab083bc",
        "MainWindowModeNormal",
    ),
    (
        "f6168734-8b8d-4a88-ab39-ef6b51758e83",
        "MainWindowModeWorkplace",
    ),
    ("1e50809b-73ed-4935-bb77-2616c4cabdf5", "MobileClient"),
    ("31c3d4f6-7d02-4654-a14e-06aacafcb4fa", "Output"),
    ("e060de25-bffd-42fd-bb09-f3a788d65760", "Posting"),
    ("1c87578f-9e09-4ec0-a991-5629c87b1588", "Read"),
    ("64319ca1-f3d8-472e-82ce-5da233e6daaa", "ReadDataHistory"),
    (
        "1b762bf9-df7f-4255-bbe6-f7578f41368d",
        "ReadDataHistoryOfMissingData",
    ),
    ("d8682bbb-7800-4aa0-8590-d3cb11fe2a29", "SaveUserData"),
    ("1d306db2-d97e-4b57-9b28-5d21e838cd9e", "Set"),
    ("65b6855f-85d5-4d33-ab75-be4485326dd5", "Start"),
    ("84487e82-eb6c-4c51-ae16-3a6db17e886d", "InteractiveStart"),
    (
        "479a42c0-c3e9-4ae7-bf4a-75cebc14fec4",
        "SwitchToDataHistoryVersion",
    ),
    (
        "265eec41-3ce1-4a07-bc3b-253d44c9a4f4",
        "TechnicalSpecialistMode",
    ),
    ("29da0973-3b85-40e5-89da-bce02dbab08e", "ThickClient"),
    ("3c00c6ee-844e-4620-85e4-671e72f114d9", "ThinClient"),
    ("24abfe06-289a-48c5-8bb4-032c733e45c5", "TotalsControl"),
    ("f55a8f7f-2c65-404f-b530-093d9006adba", "UndoPosting"),
    ("287b74b8-3a66-4a76-ba27-4f1f6a93770e", "Update"),
    (
        "4d87a22d-ca7f-40ba-a367-a4eae62f4a7f",
        "UpdateDataBaseConfiguration",
    ),
    ("b162ff57-0296-483e-9af8-dc37576802cb", "UpdateDataHistory"),
    (
        "c4ab1331-e58d-4a46-ad2e-fe6d80b72aa4",
        "UpdateDataHistoryOfMissingData",
    ),
    (
        "a679c969-8ea1-4b8b-9e61-8a414ba448f4",
        "UpdateDataHistorySettings",
    ),
    (
        "5b3ea0e2-fdb9-41f6-bf6c-25747906b4cb",
        "UpdateDataHistoryVersionComment",
    ),
    ("c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1", "Use"),
    ("aa6448f2-be0f-42ea-ba26-1af7f52b5b65", "View"),
    ("9342b152-a7ae-4c79-9b7b-f4f028a36479", "ViewDataHistory"),
    ("bd33c881-192c-4ef7-a51d-b146e38c5078", "WebClient"),
];

fn parse_help_blob_pages(bytes: &[u8]) -> Option<Vec<HelpPage>> {
    parse_help_blob(bytes).map(|help| help.pages)
}

fn parse_help_blob(bytes: &[u8]) -> Option<HelpContent> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "5" {
        return None;
    }
    let page_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut index = 2usize;
    let mut pages = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        let (language, _) = parse_1c_quoted_string_with_len(fields.get(index)?.trim())?;
        index += 1;
        let payload = extract_base64_payload(fields.get(index)?.trim())?;
        index += 1;
        let content = decode_base64_mime(payload)?;
        let page = sanitize_source_path_segment(&language);
        pages.push(HelpPage {
            file_name: format!("{page}.html"),
            page,
            content,
        });
    }

    if pages.is_empty() {
        return None;
    }

    let mut files = Vec::new();
    if let Some(count) = fields
        .get(index)
        .and_then(|field| field.trim().parse::<usize>().ok())
    {
        index += 1;
        for _ in 0..count {
            let (file_name, _) = parse_1c_quoted_string_with_len(fields.get(index)?.trim())?;
            index += 1;
            if fields
                .get(index)
                .is_some_and(|field| field.trim().chars().all(|ch| ch.is_ascii_digit()))
            {
                index += 1;
            }
            let payload = extract_base64_payload(fields.get(index)?.trim())?;
            index += 1;
            files.push(HelpFile {
                file_name: sanitize_source_path_segment(&file_name),
                content: decode_base64_mime(payload)?,
            });
        }
    }

    Some(HelpContent { pages, files })
}

fn predefined_data_xsi_type(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" => Some("CatalogPredefinedItems"),
        "ChartOfCharacteristicTypes" => Some("PlanOfCharacteristicKindPredefinedItems"),
        _ => None,
    }
}

fn parse_predefined_data_blob(
    bytes: &[u8],
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(text, 0)?;
    if !matches!(fields.first()?.trim(), "0" | "1") {
        return None;
    }
    let table_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    let root_items = table_fields
        .iter()
        .find_map(|field| parse_predefined_rowset_roots(field, type_index))?;
    if let [root_item] = root_items.as_slice()
        && matches!(root_item.name.as_str(), "Элементы" | "Характеристики")
    {
        Some(root_item.children.clone())
    } else {
        Some(root_items)
    }
}

fn parse_predefined_rowset_roots(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    fields
        .iter()
        .find_map(|field| parse_predefined_children(field, type_index))
}

fn parse_predefined_item(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<PredefinedItem> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let value_count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let value_start = 3usize;
    let after_values = value_start.checked_add(value_count)?;
    if fields.len() < after_values {
        return None;
    }

    let id = parse_predefined_uuid_value(fields.get(value_start)?)?;
    let is_folder = fields
        .get(value_start + 1)
        .and_then(|field| parse_predefined_bool_value(field))
        .unwrap_or(false);
    let has_parent_ref = fields
        .get(value_start + 2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|field| field.first().map(|value| value.trim() == r##""#""##))
        .unwrap_or(false);
    let name_offset = if has_parent_ref {
        value_start + 3
    } else {
        value_start + 2
    };
    let name = fields
        .get(name_offset)
        .and_then(|field| parse_predefined_string_value(field))?;
    let code = fields
        .get(name_offset + 1)
        .and_then(|field| parse_predefined_string_value(field))
        .unwrap_or_default();
    let description = fields
        .get(name_offset + 2)
        .and_then(|field| parse_predefined_string_value(field))
        .unwrap_or_default();
    let value_types = fields
        .get(name_offset + 3)
        .and_then(|field| parse_predefined_type_value(field, type_index))
        .unwrap_or_default();
    let children = if fields
        .get(after_values)
        .is_some_and(|field| field.trim() == "1")
    {
        fields
            .get(after_values + 1)
            .and_then(|field| parse_predefined_children(field, type_index))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Some(PredefinedItem {
        id,
        name,
        code,
        description,
        value_types,
        is_folder,
        children,
    })
}

fn parse_predefined_children(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let children = fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_predefined_item(field, type_index))
        .collect::<Vec<_>>();
    if children.len() == count {
        Some(children)
    } else {
        None
    }
}

fn parse_predefined_type_value(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    parse_metadata_type_pattern(fields.get(2)?, type_index)
}

fn parse_predefined_uuid_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    let ref_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
    let uuid = ref_fields.get(1)?.trim();
    parse_uuid_field(uuid)
}

fn parse_predefined_bool_value(value: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""B""# {
        return None;
    }
    parse_1c_bool_flag(fields.get(1)?.trim())
}

fn parse_predefined_string_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""S""# {
        return None;
    }
    fields
        .get(1)
        .and_then(|field| parse_1c_quoted_string_with_len(field.trim()))
        .map(|(value, _)| value)
}

fn extract_ext_picture(bytes: &[u8]) -> Result<Vec<u8>> {
    let inflated = inflate_raw_deflate(bytes)?;
    if let Ok(text) = std::str::from_utf8(&inflated)
        && let Some(payload) = extract_base64_payload(text)
    {
        return decode_base64_mime(payload).context("failed to decode picture base64");
    }
    Ok(inflated)
}

fn ext_picture_file_name(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "Picture.png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "Picture.gif"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "Picture.jpg"
    } else if bytes.starts_with(b"BM") {
        "Picture.bmp"
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        "Picture.ico"
    } else if bytes.starts_with(b"PK\x03\x04") {
        "Picture.zip"
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim_start_matches('\u{feff}').trim_start();
        if is_svg_text(text) {
            "Picture.svg"
        } else if trimmed.starts_with('<') {
            "Picture.xml"
        } else {
            "Picture.txt"
        }
    } else {
        "Picture.bin"
    }
}

fn is_svg_content(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    is_svg_text(text)
}

fn is_svg_text(text: &str) -> bool {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("<svg") || text.starts_with("<?xml") && text.contains("<svg")
}

fn extract_base64_payload(text: &str) -> Option<&str> {
    let prefix = "{#base64:";
    let start = text.find(prefix)? + prefix.len();
    let end = text[start..].find('}')? + start;
    Some(&text[start..end])
}

fn decode_base64_mime(input: &str) -> Option<Vec<u8>> {
    let values = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if values.len() % 4 != 0 {
        return None;
    }

    let mut output = Vec::with_capacity(values.len() / 4 * 3);
    for chunk in values.chunks(4) {
        let mut decoded = [0u8; 4];
        let mut padding = 0usize;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                padding += 1;
                decoded[index] = 0;
                continue;
            }
            if padding > 0 {
                return None;
            }
            decoded[index] = base64_value(byte)?;
        }
        if padding > 2 {
            return None;
        }
        output.push((decoded[0] << 2) | (decoded[1] >> 4));
        if padding < 2 {
            output.push((decoded[1] << 4) | (decoded[2] >> 2));
        }
        if padding < 1 {
            output.push((decoded[2] << 6) | decoded[3]);
        }
    }

    Some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn format_ext_picture_xml(file_name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ExtPicture xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.17\">\r\n\
\t<Picture>\r\n\
\t\t<xr:Abs>{file_name}</xr:Abs>\r\n\
\t\t<xr:LoadTransparent>false</xr:LoadTransparent>\r\n\
\t</Picture>\r\n\
</ExtPicture>\r\n"
    )
}

fn format_help_xml(pages: &[HelpPage]) -> String {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Help xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
    );
    for page in pages {
        xml.push_str("\t<Page>");
        xml.push_str(&escape_xml_text(&page.page));
        xml.push_str("</Page>\r\n");
    }
    xml.push_str("</Help>");
    xml
}

fn format_predefined_data_xml(xsi_type: &str, items: &[PredefinedItem]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"{}\" version=\"2.20\">\r\n",
        escape_xml_text(xsi_type)
    );
    for item in items {
        push_predefined_item_xml(&mut xml, item, 1);
    }
    xml.push_str("</PredefinedData>\r\n");
    xml
}

fn push_predefined_item_xml(xml: &mut String, item: &PredefinedItem, indent: usize) {
    let tab = "\t".repeat(indent);
    xml.push_str(&format!(
        "{tab}<Item id=\"{}\">\r\n\
{tab}\t<Name>{}</Name>\r\n\
{tab}\t<Code>{}</Code>\r\n\
{tab}\t<Description>{}</Description>\r\n\
{}\
{tab}\t<IsFolder>{}</IsFolder>\r\n",
        escape_xml_text(&item.id),
        escape_xml_text(&item.name),
        escape_xml_text(&item.code),
        escape_xml_text(&item.description),
        format_predefined_type_xml(&item.value_types, indent + 1),
        xml_bool(item.is_folder),
    ));
    if !item.children.is_empty() {
        xml.push_str(&format!("{tab}\t<ChildItems>\r\n"));
        for child in &item.children {
            push_predefined_item_xml(xml, child, indent + 2);
        }
        xml.push_str(&format!("{tab}\t</ChildItems>\r\n"));
    }
    xml.push_str(&format!("{tab}</Item>\r\n"));
}

fn format_predefined_type_xml(value_types: &[ConstantValueType], indent: usize) -> String {
    if value_types.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<Type>\r\n");
    for value_type in value_types {
        match value_type {
            ConstantValueType::Reference { reference } if reference.starts_with("cfg:") => {
                xml.push_str(&format!(
                    "{tab}\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d4p1:{}</v8:Type>\r\n",
                    escape_xml_text(reference.trim_start_matches("cfg:"))
                ));
            }
            _ => {
                xml.push_str(&format!(
                    "{tab}\t<v8:Type>{}</v8:Type>\r\n",
                    metadata_type_xml_name(value_type)
                ));
            }
        }
    }
    if let Some((length, allowed_length_flag)) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::String {
            length: Some(length),
            allowed_length_flag,
        } = value_type
        {
            Some((*length, *allowed_length_flag))
        } else {
            None
        }
    }) {
        xml.push_str(&format!("{tab}\t<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{tab}\t\t<v8:Length>{length}</v8:Length>\r\n"));
        xml.push_str(&format!(
            "{tab}\t\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            predefined_string_allowed_length_xml(allowed_length_flag)
        ));
        xml.push_str(&format!("{tab}\t</v8:StringQualifiers>\r\n"));
    }
    xml.push_str(&format!("{tab}</Type>\r\n"));
    xml
}

fn predefined_string_allowed_length_xml(value: u8) -> &'static str {
    match value {
        0 => "Fixed",
        _ => "Variable",
    }
}

fn format_command_interface_xml(entries: &[CommandInterfaceEntry]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<CommandInterface xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
\t<CommandsVisibility>\r\n",
    );
    for entry in entries {
        xml.push_str(&format!(
            "\t\t<Command name=\"{}\">\r\n\
\t\t\t<Visibility>\r\n\
\t\t\t\t<xr:Common>{}</xr:Common>\r\n\
\t\t\t</Visibility>\r\n\
\t\t</Command>\r\n",
            escape_xml_text(&entry.name),
            xml_bool(entry.common)
        ));
    }
    xml.push_str("\t</CommandsVisibility>\r\n</CommandInterface>\r\n");
    xml
}

fn format_exchange_plan_content_xml(items: &[ExchangePlanContentItem]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
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
    xml.push_str("</ExchangePlanContent>\r\n");
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
    xml.push_str("\t\t\t\t<ZOrder>0</ZOrder>\r\n");
    xml.push_str("\t\t\t\t<Hyperlink>false</Hyperlink>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<Transparent>{}</Transparent>\r\n",
        xml_bool(item.properties.transparent)
    ));
    xml.push_str("\t\t\t\t<Font xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" ref=\"sys:DefaultGUIFont\" kind=\"WindowsFont\"/>\r\n");
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
    if matches!(item.tag, "Start" | "Activity" | "Condition" | "Completion") {
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

fn format_config_dump_info_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" format=\"Hierarchical\" version=\"2.20\">\r\n\
\t<ConfigVersions/>\r\n\
</ConfigDumpInfo>\r\n"
        .to_string()
}

fn format_role_rights_xml(rights: &RoleRights) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"Rights\" version=\"2.20\">\r\n\
\t<setForNewObjects>{}</setForNewObjects>\r\n\
\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n\
\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        xml_bool(rights.set_for_new_objects)
    );
    for object in &rights.objects {
        xml.push_str("\t<object>\r\n\t\t<name>");
        xml.push_str(&escape_xml_element_text(&object.name));
        xml.push_str("</name>\r\n");
        for right in &object.rights {
            xml.push_str("\t\t<right>\r\n\t\t\t<name>");
            xml.push_str(&escape_xml_element_text(&right.name));
            xml.push_str("</name>\r\n\t\t\t<value>");
            xml.push_str(xml_bool(right.value));
            xml.push_str("</value>\r\n");
            if let Some(restriction) = &right.restriction_by_condition {
                xml.push_str("\t\t\t<restrictionByCondition>\r\n");
                if let Some(field) = &restriction.field {
                    xml.push_str("\t\t\t\t<field>");
                    xml.push_str(&escape_xml_element_text(field));
                    xml.push_str("</field>\r\n");
                }
                xml.push_str("\t\t\t\t<condition>");
                xml.push_str(&escape_xml_element_text(&restriction.condition));
                xml.push_str("</condition>\r\n\t\t\t</restrictionByCondition>\r\n");
            }
            xml.push_str("\t\t</right>\r\n");
        }
        xml.push_str("\t</object>\r\n");
    }
    for template in &rights.restriction_templates {
        xml.push_str("\t<restrictionTemplate>\r\n\t\t<name>");
        xml.push_str(&escape_xml_element_text(&template.name));
        xml.push_str("</name>\r\n\t\t<condition>");
        xml.push_str(&escape_xml_element_text(&template.condition));
        xml.push_str("</condition>\r\n\t</restrictionTemplate>\r\n");
    }
    xml.push_str("</Rights>\r\n");
    xml
}

struct JobSchedule {
    begin_date: String,
    end_date: String,
    begin_time: String,
    end_time: String,
    completion_time: String,
    completion_interval: String,
    repeat_period_in_day: String,
    repeat_pause: String,
    week_day_in_month: String,
    day_in_month: String,
    week_days: Vec<String>,
    months: Vec<String>,
    weeks_period: String,
    days_repeat_period: String,
}

fn extract_schedule_xml(bytes: &[u8]) -> Result<String> {
    let inflated = inflate_raw_deflate(bytes)?;
    let text = String::from_utf8(inflated).context("schedule blob is not UTF-8")?;
    let schedule = parse_job_schedule_text(text.trim_start_matches('\u{feff}'))
        .context("failed to parse compact schedule")?;
    Ok(format_job_schedule_xml(&schedule))
}

fn parse_job_schedule_text(text: &str) -> Option<JobSchedule> {
    let fields = split_1c_braced_fields(text, 0)?;
    let mut index = 0usize;
    let begin_date = format_1c_date(fields.get(index)?.trim())?;
    index += 1;
    let end_date = format_1c_date(fields.get(index)?.trim())?;
    index += 1;
    let begin_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let end_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let completion_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let completion_interval = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let repeat_period_in_day = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let repeat_pause = parse_schedule_number(fields.get(index)?)?;
    index += 1;

    let week_days_count = fields.get(index)?.trim().parse::<usize>().ok()?;
    index += 1;
    let week_days = parse_schedule_number_list(&fields, &mut index, week_days_count)?;

    let week_day_in_month = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let day_in_month = parse_schedule_number(fields.get(index)?)?;
    index += 1;

    let months_count = fields.get(index)?.trim().parse::<usize>().ok()?;
    index += 1;
    let months = parse_schedule_number_list(&fields, &mut index, months_count)?;

    let weeks_period = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let days_repeat_period = parse_schedule_number(fields.get(index)?)?;

    Some(JobSchedule {
        begin_date,
        end_date,
        begin_time,
        end_time,
        completion_time,
        completion_interval,
        repeat_period_in_day,
        repeat_pause,
        week_day_in_month,
        day_in_month,
        week_days,
        months,
        weeks_period,
        days_repeat_period,
    })
}

fn parse_schedule_number_list(
    fields: &[&str],
    index: &mut usize,
    count: usize,
) -> Option<Vec<String>> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(parse_schedule_number(fields.get(*index)?)?);
        *index += 1;
    }
    Some(values)
}

fn parse_schedule_number(value: &str) -> Option<String> {
    let value = value.trim();
    if value.chars().all(|ch| ch.is_ascii_digit()) {
        Some(value.to_string())
    } else {
        None
    }
}

fn format_1c_date(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &value[0..4],
        &value[4..6],
        &value[6..8]
    ))
}

fn format_1c_time(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}:{}:{}",
        &value[8..10],
        &value[10..12],
        &value[12..14]
    ))
}

fn format_job_schedule_xml(schedule: &JobSchedule) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<JobSchedule xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.17\">\r\n\
\t<Schedule BeginDate=\"{}\" EndDate=\"{}\" BeginTime=\"{}\" EndTime=\"{}\" CompletionTime=\"{}\" CompletionInterval=\"{}\" RepeatPeriodInDay=\"{}\" RepeatPause=\"{}\" WeekDayInMonth=\"{}\" DayInMonth=\"{}\" WeeksPeriod=\"{}\" DaysRepeatPeriod=\"{}\">\r\n\
\t\t<ent:WeekDays>{}</ent:WeekDays>\r\n\
\t\t<ent:Months>{}</ent:Months>\r\n\
\t</Schedule>\r\n\
</JobSchedule>\r\n",
        schedule.begin_date,
        schedule.end_date,
        schedule.begin_time,
        schedule.end_time,
        schedule.completion_time,
        schedule.completion_interval,
        schedule.repeat_period_in_day,
        schedule.repeat_pause,
        schedule.week_day_in_month,
        schedule.day_in_month,
        schedule.weeks_period,
        schedule.days_repeat_period,
        schedule.week_days.join(" "),
        schedule.months.join(" ")
    )
}

fn extract_form_body_xml(bytes: &[u8], object_refs: &BTreeMap<String, String>) -> Option<String> {
    let body = parse_form_body_blob(bytes).ok()?;
    let form_fields = split_1c_braced_fields(&body.layout, 0)?;
    let properties = extract_form_body_properties(&form_fields);
    let events = extract_form_body_events(&form_fields);
    let auto_command_bar = extract_form_auto_command_bar(&form_fields);
    let attributes = extract_form_body_attributes(&body.trailing, object_refs);
    let parameters = extract_form_body_parameters(&body.trailing, object_refs);
    let commands = extract_form_body_commands(&body.trailing, object_refs);
    let child_items = extract_form_child_items(&form_fields, &attributes, &commands, object_refs);
    let command_interface = extract_form_command_interface(&body.trailing, object_refs);

    Some(format_form_body_xml(
        &properties,
        auto_command_bar.as_ref(),
        &events,
        &child_items,
        &attributes,
        &parameters,
        &commands,
        &command_interface,
    ))
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct FormBodyProperties {
    title: Vec<(String, String)>,
    width: Option<String>,
    height: Option<String>,
    window_opening_mode: Option<&'static str>,
    auto_title: Option<bool>,
    group: Option<&'static str>,
    command_bar_location: Option<&'static str>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct FormBodyEvent {
    name: String,
    handler: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormAutoCommandBar {
    id: String,
    name: String,
    horizontal_align: Option<&'static str>,
    autofill: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormAttribute {
    id: String,
    name: String,
    main_attribute: bool,
    use_always: Vec<String>,
    settings: Option<FormDynamicListSettings>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormParameter {
    name: String,
    value_types: Vec<ConstantValueType>,
    key_parameter: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormDynamicListSettings {
    manual_query: bool,
    dynamic_data_read: bool,
    query_text: Option<String>,
    main_table: Option<String>,
    list_settings: FormListSettings,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct FormListSettings {
    filter: Option<FormListSettingsStandardSection>,
    order: Option<FormListSettingsOrder>,
    conditional_appearance: Option<FormListSettingsStandardSection>,
    items_view_mode: Option<String>,
    items_user_setting_id: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct FormListSettingsStandardSection {
    view_mode: Option<String>,
    user_setting_id: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct FormListSettingsOrder {
    items: Vec<FormListSettingsOrderItem>,
    view_mode: Option<String>,
    user_setting_id: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormListSettingsOrderItem {
    field: String,
    order_type: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormCommand {
    id: String,
    reference_uuid: String,
    name: String,
    title: Vec<(String, String)>,
    tooltip: Vec<(String, String)>,
    action: String,
    functional_options: Vec<String>,
    current_row_use: Option<&'static str>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormCommandInterface {
    navigation_panel: Vec<FormCommandInterfaceItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormCommandInterfaceItem {
    command: String,
    item_type: &'static str,
    command_group: String,
    index: Option<usize>,
    default_visible: Option<bool>,
    visible_common: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FormChildItem {
    tag: &'static str,
    id: String,
    name: String,
    group: Option<&'static str>,
    item_type: Option<&'static str>,
    addition_source_item: Option<String>,
    title: Vec<(String, String)>,
    events: Vec<FormBodyEvent>,
    data_path: Option<String>,
    command_name: Option<String>,
    child_items: Vec<FormChildItem>,
}

fn extract_form_body_properties(fields: &[&str]) -> FormBodyProperties {
    FormBodyProperties {
        title: fields
            .get(10)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        width: extract_form_dimension(fields, 3),
        height: extract_form_dimension(fields, 4),
        window_opening_mode: extract_form_window_opening_mode(fields),
        auto_title: extract_form_auto_title(fields),
        group: extract_form_root_group(fields),
        command_bar_location: extract_form_command_bar_location(fields),
    }
}

fn extract_form_dimension(fields: &[&str], index: usize) -> Option<String> {
    let value = fields.get(index)?.trim();
    if value == "0" || value.parse::<u32>().is_err() {
        return None;
    }
    Some(value.to_string())
}

fn extract_form_window_opening_mode(fields: &[&str]) -> Option<&'static str> {
    match fields.get(2).map(|field| field.trim())? {
        "0" => Some("DontBlock"),
        "1" => Some("LockOwner"),
        "2" => Some("LockWholeInterface"),
        _ => None,
    }
}

fn extract_form_auto_title(fields: &[&str]) -> Option<bool> {
    match fields.get(9).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

fn extract_form_root_group(fields: &[&str]) -> Option<&'static str> {
    match (
        fields.get(11).map(|field| field.trim())?,
        fields.get(13).map(|field| field.trim()),
        fields.get(14).map(|field| field.trim()),
    ) {
        ("0", _, _) => Some("Vertical"),
        ("1", Some("0"), Some("0")) => Some("Horizontal"),
        _ => None,
    }
}

fn extract_form_command_bar_location(fields: &[&str]) -> Option<&'static str> {
    match fields.get(17).map(|field| field.trim())? {
        "0" => Some("None"),
        "2" => Some("Top"),
        "3" => Some("Bottom"),
        _ => None,
    }
}

fn extract_form_auto_command_bar(fields: &[&str]) -> Option<FormAutoCommandBar> {
    find_form_auto_command_bar(fields)
}

fn find_form_auto_command_bar(fields: &[&str]) -> Option<FormAutoCommandBar> {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if let Some(command_bar) = parse_form_auto_command_bar_fields(&nested) {
            return Some(command_bar);
        }
        if let Some(command_bar) = find_form_auto_command_bar(&nested) {
            return Some(command_bar);
        }
    }
    None
}

fn parse_form_auto_command_bar_fields(fields: &[&str]) -> Option<FormAutoCommandBar> {
    if fields.first().map(|value| value.trim()) != Some("22") {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id != "-1" {
        return None;
    }
    let (name, _) = parse_1c_quoted_string_with_len(fields.get(6)?.trim())?;
    if name.trim().is_empty() {
        return None;
    }
    Some(FormAutoCommandBar {
        id: id.to_string(),
        name,
        horizontal_align: fields
            .get(20)
            .and_then(|field| parse_form_auto_command_bar_horizontal_align(field)),
        autofill: fields
            .get(20)
            .and_then(|field| parse_form_auto_command_bar_autofill(field)),
    })
}

fn parse_form_auto_command_bar_horizontal_align(field: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(1).map(|value| value.trim())? {
        "1" => Some("Center"),
        "2" => Some("Right"),
        "3" => Some("Auto"),
        _ => None,
    }
}

fn parse_form_auto_command_bar_autofill(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(2).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn extract_form_body_events(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();
    collect_form_body_events(fields, &mut events, &mut seen);
    events
}

fn collect_form_body_events(
    fields: &[&str],
    events: &mut Vec<FormBodyEvent>,
    seen: &mut BTreeSet<(String, String)>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if is_form_child_item_fields(&nested) {
            continue;
        }
        for event in parse_form_body_event_fields(&nested) {
            if seen.insert((event.name.clone(), event.handler.clone())) {
                events.push(event);
            }
        }
        collect_form_body_events(&nested, events, seen);
    }
}

fn is_form_child_item_fields(fields: &[&str]) -> bool {
    let Some(wrapper) = fields.first().map(|value| value.trim()) else {
        return false;
    };
    form_child_item_tag(wrapper, fields).is_some()
}

fn parse_form_body_event_fields(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    for window in fields.windows(2) {
        if let Some(event) = parse_form_body_event_pair(window[0], window[1]) {
            events.push(event);
        }
    }
    events
}

fn parse_form_body_event_pair(event_field: &str, handler_field: &str) -> Option<FormBodyEvent> {
    let event = parse_form_event_identifier(event_field)?;
    let (handler, _) = parse_1c_quoted_string_with_len(handler_field.trim())?;
    let handler = handler.trim();
    if handler.is_empty() || !is_probable_form_event_handler(handler) {
        return None;
    }
    Some(FormBodyEvent {
        name: event,
        handler: handler.to_string(),
    })
}

fn parse_form_event_identifier(field: &str) -> Option<String> {
    let field = field.trim();
    let identifier = parse_1c_quoted_string_with_len(field)
        .map(|(value, _)| value)
        .unwrap_or_else(|| field.to_string());
    let identifier = identifier.trim();
    form_event_name_from_identifier(identifier)
        .map(ToOwned::to_owned)
        .or_else(|| is_uuid_text(identifier).then(|| identifier.to_string()))
}

fn form_event_name_from_identifier(identifier: &str) -> Option<&'static str> {
    match identifier {
        "OnOpen" => Some("OnOpen"),
        "BeforeClose" => Some("BeforeClose"),
        "OnClose" => Some("OnClose"),
        "OnCreateAtServer" => Some("OnCreateAtServer"),
        "OnReadAtServer" => Some("OnReadAtServer"),
        "AfterWrite" => Some("AfterWrite"),
        "BeforeWrite" => Some("BeforeWrite"),
        "BeforeWriteAtServer" => Some("BeforeWriteAtServer"),
        "AfterWriteAtServer" => Some("AfterWriteAtServer"),
        "OnWriteAtServer" => Some("OnWriteAtServer"),
        "OnLoadDataFromSettingsAtServer" => Some("OnLoadDataFromSettingsAtServer"),
        "BeforeLoadDataFromSettingsAtServer" => Some("BeforeLoadDataFromSettingsAtServer"),
        "OnSaveDataInSettingsAtServer" => Some("OnSaveDataInSettingsAtServer"),
        "BeforeLoadUserSettingsAtServer" => Some("BeforeLoadUserSettingsAtServer"),
        "OnLoadUserSettingsAtServer" => Some("OnLoadUserSettingsAtServer"),
        "OnSaveUserSettingsAtServer" => Some("OnSaveUserSettingsAtServer"),
        "BeforeLoadVariantAtServer" => Some("BeforeLoadVariantAtServer"),
        "OnLoadVariantAtServer" => Some("OnLoadVariantAtServer"),
        "OnSaveVariantAtServer" => Some("OnSaveVariantAtServer"),
        "OnUpdateUserSettingSetAtServer" => Some("OnUpdateUserSettingSetAtServer"),
        "FillCheckProcessingAtServer" => Some("FillCheckProcessingAtServer"),
        "ChoiceProcessing" => Some("ChoiceProcessing"),
        "NotificationProcessing" => Some("NotificationProcessing"),
        "ExternalEvent" => Some("ExternalEvent"),
        "Opening" => Some("Opening"),
        "OnReopen" => Some("OnReopen"),
        "OnActivate" => Some("OnActivate"),
        "OnMainServerAvailabilityChange" => Some("OnMainServerAvailabilityChange"),
        "3ccc650e-f631-4cae-8e33-3eaac610b5f9" => Some("OnOpen"),
        "1d632984-de3c-4b4b-ad9f-d69682a10182" => Some("ChoiceProcessing"),
        "3699f6a3-9a2a-4c82-a775-6ff4824a08ca" => Some("NotificationProcessing"),
        "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b" => Some("OnCreateAtServer"),
        _ => None,
    }
}

fn is_probable_form_event_handler(value: &str) -> bool {
    if value.len() > 512 || value.chars().any(char::is_whitespace) {
        return false;
    }
    value.chars().all(|ch| {
        ch == '_' || ch.is_alphanumeric() || ('А'..='я').contains(&ch) || ch == 'ё' || ch == 'Ё'
    })
}

fn extract_form_item_assets(bytes: &[u8]) -> Vec<FormItemAsset> {
    let Ok(inflated) = inflate_raw_deflate(bytes) else {
        return Vec::new();
    };
    let Ok(text) = String::from_utf8(inflated) else {
        return Vec::new();
    };
    let text = text.trim_start_matches('\u{feff}');
    if !split_1c_braced_fields(text, 0)
        .and_then(|fields| fields.first().map(|value| value.trim() == "4"))
        .unwrap_or(false)
    {
        return Vec::new();
    }

    let mut assets = Vec::new();
    let mut occurrences_by_item = BTreeMap::<String, usize>::new();
    let mut offset = 0usize;
    let prefix = "{#base64:";
    while let Some(relative_start) = text[offset..].find(prefix) {
        let marker_start = offset + relative_start;
        let payload_start = marker_start + prefix.len();
        let Some(relative_end) = text[payload_start..].find('}') else {
            break;
        };
        let payload_end = payload_start + relative_end;
        if let Some(content) = decode_base64_mime(&text[payload_start..payload_end])
            && is_form_item_picture_content(&content)
            && let Some(item_name) = nearest_form_item_name(text, marker_start)
        {
            let occurrence = occurrences_by_item.entry(item_name.clone()).or_insert(0);
            let file_name = form_item_picture_file_name(&item_name, &content, *occurrence);
            *occurrence += 1;
            assets.push(FormItemAsset {
                item_name,
                file_name,
                content,
            });
        }
        offset = payload_end + 1;
    }

    dedup_form_item_assets(assets)
}

fn is_form_item_picture_content(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"\x00\x00\x01\x00")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"BM")
        || is_svg_content(bytes)
}

fn nearest_form_item_name(text: &str, marker_start: usize) -> Option<String> {
    nearest_form_item_name_in_window(text, marker_start, 4096)
        .or_else(|| nearest_form_item_name_in_window(text, marker_start, 12_288))
}

fn nearest_form_item_name_in_window(
    text: &str,
    marker_start: usize,
    window_size: usize,
) -> Option<String> {
    let mut window_start = marker_start.saturating_sub(window_size);
    while window_start > 0 && !text.is_char_boundary(window_start) {
        window_start -= 1;
    }
    let window = &text[window_start..marker_start];
    let mut candidates = Vec::<String>::new();
    let mut offset = 0usize;
    while let Some(relative_quote) = window[offset..].find('"') {
        let quote_start = offset + relative_quote;
        let content_start = quote_start + 1;
        let Some(relative_end) = window[content_start..].find('"') else {
            break;
        };
        let quote_end = content_start + relative_end;
        let value = &window[content_start..quote_end];
        let before = window[..quote_start].trim_end().chars().last();
        let after = window[quote_end + 1..].trim_start().chars().next();
        if before == Some(',') && after == Some(',') && is_probable_form_item_name(value) {
            candidates.push(value.replace("\"\"", "\""));
        }
        offset = quote_end + 1;
    }
    candidates.pop()
}

fn is_probable_form_item_name(value: &str) -> bool {
    if value.len() < 3
        || matches!(
            value,
            "Pattern" | "DataParameters" | "Settings" | "Use" | "ru"
        )
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    value.chars().all(|ch| {
        ch == '_' || ch.is_alphanumeric() || ('А'..='я').contains(&ch) || ch == 'ё' || ch == 'Ё'
    })
}

fn form_item_picture_file_name(item_name: &str, content: &[u8], occurrence: usize) -> String {
    let property_name = if item_name.contains("ИндексКартинки") {
        if occurrence == 0 {
            "HeaderPicture"
        } else {
            "ValuesPicture"
        }
    } else if item_name.contains("Авторегистрация") || item_name.ends_with("Пиктограмма")
    {
        "ValuesPicture"
    } else if (item_name.starts_with("Дерево") || item_name.starts_with("Список"))
        && !item_name.contains("КонтекстноеМеню")
        && !item_name.contains("Добавить")
        && !item_name.contains("Удалить")
        && !item_name.contains("Показать")
    {
        "RowsPicture"
    } else {
        "Picture"
    };
    let extension = ext_picture_file_name(content)
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("bin");
    format!("{property_name}.{extension}")
}

fn extract_form_body_attributes(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormAttribute> {
    let Some(fields) = trailing
        .first()
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    if fields.first().map(|field| field.trim()) != Some("4") {
        return Vec::new();
    }
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_form_attribute(field, object_refs))
        .collect()
}

fn parse_form_attribute(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAttribute> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("9") {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = parse_1c_quoted_string_with_len(fields.get(3)?.trim())?.0;
    if name.is_empty() {
        return None;
    }
    let main_attribute = fields.get(10).map(|value| value.trim()) == Some("1");
    let settings = fields
        .get(14)
        .and_then(|field| parse_form_dynamic_list_settings(field, object_refs));
    let use_always = settings
        .as_ref()
        .map(|settings| form_dynamic_list_use_always(&name, settings))
        .unwrap_or_default();
    Some(FormAttribute {
        id: id.to_string(),
        name,
        main_attribute,
        use_always,
        settings,
    })
}

fn parse_form_dynamic_list_settings(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormDynamicListSettings> {
    let settings_fields = split_1c_braced_fields(field.trim(), 0)?;
    let mut manual_query = false;
    let mut dynamic_data_read = false;
    let mut query_text = None;
    let mut main_table = None;
    let mut list_settings = FormListSettings::default();
    for window in settings_fields.windows(2) {
        let key = parse_1c_quoted_string_with_len(window[0].trim())
            .map(|(value, _)| value)
            .unwrap_or_default();
        match key.as_str() {
            "QueryText" => query_text = parse_form_setting_string(window[1]),
            "MainTable" => main_table = parse_form_main_table_ref(window[1], object_refs),
            "ManualQuery" => manual_query = parse_form_setting_bool(window[1]).unwrap_or(false),
            "DynamicalDataSelection" => {
                dynamic_data_read = !parse_form_setting_bool(window[1]).unwrap_or(true)
            }
            "Filter" => {
                list_settings.filter =
                    parse_form_list_settings_standard_section(window[1], "Filter")
            }
            "Order" => list_settings.order = parse_form_list_settings_order(window[1]),
            "ConditionalAppearance" => {
                list_settings.conditional_appearance =
                    parse_form_list_settings_standard_section(window[1], "ConditionalAppearance")
            }
            "ItemsViewMode" => list_settings.items_view_mode = parse_form_setting_string(window[1]),
            "ItemsUserSettingID" => {
                list_settings.items_user_setting_id = parse_form_setting_string(window[1])
            }
            _ => {}
        }
    }
    if query_text.is_none()
        && main_table.is_none()
        && !manual_query
        && !dynamic_data_read
        && list_settings.filter.is_none()
        && list_settings.order.is_none()
        && list_settings.conditional_appearance.is_none()
        && list_settings.items_view_mode.is_none()
        && list_settings.items_user_setting_id.is_none()
    {
        return None;
    }
    Some(FormDynamicListSettings {
        manual_query,
        dynamic_data_read,
        query_text,
        main_table,
        list_settings,
    })
}

fn form_dynamic_list_use_always(
    attribute_name: &str,
    settings: &FormDynamicListSettings,
) -> Vec<String> {
    let mut fields = Vec::new();
    if let Some(query_text) = &settings.query_text {
        for field in ["Наименование", "Ссылка"] {
            if query_text.contains(field) {
                fields.push(format!("{attribute_name}.{field}"));
            }
        }
    }
    fields
}

fn parse_form_setting_string(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"S\"") {
        return None;
    }
    parse_1c_quoted_string_with_len(fields.get(1)?.trim()).map(|(value, _)| value)
}

fn parse_form_setting_bool(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"B\"") {
        return None;
    }
    match fields.get(1).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn xml_local_name(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit_once(':')
        .map(|(_, local)| local)
        .unwrap_or(name)
        .to_string()
}

fn path_ends_with(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

fn parse_form_list_settings_order(field: &str) -> Option<FormListSettingsOrder> {
    let payload = extract_base64_payload(field)?;
    let xml = decode_base64_mime(payload)?;
    let xml = String::from_utf8(xml).ok()?;
    parse_form_list_settings_order_xml(&xml)
}

fn parse_form_list_settings_standard_section(
    field: &str,
    root_name: &str,
) -> Option<FormListSettingsStandardSection> {
    let payload = extract_base64_payload(field)?;
    let xml = decode_base64_mime(payload)?;
    let xml = String::from_utf8(xml).ok()?;
    parse_form_list_settings_standard_section_xml(&xml, root_name)
}

fn parse_form_list_settings_standard_section_xml(
    xml: &str,
    root_name: &str,
) -> Option<FormListSettingsStandardSection> {
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = String::new();
    let mut section = FormListSettingsStandardSection::default();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if matches!(local.as_str(), "viewMode" | "userSettingID") {
                    text.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(value)) => {
                if path_ends_with(&path, &[root_name, "viewMode"])
                    || path_ends_with(&path, &[root_name, "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::CData(value)) => {
                if path_ends_with(&path, &[root_name, "viewMode"])
                    || path_ends_with(&path, &[root_name, "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "viewMode" if path_ends_with(&path, &[root_name, "viewMode"]) => {
                        section.view_mode = Some(text.trim().to_string());
                    }
                    "userSettingID" if path_ends_with(&path, &[root_name, "userSettingID"]) => {
                        section.user_setting_id = Some(text.trim().to_string());
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(local.as_str(), "viewMode" | "userSettingID") {
                    text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
        buffer.clear();
    }

    (section.view_mode.is_some() || section.user_setting_id.is_some()).then_some(section)
}

fn parse_form_list_settings_order_xml(xml: &str) -> Option<FormListSettingsOrder> {
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = String::new();
    let mut order = FormListSettingsOrder::default();
    let mut current_item = None::<FormListSettingsOrderItem>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if matches!(
                    local.as_str(),
                    "field" | "orderType" | "viewMode" | "userSettingID"
                ) {
                    text.clear();
                }
                if local == "item" && path.last().map(String::as_str) == Some("Order") {
                    current_item = Some(FormListSettingsOrderItem {
                        field: String::new(),
                        order_type: None,
                    });
                }
                path.push(local);
            }
            Ok(Event::Text(value)) => {
                if path_ends_with(&path, &["Order", "item", "field"])
                    || path_ends_with(&path, &["Order", "item", "orderType"])
                    || path_ends_with(&path, &["Order", "viewMode"])
                    || path_ends_with(&path, &["Order", "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::CData(value)) => {
                if path_ends_with(&path, &["Order", "item", "field"])
                    || path_ends_with(&path, &["Order", "item", "orderType"])
                    || path_ends_with(&path, &["Order", "viewMode"])
                    || path_ends_with(&path, &["Order", "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "field" if path_ends_with(&path, &["Order", "item", "field"]) => {
                        if let Some(item) = current_item.as_mut() {
                            item.field = text.trim().to_string();
                        }
                    }
                    "orderType" if path_ends_with(&path, &["Order", "item", "orderType"]) => {
                        if let Some(item) = current_item.as_mut() {
                            item.order_type = Some(text.trim().to_string());
                        }
                    }
                    "item" if path_ends_with(&path, &["Order", "item"]) => {
                        if let Some(item) = current_item.take()
                            && !item.field.is_empty()
                        {
                            order.items.push(item);
                        }
                    }
                    "viewMode" if path_ends_with(&path, &["Order", "viewMode"]) => {
                        order.view_mode = Some(text.trim().to_string());
                    }
                    "userSettingID" if path_ends_with(&path, &["Order", "userSettingID"]) => {
                        order.user_setting_id = Some(text.trim().to_string());
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(
                    local.as_str(),
                    "field" | "orderType" | "viewMode" | "userSettingID"
                ) {
                    text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
        buffer.clear();
    }

    (!order.items.is_empty() || order.view_mode.is_some() || order.user_setting_id.is_some())
        .then_some(order)
}

fn parse_form_main_table_ref(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"#\"") {
        return None;
    }
    fields.iter().skip(1).find_map(|value| {
        parse_non_zero_uuid(value).and_then(|uuid| object_refs.get(&uuid).cloned())
    })
}

fn extract_form_body_parameters(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormParameter> {
    let Some(fields) = trailing
        .get(1)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_form_parameter(field, object_refs))
        .collect()
}

fn parse_form_parameter(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormParameter> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let name = parse_1c_quoted_string_with_len(fields.get(1)?.trim())?.0;
    if name.trim().is_empty() {
        return None;
    }
    let value_types = fields
        .get(2)
        .and_then(|field| parse_metadata_type_pattern(field, object_refs))?;
    let key_parameter = match fields.get(3).map(|field| field.trim()) {
        Some("1") => true,
        Some("0") | None => false,
        _ => return None,
    };
    Some(FormParameter {
        name,
        value_types,
        key_parameter,
    })
}

fn extract_form_body_commands(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormCommand> {
    let Some(fields) = trailing
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_form_command(field, object_refs))
        .collect()
}

fn parse_form_command(field: &str, object_refs: &BTreeMap<String, String>) -> Option<FormCommand> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("11") {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    let reference_uuid = identity
        .get(1)
        .and_then(|value| parse_non_zero_uuid(value.trim()))?;
    let name = parse_1c_quoted_string_with_len(fields.get(2)?.trim())?.0;
    let action = parse_1c_quoted_string_with_len(fields.get(8)?.trim())?.0;
    if id.is_empty() || name.is_empty() || action.is_empty() {
        return None;
    }
    Some(FormCommand {
        id: id.to_string(),
        reference_uuid,
        name,
        title: fields
            .get(3)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        tooltip: fields
            .get(4)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        action,
        functional_options: fields
            .get(12)
            .map(|field| parse_form_reference_list(field, object_refs))
            .unwrap_or_default(),
        current_row_use: parse_form_current_row_use(fields.get(9).copied()),
    })
}

fn parse_form_localized_strings(field: &str) -> Vec<(String, String)> {
    parse_1c_synonyms(field)
}

fn parse_form_reference_list(field: &str, object_refs: &BTreeMap<String, String>) -> Vec<String> {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return Vec::new();
    };
    fields
        .iter()
        .filter_map(|value| {
            parse_non_zero_uuid(value).and_then(|uuid| object_refs.get(&uuid).cloned())
        })
        .collect()
}

fn parse_form_current_row_use(field: Option<&str>) -> Option<&'static str> {
    match field.map(str::trim)? {
        "3" => Some("DontUse"),
        _ => None,
    }
}

fn extract_form_child_items(
    fields: &[&str],
    attributes: &[FormAttribute],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormChildItem> {
    let main_data_path = attributes
        .iter()
        .find(|attribute| attribute.main_attribute)
        .or_else(|| attributes.first())
        .map(|attribute| attribute.name.as_str());
    let table_name_by_id = form_table_names_by_id(fields);
    let table_column_names_by_id = form_table_column_names_by_id(fields);
    parse_form_child_item_pairs(
        fields,
        main_data_path,
        None,
        &table_name_by_id,
        &table_column_names_by_id,
        commands,
        object_refs,
    )
    .unwrap_or_default()
}

fn parse_form_child_item_pairs(
    fields: &[&str],
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<FormChildItem>> {
    let mut best = Vec::new();
    for index in 0..fields.len() {
        let Some(count) = parse_form_child_item_count(fields[index]) else {
            continue;
        };
        let mut items = Vec::new();
        let mut cursor = index + 1;
        let mut complete = true;
        for _ in 0..count {
            let Some(field) = fields.get(cursor + 1) else {
                complete = false;
                break;
            };
            let Some(item) = parse_form_child_item(
                field,
                main_data_path,
                parent_data_path,
                table_name_by_id,
                table_column_names_by_id,
                commands,
                object_refs,
            ) else {
                complete = false;
                break;
            };
            items.push(item);
            cursor += 2;
        }
        if complete && items.len() > best.len() {
            best = items;
        }
    }
    if best.is_empty() { None } else { Some(best) }
}

fn parse_form_child_item_count(value: &str) -> Option<usize> {
    let count = value.trim().parse::<usize>().ok()?;
    (1..=200).contains(&count).then_some(count)
}

fn parse_form_child_item(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let wrapper = fields.first()?.trim();
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id == "0" {
        return None;
    }
    let tag = form_child_item_tag(wrapper, &fields)?;
    let name = parse_form_child_item_name(wrapper, &fields)?;
    let data_path = parse_form_child_item_data_path(
        tag,
        &fields,
        &name,
        id,
        main_data_path,
        parent_data_path,
        table_name_by_id,
        table_column_names_by_id,
    );
    let child_parent_data_path = data_path.as_deref().or(parent_data_path);
    let child_items = parse_form_child_item_pairs(
        &fields,
        main_data_path,
        child_parent_data_path,
        table_name_by_id,
        table_column_names_by_id,
        commands,
        object_refs,
    )
    .unwrap_or_default();
    Some(FormChildItem {
        tag,
        id: id.to_string(),
        name,
        group: (tag == "UsualGroup").then_some("Vertical"),
        item_type: if tag == "Button" {
            fields
                .get(7)
                .and_then(|field| parse_form_button_type(field))
        } else if tag.ends_with("Addition") {
            fields
                .get(5)
                .and_then(|field| parse_form_search_addition_type(field))
        } else {
            None
        },
        addition_source_item: if tag.ends_with("Addition") {
            fields
                .get(19)
                .and_then(|field| parse_form_search_addition_source_item(field, table_name_by_id))
        } else {
            None
        },
        title: parse_form_child_item_title(wrapper, &fields),
        events: parse_form_child_item_event_fields(&fields),
        data_path,
        command_name: if tag == "Button" {
            fields
                .get(8)
                .and_then(|field| parse_form_button_command_name(field, commands, object_refs))
        } else {
            None
        },
        child_items,
    })
}

fn parse_form_button_type(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("UsualButton"),
        "1" => Some("CommandBarButton"),
        "2" => Some("Hyperlink"),
        _ => None,
    }
}

fn parse_form_search_addition_type(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("SearchStringRepresentation"),
        "1" => Some("ViewStatusRepresentation"),
        "2" => Some("SearchControl"),
        _ => None,
    }
}

fn parse_form_search_addition_source_item(
    field: &str,
    table_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let table_id = fields.first()?.trim();
    table_name_by_id.get(table_id).cloned()
}

fn form_child_item_tag(wrapper: &str, fields: &[&str]) -> Option<&'static str> {
    match wrapper {
        "22" => match fields.get(5).map(|value| value.trim())? {
            "0" => Some("CommandBar"),
            "1" => Some("Popup"),
            "5" => Some("UsualGroup"),
            "6" => Some("ButtonGroup"),
            _ => None,
        },
        "34" => Some("Button"),
        "48" => {
            if fields.get(4).map(|value| value.trim()) == Some("1") {
                Some("LabelField")
            } else {
                Some("InputField")
            }
        }
        "6" => match fields.get(5).map(|value| value.trim())? {
            "0" => Some("SearchStringAddition"),
            "1" => Some("ViewStatusAddition"),
            "2" => Some("SearchControlAddition"),
            _ => None,
        },
        "73" => Some("Table"),
        _ => None,
    }
}

fn parse_form_child_item_name(wrapper: &str, fields: &[&str]) -> Option<String> {
    let indexes: &[usize] = match wrapper {
        "73" | "34" => &[5],
        "48" => &[6, 7],
        _ => &[6],
    };
    indexes.iter().find_map(|index| {
        parse_1c_quoted_string_with_len(fields.get(*index)?.trim())
            .map(|(value, _)| value)
            .filter(|value| !value.is_empty())
    })
}

fn parse_form_child_item_title(wrapper: &str, fields: &[&str]) -> Vec<(String, String)> {
    let indexes: &[usize] = match wrapper {
        "73" => &[9],
        "34" => &[6],
        "48" => &[9, 10],
        _ => &[7],
    };
    indexes
        .iter()
        .find_map(|index| {
            let values = fields
                .get(*index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

fn parse_form_child_item_event_fields(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if let Some(event) = parse_form_child_item_event_record(&nested) {
            events.push(event);
        }
    }
    for window in fields.windows(2) {
        if let Some(event) = parse_form_child_item_event_pair(window[0], window[1]) {
            events.push(event);
        }
    }
    events
}

fn parse_form_child_item_event_record(fields: &[&str]) -> Option<FormBodyEvent> {
    if fields.first().map(|value| value.trim()) != Some("1") {
        return None;
    }
    parse_form_child_item_event_pair(fields.get(1)?, fields.get(2)?)
}

fn parse_form_child_item_event_pair(
    event_field: &str,
    handler_field: &str,
) -> Option<FormBodyEvent> {
    let event = parse_form_child_item_event_identifier(event_field)?;
    let (handler, _) = parse_1c_quoted_string_with_len(handler_field.trim())?;
    let handler = handler.trim();
    if handler.is_empty() || !is_probable_form_event_handler(handler) {
        return None;
    }
    Some(FormBodyEvent {
        name: event,
        handler: handler.to_string(),
    })
}

fn parse_form_child_item_event_identifier(field: &str) -> Option<String> {
    let field = field.trim();
    let identifier = parse_1c_quoted_string_with_len(field)
        .map(|(value, _)| value)
        .unwrap_or_else(|| field.to_string());
    let identifier = identifier.trim();
    match identifier {
        "ActivationProcessing" => Some("ActivationProcessing".to_string()),
        "AdditionalDetailProcessing" => Some("AdditionalDetailProcessing".to_string()),
        "AutoComplete" => Some("AutoComplete".to_string()),
        "OnGetDataAtServer" => Some("OnGetDataAtServer".to_string()),
        "OnChange" => Some("OnChange".to_string()),
        "StartChoice" => Some("StartChoice".to_string()),
        "StartListChoice" => Some("StartListChoice".to_string()),
        "ValueChoice" => Some("ValueChoice".to_string()),
        "Click" => Some("Click".to_string()),
        "OnClick" => Some("OnClick".to_string()),
        "Clearing" => Some("Clearing".to_string()),
        "URLProcessing" => Some("URLProcessing".to_string()),
        "URLGetProcessing" => Some("URLGetProcessing".to_string()),
        "URLListGetProcessing" => Some("URLListGetProcessing".to_string()),
        "TextEditEnd" => Some("TextEditEnd".to_string()),
        "EditTextChange" => Some("EditTextChange".to_string()),
        "OnActivateCell" => Some("OnActivateCell".to_string()),
        "OnActivateField" => Some("OnActivateField".to_string()),
        "OnActivateRow" => Some("OnActivateRow".to_string()),
        "97365900-eadf-4dfd-a9aa-fbb9ecabd079" => Some("OnGetDataAtServer".to_string()),
        "BeforeAddRow" => Some("BeforeAddRow".to_string()),
        "Creating" => Some("Creating".to_string()),
        "OnCurrentPageChange" => Some("OnCurrentPageChange".to_string()),
        "OnCurrentParentChange" => Some("OnCurrentParentChange".to_string()),
        "OnEditEnd" => Some("OnEditEnd".to_string()),
        "BeforeEditEnd" => Some("BeforeEditEnd".to_string()),
        "BeforeDeleteRow" => Some("BeforeDeleteRow".to_string()),
        "OnStartEdit" => Some("OnStartEdit".to_string()),
        "Selection" => Some("Selection".to_string()),
        "BeforeRowChange" => Some("BeforeRowChange".to_string()),
        "AfterDeleteRow" => Some("AfterDeleteRow".to_string()),
        "BeforeCollapse" => Some("BeforeCollapse".to_string()),
        "BeforeExpand" => Some("BeforeExpand".to_string()),
        "BeforePrint" => Some("BeforePrint".to_string()),
        "DetailProcessing" => Some("DetailProcessing".to_string()),
        "DocumentComplete" => Some("DocumentComplete".to_string()),
        "Drag" => Some("Drag".to_string()),
        "DragCheck" => Some("DragCheck".to_string()),
        "DragEnd" => Some("DragEnd".to_string()),
        "DragStart" => Some("DragStart".to_string()),
        "MultipleValuesDelete" => Some("MultipleValuesDelete".to_string()),
        "NavigationProcessing" => Some("NavigationProcessing".to_string()),
        "NewWriteProcessing" => Some("NewWriteProcessing".to_string()),
        "OnChangeAreaContent" => Some("OnChangeAreaContent".to_string()),
        "OnChangeDisplaySettings" => Some("OnChangeDisplaySettings".to_string()),
        "OnIntervalEditEnd" => Some("OnIntervalEditEnd".to_string()),
        "OnPeriodOutput" => Some("OnPeriodOutput".to_string()),
        "RefreshRequestProcessing" => Some("RefreshRequestProcessing".to_string()),
        "Tuning" => Some("Tuning".to_string()),
        _ => parse_form_event_identifier(identifier),
    }
}

fn parse_form_child_item_data_path(
    tag: &str,
    fields: &[&str],
    name: &str,
    id: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    match tag {
        "Table" => main_data_path.map(ToOwned::to_owned),
        "InputField" | "LabelField" => parent_data_path.map(|parent| format!("{parent}.{name}")),
        "Button" => fields.get(9).and_then(|field| {
            parse_form_button_data_path(field, table_name_by_id, table_column_names_by_id)
        }),
        _ => table_name_by_id.get(id).cloned(),
    }
}

fn parse_form_button_command_name(
    field: &str,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let kind = fields.first()?.trim();
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    if kind == "0" {
        return form_standard_command_name(&uuid)
            .map(ToOwned::to_owned)
            .or_else(|| object_refs.get(&uuid).cloned());
    }
    commands
        .iter()
        .find(|command| command.id == kind && command.reference_uuid == uuid)
        .map(|command| format!("Form.Command.{}", command.name))
}

fn form_standard_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "4f834c38-add1-45e4-a9f3-cefe3efac5c9" => Some("Form.StandardCommand.Create"),
        "39bb0fe9-771d-4dd5-8a6e-2d16984523af" => Some("Form.StandardCommand.Help"),
        _ => None,
    }
}

fn form_table_names_by_id(fields: &[&str]) -> BTreeMap<String, String> {
    let mut tables = BTreeMap::new();
    collect_form_table_names(fields, &mut tables);
    tables
}

fn form_table_column_names_by_id(fields: &[&str]) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut tables = BTreeMap::new();
    collect_form_table_column_names(fields, &mut tables);
    tables
}

fn collect_form_table_names(fields: &[&str], tables: &mut BTreeMap<String, String>) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if nested.first().map(|value| value.trim()) == Some("73")
            && let Some(identity) = nested
                .get(1)
                .and_then(|field| split_1c_braced_fields(field, 0))
            && let (Some(id), Some(name)) = (
                identity.first().map(|value| value.trim()),
                parse_form_child_item_name("73", &nested),
            )
        {
            tables.insert(id.to_string(), name);
        }
        collect_form_table_names(&nested, tables);
    }
}

fn collect_form_table_column_names(
    fields: &[&str],
    tables: &mut BTreeMap<String, BTreeMap<String, String>>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if nested.first().map(|value| value.trim()) == Some("73")
            && let Some(identity) = nested
                .get(1)
                .and_then(|field| split_1c_braced_fields(field, 0))
            && let Some(table_id) = identity.first().map(|value| value.trim().to_string())
        {
            let mut columns = BTreeMap::new();
            collect_form_table_column_names_for_table(&nested, &mut columns);
            if !columns.is_empty() {
                tables.insert(table_id, columns);
            }
        }
        collect_form_table_column_names(&nested, tables);
    }
}

fn collect_form_table_column_names_for_table(
    fields: &[&str],
    columns: &mut BTreeMap<String, String>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        let wrapper = nested.first().map(|value| value.trim()).unwrap_or_default();
        if matches!(
            form_child_item_tag(wrapper, &nested),
            Some("InputField" | "LabelField")
        ) && let Some(identity) = nested
            .get(1)
            .and_then(|field| split_1c_braced_fields(field, 0))
            && let (Some(id), Some(name)) = (
                identity.first().map(|value| value.trim().to_string()),
                parse_form_child_item_name(wrapper, &nested),
            )
        {
            columns.insert(id, name);
        }
        collect_form_table_column_names_for_table(&nested, columns);
    }
}

fn parse_form_button_data_path(
    field: &str,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("2") {
        return None;
    }
    let table = fields
        .get(1)
        .and_then(|field| split_1c_braced_fields(field, 0))?;
    let table_id = table.first()?.trim();
    let table_name = table_name_by_id.get(table_id)?;
    let column = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|fields| fields.first().map(|value| value.trim().to_string()))?;
    let field_name = if column == "8" {
        "Ссылка".to_string()
    } else {
        table_column_names_by_id
            .get(table_id)
            .and_then(|columns| columns.get(&column))
            .cloned()?
    };
    Some(format!("Items.{table_name}.CurrentData.{field_name}"))
}

fn extract_form_command_interface(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterface> {
    let fields = trailing
        .get(3)
        .and_then(|field| split_1c_braced_fields(field, 0))?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let mut navigation_panel = Vec::new();
    for field in fields.iter().skip(2) {
        if let Some(item) = parse_form_command_interface_item(field, object_refs) {
            navigation_panel.push(item);
        }
    }
    if navigation_panel.is_empty() {
        return None;
    }
    Some(FormCommandInterface { navigation_panel })
}

fn parse_form_command_interface_item(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterfaceItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("3") {
        return None;
    }
    let command = parse_form_object_reference(fields.get(2)?, object_refs)?;
    let command_group = fields
        .get(5)
        .and_then(|field| parse_form_command_group_reference(field, object_refs))?;
    let index = fields
        .get(6)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|index| *index > 0);
    let default_visible = match fields.get(7).map(|value| value.trim()) {
        Some("0") => Some(false),
        Some("1") => Some(true),
        _ => None,
    };
    Some(FormCommandInterfaceItem {
        command,
        item_type: "Added",
        command_group,
        index,
        default_visible,
        visible_common: fields
            .get(8)
            .and_then(|value| parse_form_nested_common_bool(value)),
    })
}

fn parse_form_object_reference(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    object_refs.get(&uuid).cloned()
}

fn parse_form_command_group_reference(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    form_standard_command_group_name(&uuid)
        .map(ToOwned::to_owned)
        .or_else(|| object_refs.get(&uuid).cloned())
}

fn form_standard_command_group_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => Some("FormNavigationPanelGoTo"),
        _ => None,
    }
}

fn parse_form_nested_common_bool(field: &str) -> Option<bool> {
    if field.contains(r#"{"B",1}"#) {
        Some(true)
    } else if field.contains(r#"{"B",0}"#) {
        Some(false)
    } else {
        None
    }
}

fn dedup_form_item_assets(assets: Vec<FormItemAsset>) -> Vec<FormItemAsset> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut deduped = Vec::new();
    for asset in assets {
        if seen.insert((asset.item_name.clone(), asset.file_name.clone())) {
            deduped.push(asset);
        }
    }
    deduped
}

fn format_form_body_xml(
    properties: &FormBodyProperties,
    auto_command_bar: Option<&FormAutoCommandBar>,
    events: &[FormBodyEvent],
    child_items: &[FormChildItem],
    attributes: &[FormAttribute],
    parameters: &[FormParameter],
    commands: &[FormCommand],
    command_interface: &Option<FormCommandInterface>,
) -> String {
    let mut xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
"
    .to_string();
    xml.push_str(&format_form_localized_section(
        "Title",
        &properties.title,
        1,
    ));
    if let Some(width) = &properties.width {
        xml.push_str(&format!("\t<Width>{}</Width>\r\n", escape_xml_text(width)));
    }
    if let Some(height) = &properties.height {
        xml.push_str(&format!(
            "\t<Height>{}</Height>\r\n",
            escape_xml_text(height)
        ));
    }
    if let Some(window_opening_mode) = properties.window_opening_mode {
        xml.push_str(&format!(
            "\t<WindowOpeningMode>{}</WindowOpeningMode>\r\n",
            escape_xml_text(window_opening_mode)
        ));
    }
    if properties.auto_title == Some(false) {
        xml.push_str("\t<AutoTitle>false</AutoTitle>\r\n");
    }
    if let Some(group) = properties.group {
        xml.push_str(&format!("\t<Group>{}</Group>\r\n", escape_xml_text(group)));
    }
    if let Some(command_bar_location) = properties.command_bar_location {
        xml.push_str(&format!(
            "\t<CommandBarLocation>{}</CommandBarLocation>\r\n",
            escape_xml_text(command_bar_location)
        ));
    }
    if let Some(command_bar) = auto_command_bar {
        if command_bar.horizontal_align.is_some() || command_bar.autofill == Some(false) {
            xml.push_str(&format!(
                "\t<AutoCommandBar name=\"{}\" id=\"{}\">\r\n",
                escape_xml_text(&command_bar.name),
                escape_xml_text(&command_bar.id)
            ));
            if let Some(horizontal_align) = command_bar.horizontal_align {
                xml.push_str(&format!(
                    "\t\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
                    escape_xml_text(horizontal_align)
                ));
            }
            if command_bar.autofill == Some(false) {
                xml.push_str("\t\t<Autofill>false</Autofill>\r\n");
            }
            xml.push_str("\t</AutoCommandBar>\r\n");
        } else {
            xml.push_str(&format!(
                "\t<AutoCommandBar name=\"{}\" id=\"{}\"/>\r\n",
                escape_xml_text(&command_bar.name),
                escape_xml_text(&command_bar.id)
            ));
        }
    }
    if !events.is_empty() {
        xml.push_str("\t<Events>\r\n");
        for event in events {
            xml.push_str(&format!(
                "\t\t<Event name=\"{}\">{}</Event>\r\n",
                escape_xml_text(&event.name),
                escape_xml_text(&event.handler)
            ));
        }
        xml.push_str("\t</Events>\r\n");
    }
    xml.push_str(&format_form_child_items_xml(child_items, 1));
    xml.push_str(&format_form_attributes_xml(attributes));
    xml.push_str(&format_form_parameters_xml(parameters));
    if !commands.is_empty() {
        xml.push_str("\t<Commands>\r\n");
        for command in commands {
            xml.push_str(&format!(
                "\t\t<Command name=\"{}\" id=\"{}\">\r\n",
                escape_xml_text(&command.name),
                escape_xml_text(&command.id)
            ));
            xml.push_str(&format_form_localized_section("Title", &command.title, 3));
            xml.push_str(&format_form_localized_section(
                "ToolTip",
                &command.tooltip,
                3,
            ));
            xml.push_str(&format!(
                "\t\t\t<Action>{}</Action>\r\n",
                escape_xml_text(&command.action)
            ));
            if !command.functional_options.is_empty() {
                xml.push_str("\t\t\t<FunctionalOptions>\r\n");
                for item in &command.functional_options {
                    xml.push_str(&format!(
                        "\t\t\t\t<Item>{}</Item>\r\n",
                        escape_xml_text(item)
                    ));
                }
                xml.push_str("\t\t\t</FunctionalOptions>\r\n");
            }
            if let Some(current_row_use) = command.current_row_use {
                xml.push_str(&format!(
                    "\t\t\t<CurrentRowUse>{}</CurrentRowUse>\r\n",
                    escape_xml_text(current_row_use)
                ));
            }
            xml.push_str("\t\t</Command>\r\n");
        }
        xml.push_str("\t</Commands>\r\n");
    }
    if let Some(command_interface) = command_interface {
        xml.push_str(&format_form_command_interface_xml(command_interface));
    }
    xml.push_str("</Form>\r\n");
    xml
}

fn format_form_child_items_xml(items: &[FormChildItem], indent: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<ChildItems>\r\n");
    for item in items {
        xml.push_str(&format_form_child_item_xml(item, indent + 1));
    }
    xml.push_str(&format!("{tab}</ChildItems>\r\n"));
    xml
}

fn format_form_child_item_xml(item: &FormChildItem, indent: usize) -> String {
    let tab = "\t".repeat(indent);
    let mut xml = format!(
        "{tab}<{} name=\"{}\" id=\"{}\">\r\n",
        item.tag,
        escape_xml_text(&item.name),
        escape_xml_text(&item.id)
    );
    if item.tag.ends_with("Addition") {
        if item.addition_source_item.is_some() || item.item_type.is_some() {
            xml.push_str(&format!("{tab}\t<AdditionSource>\r\n"));
            if let Some(source_item) = &item.addition_source_item {
                xml.push_str(&format!(
                    "{tab}\t\t<Item>{}</Item>\r\n",
                    escape_xml_text(source_item)
                ));
            }
            if let Some(item_type) = item.item_type {
                xml.push_str(&format!(
                    "{tab}\t\t<Type>{}</Type>\r\n",
                    escape_xml_text(item_type)
                ));
            }
            xml.push_str(&format!("{tab}\t</AdditionSource>\r\n"));
        }
    } else if let Some(item_type) = item.item_type {
        xml.push_str(&format!(
            "{tab}\t<Type>{}</Type>\r\n",
            escape_xml_text(item_type)
        ));
    }
    if let Some(command_name) = &item.command_name {
        xml.push_str(&format!(
            "{tab}\t<CommandName>{}</CommandName>\r\n",
            escape_xml_text(command_name)
        ));
    }
    if let Some(data_path) = &item.data_path {
        xml.push_str(&format!(
            "{tab}\t<DataPath>{}</DataPath>\r\n",
            escape_xml_text(data_path)
        ));
    }
    if let Some(group) = item.group {
        xml.push_str(&format!(
            "{tab}\t<Group>{}</Group>\r\n",
            escape_xml_text(group)
        ));
    }
    xml.push_str(&format_form_localized_section(
        "Title",
        &item.title,
        indent + 1,
    ));
    if !item.events.is_empty() {
        xml.push_str(&format!("{tab}\t<Events>\r\n"));
        for event in &item.events {
            xml.push_str(&format!(
                "{tab}\t\t<Event name=\"{}\">{}</Event>\r\n",
                escape_xml_text(&event.name),
                escape_xml_text(&event.handler)
            ));
        }
        xml.push_str(&format!("{tab}\t</Events>\r\n"));
    }
    xml.push_str(&format_form_child_items_xml(&item.child_items, indent + 1));
    xml.push_str(&format!("{tab}</{}>\r\n", item.tag));
    xml
}

fn format_form_attributes_xml(attributes: &[FormAttribute]) -> String {
    if attributes.is_empty() {
        return "\t<Attributes/>\r\n".to_string();
    }
    let mut xml = "\t<Attributes>\r\n".to_string();
    for attribute in attributes {
        xml.push_str(&format!(
            "\t\t<Attribute name=\"{}\" id=\"{}\">\r\n",
            escape_xml_text(&attribute.name),
            escape_xml_text(&attribute.id)
        ));
        if attribute.settings.is_some() {
            xml.push_str("\t\t\t<Type>\r\n");
            xml.push_str("\t\t\t\t<v8:Type>cfg:DynamicList</v8:Type>\r\n");
            xml.push_str("\t\t\t</Type>\r\n");
        }
        if attribute.main_attribute {
            xml.push_str("\t\t\t<MainAttribute>true</MainAttribute>\r\n");
        }
        if !attribute.use_always.is_empty() {
            xml.push_str("\t\t\t<UseAlways>\r\n");
            for field in &attribute.use_always {
                xml.push_str(&format!(
                    "\t\t\t\t<Field>{}</Field>\r\n",
                    escape_xml_text(field)
                ));
            }
            xml.push_str("\t\t\t</UseAlways>\r\n");
        }
        if let Some(settings) = &attribute.settings {
            xml.push_str("\t\t\t<Settings xsi:type=\"DynamicList\">\r\n");
            if settings.manual_query {
                xml.push_str("\t\t\t\t<ManualQuery>true</ManualQuery>\r\n");
            }
            if settings.dynamic_data_read {
                xml.push_str("\t\t\t\t<DynamicDataRead>true</DynamicDataRead>\r\n");
            }
            if let Some(query_text) = &settings.query_text {
                xml.push_str(&format!(
                    "\t\t\t\t<QueryText>{}</QueryText>\r\n",
                    escape_xml_text(query_text)
                ));
            }
            if let Some(main_table) = &settings.main_table {
                xml.push_str(&format!(
                    "\t\t\t\t<MainTable>{}</MainTable>\r\n",
                    escape_xml_text(main_table)
                ));
            }
            xml.push_str(&format_form_list_settings_xml(&settings.list_settings));
            xml.push_str("\t\t\t</Settings>\r\n");
        }
        xml.push_str("\t\t</Attribute>\r\n");
    }
    xml.push_str("\t</Attributes>\r\n");
    xml
}

fn format_form_list_settings_xml(settings: &FormListSettings) -> String {
    if settings.filter.is_none()
        && settings.order.is_none()
        && settings.conditional_appearance.is_none()
        && settings.items_view_mode.is_none()
        && settings.items_user_setting_id.is_none()
    {
        return String::new();
    }
    let mut xml = "\t\t\t\t<ListSettings>\r\n".to_string();
    if let Some(filter) = &settings.filter {
        xml.push_str(&format_form_list_settings_standard_section_xml(
            "filter", filter,
        ));
    }
    if let Some(order) = &settings.order {
        xml.push_str("\t\t\t\t\t<dcsset:order>\r\n");
        for item in &order.items {
            xml.push_str("\t\t\t\t\t\t<dcsset:item xsi:type=\"dcsset:OrderItemField\">\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t\t\t<dcsset:field>{}</dcsset:field>\r\n",
                escape_xml_text(&item.field)
            ));
            if let Some(order_type) = &item.order_type {
                xml.push_str(&format!(
                    "\t\t\t\t\t\t\t<dcsset:orderType>{}</dcsset:orderType>\r\n",
                    escape_xml_text(order_type)
                ));
            }
            xml.push_str("\t\t\t\t\t\t</dcsset:item>\r\n");
        }
        if let Some(view_mode) = &order.view_mode {
            xml.push_str(&format!(
                "\t\t\t\t\t\t<dcsset:viewMode>{}</dcsset:viewMode>\r\n",
                escape_xml_text(view_mode)
            ));
        }
        if let Some(user_setting_id) = &order.user_setting_id {
            xml.push_str(&format!(
                "\t\t\t\t\t\t<dcsset:userSettingID>{}</dcsset:userSettingID>\r\n",
                escape_xml_text(user_setting_id)
            ));
        }
        xml.push_str("\t\t\t\t\t</dcsset:order>\r\n");
    }
    if let Some(conditional_appearance) = &settings.conditional_appearance {
        xml.push_str(&format_form_list_settings_standard_section_xml(
            "conditionalAppearance",
            conditional_appearance,
        ));
    }
    if let Some(items_view_mode) = &settings.items_view_mode {
        xml.push_str(&format!(
            "\t\t\t\t\t<dcsset:itemsViewMode>{}</dcsset:itemsViewMode>\r\n",
            escape_xml_text(items_view_mode)
        ));
    }
    if let Some(items_user_setting_id) = &settings.items_user_setting_id {
        xml.push_str(&format!(
            "\t\t\t\t\t<dcsset:itemsUserSettingID>{}</dcsset:itemsUserSettingID>\r\n",
            escape_xml_text(items_user_setting_id)
        ));
    }
    xml.push_str("\t\t\t\t</ListSettings>\r\n");
    xml
}

fn format_form_list_settings_standard_section_xml(
    name: &str,
    section: &FormListSettingsStandardSection,
) -> String {
    let mut xml = format!("\t\t\t\t\t<dcsset:{name}>\r\n");
    if let Some(view_mode) = &section.view_mode {
        xml.push_str(&format!(
            "\t\t\t\t\t\t<dcsset:viewMode>{}</dcsset:viewMode>\r\n",
            escape_xml_text(view_mode)
        ));
    }
    if let Some(user_setting_id) = &section.user_setting_id {
        xml.push_str(&format!(
            "\t\t\t\t\t\t<dcsset:userSettingID>{}</dcsset:userSettingID>\r\n",
            escape_xml_text(user_setting_id)
        ));
    }
    xml.push_str(&format!("\t\t\t\t\t</dcsset:{name}>\r\n"));
    xml
}

fn format_form_parameters_xml(parameters: &[FormParameter]) -> String {
    if parameters.is_empty() {
        return String::new();
    }
    let mut xml = "\t<Parameters>\r\n".to_string();
    for parameter in parameters {
        xml.push_str(&format!(
            "\t\t<Parameter name=\"{}\">\r\n",
            escape_xml_text(&parameter.name)
        ));
        xml.push_str(&format_metadata_types_xml(&parameter.value_types));
        if parameter.key_parameter {
            xml.push_str("\t\t\t<KeyParameter>true</KeyParameter>\r\n");
        }
        xml.push_str("\t\t</Parameter>\r\n");
    }
    xml.push_str("\t</Parameters>\r\n");
    xml
}

fn format_form_localized_section(name: &str, values: &[(String, String)], indent: usize) -> String {
    if values.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<{}>\r\n", name);
    for (lang, content) in values {
        xml.push_str(&format!(
            "{tab}\t<v8:item>\r\n{tab}\t\t<v8:lang>{}</v8:lang>\r\n{tab}\t\t<v8:content>{}</v8:content>\r\n{tab}\t</v8:item>\r\n",
            escape_xml_text(lang),
            escape_xml_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</{}>\r\n", name));
    xml
}

fn format_form_command_interface_xml(command_interface: &FormCommandInterface) -> String {
    let mut xml = "\t<CommandInterface>\r\n".to_string();
    if !command_interface.navigation_panel.is_empty() {
        xml.push_str("\t\t<NavigationPanel>\r\n");
        for item in &command_interface.navigation_panel {
            xml.push_str("\t\t\t<Item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t<Command>{}</Command>\r\n",
                escape_xml_text(&item.command)
            ));
            xml.push_str(&format!(
                "\t\t\t\t<Type>{}</Type>\r\n",
                escape_xml_text(item.item_type)
            ));
            xml.push_str(&format!(
                "\t\t\t\t<CommandGroup>{}</CommandGroup>\r\n",
                escape_xml_text(&item.command_group)
            ));
            if let Some(index) = item.index {
                xml.push_str(&format!("\t\t\t\t<Index>{index}</Index>\r\n"));
            }
            if let Some(default_visible) = item.default_visible {
                xml.push_str(&format!(
                    "\t\t\t\t<DefaultVisible>{}</DefaultVisible>\r\n",
                    xml_bool(default_visible)
                ));
            }
            if let Some(common) = item.visible_common {
                xml.push_str("\t\t\t\t<Visible>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:Common>{}</xr:Common>\r\n",
                    xml_bool(common)
                ));
                xml.push_str("\t\t\t\t</Visible>\r\n");
            }
            xml.push_str("\t\t\t</Item>\r\n");
        }
        xml.push_str("\t\t</NavigationPanel>\r\n");
    }
    xml.push_str("\t</CommandInterface>\r\n");
    xml
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
    let merges = parse_moxel_merges(&fields);
    let areas = parse_moxel_areas(&fields);
    let print_area = parse_moxel_print_area(&fields);
    trim_moxel_trailing_empty_rows(&mut rows, &areas, &merges);
    compact_moxel_empty_row_ranges(&mut rows);
    let (column_sets, row_column_ids) = parse_moxel_column_sets(&fields);
    let fonts = parse_moxel_fonts(&fields);
    let pictures = parse_moxel_pictures(&fields, object_refs);
    let style_refs = parse_moxel_style_refs(&fields, object_refs);
    let default_format = parse_moxel_default_format(&fields, object_refs);
    let print_settings = parse_moxel_print_settings(&fields);
    let empty_headers_footers = parse_moxel_empty_headers_footers(&fields);
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
    let format_offset = column_format_slots.saturating_sub(1);
    for row in &mut rows {
        if let Some(columns_id) = row_column_ids.get(&row.index) {
            row.columns_id = Some(columns_id.clone());
        }
        if row.format_index > 1 {
            row.format_index += format_offset;
        }
        for cell in &mut row.cells {
            if cell.format_index > 0 {
                cell.format_index += format_offset;
            }
        }
    }
    let max_format_index = rows
        .iter()
        .fold(column_format_slots.max(1), |max_index, row| {
            let row_max = row.cells.iter().fold(row.format_index, |cell_max, cell| {
                cell_max.max(cell.format_index)
            });
            max_index.max(row_max)
        });
    let default_format_width = parse_moxel_default_format_width(&fields, column_format_slots);
    let height = moxel_spreadsheet_height(&rows, &merges, &areas);
    let drawings = parse_moxel_drawings(&fields);
    let drawing_format_indices = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .collect::<BTreeSet<_>>();
    let (column_formats, formats) = parse_moxel_formats(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
    );
    let all_formats = column_formats
        .iter()
        .chain(formats.iter())
        .cloned()
        .collect::<Vec<_>>();
    let has_sparse_column_sets = column_sets
        .iter()
        .any(|column_set| column_set.columns.len() != column_set.size);
    let lines = parse_moxel_lines(&fields, &all_formats, has_sparse_column_sets);
    let drawing_max_format_index = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    let max_format_index = max_format_index.max(drawing_max_format_index);
    let default_format_index = moxel_default_format_index(
        &column_sets,
        print_settings.as_ref(),
        !default_format.is_empty() || default_format_width.is_some(),
        max_format_index + 1,
    );
    Some(MoxelSpreadsheet {
        column_count,
        column_sets,
        column_formats,
        default_format_width,
        default_format,
        formats,
        rows,
        merges,
        areas,
        print_area,
        print_settings,
        lines,
        fonts,
        drawings,
        pictures,
        empty_headers_footers,
        default_format_index,
        height,
    })
}

fn default_moxel_column_sets(column_count: usize) -> Vec<MoxelColumnSet> {
    vec![MoxelColumnSet {
        id: None,
        size: column_count,
        columns: (0..column_count)
            .map(|index| MoxelColumn {
                index,
                format_index: index + 1,
            })
            .collect(),
    }]
}

fn parse_moxel_column_sets(fields: &[&str]) -> (Vec<MoxelColumnSet>, BTreeMap<usize, String>) {
    for index in 0..fields.len() {
        let Some(default_set) = parse_moxel_column_set(fields[index]) else {
            continue;
        };
        if default_set.id.is_some() || index + 2 >= fields.len() {
            continue;
        }
        let Some(_height) = fields
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
        return (column_sets, row_column_ids);
    }
    (Vec::new(), BTreeMap::new())
}

fn parse_moxel_column_set(text: &str) -> Option<MoxelColumnSet> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 || fields.get(1)?.trim() != "0" {
        return None;
    }
    let declared_count = fields.first()?.trim().parse::<usize>().ok()?;
    let count = fields.get(3)?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 2048 || declared_count == 0 || fields.len() != count * 2 + 4 {
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
        columns.push(MoxelColumn {
            index: fields
                .get(column_index * 2 + 4)?
                .trim()
                .parse::<usize>()
                .ok()?,
            format_index: fields
                .get(column_index * 2 + 5)?
                .trim()
                .parse::<usize>()
                .ok()?,
        });
    }
    Some(MoxelColumnSet {
        id,
        size: declared_count,
        columns,
    })
}

fn normalize_moxel_column_set_format_indices(column_sets: &mut [MoxelColumnSet]) {
    let min_format_index = column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .map(|column| column.format_index)
        .min()
        .unwrap_or(1);
    let raw_offset = min_format_index.saturating_sub(1);
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        column.format_index = column.format_index.saturating_sub(raw_offset).max(1);
    }
}

fn moxel_column_format_slots(column_sets: &[MoxelColumnSet], column_count: usize) -> usize {
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .map(|column| column.format_index)
        .max()
        .unwrap_or(column_count)
}

fn moxel_default_format_index(
    column_sets: &[MoxelColumnSet],
    print_settings: Option<&MoxelPrintSettings>,
    has_default_format: bool,
    fallback: usize,
) -> Option<usize> {
    if has_default_format {
        return Some(fallback);
    }
    if print_settings.is_some() && column_sets.len() > 1 {
        return Some(
            column_sets
                .get(1)
                .and_then(|column_set| {
                    column_set
                        .columns
                        .iter()
                        .map(|column| column.format_index)
                        .max()
                })
                .unwrap_or(fallback),
        );
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
) {
    let Some(material_limit) = areas
        .iter()
        .map(|area| area.end_row.max(0) as usize + 1)
        .chain(
            merges
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
    let mut rows = Vec::new();
    let mut index = 3usize;
    let mut expected_row_index = 0usize;
    let mut next_contiguous_index = None;
    while index < fields.len() {
        if let Some((row, next_index)) =
            parse_moxel_row_at_for_scanning(fields, index, expected_row_index)
        {
            if expected_row_index > 0
                && is_moxel_compactable_empty_row(&row)
                && next_contiguous_index != Some(index)
            {
                index += 1;
                continue;
            }
            rows.push(row);
            expected_row_index += 1;
            next_contiguous_index = Some(next_index);
            index = next_index;
        } else {
            index += 1;
        }
    }
    rows
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
    let detail_parameter = if cell_kind == "24" {
        fields.get(2).and_then(|value| parse_1c_string(value))
    } else {
        None
    };
    let localized_index = if cell_kind == "24" { 3 } else { 2 };
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
    fields
        .iter()
        .filter_map(|field| parse_moxel_area_list(field))
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
                height: Some(height_raw / 10),
                bold: weight >= 700,
                italic: fields.get(4)?.trim() != "0",
                underline: fields.get(5)?.trim() != "0",
                strikeout: fields.get(6)?.trim() != "0",
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
        _ => None,
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
    let lines = fields
        .iter()
        .filter_map(|field| parse_moxel_line(field))
        .collect::<Vec<_>>();
    let uses_drawing_line = formats.iter().any(|format| format.drawing_border.is_some());
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
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
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
    Some(MoxelPicture {
        index: fields.get(1)?.trim().parse::<usize>().ok()?,
        ref_name,
    })
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
    let picture_size = match fields.get(11)?.trim().parse::<usize>().ok()? {
        1 => "Stretch",
        _ => return None,
    };
    let z_order = fields.get(12)?.trim().parse::<usize>().ok()?;
    Some(MoxelDrawing {
        format_index: format_fields.get(1)?.trim().parse::<usize>().ok()?,
        begin_row,
        begin_row_offset,
        end_row,
        end_row_offset,
        begin_column,
        begin_column_offset,
        end_column,
        end_column_offset,
        auto_size: fields.get(10)?.trim() == "0",
        picture_size,
        z_order,
        picture_index: z_order,
    })
}

fn parse_moxel_default_format_width(fields: &[&str], column_count: usize) -> Option<usize> {
    let widths = fields
        .iter()
        .filter_map(|field| parse_moxel_column_width(field))
        .collect::<Vec<_>>();
    if widths.len() <= column_count {
        return None;
    }
    widths.first().copied()
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
    if count != 18 || fields.len() != count * 2 + 2 {
        return None;
    }
    let mut settings = MoxelPrintSettings::default();
    for pair in fields[2..].chunks_exact(2) {
        let key = pair.first()?.trim().parse::<usize>().ok()?;
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
            _ => {}
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
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    for index in 0..fields.len() {
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
            let Some(mut format) = parse_moxel_format(field, style_refs) else {
                formats.clear();
                break;
            };
            if drawing_format_indices.contains(&(format_offset + 1)) {
                format.drawing_border = format.left_border;
                format.left_border = None;
            }
            formats.push(format);
        }
        if formats.len() == count {
            let trailing_drawing_count = (1..=count)
                .rev()
                .take_while(|format_index| drawing_format_indices.contains(format_index))
                .count();
            let column_start = count.saturating_sub(trailing_drawing_count + column_count);
            let column_end = count.saturating_sub(trailing_drawing_count);
            let mut body_formats = formats;
            let trailing_formats = body_formats.split_off(column_end);
            let column_formats = body_formats.split_off(column_start);
            body_formats.extend(trailing_formats);
            return (column_formats, body_formats);
        }
    }
    (Vec::new(), Vec::new())
}

fn parse_moxel_format(text: &str, style_refs: &[Option<String>]) -> Option<MoxelFormat> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let bits = moxel_format_bits(flags)?;
    if bits.len() + 1 != fields.len() {
        return None;
    }
    let values = bits
        .iter()
        .copied()
        .zip(fields.iter().skip(1).copied())
        .collect::<Vec<_>>();
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
    Some(MoxelFormat {
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
        height: parse_moxel_format_usize(&values, 6),
        border_color: parse_moxel_format_style_ref(&values, 5, style_refs),
        width: parse_moxel_format_usize(&values, 7),
        horizontal_alignment: parse_moxel_format_usize(&values, 8)
            .and_then(moxel_horizontal_alignment),
        vertical_alignment: parse_moxel_format_usize(&values, 9).and_then(moxel_vertical_alignment),
        back_color: parse_moxel_format_style_ref(&values, 11, style_refs),
        text_color: parse_moxel_format_style_ref(&values, 10, style_refs),
        text_placement: parse_moxel_format_usize(&values, 14).and_then(moxel_text_placement),
        fill_type: parse_moxel_format_usize(&values, 15).and_then(moxel_fill_type),
        drawing_border: None,
        by_selected_columns: parse_moxel_format_usize(&values, 20)
            .and_then(moxel_by_selected_columns),
        details_use: parse_moxel_format_usize(&values, 19).and_then(moxel_details_use),
        hyper_link: parse_moxel_format_usize(&values, 26).and_then(moxel_hyper_link),
        protection: parse_moxel_format_usize(&values, 16).and_then(moxel_protection),
        indent: parse_moxel_format_usize(&values, 30),
        auto_indent: parse_moxel_format_usize(&values, 31),
        mask: parse_moxel_format_usize(&values, 34).and_then(moxel_mask),
        pic_index: parse_moxel_format_usize(&values, 35),
        picture_size_mode: parse_moxel_format_usize(&values, 36).and_then(moxel_picture_size_mode),
        pic_horizontal_alignment: parse_moxel_format_usize(&values, 37)
            .and_then(moxel_picture_alignment),
        pic_vertical_alignment: parse_moxel_format_usize(&values, 38)
            .and_then(moxel_picture_alignment),
    })
}

fn moxel_format_bits(flags: u64) -> Option<Vec<u8>> {
    if flags == 0 {
        return None;
    }
    let mut bits = Vec::new();
    for bit in 0..64 {
        if flags & (1u64 << bit) == 0 {
            continue;
        }
        if !matches!(
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
                | 14
                | 15
                | 16
                | 19
                | 20
                | 26
                | 30
                | 31
                | 34
                | 35
                | 36
                | 37
                | 38
        ) {
            return None;
        }
        bits.push(bit);
    }
    Some(bits)
}

fn parse_moxel_format_usize(values: &[(u8, &str)], bit: u8) -> Option<usize> {
    values
        .iter()
        .find(|(value_bit, _)| *value_bit == bit)
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
}

fn parse_moxel_format_style_ref(
    values: &[(u8, &str)],
    bit: u8,
    style_refs: &[Option<String>],
) -> Option<String> {
    parse_moxel_format_usize(values, bit)
        .and_then(|index| style_refs.get(index))
        .cloned()
        .flatten()
}

fn parse_moxel_style_refs(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let mut style_refs = Vec::new();
    for field in fields {
        if let Some(style_ref) = parse_moxel_style_ref_slot(field, object_refs) {
            style_refs.push(style_ref);
            continue;
        }
        style_refs.extend(parse_moxel_embedded_style_refs(field, object_refs));
    }
    style_refs
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
            "-1" | "-3" => Some(None),
            "-10" => Some(Some("style:FieldBackColor".to_string())),
            "-7" => Some(Some("style:ButtonBackColor".to_string())),
            "-28" => Some(Some("style:ReportLineColor".to_string())),
            "0" => {
                let uuid = parse_uuid_field(payload.get(1)?.trim())?;
                Some(moxel_style_ref_for_uuid(&uuid, object_refs))
            }
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
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_moxel_embedded_style_ref(field, container_kind, object_refs))
        .collect()
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
    kind: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    match (uuid, container_kind, kind) {
        ("f527dc88-1d39-40b3-bcbb-d98b690ead68", _, Some("2")) => {
            Some("style:FieldBackColor".to_string())
        }
        _ => moxel_style_ref_for_uuid(uuid, object_refs),
    }
}

fn parse_moxel_web_color(value: &str) -> Option<String> {
    let name = match value.parse::<u32>().ok()? {
        21 => "Crimson",
        48 => "Gainsboro",
        64 => "LemonChiffon",
        79 => "LightYellow",
        108 => "PaleGoldenrod",
        121 => "RoyalBlue",
        _ => return None,
    };
    Some(format!("d3p1:{name}"))
}

fn parse_moxel_style_color(value: &str) -> Option<String> {
    match value.parse::<u32>().ok()? {
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
        24 => Some("Center"),
        _ => None,
    }
}

fn moxel_text_placement(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Auto"),
        2 => Some("Block"),
        3 => Some("Wrap"),
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

fn moxel_hyper_link(value: usize) -> Option<bool> {
    match value {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

fn moxel_picture_size_mode(value: usize) -> Option<&'static str> {
    match value {
        6 => Some("Proportionally"),
        _ => None,
    }
}

fn moxel_picture_alignment(value: usize) -> Option<&'static str> {
    match value {
        2 | 24 => Some("Center"),
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
        _ => return None,
    };
    Some(MoxelLine {
        style,
        line_type: "v8ui:SpreadsheetDocumentCellLineType",
        width: 1,
    })
}

fn parse_moxel_merges(fields: &[&str]) -> Vec<MoxelMerge> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_merge_list(field))
        .next()
        .unwrap_or_default()
}

fn parse_moxel_merge_list(text: &str) -> Option<Vec<MoxelMerge>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 4096 || fields.len() != count + 1 {
        return None;
    }
    let mut merges = Vec::with_capacity(count);
    for field in fields.iter().skip(1) {
        let merge = parse_moxel_merge(field)?;
        merges.push(merge);
    }
    Some(merges)
}

fn parse_moxel_merge(text: &str) -> Option<MoxelMerge> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 {
        return None;
    }
    let begin_column = fields.first()?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(1)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    if begin_row < 0 || begin_column < 0 || end_row < begin_row || end_column < begin_column {
        return None;
    }
    Some(MoxelMerge {
        row: begin_row,
        column: begin_column,
        height: end_row - begin_row,
        width: end_column - begin_column,
    })
}

fn parse_moxel_area_list(text: &str) -> Option<Vec<MoxelArea>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 512 || fields.len() != count * 2 + 1 {
        return None;
    }
    let mut areas = Vec::with_capacity(count);
    for index in 0..count {
        let name = parse_1c_string(fields.get(index * 2 + 1)?)?;
        let area = parse_moxel_area(fields.get(index * 2 + 2)?, name)?;
        areas.push(area);
    }
    Some(areas)
}

fn parse_moxel_area(text: &str, name: String) -> Option<MoxelArea> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let bounds = split_1c_braced_fields(fields.get(1)?, 0)?;
    parse_moxel_bounds_area(&bounds, name)
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
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
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
        push_moxel_columns_xml(&mut xml, column_set);
    }
    for row in &spreadsheet.rows {
        push_moxel_row_xml(&mut xml, row);
    }
    if spreadsheet.empty_headers_footers {
        push_moxel_empty_headers_footers_xml(&mut xml);
    }
    xml.push_str("\t<templateMode>true</templateMode>\r\n");
    for drawing in &spreadsheet.drawings {
        push_moxel_drawing_xml(&mut xml, drawing);
    }
    if let Some(default_format_index) = spreadsheet.default_format_index {
        xml.push_str(&format!(
            "\t<defaultFormatIndex>{default_format_index}</defaultFormatIndex>\r\n"
        ));
    }
    xml.push_str(&format!("\t<height>{}</height>\r\n", spreadsheet.height));
    xml.push_str(&format!("\t<vgRows>{}</vgRows>\r\n", spreadsheet.height));
    for merge in &spreadsheet.merges {
        push_moxel_merge_xml(&mut xml, merge);
    }
    for area in &spreadsheet.areas {
        push_moxel_area_xml(&mut xml, area);
    }
    if let Some(print_area) = &spreadsheet.print_area {
        push_moxel_print_area_xml(&mut xml, print_area);
    }
    if let Some(print_settings) = &spreadsheet.print_settings {
        push_moxel_print_settings_xml(&mut xml, print_settings);
    }
    for line in &spreadsheet.lines {
        push_moxel_line_xml(&mut xml, line);
    }
    for font in &spreadsheet.fonts {
        push_moxel_font_xml(&mut xml, font);
    }
    let format_count = spreadsheet
        .default_format_index
        .unwrap_or(0)
        .max(spreadsheet.column_formats.len() + spreadsheet.formats.len())
        .max(1);
    for format_index in 1..=format_count {
        push_moxel_format_xml(&mut xml, spreadsheet, format_index);
    }
    for picture in &spreadsheet.pictures {
        push_moxel_picture_xml(&mut xml, picture);
    }
    xml.push_str("</document>\r\n");
    xml
}

fn push_moxel_columns_xml(xml: &mut String, column_set: &MoxelColumnSet) {
    xml.push_str("\t<columns>\r\n");
    if let Some(id) = &column_set.id {
        xml.push_str(&format!("\t\t<id>{}</id>\r\n", escape_xml_text(id)));
    }
    xml.push_str(&format!("\t\t<size>{}</size>\r\n", column_set.size));
    for column in &column_set.columns {
        let column_index = column.index;
        let format_index = column.format_index;
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
    push_moxel_format_usize(xml, "height", format.height);
    push_moxel_format_text(xml, "borderColor", format.border_color.as_deref());
    push_moxel_format_usize(xml, "width", format.width);
    push_moxel_format_text(xml, "horizontalAlignment", format.horizontal_alignment);
    push_moxel_format_text(xml, "verticalAlignment", format.vertical_alignment);
    push_moxel_format_text(xml, "backColor", format.back_color.as_deref());
    push_moxel_format_text(xml, "textColor", format.text_color.as_deref());
    push_moxel_format_text(xml, "textPlacement", format.text_placement);
    push_moxel_format_text(xml, "fillType", format.fill_type);
    push_moxel_format_usize(xml, "drawingBorder", format.drawing_border);
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
    push_moxel_format_usize(xml, "indent", format.indent);
    push_moxel_format_usize(xml, "autoIndent", format.auto_indent);
    if let Some(mask) = format.mask {
        if mask.is_empty() {
            xml.push_str("\t\t<mask/>\r\n");
        } else {
            xml.push_str(&format!("\t\t<mask>{}</mask>\r\n", escape_xml_text(mask)));
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
    xml.push_str("\t</format>\r\n");
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
    if spreadsheet.default_format_index == Some(format_index) {
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

fn push_moxel_format_bool(xml: &mut String, tag: &str, value: Option<bool>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{}</{tag}>\r\n", xml_bool(value)));
    }
}

fn push_moxel_format_text(xml: &mut String, tag: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "\t\t<{tag}>{}</{tag}>\r\n",
            escape_xml_text(value)
        ));
    }
}

fn push_moxel_picture_xml(xml: &mut String, picture: &MoxelPicture) {
    xml.push_str("\t<picture>\r\n");
    xml.push_str(&format!("\t\t<index>{}</index>\r\n", picture.index));
    if let Some(ref_name) = &picture.ref_name {
        xml.push_str(&format!(
            "\t\t<picture t=\"false\" ref=\"{}\"/>\r\n",
            escape_xml_text(ref_name)
        ));
    } else {
        xml.push_str("\t\t<picture/>\r\n");
    }
    xml.push_str("\t</picture>\r\n");
}

fn push_moxel_drawing_xml(xml: &mut String, drawing: &MoxelDrawing) {
    xml.push_str("\t<drawing>\r\n");
    xml.push_str("\t\t<drawingType>Picture</drawingType>\r\n");
    xml.push_str(&format!("\t\t<id>{}</id>\r\n", drawing.z_order));
    xml.push_str(&format!(
        "\t\t<formatIndex>{}</formatIndex>\r\n",
        drawing.format_index
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
    xml.push_str(&format!("\t\t<r>{}</r>\r\n", merge.row));
    xml.push_str(&format!("\t\t<c>{}</c>\r\n", merge.column));
    if merge.height > 0 {
        xml.push_str(&format!("\t\t<h>{}</h>\r\n", merge.height));
    }
    if merge.width > 0 {
        xml.push_str(&format!("\t\t<w>{}</w>\r\n", merge.width));
    }
    xml.push_str("\t</merge>\r\n");
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
        xml.push_str(&format!(" ref=\"{}\"", escape_xml_text(ref_name)));
    }
    if let Some(face_name) = &font.face_name {
        xml.push_str(&format!(" faceName=\"{}\"", escape_xml_text(face_name)));
    }
    if let Some(height) = font.height {
        xml.push_str(&format!(" height=\"{height}\""));
    }
    xml.push_str(&format!(
        " bold=\"{}\" italic=\"{}\" underline=\"{}\" strikeout=\"{}\" kind=\"{}\"",
        font.bold, font.italic, font.underline, font.strikeout, font.kind
    ));
    if let Some(scale) = font.scale {
        xml.push_str(&format!(" scale=\"{scale}\""));
    }
    xml.push_str("/>\r\n");
}

fn push_moxel_area_xml(xml: &mut String, area: &MoxelArea) {
    xml.push_str("\t<namedItem xsi:type=\"NamedItemCells\">\r\n");
    xml.push_str(&format!(
        "\t\t<name>{}</name>\r\n",
        escape_xml_text(&area.name)
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
    xml.push_str("\t</printArea>\r\n");
}

fn push_moxel_row_xml(xml: &mut String, row: &MoxelRow) {
    xml.push_str(&format!(
        "\t<rowsItem>\r\n\t\t<index>{}</index>\r\n",
        row.index
    ));
    if let Some(index_to) = row.index_to {
        xml.push_str(&format!("\t\t<indexTo>{index_to}</indexTo>\r\n"));
    }
    xml.push_str("\t\t<row>\r\n");
    if row.format_index > 1 {
        xml.push_str(&format!(
            "\t\t\t<formatIndex>{}</formatIndex>\r\n",
            row.format_index
        ));
    }
    if let Some(columns_id) = &row.columns_id {
        xml.push_str(&format!(
            "\t\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
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
        xml.push_str(&format!("\t\t\t\t\t<f>{}</f>\r\n", cell.format_index));
        if let Some(text) = &cell.text {
            xml.push_str("\t\t\t\t\t<tl>\r\n");
            xml.push_str("\t\t\t\t\t\t<v8:item>\r\n");
            xml.push_str("\t\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(text)
            ));
            xml.push_str("\t\t\t\t\t\t</v8:item>\r\n");
            xml.push_str("\t\t\t\t\t</tl>\r\n");
        } else if cell.empty_text {
            xml.push_str("\t\t\t\t\t<tl/>\r\n");
        }
        if let Some(parameter) = &cell.parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<parameter>{}</parameter>\r\n",
                escape_xml_text(parameter)
            ));
        }
        if let Some(detail_parameter) = &cell.detail_parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<detailParameter>{}</detailParameter>\r\n",
                escape_xml_text(detail_parameter)
            ));
        }
        xml.push_str("\t\t\t\t</c>\r\n");
        xml.push_str("\t\t\t</c>\r\n");
        expected_column = cell.column_index + 1;
    }
    xml.push_str("\t\t</row>\r\n\t</rowsItem>\r\n");
}

fn module_body_paths(rows: &[ConfigRow]) -> BTreeMap<String, PathBuf> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = configuration_module_body_paths(&file_names);

    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some(entries) =
            parse_module_body_source_paths_from_metadata_blob(&bytes, &row.file_name, &file_names)
        else {
            continue;
        };
        paths.extend(entries);
    }
    paths.extend(form_module_body_paths(rows, &file_names));

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
    rows: &[ConfigRow],
    file_names: &BTreeSet<&str>,
) -> BTreeMap<String, PathBuf> {
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in build_form_source_reference_index(rows) {
        let body_id = format!("{form_uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let mut form_dir = form_ref.relative_path;
        form_dir.set_extension("");
        paths.insert(
            body_id,
            form_dir.join("Ext").join("Form").join("Module.bsl"),
        );
    }
    paths
}

fn unpack_form_body_module_text(blob: &[u8]) -> Option<Vec<u8>> {
    let module_text = parse_form_body_blob(blob).ok()?.module_text;
    if module_text.trim().is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(3 + module_text.len());
    bytes.extend_from_slice(b"\xEF\xBB\xBF");
    bytes.extend_from_slice(module_text.as_bytes());
    Some(bytes)
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

fn parse_module_body_source_paths_from_metadata_blob(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
) -> Option<BTreeMap<String, PathBuf>> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let object_code = parse_metadata_object_code(text)?;
    let header = parse_metadata_header_from_text(text, uuid)?;

    let (kind, folder) = if object_code == 12 {
        ("CommonModule", "CommonModules")
    } else {
        metadata_source_for_text(object_code, text, uuid)?
    };
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
        text,
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

fn nested_command_headers_from_text(text: &str, owner_uuid: &str) -> Vec<MetadataHeader> {
    nested_headers_from_text_inside_object_code(text, owner_uuid, 9)
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
            headers.push(header);
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

fn module_owner_source_path(kind: &str, folder: &str, name: &str, suffix: &str) -> Option<PathBuf> {
    let module_file = match (kind, suffix) {
        ("CommonModule", "0") | ("HTTPService", "0") | ("WebService", "0") => Some("Module.bsl"),
        ("Bot", "1") | ("IntegrationService", "0") => Some("Module.bsl"),
        ("CommonCommand", "2") => Some("CommandModule.bsl"),
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
    };
    module_file.map(|module_file| {
        PathBuf::from(folder)
            .join(sanitize_source_path_segment(name))
            .join("Ext")
            .join(module_file)
    })
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

struct MetadataHeader {
    uuid: String,
    name: String,
    synonyms: Vec<(String, String)>,
    comment: String,
    template_type_code: Option<u32>,
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
    owners_empty: bool,
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
}

struct GeneratedTypeEntry {
    name: String,
    category: &'static str,
    type_id: String,
    value_id: String,
}

struct ConstantProperties {
    value_type: ConstantValueType,
    use_standard_commands: bool,
}

struct DefinedTypeProperties {
    value_types: Vec<ConstantValueType>,
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

struct StyleBodyItem {
    name: String,
    standard_order: Option<usize>,
    value_xml: String,
}

struct TypedMetadataProperties {
    value_types: Vec<ConstantValueType>,
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
    DateTime,
    Reference {
        reference: String,
    },
}

fn build_metadata_type_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some(entries) = parse_generated_type_entries_from_blob(&bytes, &row.file_name) else {
            continue;
        };
        for (type_id, reference) in entries {
            index.insert(type_id, reference);
        }
    }
    index
}

fn parse_generated_type_entries_from_blob(
    blob: &[u8],
    uuid: &str,
) -> Option<Vec<(String, String)>> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let object_code = parse_metadata_object_code(text)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
    let root_fields = split_1c_braced_fields(text, 0)?;
    let object_text = *root_fields.get(1)?;
    let fields = split_1c_braced_fields(object_text, 0)?;
    let mut entries = Vec::new();

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
    }
    if object_code == 37 {
        push_indexed_generated_type(&mut entries, &fields, 1, "ExchangePlanObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "ExchangePlanRef", &header.name);
    }
    if object_code == 40 {
        push_indexed_generated_type(&mut entries, &fields, 1, "DocumentObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "DocumentRef", &header.name);
    }
    if object_code == 57 {
        push_indexed_generated_type(&mut entries, &fields, 1, "CatalogObject", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 3, "CatalogRef", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 5, "CatalogSelection", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 7, "CatalogList", &header.name);
        push_indexed_generated_type(&mut entries, &fields, 34, "CatalogManager", &header.name);
    }
    let header_index = metadata_header_field_index(&fields, uuid);

    if object_code == 20 && header_index == Some(5) {
        if let Some(type_id) = fields.get(1).copied().and_then(parse_uuid_field) {
            entries.push((type_id, format!("cfg:EnumRef.{}", header.name)));
        }
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
        form_refs,
        template_refs,
        &BTreeMap::new(),
    )
}

fn extract_metadata_source_xml_with_refs(
    blob: &[u8],
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<ExtractedMetadataSourceXml> {
    if uuid.contains('.') {
        return None;
    }
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    if let Some(xml) = extract_configuration_source_xml(text, uuid) {
        return Some(ExtractedMetadataSourceXml {
            relative_path: PathBuf::from("Configuration.xml"),
            xml: xml.into_bytes(),
        });
    }
    let object_code = parse_metadata_object_code(text)?;
    if object_code == 12 {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let flags = parse_common_module_flags_from_text(text, uuid)?;
        let relative_path = PathBuf::from("CommonModules")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_common_module_source_xml(&header, &flags).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 16 {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let constant = parse_constant_properties_from_text(text, uuid, type_index)?;
        let relative_path = PathBuf::from("Constants")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_constant_source_xml(&header, &constant).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 3
        && metadata_source_for_text(object_code, text, uuid)
            .is_some_and(|(kind, _)| kind == "CommandGroup")
    {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let command_group = parse_command_group_properties_from_text(text, uuid, object_refs)?;
        let relative_path = PathBuf::from("CommandGroups")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_command_group_source_xml(&header, &command_group).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 3
        && metadata_source_for_text(object_code, text, uuid)
            .is_some_and(|(kind, _)| kind == "StyleItem")
    {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let style_item = parse_style_item_properties_from_text(text, uuid)?;
        let relative_path = PathBuf::from("StyleItems")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_style_item_source_xml(&header, &style_item).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 0 && is_defined_type_metadata_text(text, uuid) {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let defined_type = parse_defined_type_properties_from_text(text, uuid, type_index)?;
        let relative_path = PathBuf::from("DefinedTypes")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_defined_type_source_xml(&header, &defined_type).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if is_form_metadata_text(text, uuid) {
        let header = parse_metadata_header_from_text(text, uuid)?;
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
        let xml = format_form_source_xml(kind, &header).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if is_template_metadata_text(text, uuid) {
        let header = parse_metadata_header_from_text(text, uuid)?;
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
        let xml = format_template_source_xml(kind, &header, template_type).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    let (kind, folder) = metadata_source_for_text(object_code, text, uuid)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
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
    let xml = if kind == "Catalog" {
        let catalog = parse_catalog_properties_from_text(text, uuid, form_refs)?;
        format_catalog_source_xml(&header, &catalog).into_bytes()
    } else if is_typed_metadata_source(kind) {
        let typed = parse_typed_metadata_properties_from_text(text, uuid, type_index)?;
        format_typed_metadata_source_xml(kind, &header, &typed).into_bytes()
    } else {
        format_metadata_source_xml(kind, &header).into_bytes()
    };

    Some(ExtractedMetadataSourceXml { relative_path, xml })
}

fn parse_metadata_object_code(text: &str) -> Option<u32> {
    let after_root = text.trim_start().strip_prefix("{1,")?;
    let after_root = after_root.trim_start();
    let after_open = after_root.strip_prefix('{')?;
    let digits = after_open
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn metadata_source_for_text(
    code: u32,
    text: &str,
    uuid: &str,
) -> Option<(&'static str, &'static str)> {
    let fields = metadata_object_fields(text)?;
    let header_index = metadata_header_field_index(&fields, uuid);

    match code {
        0 if header_index == Some(1) && field_starts_with(fields.get(2), "{0,") => {
            Some(("FunctionalOptionsParameter", "FunctionalOptionsParameters"))
        }
        0 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("Language", "Languages"))
        }
        0 if header_index == Some(1) => Some(("IntegrationService", "IntegrationServices")),
        1 if header_index == Some(1) && field_starts_with(fields.get(2), r#"{"Pattern""#) => {
            Some(("EventSubscription", "EventSubscriptions"))
        }
        1 if header_index == Some(1) && field_starts_with(fields.get(1), "{2,") => {
            Some(("SessionParameter", "SessionParameters"))
        }
        1 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("XDTOPackage", "XDTOPackages"))
        }
        1 if header_index == Some(1) => Some(("Bot", "Bots")),
        2 if contains_wrapped_metadata_object_code(text, 9, uuid) => {
            Some(("CommonCommand", "CommonCommands"))
        }
        2 if header_index == Some(2) && field_is_quoted_string(fields.get(1)) => {
            Some(("HTTPService", "HTTPServices"))
        }
        2 if header_index == Some(2) && field_starts_with(fields.get(1), "{") => {
            Some(("WSReference", "WSReferences"))
        }
        4 if header_index == Some(2) && field_is_quoted_string(fields.get(1)) => {
            Some(("WebService", "WebServices"))
        }
        2 if header_index == Some(1)
            && fields.get(2).copied().and_then(parse_uuid_field).is_some()
            && field_starts_with(fields.get(3), "{0,") =>
        {
            Some(("FunctionalOption", "FunctionalOptions"))
        }
        2 if header_index == Some(1) && field_starts_with(fields.get(1), "{0,") => {
            Some(("SettingsStorage", "SettingsStorages"))
        }
        3 if header_index == Some(6) => Some(("CommandGroup", "CommandGroups")),
        3 if header_index == Some(3) => Some(("StyleItem", "StyleItems")),
        3 if header_index == Some(1) && fields.len() == 2 => Some(("Style", "Styles")),
        3 if header_index == Some(1) => Some(("DocumentNumerator", "DocumentNumerators")),
        2 if header_index == Some(1)
            && field_is_quoted_string(fields.get(2))
            && field_is_quoted_string(fields.get(3)) =>
        {
            Some(("ScheduledJob", "ScheduledJobs"))
        }
        4 if is_common_template_metadata_fields(&fields, uuid) => {
            Some(("CommonTemplate", "CommonTemplates"))
        }
        4 if header_index == Some(1) => Some(("CommonPicture", "CommonPictures")),
        5 => Some(("CommonAttribute", "CommonAttributes")),
        6 if header_index == Some(1) => Some(("Role", "Roles")),
        6 => Some(("Sequence", "Sequences")),
        9 => Some(("CommonCommand", "CommonCommands")),
        14 => Some(("FilterCriterion", "FilterCriteria")),
        16 => Some(("Constant", "Constants")),
        17 => Some(("DataProcessor", "DataProcessors")),
        19 => Some(("Report", "Reports")),
        20 if header_index == Some(5) => Some(("Enum", "Enums")),
        20 if header_index == Some(3) => Some(("Report", "Reports")),
        21 => Some(("CalculationRegister", "CalculationRegisters")),
        22 if header_index == Some(1) => Some(("Subsystem", "Subsystems")),
        22 if field_is_unsigned_integer(fields.get(1)) => {
            Some(("AccountingRegister", "AccountingRegisters"))
        }
        26 => Some(("DocumentJournal", "DocumentJournals")),
        28 => Some(("AccumulationRegister", "AccumulationRegisters")),
        30 => Some(("BusinessProcess", "BusinessProcesses")),
        32 => Some(("ChartOfAccounts", "ChartsOfAccounts")),
        33 if header_index == Some(1) => Some(("Task", "Tasks")),
        33 => Some(("InformationRegister", "InformationRegisters")),
        34 => Some(("ChartOfCharacteristicTypes", "ChartsOfCharacteristicTypes")),
        35 => Some(("ChartOfCalculationTypes", "ChartsOfCalculationTypes")),
        37 => Some(("ExchangePlan", "ExchangePlans")),
        40 => Some(("Document", "Documents")),
        57 => Some(("Catalog", "Catalogs")),
        _ => None,
    }
}

fn is_typed_metadata_source(kind: &str) -> bool {
    matches!(kind, "SessionParameter" | "CommonAttribute")
}

fn contains_wrapped_metadata_object_code(text: &str, code: u32, uuid: &str) -> bool {
    let marker = format!("{{1,0,{uuid}}}");
    let code_marker = format!("{{{code},");
    text.contains(&marker) && text.contains(&code_marker)
}

fn is_form_metadata_text(text: &str, uuid: &str) -> bool {
    parse_metadata_object_code(text) == Some(0)
        && (contains_wrapped_metadata_object_code(text, 13, uuid)
            || contains_wrapped_metadata_object_code(text, 14, uuid))
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

fn parse_catalog_properties_from_text(
    text: &str,
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
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
    let owners_empty = parse_catalog_owners_empty(fields.get(12).copied());
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
        owners_empty,
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
    })
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

fn parse_catalog_owners_empty(field: Option<&str>) -> bool {
    field
        .map(|value| {
            value
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
                == "{0,0}"
        })
        .unwrap_or(false)
}

fn parse_catalog_form_ref(
    field: Option<&str>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<String> {
    let uuid = parse_non_zero_uuid(field?)?;
    form_refs.get(&uuid).and_then(form_source_reference_name)
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

fn catalog_standard_attribute_name(code: &str) -> Option<&'static str> {
    match code {
        "-3" => Some("Description"),
        "-2" => Some("Code"),
        _ => None,
    }
}

fn form_source_reference_name(form_ref: &FormSourceReference) -> Option<String> {
    let parts = form_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    if parts.len() == 2 && parts.first() == Some(&"CommonForms") {
        let form_name = Path::new(parts[1]).file_stem()?.to_str()?;
        return Some(format!("CommonForm.{form_name}"));
    }
    if parts.len() == 4 && parts.get(2) == Some(&"Forms") {
        let owner_kind = metadata_kind_for_source_folder(parts[0])?;
        let owner_name = parts[1];
        let form_name = Path::new(parts[3]).file_stem()?.to_str()?;
        return Some(format!("{owner_kind}.{owner_name}.Form.{form_name}"));
    }
    None
}

fn metadata_kind_for_source_folder(folder: &str) -> Option<&'static str> {
    match folder {
        "Catalogs" => Some("Catalog"),
        "Documents" => Some("Document"),
        "DocumentJournals" => Some("DocumentJournal"),
        "Enums" => Some("Enum"),
        "Reports" => Some("Report"),
        "DataProcessors" => Some("DataProcessor"),
        "InformationRegisters" => Some("InformationRegister"),
        "AccumulationRegisters" => Some("AccumulationRegister"),
        "AccountingRegisters" => Some("AccountingRegister"),
        "CalculationRegisters" => Some("CalculationRegister"),
        "ChartsOfAccounts" => Some("ChartOfAccounts"),
        "ChartsOfCharacteristicTypes" => Some("ChartOfCharacteristicTypes"),
        "ChartsOfCalculationTypes" => Some("ChartOfCalculationTypes"),
        "BusinessProcesses" => Some("BusinessProcess"),
        "Tasks" => Some("Task"),
        "ExchangePlans" => Some("ExchangePlan"),
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
    let use_standard_commands = parse_1c_bool_flag(constant_fields.get(7)?.trim())?;

    Some(ConstantProperties {
        value_type,
        use_standard_commands,
    })
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

    Some(DefinedTypeProperties { value_types })
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
        }
        if ref_fields.first()?.trim() == "-13" {
            return Some((Some("StdPicture.Print".to_string()), load_transparent));
        }
    }
    Some((None, load_transparent))
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
        _ => None,
    }
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
        _ => return None,
    };
    let order = STANDARD_STYLE_ITEM_CODES
        .iter()
        .position(|item_code| *item_code == code)?;
    Some((order, name))
}

const STANDARD_STYLE_ITEM_CODES: &[i32] = &[
    -1, -11, -3, -15, -7, -13, -21, -10, -14, -23, -24, -16, -17, -22, -25, -26, -27, -28, -18,
    -20, -30, -31, -32, -33, -34, -35, -36, -37, -38,
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
        0 => Some(format!("#{:06X}", code.max(0) as u32 & 0x00ff_ffff)),
        2 => style_web_color_name(code).map(ToOwned::to_owned),
        3 => style_system_color_name(code).map(ToOwned::to_owned),
        _ => None,
    }
}

fn style_web_color_name(code: i32) -> Option<&'static str> {
    match code {
        8 => Some("web:Black"),
        10 => Some("web:Blue"),
        20 => Some("web:Cream"),
        23 => Some("web:DarkBlue"),
        33 => Some("web:DarkRed"),
        37 => Some("web:DarkSlateGray"),
        44 => Some("web:FireBrick"),
        45 => Some("web:FloralWhite"),
        46 => Some("web:ForestGreen"),
        48 => Some("web:Gainsboro"),
        52 => Some("web:Gray"),
        53 => Some("web:Green"),
        55 => Some("web:HoneyDew"),
        67 => Some("web:LightCyan"),
        68 => Some("web:LightGoldenRod"),
        69 => Some("web:LightGoldenRodYellow"),
        71 => Some("web:LightGray"),
        72 => Some("web:LightPink"),
        79 => Some("web:LightYellow"),
        84 => Some("web:Maroon"),
        97 => Some("web:MintCream"),
        98 => Some("web:MistyRose"),
        119 => Some("web:Red"),
        120 => Some("web:RosyBrown"),
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
        -11 => Some("style:FormTextColor"),
        -3 => Some("style:ButtonBackColor"),
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
    let raw = font_fields.get(3).copied().unwrap_or("{0}");
    let raw_fields = split_1c_braced_fields(raw, 0).unwrap_or_default();
    let mut attrs = Vec::<(&str, String)>::new();
    if kind == "2" {
        if let Some(code) = raw_fields.first().map(|field| field.trim())
            && code == "-31"
        {
            attrs.push(("ref", "style:NormalTextFont".to_string()));
            attrs.push(("kind", "StyleItem".to_string()));
        }
    } else if kind == "0" {
        attrs.push(("kind", "Absolute".to_string()));
        attrs.push(("faceName", "Arial".to_string()));
    }
    if kind == "0"
        && let Some(height) = font_fields.get(2).map(|field| field.trim())
        && height != "0"
    {
        attrs.push(("height", height.to_string()));
    }
    let bold = font_fields
        .get(4)
        .and_then(|field| field.trim().parse::<i32>().ok())
        .map(|weight| weight >= 700)
        .unwrap_or(false);
    let italic = font_fields
        .get(5)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let underline = font_fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let strikeout = font_fields
        .get(7)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let scale = font_fields
        .get(9)
        .map(|field| field.trim())
        .unwrap_or("100");
    attrs.push(("bold", xml_bool(bold).to_string()));
    attrs.push(("italic", xml_bool(italic).to_string()));
    attrs.push(("underline", xml_bool(underline).to_string()));
    attrs.push(("strikeout", xml_bool(strikeout).to_string()));
    if !attrs.iter().any(|(name, _)| *name == "kind") {
        attrs.push(("kind", "StyleItem".to_string()));
    }
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
        r#""N""# if element.len() == 4 => Some(ConstantValueType::Number {
            digits: element.get(1)?.trim().parse().ok()?,
            fraction_digits: element.get(2)?.trim().parse().ok()?,
            allowed_sign_flag: element.get(3)?.trim().parse().ok()?,
        }),
        r#""D""# => Some(ConstantValueType::DateTime),
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

fn builtin_type_reference(type_id: &str) -> Option<&'static str> {
    match type_id {
        "e199ca70-93cf-46ce-a54b-6edc88c3a296" => Some("v8:ValueStorage"),
        "fc01b5df-97fe-449b-83d4-218a090e681e" => Some("v8:UUID"),
        "3ee983d7-ace7-40f9-bb7e-2e916fcddd56" => Some("v8:FixedStructure"),
        "4500381b-db30-4a10-9db4-990038032acf" => Some("v8:FixedArray"),
        "220455ea-6c85-4513-996f-bbe79ed07774" => Some("v8:FixedMap"),
        "0a52f9de-73ea-4507-81e8-66217bead73a" => Some("cfg:ExchangePlanRef"),
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

fn parse_metadata_header_from_text(text: &str, uuid: &str) -> Option<MetadataHeader> {
    let marker = format!("{{1,0,{uuid}}},");
    let mut offset = text.find(&marker)? + marker.len();
    offset = skip_ascii_ws_at(text, offset);
    let (name, consumed) = parse_1c_quoted_string_with_len(&text[offset..])?;
    offset += consumed;
    offset = expect_comma_at(text, offset)?;
    offset = skip_ascii_ws_at(text, offset);
    let synonym_end = scan_1c_braced_value(text, offset)?;
    let synonyms = parse_1c_synonyms(&text[offset..synonym_end]);
    offset = expect_comma_at(text, synonym_end)?;
    offset = skip_ascii_ws_at(text, offset);
    let (comment, _) = parse_1c_quoted_string_with_len(&text[offset..])?;

    Some(MetadataHeader {
        uuid: uuid.to_string(),
        name,
        synonyms,
        comment,
        template_type_code: template_type_code_from_metadata_text(text, uuid),
    })
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

fn format_metadata_source_xml(kind: &str, header: &MetadataHeader) -> String {
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
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.21\">\r\n\
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
    )
}

fn format_form_source_xml(kind: &str, header: &MetadataHeader) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n\
\t<{kind} uuid=\"{uuid}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{name}</Name>\r\n",
        uuid = escape_xml_text(&header.uuid),
        name = escape_xml_text(&header.name),
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
                escape_xml_text(content)
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
    xml.push_str(
        "\t\t\t<FormType>Managed</FormType>\r\n\
\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\
\t\t\t<UsePurposes>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value>\r\n\
\t\t\t</UsePurposes>\r\n\
\t\t\t<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>\r\n",
    );
    if kind == "CommonForm" {
        xml.push_str(
            "\t\t\t<UseStandardCommands>false</UseStandardCommands>\r\n\
\t\t\t<ExtendedPresentation/>\r\n\
\t\t\t<Explanation/>\r\n",
        );
    }
    xml.push_str(&format!(
        "\t\t</Properties>\r\n\
\t</{kind}>\r\n\
</MetaDataObject>"
    ));
    xml
}

fn format_catalog_source_xml(header: &MetadataHeader, catalog: &CatalogProperties) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n\
\t<Catalog uuid=\"{uuid}\">\r\n",
        uuid = escape_xml_text(&header.uuid),
    );

    if !catalog.generated_types.is_empty() {
        xml.push_str("\t\t<InternalInfo>\r\n");
        for generated_type in &catalog.generated_types {
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
    }

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
                escape_xml_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(content)
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
    if catalog.owners_empty {
        xml.push_str("\t\t\t<Owners/>\r\n");
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
\t\t\t<QuickChoice>true</QuickChoice>\r\n\
\t\t\t<ChoiceMode>BothWays</ChoiceMode>\r\n",
    );
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
    xml.push_str(
        "\t\t\t<CreateOnInput>DontUse</CreateOnInput>\r\n\
\t\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>\r\n\
\t\t\t<DataHistory>DontUse</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
    );
    xml.push_str("\t\t</Properties>\r\n\t</Catalog>\r\n</MetaDataObject>");
    xml
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
    xml.push_str(&format!(
        "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n\
\t\t\t\t\t<xr:LinkByType/>\r\n\
\t\t\t\t\t<xr:FillChecking>{}</xr:FillChecking>\r\n\
\t\t\t\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
\t\t\t\t\t<xr:FillFromFillingValue>{}</xr:FillFromFillingValue>\r\n\
\t\t\t\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
\t\t\t\t\t<xr:TypeReductionMode>{}</xr:TypeReductionMode>\r\n\
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
\t\t\t\t\t<xr:ChoiceParameterLinks/>\r\n",
        escape_xml_text(attribute.name),
        attribute.fill_checking,
        xml_bool(attribute.fill_from_filling_value),
        attribute.type_reduction_mode
    ));
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
            escape_xml_text(lang)
        ));
        xml.push_str(&format!(
            "{indent}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_text(content)
        ));
        xml.push_str(&format!("{indent}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</{name}>\r\n"));
}

fn format_template_source_xml(kind: &str, header: &MetadataHeader, template_type: &str) -> String {
    let mut xml = format_metadata_source_xml(kind, header);
    let insert = format!(
        "\t\t\t<TemplateType>{}</TemplateType>\r\n",
        escape_xml_text(template_type)
    );
    xml = xml.replace("\t\t</Properties>", &format!("{insert}\t\t</Properties>"));
    xml
}

fn format_command_group_source_xml(
    header: &MetadataHeader,
    properties: &CommandGroupProperties,
) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
\t<CommandGroup uuid=\"{}\">\r\n\
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
                escape_xml_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    xml.push_str(&format!(
        "\t\t\t<Comment>{}</Comment>\r\n\
\t\t\t<Representation>{}</Representation>\r\n",
        escape_xml_text(&header.comment),
        properties.representation
    ));
    if properties.tooltip.is_empty() {
        xml.push_str("\t\t\t<ToolTip/>\r\n");
    } else {
        xml.push_str("\t\t\t<ToolTip>\r\n");
        for (lang, content) in &properties.tooltip {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</ToolTip>\r\n");
    }
    xml.push_str("\t\t\t<Picture>\r\n");
    match &properties.picture_ref {
        Some(reference) => {
            xml.push_str(&format!(
                "\t\t\t\t<xr:Ref>{}</xr:Ref>\r\n",
                escape_xml_text(reference)
            ));
        }
        None => xml.push_str("\t\t\t\t<xr:Ref/>\r\n"),
    }
    xml.push_str(&format!(
        "\t\t\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
\t\t\t</Picture>\r\n\
\t\t\t<Category>{}</Category>\r\n\
\t\t</Properties>\r\n\
\t</CommandGroup>\r\n\
</MetaDataObject>\r\n",
        xml_bool(properties.picture_load_transparent),
        properties.category
    ));
    xml
}

fn format_style_item_source_xml(
    header: &MetadataHeader,
    properties: &StyleItemProperties,
) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
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
                escape_xml_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    xml.push_str(&format!(
        "\t\t\t<Comment>{}</Comment>\r\n\
\t\t\t<Type>{}</Type>\r\n\
\t\t\t{}\r\n\
\t\t</Properties>\r\n\
\t</StyleItem>\r\n\
</MetaDataObject>\r\n",
        escape_xml_text(&header.comment),
        properties.item_type,
        properties.value_xml
    ));
    xml
}

fn format_common_module_source_xml(header: &MetadataHeader, flags: &CommonModuleFlags) -> String {
    let mut xml = format_metadata_source_xml("CommonModule", header);
    let insert = format!(
        "\t\t\t<Global>{}</Global>\r\n\
\t\t\t<ClientManagedApplication>{}</ClientManagedApplication>\r\n\
\t\t\t<Server>{}</Server>\r\n\
\t\t\t<ExternalConnection>{}</ExternalConnection>\r\n\
\t\t\t<ClientOrdinaryApplication>{}</ClientOrdinaryApplication>\r\n\
\t\t\t<ServerCall>{}</ServerCall>\r\n\
\t\t\t<Privileged>{}</Privileged>\r\n\
\t\t\t<ReturnValuesReuse>{}</ReturnValuesReuse>\r\n",
        xml_bool(flags.global),
        xml_bool(flags.client_managed_application),
        xml_bool(flags.server),
        xml_bool(flags.external_connection),
        xml_bool(flags.client_ordinary_application),
        xml_bool(flags.server_call),
        xml_bool(flags.privileged),
        return_values_reuse_xml(flags.return_values_reuse),
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_constant_source_xml(header: &MetadataHeader, constant: &ConstantProperties) -> String {
    let mut xml = format_metadata_source_xml("Constant", header).replace(
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"",
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
    );
    let insert = format!(
        "{}\
\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
        format_constant_type_xml(&constant.value_type),
        xml_bool(constant.use_standard_commands),
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_defined_type_source_xml(
    header: &MetadataHeader,
    defined_type: &DefinedTypeProperties,
) -> String {
    let mut xml = format_metadata_source_xml("DefinedType", header).replace(
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"",
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
    );
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
) -> String {
    let mut xml = format_metadata_source_xml(kind, header).replace(
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"",
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
    );
    let insert = format_metadata_types_xml(&typed.value_types);
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_metadata_types_xml(value_types: &[ConstantValueType]) -> String {
    let mut xml = "\t\t\t<Type>\r\n".to_string();
    for value_type in value_types {
        xml.push_str(&format!(
            "\t\t\t\t<v8:Type>{}</v8:Type>\r\n",
            metadata_type_xml_name(value_type)
        ));
    }
    xml.push_str("\t\t\t</Type>\r\n");

    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length: Some(length),
            allowed_length_flag,
        } => Some((*length, *allowed_length_flag)),
        _ => None,
    }) {
        xml.push_str("\t\t\t<StringQualifiers>\r\n");
        xml.push_str(&format!("\t\t\t\t<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "\t\t\t\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str("\t\t\t</StringQualifiers>\r\n");
    }

    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str("\t\t\t<NumberQualifiers>\r\n");
        xml.push_str(&format!("\t\t\t\t<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "\t\t\t\t<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "\t\t\t\t<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str("\t\t\t</NumberQualifiers>\r\n");
    }

    xml
}

fn metadata_type_xml_name(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Boolean => "xs:boolean".to_string(),
        ConstantValueType::String { .. } => "xs:string".to_string(),
        ConstantValueType::Number { .. } => "xs:decimal".to_string(),
        ConstantValueType::DateTime => "xs:dateTime".to_string(),
        ConstantValueType::Reference { reference, .. } => reference.clone(),
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
            let mut xml =
                "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>xs:string</v8:Type>\r\n\t\t\t</Type>\r\n"
                    .to_string();
            if let Some(length) = length {
                xml.push_str("\t\t\t<StringQualifiers>\r\n");
                xml.push_str(&format!("\t\t\t\t<v8:Length>{length}</v8:Length>\r\n"));
                xml.push_str(&format!(
                    "\t\t\t\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
                    string_allowed_length_xml(*allowed_length_flag)
                ));
                xml.push_str("\t\t\t</StringQualifiers>\r\n");
            }
            xml
        }
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => format!(
            "\t\t\t<Type>\r\n\
\t\t\t\t<v8:Type>xs:decimal</v8:Type>\r\n\
\t\t\t</Type>\r\n\
\t\t\t<NumberQualifiers>\r\n\
\t\t\t\t<v8:Digits>{digits}</v8:Digits>\r\n\
\t\t\t\t<v8:FractionDigits>{fraction_digits}</v8:FractionDigits>\r\n\
\t\t\t\t<v8:AllowedSign>{}</v8:AllowedSign>\r\n\
\t\t\t</NumberQualifiers>\r\n",
            number_allowed_sign_xml(*allowed_sign_flag)
        ),
        ConstantValueType::DateTime => {
            "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>xs:dateTime</v8:Type>\r\n\t\t\t</Type>\r\n"
                .to_string()
        }
        ConstantValueType::Reference { reference, .. } => format!(
            "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>{}</v8:Type>\r\n\t\t\t</Type>\r\n",
            escape_xml_text(reference)
        ),
    }
}

fn string_allowed_length_xml(value: u8) -> &'static str {
    match value {
        1 => "Fixed",
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

fn fetch_rows(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    let sql = build_fetch_rows_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv(sqlcmd, server, &sql)?;
    let chunks = parse_config_chunk_rows(&stdout)
        .with_context(|| format!("failed to parse {table} row chunks for {database}"))?;
    assemble_config_rows(chunks)
        .with_context(|| format!("failed to assemble {table} row chunks for {database}"))
}

fn build_fetch_rows_sql(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> String {
    let filter = if selected_file_names.is_empty() {
        String::new()
    } else {
        let values = selected_file_names
            .iter()
            .map(|value| format!("N'{}'", quote_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WHERE FileName IN ({values})\n")
    };

    format!(
        "SET NOCOUNT ON;\n\
         DECLARE @chunk_size int = {chunk_size};\n\
         WITH SourceRows AS (\n\
             SELECT FileName, PartNo, DataSize, BinaryData\n\
             FROM {qualified_table}\n\
             {filter}\
         )\n\
         SELECT rows.FileName AS file_name,\n\
                rows.PartNo AS part_no,\n\
                rows.DataSize AS data_size,\n\
                chunks.chunk_index,\n\
                CONVERT(varchar(max), SUBSTRING(rows.BinaryData, chunks.chunk_index * @chunk_size + 1, @chunk_size), 2) AS binary_hex\n\
         FROM SourceRows rows\n\
         CROSS APPLY (\n\
             SELECT chunk_count = CASE\n\
                 WHEN DATALENGTH(rows.BinaryData) = 0 THEN 1\n\
                 ELSE (DATALENGTH(rows.BinaryData) + @chunk_size - 1) / @chunk_size\n\
             END\n\
         ) counts\n\
         CROSS APPLY (\n\
             SELECT TOP (counts.chunk_count)\n\
                    ROW_NUMBER() OVER (ORDER BY (SELECT NULL)) - 1 AS chunk_index\n\
             FROM sys.all_objects a CROSS JOIN sys.all_objects b\n\
         ) chunks\n\
         ORDER BY rows.FileName, rows.PartNo, chunks.chunk_index\n\
         ;",
        chunk_size = SQLCMD_BINARY_CHUNK_SIZE,
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

fn parse_config_chunk_rows(stdout: &str) -> Result<Vec<ConfigChunkRow>> {
    let mut rows = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if is_sqlcmd_header_or_separator(line) {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 5 {
            bail!(
                "unexpected sqlcmd row chunk line {}: expected 5 tab-separated fields, got {}",
                line_index + 1,
                fields.len()
            );
        }
        rows.push(ConfigChunkRow {
            file_name: fields[0].trim_end().to_string(),
            part_no: fields[1]
                .trim()
                .parse()
                .with_context(|| format!("invalid part_no on chunk line {}", line_index + 1))?,
            data_size: fields[2]
                .trim()
                .parse()
                .with_context(|| format!("invalid data_size on chunk line {}", line_index + 1))?,
            chunk_index: fields[3]
                .trim()
                .parse()
                .with_context(|| format!("invalid chunk_index on chunk line {}", line_index + 1))?,
            binary_hex: fields[4].trim_end().to_string(),
        });
    }
    Ok(rows)
}

fn is_sqlcmd_header_or_separator(line: &str) -> bool {
    if line
        .split('\t')
        .next()
        .is_some_and(|field| field.trim() == "file_name")
    {
        return true;
    }
    line.chars().all(|ch| ch == '-' || ch == '\t' || ch == ' ')
}

fn assemble_config_rows(chunks: Vec<ConfigChunkRow>) -> Result<Vec<ConfigRow>> {
    let mut parts = BTreeMap::<(String, i32), ConfigRow>::new();
    let mut expected_chunk = BTreeMap::<(String, i32), i32>::new();

    for chunk in chunks {
        let key = (chunk.file_name.clone(), chunk.part_no);
        let expected = expected_chunk.entry(key.clone()).or_insert(0);
        if chunk.chunk_index != *expected {
            bail!(
                "Config row {} part {} chunk order gap: expected {}, got {}",
                chunk.file_name,
                chunk.part_no,
                expected,
                chunk.chunk_index
            );
        }
        *expected += 1;

        parts
            .entry(key)
            .and_modify(|row| {
                row.binary_hex.push_str(&chunk.binary_hex);
            })
            .or_insert_with(|| ConfigRow {
                file_name: chunk.file_name,
                part_no: chunk.part_no,
                data_size: chunk.data_size,
                binary_hex: chunk.binary_hex,
            });
    }

    let mut rows = BTreeMap::<String, ConfigRow>::new();
    let mut expected_part = BTreeMap::<String, i32>::new();
    for part in parts.into_values() {
        let expected = expected_part.entry(part.file_name.clone()).or_insert(0);
        if part.part_no != *expected {
            bail!(
                "Config row {} part order gap: expected {}, got {}",
                part.file_name,
                expected,
                part.part_no
            );
        }
        *expected += 1;

        rows.entry(part.file_name.clone())
            .and_modify(|row| {
                if row.data_size != part.data_size {
                    row.data_size = part.data_size;
                }
                row.binary_hex.push_str(&part.binary_hex);
            })
            .or_insert_with(|| ConfigRow {
                file_name: part.file_name,
                part_no: 0,
                data_size: part.data_size,
                binary_hex: part.binary_hex,
            });
    }

    for row in rows.values() {
        let binary_bytes = row.binary_hex.len() / 2;
        if binary_bytes != row.data_size as usize {
            bail!(
                "Config row {} DataSize {} does not match assembled BinaryData length {}",
                row.file_name,
                row.data_size,
                binary_bytes
            );
        }
    }

    Ok(rows.into_values().collect())
}

const SQLCMD_BINARY_CHUNK_SIZE: usize = 16 * 1024;

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

fn run_sql_capture_tsv(sqlcmd: &Path, server: &str, sql: &str) -> Result<String> {
    let output = Command::new(sqlcmd)
        .arg("-C")
        .arg("-S")
        .arg(server)
        .arg("-s")
        .arg("\t")
        .arg("-w")
        .arg("65535")
        .arg("-y")
        .arg("0")
        .arg("-Y")
        .arg("0")
        .arg("-Q")
        .arg(sql)
        .output()
        .with_context(|| format!("failed to run {}", sqlcmd.display()))?;
    if !output.status.success() {
        bail!(
            "sqlcmd failed with exit code {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
fn normalize_sqlcmd_json(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

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
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    use crate::module_blob::{
        ReturnValuesReuse, pack_module_blob_bytes, pack_moxel_spreadsheet_blob_from_xml,
        pack_simple_metadata_blob_from_xml, parse_common_module_xml_properties,
        parse_simple_metadata_xml_properties,
    };

    #[test]
    fn decodes_plain_hex_and_sql_hex() {
        assert_eq!(decode_hex("efbbbf").unwrap(), vec![0xef, 0xbb, 0xbf]);
        assert_eq!(decode_hex("0x0102ff").unwrap(), vec![1, 2, 255]);
    }

    #[test]
    fn rejects_odd_hex_length() {
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn extracts_json_array_from_sqlcmd_noise() {
        let stdout = "Changed database context.\r\n[{\"a\":1}]\r\n(1 row affected)";
        assert_eq!(extract_json_array(stdout, "test").unwrap(), r#"[{"a":1}]"#);
    }

    #[test]
    fn normalizes_wrapped_sqlcmd_json() {
        let stdout = "Changed database context.\r\n[{\"binary_hex\":\"AABB\r\nCCDD\"}]\r\n";
        let json = extract_json_array(stdout, "test").unwrap();
        let normalized = normalize_sqlcmd_json(&json);
        let rows: Vec<serde_json::Value> = serde_json::from_str(&normalized).unwrap();

        assert_eq!(rows[0]["binary_hex"], "AABBCCDD");
    }

    #[test]
    fn fetch_rows_sql_chunks_large_binary_values() {
        let selected = BTreeSet::from(["aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()]);
        let sql = build_fetch_rows_sql("TestDb", "Config", &selected);

        assert!(sql.contains("DECLARE @chunk_size int = 16384"));
        assert!(sql.contains("FROM [TestDb].dbo.[Config]"));
        assert!(sql.contains("SUBSTRING(rows.BinaryData"));
        assert!(sql.contains("chunks.chunk_index * @chunk_size + 1"));
        assert!(sql.contains("WHERE FileName IN (N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa')"));
        assert!(sql.contains("ORDER BY rows.FileName, rows.PartNo, chunks.chunk_index"));
        assert!(!sql.contains("FOR JSON"));
    }

    #[test]
    fn parses_config_chunk_rows_from_sqlcmd_tsv() {
        let rows = parse_config_chunk_rows(
            "file_name\tpart_no\tdata_size\tchunk_index\tbinary_hex\r\n\
             ---------\t-------\t---------\t-----------\t----------\r\n\
             large   \t0\t4\t0\tAABB   \r\n\
             large\t0\t4\t1\tCCDD\r\n",
        )
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].file_name, "large");
        assert_eq!(rows[0].part_no, 0);
        assert_eq!(rows[0].data_size, 4);
        assert_eq!(rows[0].chunk_index, 0);
        assert_eq!(rows[0].binary_hex, "AABB");
        assert_eq!(rows[1].chunk_index, 1);
        assert_eq!(rows[1].binary_hex, "CCDD");
    }

    #[test]
    fn assembles_config_rows_from_ordered_chunks() {
        let rows = assemble_config_rows(vec![
            ConfigChunkRow {
                file_name: "large".to_string(),
                part_no: 0,
                data_size: 4,
                chunk_index: 0,
                binary_hex: "AABB".to_string(),
            },
            ConfigChunkRow {
                file_name: "large".to_string(),
                part_no: 0,
                data_size: 4,
                chunk_index: 1,
                binary_hex: "CCDD".to_string(),
            },
            ConfigChunkRow {
                file_name: "small".to_string(),
                part_no: 0,
                data_size: 1,
                chunk_index: 0,
                binary_hex: "EE".to_string(),
            },
        ])
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].file_name, "large");
        assert_eq!(rows[0].binary_hex, "AABBCCDD");
        assert_eq!(rows[1].file_name, "small");
        assert_eq!(rows[1].binary_hex, "EE");
    }

    #[test]
    fn assembles_config_rows_from_multiple_physical_parts() {
        let rows = assemble_config_rows(vec![
            ConfigChunkRow {
                file_name: "large".to_string(),
                part_no: 0,
                data_size: 4,
                chunk_index: 0,
                binary_hex: "AABB".to_string(),
            },
            ConfigChunkRow {
                file_name: "large".to_string(),
                part_no: 1,
                data_size: 4,
                chunk_index: 0,
                binary_hex: "CCDD".to_string(),
            },
        ])
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].file_name, "large");
        assert_eq!(rows[0].part_no, 0);
        assert_eq!(rows[0].data_size, 4);
        assert_eq!(rows[0].binary_hex, "AABBCCDD");
    }

    #[test]
    fn rejects_config_row_chunk_order_gaps() {
        let err = assemble_config_rows(vec![ConfigChunkRow {
            file_name: "large".to_string(),
            part_no: 0,
            data_size: 4,
            chunk_index: 1,
            binary_hex: "AABB".to_string(),
        }])
        .unwrap_err();

        assert!(err.to_string().contains("chunk order gap"));
    }

    #[test]
    fn sanitizes_storage_file_names() {
        assert_eq!(
            safe_storage_file_name("abc/def:ghi", 0),
            "abc_def_ghi__part0"
        );
        assert_eq!(safe_storage_file_name("", 2), "row__part2");
    }

    #[test]
    fn expands_selected_file_names_with_module_pairs() {
        let selected = expand_selected_file_names(&[
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.2".to_string(),
            "".to_string(),
        ]);

        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.1"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.2"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.3"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.15"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.16"));
        assert!(selected.contains("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"));
        assert!(selected.contains("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.2"));
        assert_eq!(selected.len(), 13);
    }

    #[test]
    fn maps_additional_object_family_module_suffixes_to_source_layout() {
        let cases = [
            (
                "Report",
                "Reports",
                "Sales",
                "0",
                PathBuf::from("Reports/Sales/Ext/ObjectModule.bsl"),
            ),
            (
                "Report",
                "Reports",
                "Sales",
                "2",
                PathBuf::from("Reports/Sales/Ext/ManagerModule.bsl"),
            ),
            (
                "DataProcessor",
                "DataProcessors",
                "Import",
                "0",
                PathBuf::from("DataProcessors/Import/Ext/ObjectModule.bsl"),
            ),
            (
                "DataProcessor",
                "DataProcessors",
                "Import",
                "2",
                PathBuf::from("DataProcessors/Import/Ext/ManagerModule.bsl"),
            ),
            (
                "Document",
                "Documents",
                "Invoice",
                "0",
                PathBuf::from("Documents/Invoice/Ext/ObjectModule.bsl"),
            ),
            (
                "Document",
                "Documents",
                "Invoice",
                "2",
                PathBuf::from("Documents/Invoice/Ext/ManagerModule.bsl"),
            ),
            (
                "InformationRegister",
                "InformationRegisters",
                "Prices",
                "1",
                PathBuf::from("InformationRegisters/Prices/Ext/RecordSetModule.bsl"),
            ),
            (
                "AccumulationRegister",
                "AccumulationRegisters",
                "Sales",
                "1",
                PathBuf::from("AccumulationRegisters/Sales/Ext/RecordSetModule.bsl"),
            ),
            (
                "AccumulationRegister",
                "AccumulationRegisters",
                "Sales",
                "2",
                PathBuf::from("AccumulationRegisters/Sales/Ext/ManagerModule.bsl"),
            ),
            (
                "DocumentJournal",
                "DocumentJournals",
                "Interactions",
                "1",
                PathBuf::from("DocumentJournals/Interactions/Ext/ManagerModule.bsl"),
            ),
            (
                "SettingsStorage",
                "SettingsStorages",
                "ReportVariants",
                "8",
                PathBuf::from("SettingsStorages/ReportVariants/Ext/ManagerModule.bsl"),
            ),
            (
                "Enum",
                "Enums",
                "Status",
                "0",
                PathBuf::from("Enums/Status/Ext/ManagerModule.bsl"),
            ),
            (
                "Task",
                "Tasks",
                "PerformerTask",
                "6",
                PathBuf::from("Tasks/PerformerTask/Ext/ObjectModule.bsl"),
            ),
            (
                "Task",
                "Tasks",
                "PerformerTask",
                "7",
                PathBuf::from("Tasks/PerformerTask/Ext/ManagerModule.bsl"),
            ),
            (
                "BusinessProcess",
                "BusinessProcesses",
                "Task",
                "6",
                PathBuf::from("BusinessProcesses/Task/Ext/ObjectModule.bsl"),
            ),
            (
                "BusinessProcess",
                "BusinessProcesses",
                "Task",
                "8",
                PathBuf::from("BusinessProcesses/Task/Ext/ManagerModule.bsl"),
            ),
            (
                "ChartOfCharacteristicTypes",
                "ChartsOfCharacteristicTypes",
                "Kinds",
                "15",
                PathBuf::from("ChartsOfCharacteristicTypes/Kinds/Ext/ObjectModule.bsl"),
            ),
            (
                "ChartOfCharacteristicTypes",
                "ChartsOfCharacteristicTypes",
                "Kinds",
                "16",
                PathBuf::from("ChartsOfCharacteristicTypes/Kinds/Ext/ManagerModule.bsl"),
            ),
            (
                "CommonCommand",
                "CommonCommands",
                "OpenSettings",
                "2",
                PathBuf::from("CommonCommands/OpenSettings/Ext/CommandModule.bsl"),
            ),
            (
                "Constant",
                "Constants",
                "UseFeature",
                "0",
                PathBuf::from("Constants/UseFeature/Ext/ValueManagerModule.bsl"),
            ),
            (
                "Constant",
                "Constants",
                "UseFeature",
                "1",
                PathBuf::from("Constants/UseFeature/Ext/ManagerModule.bsl"),
            ),
            (
                "Bot",
                "Bots",
                "Notify",
                "1",
                PathBuf::from("Bots/Notify/Ext/Module.bsl"),
            ),
            (
                "IntegrationService",
                "IntegrationServices",
                "MessageExchange",
                "0",
                PathBuf::from("IntegrationServices/MessageExchange/Ext/Module.bsl"),
            ),
            (
                "Sequence",
                "Sequences",
                "Documents",
                "0",
                PathBuf::from("Sequences/Documents/Ext/RecordSetModule.bsl"),
            ),
        ];

        for (kind, folder, name, suffix, expected) in cases {
            assert_eq!(
                module_owner_source_path(kind, folder, name, suffix),
                Some(expected)
            );
        }
    }

    #[test]
    fn writes_document_additional_indexes_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let document_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{40,\r\n{{3,\r\n{{1,0,{uuid}}},\"Order\",{{1,\"en\",\"Order\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0}},0}}"
            )
            .as_bytes(),
        );
        let additional_indexes =
            b"\xef\xbb\xbf<AdditionalIndexes><AdditionalIndex id=\"idx\"/></AdditionalIndexes>"
                .to_vec();
        let additional_indexes_blob = deflate_for_test(&additional_indexes);
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: document_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&document_metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.3"),
                part_no: 0,
                data_size: additional_indexes_blob.len() as i64,
                binary_hex: encode_hex_for_test(&additional_indexes_blob),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        assert_eq!(
            fs::read(root.join("Documents/Order/Ext/AdditionalIndexes.xml")).unwrap(),
            additional_indexes
        );
        let row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.3"))
            .unwrap();
        assert_eq!(
            row.source_asset_path.as_deref(),
            Some("Documents/Order/Ext/AdditionalIndexes.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn quotes_sql_string_literals() {
        assert_eq!(quote_string("a'b"), "a''b");
    }

    #[test]
    fn extracts_module_text_from_dumped_rows() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let packed = pack_module_blob_bytes(text, None, None).unwrap();
        let row = ConfigRow {
            file_name: "module-id.0".to_string(),
            part_no: 0,
            data_size: packed.blob.len() as i64,
            binary_hex: encode_hex_for_test(&packed.blob),
        };

        let dumped = dump_table_rows(&root, "Config", vec![row], false, true, false).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        let module_text_path = dumped.rows[0].module_text_path.as_ref().unwrap();
        let written = fs::read(root.join(module_text_path)).unwrap();
        assert_eq!(written, text);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dumps_table_rows_preserve_input_order() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let rows = vec![
            ConfigRow {
                file_name: "third".to_string(),
                part_no: 0,
                data_size: 3,
                binary_hex: "010203".to_string(),
            },
            ConfigRow {
                file_name: "first".to_string(),
                part_no: 0,
                data_size: 1,
                binary_hex: "04".to_string(),
            },
            ConfigRow {
                file_name: "second".to_string(),
                part_no: 0,
                data_size: 2,
                binary_hex: "0506".to_string(),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, false).unwrap();

        assert_eq!(
            dumped
                .rows
                .iter()
                .map(|row| row.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["third", "first", "second"]
        );
        assert_eq!(dumped.binary_bytes, 6);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_source_asset_primary_paths() {
        let source_assets = BTreeMap::from([
            (
                "first".to_string(),
                SourceAsset {
                    primary_path: PathBuf::from("Shared/Ext/Template.xml"),
                    kind: SourceAssetKind::Binary,
                },
            ),
            (
                "second".to_string(),
                SourceAsset {
                    primary_path: PathBuf::from("Shared/Ext/Template.xml"),
                    kind: SourceAssetKind::Binary,
                },
            ),
        ]);

        let error = ensure_unique_source_asset_paths(&source_assets)
            .expect_err("duplicate source asset paths must be rejected");

        assert!(error.to_string().contains("produced by both"));
    }

    #[test]
    fn writes_common_module_text_to_source_layout_when_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{12,\r\n{{3,\r\n{{1,0,{uuid}}},\"TestModule\",{{0}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let body = pack_module_blob_bytes(text, None, None).unwrap().blob;
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, false).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        let expected = PathBuf::from("CommonModules")
            .join("TestModule")
            .join("Ext")
            .join("Module.bsl");
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.module_text_path.as_deref(),
            Some("CommonModules/TestModule/Ext/Module.bsl")
        );
        assert_eq!(fs::read(root.join(expected)).unwrap(), text);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_configuration_module_text_to_source_layout_without_metadata_row() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let ordinary_text = b"Procedure OnStart()\r\nEndProcedure\r\n";
        let external_text = b"Procedure OnConnect()\r\nEndProcedure\r\n";
        let managed_text = b"Procedure BeforeStart()\r\nEndProcedure\r\n";
        let session_text = b"Procedure SetSessionParameters(Names)\r\nEndProcedure\r\n";
        let ordinary_body = pack_module_blob_bytes(ordinary_text, None, None)
            .unwrap()
            .blob;
        let external_body = pack_module_blob_bytes(external_text, None, None)
            .unwrap()
            .blob;
        let managed_body = pack_module_blob_bytes(managed_text, None, None)
            .unwrap()
            .blob;
        let session_body = pack_module_blob_bytes(session_text, None, None)
            .unwrap()
            .blob;
        let png = b"\x89PNG\r\n\x1a\n";
        let splash_blob = deflate_for_test(b"{1,{0,0,-1,-1},{{#base64:iVBORw0KGgo=}}}");
        let parent_blob = b"parent-cf".to_vec();
        let home_page_work_area = b"\xEF\xBB\xBF<HomePageWorkArea/>".to_vec();
        let home_page_work_area_blob = deflate_for_test(&home_page_work_area);
        let mobile_signature = b"\xEF\xBB\xBF{2,\"\",\"\",{0},0}".to_vec();
        let mobile_signature_blob = deflate_for_test(&mobile_signature);
        let main_section_command_interface = deflate_for_test(
            b"{7,1,1,{0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{{0,{{0,{{\"B\",1}},0}}}},0,0,0}",
        );
        let command_interface = deflate_for_test(
            b"{7,1,1,{0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{{0,{{0,{{\"B\",1}},0}}}},0,0,0}",
        );
        let client_application_interface = b"\xEF\xBB\xBF<ClientApplicationInterface/>".to_vec();
        let client_application_interface_blob = deflate_for_test(&client_application_interface);
        let main_picture_blob = deflate_for_test(b"{1,{0,0,-1,-1},{{#base64:iVBORw0KGgo=}}}");
        let standalone_configuration_content = b"\xEF\xBB\xBF<StandaloneContent/>".to_vec();
        let standalone_configuration_content_blob =
            deflate_for_test(&standalone_configuration_content);
        let rows = vec![
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: ordinary_body.len() as i64,
                binary_hex: encode_hex_for_test(&ordinary_body),
            },
            ConfigRow {
                file_name: format!("{uuid}.2"),
                part_no: 0,
                data_size: splash_blob.len() as i64,
                binary_hex: encode_hex_for_test(&splash_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.4"),
                part_no: 0,
                data_size: parent_blob.len() as i64,
                binary_hex: encode_hex_for_test(&parent_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.8"),
                part_no: 0,
                data_size: home_page_work_area_blob.len() as i64,
                binary_hex: encode_hex_for_test(&home_page_work_area_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.5"),
                part_no: 0,
                data_size: external_body.len() as i64,
                binary_hex: encode_hex_for_test(&external_body),
            },
            ConfigRow {
                file_name: format!("{uuid}.6"),
                part_no: 0,
                data_size: managed_body.len() as i64,
                binary_hex: encode_hex_for_test(&managed_body),
            },
            ConfigRow {
                file_name: format!("{uuid}.7"),
                part_no: 0,
                data_size: session_body.len() as i64,
                binary_hex: encode_hex_for_test(&session_body),
            },
            ConfigRow {
                file_name: format!("{uuid}.10"),
                part_no: 0,
                data_size: mobile_signature_blob.len() as i64,
                binary_hex: encode_hex_for_test(&mobile_signature_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.9"),
                part_no: 0,
                data_size: main_section_command_interface.len() as i64,
                binary_hex: encode_hex_for_test(&main_section_command_interface),
            },
            ConfigRow {
                file_name: format!("{uuid}.a"),
                part_no: 0,
                data_size: command_interface.len() as i64,
                binary_hex: encode_hex_for_test(&command_interface),
            },
            ConfigRow {
                file_name: format!("{uuid}.b"),
                part_no: 0,
                data_size: client_application_interface_blob.len() as i64,
                binary_hex: encode_hex_for_test(&client_application_interface_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.c"),
                part_no: 0,
                data_size: main_picture_blob.len() as i64,
                binary_hex: encode_hex_for_test(&main_picture_blob),
            },
            ConfigRow {
                file_name: format!("{uuid}.f"),
                part_no: 0,
                data_size: standalone_configuration_content_blob.len() as i64,
                binary_hex: encode_hex_for_test(&standalone_configuration_content_blob),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, false).unwrap();

        assert_eq!(dumped.module_text_rows, 4);
        assert_eq!(dumped.source_asset_rows, 9);
        assert_eq!(
            fs::read(root.join("Ext/OrdinaryApplicationModule.bsl")).unwrap(),
            ordinary_text
        );
        assert_eq!(
            fs::read(root.join("Ext/ExternalConnectionModule.bsl")).unwrap(),
            external_text
        );
        assert_eq!(
            fs::read(root.join("Ext/ManagedApplicationModule.bsl")).unwrap(),
            managed_text
        );
        assert_eq!(
            fs::read(root.join("Ext/SessionModule.bsl")).unwrap(),
            session_text
        );
        assert_eq!(fs::read(root.join("Ext/Splash/Picture.png")).unwrap(), png);
        assert_eq!(
            fs::read(root.join("Ext/MainSectionPicture/Picture.png")).unwrap(),
            png
        );
        assert_eq!(
            fs::read(root.join("Ext/ParentConfigurations.bin")).unwrap(),
            parent_blob
        );
        assert_eq!(
            fs::read(root.join("Ext/HomePageWorkArea.xml")).unwrap(),
            home_page_work_area
        );
        assert_eq!(
            fs::read(root.join("Ext/MobileClientSignature.bin")).unwrap(),
            mobile_signature
        );
        assert_eq!(
            fs::read(root.join("Ext/ClientApplicationInterface.xml")).unwrap(),
            client_application_interface
        );
        assert_eq!(
            fs::read(root.join("Ext/StandaloneConfigurationContent.bin")).unwrap(),
            standalone_configuration_content
        );
        let main_section_xml =
            fs::read_to_string(root.join("Ext/MainSectionCommandInterface.xml")).unwrap();
        assert!(main_section_xml.contains("<CommandInterface"));
        assert!(
            main_section_xml.contains(r#"<Command name="0:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">"#)
        );
        let command_interface_xml =
            fs::read_to_string(root.join("Ext/CommandInterface.xml")).unwrap();
        assert!(command_interface_xml.contains("<CommandInterface"));
        assert!(
            command_interface_xml
                .contains(r#"<Command name="0:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">"#)
        );
        assert!(
            fs::read_to_string(root.join("Ext/Splash.xml"))
                .unwrap()
                .contains("<xr:Abs>Picture.png</xr:Abs>")
        );

        let splash_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.2"))
            .unwrap();
        let parent_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.4"))
            .unwrap();
        let home_page_work_area_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.8"))
            .unwrap();
        let main_picture_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.c"))
            .unwrap();
        let mobile_signature_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.10"))
            .unwrap();
        let main_section_interface_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.9"))
            .unwrap();
        let command_interface_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.a"))
            .unwrap();
        let client_application_interface_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.b"))
            .unwrap();
        let standalone_configuration_content_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.f"))
            .unwrap();
        assert_eq!(
            splash_row.source_asset_path.as_deref(),
            Some("Ext/Splash.xml")
        );
        assert_eq!(
            parent_row.source_asset_path.as_deref(),
            Some("Ext/ParentConfigurations.bin")
        );
        assert_eq!(
            home_page_work_area_row.source_asset_path.as_deref(),
            Some("Ext/HomePageWorkArea.xml")
        );
        assert_eq!(
            main_picture_row.source_asset_path.as_deref(),
            Some("Ext/MainSectionPicture.xml")
        );
        assert_eq!(
            mobile_signature_row.source_asset_path.as_deref(),
            Some("Ext/MobileClientSignature.bin")
        );
        assert_eq!(
            main_section_interface_row.source_asset_path.as_deref(),
            Some("Ext/MainSectionCommandInterface.xml")
        );
        assert_eq!(
            command_interface_row.source_asset_path.as_deref(),
            Some("Ext/CommandInterface.xml")
        );
        assert_eq!(
            client_application_interface_row
                .source_asset_path
                .as_deref(),
            Some("Ext/ClientApplicationInterface.xml")
        );
        assert_eq!(
            standalone_configuration_content_row
                .source_asset_path
                .as_deref(),
            Some("Ext/StandaloneConfigurationContent.bin")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_service_module_text_to_source_layout_when_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let http_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let http_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\"api\",\r\n{{3,\r\n{{1,0,{http_uuid}}},\"Api\",{{1,\"en\",\"API\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},2,20}},0}}"
            )
            .as_bytes(),
        );
        let web_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let web_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\"http://example.com\",\r\n{{3,\r\n{{1,0,{web_uuid}}},\"Exchange\",{{1,\"en\",\"Exchange\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{0,0}},\"exchange.1cws\",\r\n{{0}},0,20}},0}}"
            )
            .as_bytes(),
        );
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let body = pack_module_blob_bytes(text, None, None).unwrap().blob;
        let rows = vec![
            ConfigRow {
                file_name: http_uuid.to_string(),
                part_no: 0,
                data_size: http_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&http_metadata),
            },
            ConfigRow {
                file_name: format!("{http_uuid}.0"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
            ConfigRow {
                file_name: web_uuid.to_string(),
                part_no: 0,
                data_size: web_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&web_metadata),
            },
            ConfigRow {
                file_name: format!("{web_uuid}.0"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 2);
        assert!(root.join("HTTPServices/Api/Ext/Module.bsl").exists());
        assert!(root.join("WebServices/Exchange/Ext/Module.bsl").exists());
        let http_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{http_uuid}.0"))
            .unwrap();
        let web_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{web_uuid}.0"))
            .unwrap();
        assert_eq!(
            http_row.module_text_path.as_deref(),
            Some("HTTPServices/Api/Ext/Module.bsl")
        );
        assert_eq!(
            web_row.module_text_path.as_deref(),
            Some("WebServices/Exchange/Ext/Module.bsl")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_common_command_module_text_to_source_layout_when_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{1,\r\n{{2,{uuid},078a6af8-d22c-4248-9c33-7e90075a3d2c}},\r\n{{9,\r\n{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},3,\r\n{{1,\"en\",\"Open settings\"}},1,\r\n{{0,0,0}},0,\r\n{{1,aabb34e1-98c1-4bd0-bf7f-243f95437b44}},\r\n{{\"Pattern\"}},\r\n{{3,\r\n{{1,0,{uuid}}},\"OpenSettings\",{{1,\"en\",\"Open settings\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let text = b"Procedure Run(CommandParameter)\r\nEndProcedure\r\n";
        let body = pack_module_blob_bytes(text, None, None).unwrap().blob;
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.2"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        let expected = root.join("CommonCommands/OpenSettings/Ext/CommandModule.bsl");
        assert_eq!(fs::read(expected).unwrap(), text);
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.2"))
            .unwrap();
        assert_eq!(
            body_row.module_text_path.as_deref(),
            Some("CommonCommands/OpenSettings/Ext/CommandModule.bsl")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_nested_command_module_text_to_source_layout_when_owner_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let owner_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let command_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{17,835478cc-434a-480c-ad61-99801cd685ed,92b15a50-2234-40c9-af13-3d746d4b870f,\r\n{{0,\r\n{{3,\r\n{{1,0,{owner_uuid}}},\"Scanning\",{{1,\"en\",\"Scanning\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},00000000-0000-0000-0000-000000000000,1,0,86df3c66-2c45-49c1-9e7d-5d1892acb646,6ae2f4ed-a57a-49ed-a854-8795bf1e1519,00000000-0000-0000-0000-000000000000,\r\n{{0}},\r\n{{0}}\r\n}},5,\r\n{{45556acb-826a-4f73-898a-6025fc9536e1,1,\r\n{{\r\n{{0,\r\n{{1,\r\n{{2,{command_uuid},078a6af8-d22c-4248-9c33-7e90075a3d2c}},\r\n{{9,\r\n{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},3,\r\n{{1,\"en\",\"Scan sheet\"}},1,\r\n{{0,0,0}},0,\r\n{{1,bc80566a-86a5-4e87-acd4-872239385a2e}},\r\n{{\"Pattern\"}},\r\n{{3,\r\n{{1,0,{command_uuid}}},\"ScanSheet\",{{1,\"en\",\"Scan sheet\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0}}\r\n}}\r\n}},0}}\r\n}}\r\n}},\r\n{{d5b0e5ed-256d-401c-9c36-f630cafd8a62,3,0f193c89-b664-448e-bed3-2147430367f7,4c9b2506-75a8-47d3-a5d5-d946088ba14a,36eacaa1-2efd-49c0-82de-2f8972535bf2}},\r\n{{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,0}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let text = b"Procedure Run(CommandParameter)\r\nEndProcedure\r\n";
        let body = pack_module_blob_bytes(text, None, None).unwrap().blob;
        let rows = vec![
            ConfigRow {
                file_name: owner_uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{command_uuid}.2"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        let expected =
            root.join("DataProcessors/Scanning/Commands/ScanSheet/Ext/CommandModule.bsl");
        assert_eq!(fs::read(expected).unwrap(), text);
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{command_uuid}.2"))
            .unwrap();
        assert_eq!(
            body_row.module_text_path.as_deref(),
            Some("DataProcessors/Scanning/Commands/ScanSheet/Ext/CommandModule.bsl")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_constant_module_text_to_source_layout_when_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{16,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"Feature flag\",0,0,00000000-0000-0000-0000-000000000000,0}},{{\"Pattern\",{{\"B\"}}}}\r\n}},0,\r\n{{0}},\r\n{{0}},0,\"\",0,\r\n{{\"U\"}},\r\n{{\"U\"}},0,00000000-0000-0000-0000-000000000000,2,0,\r\n{{5006,0}},\r\n{{3,0,0}},\r\n{{0,0}},0,\r\n{{0}},\r\n{{\"S\",\"\"}},0,0,0}},00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,1,\r\n{{0}},1,0}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let value_text = b"Procedure BeforeWrite(Value, StandardProcessing)\r\nEndProcedure\r\n";
        let manager_text = b"Procedure SetDefault()\r\nEndProcedure\r\n";
        let value_body = pack_module_blob_bytes(value_text, None, None).unwrap().blob;
        let manager_body = pack_module_blob_bytes(manager_text, None, None)
            .unwrap()
            .blob;
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: value_body.len() as i64,
                binary_hex: encode_hex_for_test(&value_body),
            },
            ConfigRow {
                file_name: format!("{uuid}.1"),
                part_no: 0,
                data_size: manager_body.len() as i64,
                binary_hex: encode_hex_for_test(&manager_body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 2);
        assert_eq!(
            fs::read(root.join("Constants/UseFeature/Ext/ValueManagerModule.bsl")).unwrap(),
            value_text
        );
        assert_eq!(
            fs::read(root.join("Constants/UseFeature/Ext/ManagerModule.bsl")).unwrap(),
            manager_text
        );
        let value_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.0"))
            .unwrap();
        let manager_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.1"))
            .unwrap();
        assert_eq!(
            value_row.module_text_path.as_deref(),
            Some("Constants/UseFeature/Ext/ValueManagerModule.bsl")
        );
        assert_eq!(
            manager_row.module_text_path.as_deref(),
            Some("Constants/UseFeature/Ext/ManagerModule.bsl")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_object_family_module_text_to_source_layout_when_metadata_is_present() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let exchange_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let exchange_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{37,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{0,\r\n{{3,\r\n{{1,0,{exchange_uuid}}},\"Exchange\",{{1,\"en\",\"Exchange\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let register_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let register_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{33,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,66666666-6666-4666-8666-666666666666,77777777-7777-4777-8777-777777777777,88888888-8888-4888-8888-888888888888,99999999-9999-4999-8999-999999999999,aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee,bbbbbbbb-1111-4111-8111-111111111111,cccccccc-2222-4222-8222-222222222222,dddddddd-3333-4333-8333-333333333333,eeeeeeee-4444-4444-8444-444444444444,\r\n{{0,\r\n{{3,\r\n{{1,0,{register_uuid}}},\"Settings\",{{1,\"en\",\"Settings\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let report_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let report_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{19,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{0,\r\n{{3,\r\n{{1,0,{report_uuid}}},\"LateTasks\",{{1,\"en\",\"Late tasks\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let body = pack_module_blob_bytes(text, None, None).unwrap().blob;
        let mut rows = Vec::new();
        for (file_name, data) in [
            (catalog_uuid.to_string(), catalog_metadata.clone()),
            (format!("{catalog_uuid}.0"), body.clone()),
            (format!("{catalog_uuid}.3"), body.clone()),
            (exchange_uuid.to_string(), exchange_metadata.clone()),
            (format!("{exchange_uuid}.2"), body.clone()),
            (format!("{exchange_uuid}.3"), body.clone()),
            (register_uuid.to_string(), register_metadata.clone()),
            (format!("{register_uuid}.2"), body.clone()),
            (report_uuid.to_string(), report_metadata.clone()),
            (format!("{report_uuid}.0"), body.clone()),
            (format!("{report_uuid}.2"), body.clone()),
        ] {
            rows.push(ConfigRow {
                file_name,
                part_no: 0,
                data_size: data.len() as i64,
                binary_hex: encode_hex_for_test(&data),
            });
        }

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 7);
        assert!(root.join("Catalogs/Products/Ext/ObjectModule.bsl").exists());
        assert!(
            root.join("Catalogs/Products/Ext/ManagerModule.bsl")
                .exists()
        );
        assert!(
            root.join("ExchangePlans/Exchange/Ext/ObjectModule.bsl")
                .exists()
        );
        assert!(
            root.join("ExchangePlans/Exchange/Ext/ManagerModule.bsl")
                .exists()
        );
        assert!(
            root.join("InformationRegisters/Settings/Ext/ManagerModule.bsl")
                .exists()
        );
        assert!(root.join("Reports/LateTasks/Ext/ObjectModule.bsl").exists());
        assert!(
            root.join("Reports/LateTasks/Ext/ManagerModule.bsl")
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_simple_metadata_xml_from_recognized_blob() {
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let blob = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"SalesCatalog\",{{2,\"ru\",\"Продажи\",\"en\",\"Sales\"}},\"Comment\"}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("Catalogs").join("SalesCatalog.xml")
        );
        assert_eq!(properties.kind, "Catalog");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "SalesCatalog");
        assert_eq!(properties.comment, "Comment");
        assert_eq!(properties.synonyms.len(), 2);
    }

    #[test]
    fn writes_form_metadata_xml_to_owner_or_common_form_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let second_catalog_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let owned_form_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let common_form_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0,{owned_form_uuid},{common_form_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let second_catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{second_catalog_uuid}}},\"Services\",{{1,\"en\",\"Services\"}},\"\"}}\r\n}},0,{common_form_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let owned_form_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{14,\r\n{{3,\r\n{{1,0,{owned_form_uuid}}},\"ListForm\",{{1,\"en\",\"List form\"}},\"\"}},0,1,{{2,{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}},{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let common_form_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{common_form_uuid}}},\"SharedForm\",{{1,\"en\",\"Shared form\"}},\"\"}},0,1,{{2,{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}},{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: owned_form_uuid.to_string(),
                part_no: 0,
                data_size: owned_form_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&owned_form_metadata),
            },
            ConfigRow {
                file_name: second_catalog_uuid.to_string(),
                part_no: 0,
                data_size: second_catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&second_catalog_metadata),
            },
            ConfigRow {
                file_name: common_form_uuid.to_string(),
                part_no: 0,
                data_size: common_form_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&common_form_metadata),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 4);
        let owned_xml_bytes = fs::read(root.join("Catalogs/Products/Forms/ListForm.xml")).unwrap();
        assert!(owned_xml_bytes.starts_with(b"\xEF\xBB\xBF<?xml"));
        assert!(owned_xml_bytes.ends_with(b"</MetaDataObject>"));
        assert!(!owned_xml_bytes.ends_with(b"</MetaDataObject>\r\n"));
        let owned_xml = String::from_utf8(owned_xml_bytes).unwrap();
        let common_xml = fs::read_to_string(root.join("CommonForms/SharedForm.xml")).unwrap();
        assert!(owned_xml.contains("<Form uuid=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\">"));
        assert!(owned_xml.contains(r#"version="2.21""#));
        assert!(
            owned_xml.contains(r#"xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config""#)
        );
        assert!(owned_xml.contains(r#"xmlns:xr="http://v8.1c.ru/8.3/xcf/readable""#));
        assert!(owned_xml.contains("<Comment/>"));
        assert!(owned_xml.contains("<FormType>Managed</FormType>"));
        assert!(
            owned_xml
                .contains("<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>")
        );
        assert!(common_xml.contains("<CommonForm uuid=\"cccccccc-cccc-4ccc-cccc-cccccccccccc\">"));
        assert!(common_xml.contains(r#"version="2.21""#));
        assert!(
            common_xml
                .contains("<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>")
        );
        assert!(common_xml.contains("<UseStandardCommands>false</UseStandardCommands>"));
        assert!(!root.join("Catalogs/Products/Forms/SharedForm.xml").exists());
        assert!(!root.join("Catalogs/Services/Forms/SharedForm.xml").exists());

        let owned_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == owned_form_uuid)
            .unwrap();
        assert_eq!(
            owned_row.metadata_xml_path.as_deref(),
            Some("Catalogs/Products/Forms/ListForm.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_form_module_text_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let form_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0,{form_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let form_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{form_uuid}}},\"ListForm\",{{1,\"en\",\"List form\"}},\"\"}},0,1,{{0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let form_body = deflate_for_test(
            b"{4,{0},\"Procedure Run()\r\n\tMessage(\"\"Hi\"\");\r\nEndProcedure\r\n\",{0}}",
        );
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: form_uuid.to_string(),
                part_no: 0,
                data_size: form_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&form_metadata),
            },
            ConfigRow {
                file_name: format!("{form_uuid}.0"),
                part_no: 0,
                data_size: form_body.len() as i64,
                binary_hex: encode_hex_for_test(&form_body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, true).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        let module_text =
            fs::read_to_string(root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Module.bsl"))
                .unwrap();
        assert_eq!(
            module_text,
            "\u{feff}Procedure Run()\r\n\tMessage(\"Hi\");\r\nEndProcedure\r\n"
        );
        let form_xml =
            fs::read_to_string(root.join("Catalogs/Products/Forms/ListForm/Ext/Form.xml")).unwrap();
        assert!(form_xml.contains("<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\""));
        assert!(form_xml.contains("version=\"2.20\""));
        assert!(!form_xml.contains("<Events>"));
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{form_uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.module_text_path.as_deref(),
            Some("Catalogs/Products/Forms/ListForm/Ext/Form/Module.bsl")
        );
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("Catalogs/Products/Forms/ListForm/Ext/Form.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_form_events_to_body_xml() {
        let form_body = deflate_for_test(
            b"{4,{7,{0,\"OnOpen\",\"PriOtkrytii\"},{1,\"ChoiceProcessing\",\"ObrabotkaVybora\"},{2,\"AfterWrite\",\"PosleZapisi\"},{3,\"OnLoadDataFromSettingsAtServer\",\"LoadSettings\"},{4,\"FillCheckProcessingAtServer\",\"FillCheck\"},{5,\"NotAFormEvent\",\"Ignored\"},{6,\"OnClose\",\"\"}},\"\",{0}}",
        );

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(form_xml.contains("<Events>"));
        assert!(form_xml.contains(r#"<Event name="OnOpen">PriOtkrytii</Event>"#));
        assert!(form_xml.contains(r#"<Event name="ChoiceProcessing">ObrabotkaVybora</Event>"#));
        assert!(form_xml.contains(r#"<Event name="AfterWrite">PosleZapisi</Event>"#));
        assert!(
            form_xml
                .contains(r#"<Event name="OnLoadDataFromSettingsAtServer">LoadSettings</Event>"#)
        );
        assert!(
            form_xml.contains(r#"<Event name="FillCheckProcessingAtServer">FillCheck</Event>"#)
        );
        assert!(!form_xml.contains("NotAFormEvent"));
        assert!(!form_xml.contains(r#"<Event name="OnClose">"#));
    }

    #[test]
    fn extracts_form_uuid_events_and_auto_command_bar_to_body_xml() {
        let unknown_event_uuid = "213d1900-dcad-4616-9f20-3f077156a40f";
        let form_body = deflate_for_test(
            format!(r#"{{4,{{59,{{3,1d632984-de3c-4b4b-ad9f-d69682a10182,"ОбработкаВыбора",3699f6a3-9a2a-4c82-a775-6ff4824a08ca,"ОбработкаОповещения",9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,"ПриСозданииНаСервере",{unknown_event_uuid},"ПослеЗаписиНаСервере",1,0,1d632984-de3c-4b4b-ad9f-d69682a10182,0,1,3699f6a3-9a2a-4c82-a775-6ff4824a08ca,0,1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1}},{{22,{{-1,02023637-7868-4a5f-8576-835a76e0c9ba}},0,0,0,9,"ФормаКоманднаяПанель",{{1,0}}}}}},"",{{0}}}}"#).as_bytes(),
        );

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(form_xml.contains(r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#));
        assert!(form_xml.contains(r#"<Event name="ChoiceProcessing">ОбработкаВыбора</Event>"#));
        assert!(
            form_xml
                .contains(r#"<Event name="NotificationProcessing">ОбработкаОповещения</Event>"#)
        );
        assert!(
            form_xml.contains(r#"<Event name="OnCreateAtServer">ПриСозданииНаСервере</Event>"#)
        );
        assert!(form_xml.contains(&format!(
            r#"<Event name="{unknown_event_uuid}">ПослеЗаписиНаСервере</Event>"#
        )));
    }

    #[test]
    fn extracts_form_auto_command_bar_autofill_false() {
        let form_body = deflate_for_test(
            r#"{4,{59,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"ФормаКоманднаяПанель",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{1,2,0,0},0,1,0,0,0,3,3,0}},"",{0}}"#.as_bytes(),
        );

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(form_xml.contains(r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1">"#));
        assert!(form_xml.contains("<HorizontalAlign>Right</HorizontalAlign>"));
        assert!(form_xml.contains("<Autofill>false</Autofill>"));
    }

    #[test]
    fn extracts_form_top_level_title_to_body_xml() {
        let form_body = deflate_for_test(
            r#"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,2,{"ru","Сохранение настройки"},{"en","Save settings"}},0,0,1,1,1,0,1,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"ФормаКоманднаяПанель",{1,0}}},"",{0}}"#.as_bytes(),
        );

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(form_xml.contains("<Title>"));
        assert!(form_xml.contains("<v8:lang>ru</v8:lang>"));
        assert!(form_xml.contains("<v8:content>Сохранение настройки</v8:content>"));
        assert!(form_xml.contains("<v8:lang>en</v8:lang>"));
        assert!(form_xml.contains("<v8:content>Save settings</v8:content>"));
    }

    #[test]
    fn extracts_form_top_level_properties_to_body_xml() {
        let form_body = deflate_for_test(
            r#"{4,{59,0,2,80,30,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,0},1,1,0,0,1,0,3,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"ФормаКоманднаяПанель",{1,0}}},"",{0}}"#.as_bytes(),
        );

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(form_xml.contains("<Width>80</Width>"));
        assert!(form_xml.contains("<Height>30</Height>"));
        assert!(form_xml.contains("<WindowOpeningMode>LockWholeInterface</WindowOpeningMode>"));
        assert!(form_xml.contains("<AutoTitle>false</AutoTitle>"));
        assert!(form_xml.contains("<Group>Horizontal</Group>"));
        assert!(form_xml.contains("<CommandBarLocation>Bottom</CommandBarLocation>"));
    }

    #[test]
    fn extracts_form_attributes_and_commands_from_body_tail() {
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let option_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let parameter_type_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let form_body = deflate_for_test(
            format!(
                r##"{{4,{{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{{1,0}},0,0,1,1,1,0,1,1,1}},"",{{4,1,{{9,{{1}},0,"Список",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},1,0,0,0,{{0,9,"QueryText",{{"S","ВЫБРАТЬ Ссылка, Наименование ИЗ Справочник.Товары"}},"MainTable",{{"#",fc01b5df-97fe-449b-83d4-218a090e681e,{catalog_uuid}}},"DynamicalDataSelection",{{"B",0}},"ManualQuery",{{"B",1}},"Filter",{{"#",21743ff3-2db3-4cfc-9404-90ed8209437f,{{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxGaWx0ZXIgeG1sbnM9Imh0dHA6Ly92OC4xYy5ydS84LjEvZGF0YS1jb21wb3NpdGlvbi1zeXN0ZW0vc2V0dGluZ3MiIHhtbG5zOnhzPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYSIgeG1sbnM6eHNpPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYS1pbnN0YW5jZSI+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+ZGZjZWNlOWQtNTA3Ny00NDBiLWI2YjMtNDVhNWNiNDUzOGViPC91c2VyU2V0dGluZ0lEPg0KPC9GaWx0ZXI+}}}},"Order",{{"#",11743ff3-2db3-4cfc-9404-90ed8209437f,{{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxPcmRlciB4bWxucz0iaHR0cDovL3Y4LjFjLnJ1LzguMS9kYXRhLWNvbXBvc2l0aW9uLXN5c3RlbS9zZXR0aW5ncyIgeG1sbnM6eHM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDEvWE1MU2NoZW1hIiB4bWxuczp4c2k9Imh0dHA6Ly93d3cudzMub3JnLzIwMDEvWE1MU2NoZW1hLWluc3RhbmNlIj4NCgk8aXRlbSB4c2k6dHlwZT0iT3JkZXJJdGVtRmllbGQiPg0KCQk8ZmllbGQ+0J3QsNC40LzQtdC90L7QstCw0L3QuNC10J/QvtC70L3QvtC1PC9maWVsZD4NCgkJPG9yZGVyVHlwZT5Bc2M8L29yZGVyVHlwZT4NCgk8L2l0ZW0+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+ODg2MTk3NjUtY2NiMy00NmM2LWFjNTItMzhlOWM5OTJlYmQ0PC91c2VyU2V0dGluZ0lEPg0KPC9PcmRlcj4=}}}},"ConditionalAppearance",{{"#",31743ff3-2db3-4cfc-9404-90ed8209437f,{{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxDb25kaXRpb25hbEFwcGVhcmFuY2UgeG1sbnM9Imh0dHA6Ly92OC4xYy5ydS84LjEvZGF0YS1jb21wb3NpdGlvbi1zeXN0ZW0vc2V0dGluZ3MiIHhtbG5zOnhzPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYSIgeG1sbnM6eHNpPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYS1pbnN0YW5jZSI+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+Yjc1ZmVjY2UtOTQyYi00YWVkLWFiYzktZTZhMDJlNDYwZmIzPC91c2VyU2V0dGluZ0lEPg0KPC9Db25kaXRpb25hbEFwcGVhcmFuY2U+}}}},"ItemsViewMode",{{"S","Normal"}},"ItemsUserSettingID",{{"S","911b6018-f537-43e8-a417-da56b22f9aec"}}}},{{0,0}}}}}},{{0,1,{{0,"Счет",{{"Pattern",{{"#",{parameter_type_uuid}}}}},1}}}},{{0,1,{{11,{{2,409b9a53-7f7e-4178-86c1-33176c7c7a7a}},"Выполнить",{{1,1,{{"ru","Выполнить"}}}},{{1,1,{{"ru","Выполнить действие"}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0,0}},{{4,0,{{0}},"",-1,-1,1,0,""}},"Выполнить",3,0,0,{{0,1,{option_uuid}}},1,0,1,0,0,1,0,0}}}},{{0}},0,0}}"##
            )
            .as_bytes(),
        );
        let object_refs = BTreeMap::from([
            (catalog_uuid.to_string(), "Catalog.Товары".to_string()),
            (
                option_uuid.to_string(),
                "FunctionalOption.ИспользоватьФункцию".to_string(),
            ),
            (
                parameter_type_uuid.to_string(),
                "cfg:ChartOfAccountsRef.Хозрасчетный".to_string(),
            ),
        ]);

        let form_xml = extract_form_body_xml(&form_body, &object_refs).unwrap();

        assert!(form_xml.contains(r#"<Attribute name="Список" id="1">"#));
        assert!(form_xml.contains("<v8:Type>cfg:DynamicList</v8:Type>"));
        assert!(form_xml.contains("<MainAttribute>true</MainAttribute>"));
        assert!(form_xml.contains("<Field>Список.Наименование</Field>"));
        assert!(form_xml.contains("<Field>Список.Ссылка</Field>"));
        assert!(form_xml.contains("<ManualQuery>true</ManualQuery>"));
        assert!(form_xml.contains("<DynamicDataRead>true</DynamicDataRead>"));
        assert!(form_xml.contains("<MainTable>Catalog.Товары</MainTable>"));
        assert!(form_xml.contains("<ListSettings>"));
        assert!(form_xml.contains("<dcsset:filter>"));
        assert!(form_xml.contains(
            "<dcsset:userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</dcsset:userSettingID>"
        ));
        assert!(form_xml.contains("<dcsset:order>"));
        assert!(form_xml.contains("<dcsset:field>НаименованиеПолное</dcsset:field>"));
        assert!(form_xml.contains("<dcsset:orderType>Asc</dcsset:orderType>"));
        assert!(form_xml.contains("<dcsset:viewMode>Normal</dcsset:viewMode>"));
        assert!(form_xml.contains(
            "<dcsset:userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</dcsset:userSettingID>"
        ));
        assert!(form_xml.contains("<dcsset:conditionalAppearance>"));
        assert!(form_xml.contains(
            "<dcsset:userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</dcsset:userSettingID>"
        ));
        assert!(form_xml.contains("<dcsset:itemsViewMode>Normal</dcsset:itemsViewMode>"));
        assert!(form_xml.contains(
            "<dcsset:itemsUserSettingID>911b6018-f537-43e8-a417-da56b22f9aec</dcsset:itemsUserSettingID>"
        ));
        assert!(form_xml.contains(r#"<Parameter name="Счет">"#));
        assert!(form_xml.contains("<v8:Type>cfg:ChartOfAccountsRef.Хозрасчетный</v8:Type>"));
        assert!(form_xml.contains("<KeyParameter>true</KeyParameter>"));
        assert!(form_xml.contains(r#"<Command name="Выполнить" id="2">"#));
        assert!(form_xml.contains("<Action>Выполнить</Action>"));
        assert!(form_xml.contains("<Item>FunctionalOption.ИспользоватьФункцию</Item>"));
        assert!(form_xml.contains("<CurrentRowUse>DontUse</CurrentRowUse>"));
    }

    #[test]
    fn extracts_form_child_items_from_layout_pairs() {
        let form_uuid = "02023637-7868-4a5f-8576-835a76e0c9ba";
        let external_command_uuid = "11111111-1111-4111-8111-111111111111";
        let layout = format!(
            r#"{{59,2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{{22,{{64,{form_uuid}}},0,0,0,0,"Панель",{{1,0}},0,1,1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,{{34,{{44,{form_uuid}}},0,0,0,"Выполнить",{{1,0}},1,{{0,{external_command_uuid}}},{{2,{{25}},{{40}}}}}}}},cccccccc-cccc-4ccc-cccc-cccccccccccc,{{73,{{25,{form_uuid}}},0,1,0,"СписокТаблица",0,0,0,{{1,0}},1,dddddddd-dddd-4ddd-dddd-dddddddddddd,{{48,{{40,{form_uuid}}},0,0,0,2,"Наименование",1,0,{{1,0}},"OnChange","NameChanged","StartChoice","NameChoice","ValueChoice","NameValueChoice",213d1900-dcad-4616-9f20-3f077156a40f,"NameUuidEvent"}},"OnGetDataAtServer","RowsGetData"}}}}"#
        );
        let layout_fields = split_1c_braced_fields(&layout, 0).unwrap();
        let attributes = vec![FormAttribute {
            id: "1".to_string(),
            name: "Список".to_string(),
            main_attribute: true,
            use_always: Vec::new(),
            settings: None,
        }];
        let object_refs = BTreeMap::from([(
            external_command_uuid.to_string(),
            "DataProcessor.Loader.Command.Load".to_string(),
        )]);

        let items = extract_form_child_items(&layout_fields, &attributes, &[], &object_refs);
        let xml = format_form_child_items_xml(&items, 1);

        assert!(xml.contains(r#"<CommandBar name="Панель" id="64">"#));
        assert!(xml.contains(r#"<Button name="Выполнить" id="44">"#));
        assert!(xml.contains("<Type>CommandBarButton</Type>"));
        assert!(xml.contains("<CommandName>DataProcessor.Loader.Command.Load</CommandName>"));
        assert!(xml.contains("<DataPath>Items.СписокТаблица.CurrentData.Наименование</DataPath>"));
        assert!(xml.contains(r#"<Table name="СписокТаблица" id="25">"#));
        assert!(xml.contains("<DataPath>Список</DataPath>"));
        assert!(xml.contains(r#"<InputField name="Наименование" id="40">"#));
        assert!(xml.contains("<DataPath>Список.Наименование</DataPath>"));
        assert_eq!(
            xml.matches(r#"<Event name="OnGetDataAtServer">RowsGetData</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            xml.matches(r#"<Event name="OnChange">NameChanged</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            xml.matches(r#"<Event name="StartChoice">NameChoice</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            xml.matches(r#"<Event name="ValueChoice">NameValueChoice</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            xml.matches(
                r#"<Event name="213d1900-dcad-4616-9f20-3f077156a40f">NameUuidEvent</Event>"#
            )
            .count(),
            1
        );
    }

    #[test]
    fn extracts_form_body_xml_keeps_child_events_nested() {
        let form_uuid = "02023637-7868-4a5f-8576-835a76e0c9ba";
        let layout = format!(
            r#"{{59,1,cccccccc-cccc-4ccc-cccc-cccccccccccc,{{73,{{25,{form_uuid}}},0,1,0,"Список",0,0,0,{{1,0}},1,dddddddd-dddd-4ddd-dddd-dddddddddddd,{{48,{{40,{form_uuid}}},0,0,0,2,"Наименование",1,0,{{1,0}},"OnChange","NameChanged",213d1900-dcad-4616-9f20-3f077156a40f,"NameUuidEvent"}},{{1,97365900-eadf-4dfd-a9aa-fbb9ecabd079,"RowsGetData",1,0,97365900-eadf-4dfd-a9aa-fbb9ecabd079,0,1}}}}}}"#
        );
        let form_body = deflate_for_test(format!(r#"{{4,{layout},"",{{0}}}}"#).as_bytes());

        let form_xml = extract_form_body_xml(&form_body, &BTreeMap::new()).unwrap();

        assert!(!form_xml.contains("\r\n\t<Events>\r\n"));
        assert_eq!(
            form_xml
                .matches(r#"<Event name="OnGetDataAtServer">RowsGetData</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            form_xml
                .matches(r#"<Event name="OnChange">NameChanged</Event>"#)
                .count(),
            1
        );
        assert_eq!(
            form_xml
                .matches(
                    r#"<Event name="213d1900-dcad-4616-9f20-3f077156a40f">NameUuidEvent</Event>"#
                )
                .count(),
            1
        );
    }

    #[test]
    fn extracts_form_button_type_from_layout_code() {
        for (code, expected_type) in [
            ("0", "UsualButton"),
            ("1", "CommandBarButton"),
            ("2", "Hyperlink"),
        ] {
            let item = parse_form_child_item(
                &format!(
                    r#"{{34,{{44,02023637-7868-4a5f-8576-835a76e0c9ba}},0,0,0,"Button",{{1,0}},{code},{{0}},{{0}}}}"#
                ),
                None,
                None,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &[],
                &BTreeMap::new(),
            )
            .unwrap();

            assert_eq!(item.item_type, Some(expected_type));
        }
    }

    #[test]
    fn extracts_form_search_addition_type_from_layout_code() {
        let mut items = Vec::new();
        let table_name_by_id = BTreeMap::from([("25".to_string(), "Rows".to_string())]);
        for (code, expected_tag, expected_type) in [
            ("0", "SearchStringAddition", "SearchStringRepresentation"),
            ("1", "ViewStatusAddition", "ViewStatusRepresentation"),
            ("2", "SearchControlAddition", "SearchControl"),
        ] {
            let item = parse_form_child_item(
                &format!(
                    r#"{{6,{{44,02023637-7868-4a5f-8576-835a76e0c9ba}},0,0,0,{code},"SearchAddition",{{1,0}},{{1,0}},1,1,0,1,{{1,0}},0,0,0,0,0,{{25,{code}}}}}"#
                ),
                None,
                None,
                &table_name_by_id,
                &BTreeMap::new(),
                &[],
                &BTreeMap::new(),
            )
            .unwrap();

            assert_eq!(item.tag, expected_tag);
            assert_eq!(item.item_type, Some(expected_type));
            assert_eq!(item.addition_source_item.as_deref(), Some("Rows"));
            items.push(item);
        }

        let xml = format_form_child_items_xml(&items, 1);

        assert!(xml.contains("<SearchStringAddition"));
        assert_eq!(xml.matches("<Item>Rows</Item>").count(), 3);
        assert!(xml.contains("<Type>SearchStringRepresentation</Type>"));
        assert!(xml.contains("<ViewStatusAddition"));
        assert!(xml.contains("<Type>ViewStatusRepresentation</Type>"));
        assert!(xml.contains("<SearchControlAddition"));
        assert!(xml.contains("<Type>SearchControl</Type>"));
    }

    #[test]
    fn extracts_form_command_interface_navigation_panel() {
        let first_command_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let second_command_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let form_body = deflate_for_test(
            format!(
                r#"{{4,{{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1}}, "",{{0}},{{0,0}},{{0,0}},{{0,2,{{3,0,{{0,{first_command_uuid}}},{{0}},1,{{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1}},1,0,{{0,{{0,{{"B",0}},0}}}}}},{{3,1,{{0,{second_command_uuid}}},{{0}},1,{{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1}},0,0,{{0,{{0,{{"B",0}},0}}}}}}}},{{0}},0,0}}"#
            )
            .as_bytes(),
        );
        let object_refs = BTreeMap::from([
            (
                first_command_uuid.to_string(),
                "DataProcessor.Loader.Command.Load".to_string(),
            ),
            (
                second_command_uuid.to_string(),
                "InformationRegister.Rates.Command.Import".to_string(),
            ),
        ]);

        let form_xml = extract_form_body_xml(&form_body, &object_refs).unwrap();

        assert!(form_xml.contains("<CommandInterface>"));
        assert!(form_xml.contains("<NavigationPanel>"));
        assert!(form_xml.contains("<Command>DataProcessor.Loader.Command.Load</Command>"));
        assert!(form_xml.contains("<Command>InformationRegister.Rates.Command.Import</Command>"));
        assert_eq!(
            form_xml
                .matches("<CommandGroup>FormNavigationPanelGoTo</CommandGroup>")
                .count(),
            2
        );
        assert!(form_xml.contains("<Index>1</Index>"));
        assert_eq!(
            form_xml
                .matches("<DefaultVisible>false</DefaultVisible>")
                .count(),
            2
        );
        assert_eq!(form_xml.matches("<xr:Common>false</xr:Common>").count(), 2);
    }

    #[test]
    fn writes_form_item_pictures_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let form_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0,{form_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let form_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{form_uuid}}},\"ListForm\",{{1,\"en\",\"List form\"}},\"\"}},0,1,{{0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let form_body = deflate_for_test(
            "{4,{0},\"\",{2,{31,{59,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,\"ДеревоТоваров\",{1,0},{0},\"\",-1,-1,0,{#base64:iVBORw0KGgo=}},{31,{60,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,\"ДеревоТоваровАвторегистрация\",{1,0},{0},\"\",-1,-1,0,{#base64:iVBORw0KGgo=}}}}".as_bytes(),
        );
        let assets = extract_form_item_assets(&form_body);
        assert_eq!(
            assets
                .iter()
                .map(|asset| (asset.item_name.as_str(), asset.file_name.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("ДеревоТоваров", "RowsPicture.png"),
                ("ДеревоТоваровАвторегистрация", "ValuesPicture.png")
            ]
        );
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: form_uuid.to_string(),
                part_no: 0,
                data_size: form_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&form_metadata),
            },
            ConfigRow {
                file_name: format!("{form_uuid}.0"),
                part_no: 0,
                data_size: form_body.len() as i64,
                binary_hex: encode_hex_for_test(&form_body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        assert!(
            root.join(
                "Catalogs/Products/Forms/ListForm/Ext/Form/Items/ДеревоТоваров/RowsPicture.png"
            )
            .exists()
        );
        assert!(root
            .join("Catalogs/Products/Forms/ListForm/Ext/Form/Items/ДеревоТоваровАвторегистрация/ValuesPicture.png")
            .exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recognizes_form_item_picture_asset_formats() {
        assert!(is_form_item_picture_content(b"\x89PNG\r\n\x1a\npayload"));
        assert!(is_form_item_picture_content(b"GIF87apayload"));
        assert!(is_form_item_picture_content(b"\x00\x00\x01\x00payload"));
        assert!(is_form_item_picture_content(b"\xff\xd8\xff\xe0payload"));
        assert!(is_form_item_picture_content(b"BMpayload"));
        assert!(is_form_item_picture_content(b"<svg/>"));
        assert!(is_form_item_picture_content(
            b"<?xml version=\"1.0\"?><svg/>"
        ));
        assert!(!is_form_item_picture_content(b"plain text"));
    }

    #[test]
    fn extracts_chart_of_characteristic_types_xml_from_metadata_blob() {
        let uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let blob = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{34,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"ExpenseItems\",{{1,\"en\",\"Expense items\"}},\"\"}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("ChartsOfCharacteristicTypes").join("ExpenseItems.xml")
        );
        assert_eq!(properties.kind, "ChartOfCharacteristicTypes");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "ExpenseItems");
    }

    #[test]
    fn extracts_common_command_xml_from_metadata_blob() {
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{1,\r\n{{2,{uuid},078a6af8-d22c-4248-9c33-7e90075a3d2c}},\r\n{{9,\r\n{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},3,\r\n{{1,\"en\",\"Open settings\"}},1,\r\n{{0,0,0}},0,\r\n{{1,aabb34e1-98c1-4bd0-bf7f-243f95437b44}},\r\n{{\"Pattern\"}},\r\n{{3,\r\n{{1,0,{uuid}}},\"OpenSettings\",{{1,\"en\",\"Open settings\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("CommonCommands").join("OpenSettings.xml")
        );
        assert_eq!(properties.kind, "CommonCommand");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "OpenSettings");
    }

    #[test]
    fn writes_command_group_metadata_xml_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let group_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let picture_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let group_metadata = deflate_for_test(
            format!(
                "{{1,{{3,{{4,1,{{0,{picture_uuid}}},\"\",-1,-1,0,0,\"\"}},8,1,{{0}},{{0}},{{3,{{1,0,{group_uuid}}},\"Admin\",{{1,\"en\",\"Admin\"}},\"\"}}}},0}}"
            )
            .as_bytes(),
        );
        let picture_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\r\n{{3,\r\n{{1,0,{picture_uuid}}},\"AdminPicture\",{{1,\"en\",\"Admin picture\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0}},0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: picture_uuid.to_string(),
                part_no: 0,
                data_size: picture_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&picture_metadata),
            },
            ConfigRow {
                file_name: group_uuid.to_string(),
                part_no: 0,
                data_size: group_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&group_metadata),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 2);
        let xml = fs::read_to_string(root.join("CommandGroups/Admin.xml")).unwrap();
        assert!(xml.contains(r#"<CommandGroup uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">"#));
        assert!(xml.contains("<Representation>Picture</Representation>"));
        assert!(xml.contains("<xr:Ref>CommonPicture.AdminPicture</xr:Ref>"));
        assert!(xml.contains("<xr:LoadTransparent>false</xr:LoadTransparent>"));
        assert!(xml.contains("<Category>FormCommandBar</Category>"));
        let group_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == group_uuid)
            .unwrap();
        assert_eq!(
            group_row.metadata_xml_path.as_deref(),
            Some("CommandGroups/Admin.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_style_item_metadata_xml_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let color_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let font_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let color_metadata = deflate_for_test(
            format!(
                "{{1,{{3,0,{{\"#\",9cd510c7-abfc-11d4-9434-004095e12fc7,2,{{3,2,{{37}}}}}},{{3,{{1,0,{color_uuid}}},\"DoneColor\",{{1,\"en\",\"Done color\"}},\"\"}}}},0}}"
            )
            .as_bytes(),
        );
        let font_metadata = deflate_for_test(
            format!(
                "{{1,{{3,1,{{\"#\",9cd510c8-abfc-11d4-9434-004095e12fc7,1,{{7,2,60,{{-31}},700,0,0,0,1,100}},0}},{{3,{{1,0,{font_uuid}}},\"ImportantFont\",{{1,\"en\",\"Important font\"}},\"\"}}}},0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: color_uuid.to_string(),
                part_no: 0,
                data_size: color_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&color_metadata),
            },
            ConfigRow {
                file_name: font_uuid.to_string(),
                part_no: 0,
                data_size: font_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&font_metadata),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 2);
        let color_xml = fs::read_to_string(root.join("StyleItems/DoneColor.xml")).unwrap();
        assert!(color_xml.contains("<Type>Color</Type>"));
        assert!(color_xml.contains(r#"<Value xsi:type="v8ui:Color">web:DarkSlateGray</Value>"#));
        let font_xml = fs::read_to_string(root.join("StyleItems/ImportantFont.xml")).unwrap();
        assert!(font_xml.contains("<Type>Font</Type>"));
        assert!(font_xml.contains(r#"<Value xsi:type="v8ui:Font" ref="style:NormalTextFont""#));
        assert!(font_xml.contains(r#"bold="true""#));
        let color_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == color_uuid)
            .unwrap();
        assert_eq!(
            color_row.metadata_xml_path.as_deref(),
            Some("StyleItems/DoneColor.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_common_picture_xml_and_asset_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let text_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\r\n{{3,\r\n{{1,0,{uuid}}},\"Address\",{{1,\"en\",\"Address\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0}},0}}"
            )
            .as_bytes(),
        );
        let text_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\r\n{{3,\r\n{{1,0,{text_uuid}}},\"DocumentKinds\",{{1,\"en\",\"Document kinds\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0}},0}}"
            )
            .as_bytes(),
        );
        let zip = b"PK\x03\x04";
        let picture = deflate_for_test(b"{1,{0,0,-1,-1},{{#base64:UEsDBA==}}}");
        let text_picture = deflate_for_test(b"1;Passport;Pass\r\n");
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: picture.len() as i64,
                binary_hex: encode_hex_for_test(&picture),
            },
            ConfigRow {
                file_name: text_uuid.to_string(),
                part_no: 0,
                data_size: text_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&text_metadata),
            },
            ConfigRow {
                file_name: format!("{text_uuid}.0"),
                part_no: 0,
                data_size: text_picture.len() as i64,
                binary_hex: encode_hex_for_test(&text_picture),
            },
        ];
        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 2);
        assert_eq!(dumped.source_asset_rows, 2);
        assert!(root.join("CommonPictures/Address.xml").exists());
        assert_eq!(
            fs::read(root.join("CommonPictures/Address/Ext/Picture/Picture.zip")).unwrap(),
            zip
        );
        assert!(
            fs::read_to_string(root.join("CommonPictures/Address/Ext/Picture.xml"))
                .unwrap()
                .contains("<xr:Abs>Picture.zip</xr:Abs>")
        );
        assert_eq!(
            fs::read(root.join("CommonPictures/DocumentKinds/Ext/Picture/Picture.txt")).unwrap(),
            b"1;Passport;Pass\r\n"
        );
        assert!(
            fs::read_to_string(root.join("CommonPictures/DocumentKinds/Ext/Picture.xml"))
                .unwrap()
                .contains("<xr:Abs>Picture.txt</xr:Abs>")
        );
        let metadata_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == uuid)
            .unwrap();
        let picture_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.0"))
            .unwrap();
        assert_eq!(
            metadata_row.metadata_xml_path.as_deref(),
            Some("CommonPictures/Address.xml")
        );
        assert_eq!(
            picture_row.source_asset_path.as_deref(),
            Some("CommonPictures/Address/Ext/Picture.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_scheduled_job_schedule_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"LoadRates\",{{1,\"en\",\"Load rates\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\"\",\"Load rates\",1,1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,\"LoadRates\",3,10}},0}}"
            )
            .as_bytes(),
        );
        let schedule = deflate_for_test(
            b"{00010101000000,00010101000000,00010101080000,00010101170000,00010101000000,0,60,0,2,6,7,0,1,12,1,2,3,4,5,6,7,8,9,10,11,12,1,0}",
        );
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: schedule.len() as i64,
                binary_hex: encode_hex_for_test(&schedule),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        let xml =
            fs::read_to_string(root.join("ScheduledJobs/LoadRates/Ext/Schedule.xml")).unwrap();
        assert!(xml.contains("BeginTime=\"08:00:00\""));
        assert!(xml.contains("EndTime=\"17:00:00\""));
        assert!(xml.contains("RepeatPeriodInDay=\"60\""));
        assert!(xml.contains("<ent:WeekDays>6 7</ent:WeekDays>"));
        assert!(xml.contains("<ent:Months>1 2 3 4 5 6 7 8 9 10 11 12</ent:Months>"));
        let schedule_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.0"))
            .unwrap();
        assert_eq!(
            schedule_row.source_asset_path.as_deref(),
            Some("ScheduledJobs/LoadRates/Ext/Schedule.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_help_xml_and_html_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let help = deflate_for_test(
            b"{5,1,\"ru\",{#base64:PGh0bWw+PC9odG1sPg==},1,\"shot.png\",1,{#base64:iVBORw0KGgo=}}",
        );
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.5"),
                part_no: 0,
                data_size: help.len() as i64,
                binary_hex: encode_hex_for_test(&help),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        let help_xml = fs::read(root.join("Catalogs/Products/Ext/Help.xml")).unwrap();
        assert!(help_xml.starts_with(b"\xEF\xBB\xBF<?xml"));
        assert!(help_xml.ends_with(b"</Help>"));
        assert!(!help_xml.ends_with(b"</Help>\r\n"));
        let help_xml_text = String::from_utf8(help_xml).unwrap();
        assert!(help_xml_text.contains(r#"version="2.21""#));
        assert!(help_xml_text.contains("<Page>ru</Page>"));
        assert_eq!(
            fs::read(root.join("Catalogs/Products/Ext/Help/ru.html")).unwrap(),
            b"<html></html>"
        );
        assert_eq!(
            fs::read(root.join("Catalogs/Products/Ext/Help/_files/shot.png")).unwrap(),
            b"\x89PNG\r\n\x1a\n"
        );
        let help_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.5"))
            .unwrap();
        assert_eq!(
            help_row.source_asset_path.as_deref(),
            Some("Catalogs/Products/Ext/Help.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prefers_new_object_help_suffix_when_legacy_help_blob_remains() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let legacy_help = deflate_for_test(b"{5,1,\"ru\",{#base64:b2xk}}");
        let current_help = deflate_for_test(b"{5,1,\"ru\",{#base64:bmV3}}");
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.1"),
                part_no: 0,
                data_size: legacy_help.len() as i64,
                binary_hex: encode_hex_for_test(&legacy_help),
            },
            ConfigRow {
                file_name: format!("{uuid}.5"),
                part_no: 0,
                data_size: current_help.len() as i64,
                binary_hex: encode_hex_for_test(&current_help),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        assert_eq!(
            fs::read(root.join("Catalogs/Products/Ext/Help/ru.html")).unwrap(),
            b"new"
        );
        assert_eq!(
            dumped
                .rows
                .iter()
                .find(|row| row.file_name == format!("{uuid}.1"))
                .unwrap()
                .source_asset_path,
            None
        );
        assert_eq!(
            dumped
                .rows
                .iter()
                .find(|row| row.file_name == format!("{uuid}.5"))
                .unwrap()
                .source_asset_path
                .as_deref(),
            Some("Catalogs/Products/Ext/Help.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_predefined_data_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let type_uuid = "ae135932-4f94-44df-92c1-c91f15a92848";
        let folder_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let item_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let predefined = deflate_for_test(
            format!(
                "{{0,{{1,{{7}},{{2,{{1,1,{{2,0,5,{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Элементы\"}},{{\"S\",\"\"}},1,{{1,1,{{2,1,7,{{\"#\",{type_uuid},{{1,{folder_uuid}}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Folder\"}},{{\"S\",\"F\"}},{{\"S\",\"Folder description\"}},{{\"N\",0}},1,{{1,1,{{2,2,7,{{\"#\",{type_uuid},{{1,{item_uuid}}}}},{{\"B\",0}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Item\"}},{{\"S\",\"I\"}},{{\"S\",\"Item description\"}},{{\"N\",0}},0}}}}}}}}}}}}}},-1,3}}}}"
            )
            .as_bytes(),
        );
        assert!(parse_predefined_data_blob(&predefined, &BTreeMap::new()).is_some());
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: format!("{catalog_uuid}.1c"),
                part_no: 0,
                data_size: predefined.len() as i64,
                binary_hex: encode_hex_for_test(&predefined),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        let xml = fs::read_to_string(root.join("Catalogs/Products/Ext/Predefined.xml")).unwrap();
        assert!(xml.contains(r#"xsi:type="CatalogPredefinedItems""#));
        assert!(xml.contains(&format!(r#"<Item id="{folder_uuid}">"#)));
        assert!(xml.contains("<Name>Folder</Name>"));
        assert!(xml.contains("<Code>F</Code>"));
        assert!(xml.contains("<Description>Folder description</Description>"));
        assert!(xml.contains("<IsFolder>true</IsFolder>"));
        assert!(xml.contains(&format!(r#"<Item id="{item_uuid}">"#)));
        assert!(xml.contains("<Name>Item</Name>"));
        assert!(xml.contains("<IsFolder>false</IsFolder>"));
        let predefined_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{catalog_uuid}.1c"))
            .unwrap();
        assert_eq!(
            predefined_row.source_asset_path.as_deref(),
            Some("Catalogs/Products/Ext/Predefined.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_chart_of_characteristic_types_predefined_data_with_types() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let chart_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let object_type_uuid = "11111111-1111-4111-8111-111111111111";
        let ref_type_uuid = "22222222-2222-4222-8222-222222222222";
        let selection_type_uuid = "33333333-3333-4333-8333-333333333333";
        let value_type_uuid = "f5c65050-3bbb-11d5-b988-0050bae0a95d";
        let predefined_type_uuid = "ae135932-4f94-44df-92c1-c91f15a92848";
        let string_item_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let ref_item_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let metadata = deflate_for_test(
            format!(
                "{{1,{{34,{object_type_uuid},00000000-0000-0000-0000-000000000000,{ref_type_uuid},00000000-0000-0000-0000-000000000000,{selection_type_uuid},{{3,{{1,0,{chart_uuid}}},\"Kinds\",{{1,\"en\",\"Kinds\"}},\"\"}}}}}}"
            )
            .as_bytes(),
        );
        let predefined = deflate_for_test(
            format!(
                "{{1,{{1,{{7}},{{2,{{1,1,{{2,0,6,{{\"#\",{predefined_type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"B\",1}},{{\"S\",\"Характеристики\"}},{{\"S\",\"\"}},{{\"S\",\"\"}},{{\"#\",{value_type_uuid},{{\"Pattern\"}}}},1,{{1,2,{{2,1,7,{{\"#\",{predefined_type_uuid},{{1,{string_item_uuid}}}}},{{\"B\",0}},{{\"S\",\"StringKind\"}},{{\"S\",\"\"}},{{\"S\",\"String kind\"}},{{\"#\",{value_type_uuid},{{\"Pattern\",{{\"S\",10,1}}}}}},{{\"N\",0}},0}},{{2,2,7,{{\"#\",{predefined_type_uuid},{{1,{ref_item_uuid}}}}},{{\"B\",0}},{{\"S\",\"RefKind\"}},{{\"S\",\"\"}},{{\"S\",\"Ref kind\"}},{{\"#\",{value_type_uuid},{{\"Pattern\",{{\"#\",{ref_type_uuid}}}}}}},{{\"N\",0}},0}}}}}}}}}},-1,1}}}}"
            )
            .as_bytes(),
        );
        let mut type_index = BTreeMap::new();
        type_index.insert(
            ref_type_uuid.to_string(),
            "cfg:ChartOfCharacteristicTypesRef.Kinds".to_string(),
        );
        let parsed = parse_predefined_data_blob(&predefined, &type_index).unwrap();
        assert_eq!(parsed.len(), 2);
        let rows = vec![
            ConfigRow {
                file_name: chart_uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{chart_uuid}.7"),
                part_no: 0,
                data_size: predefined.len() as i64,
                binary_hex: encode_hex_for_test(&predefined),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        let xml =
            fs::read_to_string(root.join("ChartsOfCharacteristicTypes/Kinds/Ext/Predefined.xml"))
                .unwrap();
        assert!(xml.contains(r#"xsi:type="PlanOfCharacteristicKindPredefinedItems""#));
        assert!(xml.contains(&format!(r#"<Item id="{string_item_uuid}">"#)));
        assert!(xml.contains("<v8:Type>xs:string</v8:Type>"));
        assert!(xml.contains("<v8:StringQualifiers>"));
        assert!(xml.contains("<v8:Length>10</v8:Length>"));
        assert!(xml.contains("<v8:AllowedLength>Variable</v8:AllowedLength>"));
        assert!(xml.contains(&format!(r#"<Item id="{ref_item_uuid}">"#)));
        assert!(
            xml.contains(r#"<v8:Type xmlns:d4p1="http://v8.1c.ru/8.1/data/enterprise/current-config">d4p1:ChartOfCharacteristicTypesRef.Kinds</v8:Type>"#)
        );
        let predefined_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{chart_uuid}.7"))
            .unwrap();
        assert_eq!(
            predefined_row.source_asset_path.as_deref(),
            Some("ChartsOfCharacteristicTypes/Kinds/Ext/Predefined.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_form_help_xml_and_html_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let form_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0,{form_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let form_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{form_uuid}}},\"ItemForm\",{{1,\"en\",\"Item form\"}},\"\"}},0,1,{{0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let help = deflate_for_test(b"{5,1,\"ru\",{#base64:PGgxPkZvcm08L2gxPg==},0}");
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: form_uuid.to_string(),
                part_no: 0,
                data_size: form_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&form_metadata),
            },
            ConfigRow {
                file_name: format!("{form_uuid}.1"),
                part_no: 0,
                data_size: help.len() as i64,
                binary_hex: encode_hex_for_test(&help),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 2);
        assert_eq!(dumped.source_asset_rows, 1);
        assert!(
            fs::read_to_string(root.join("Catalogs/Products/Forms/ItemForm/Ext/Help.xml"))
                .unwrap()
                .contains("<Page>ru</Page>")
        );
        assert_eq!(
            fs::read(root.join("Catalogs/Products/Forms/ItemForm/Ext/Help/ru.html")).unwrap(),
            b"<h1>Form</h1>"
        );
        let help_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{form_uuid}.1"))
            .unwrap();
        assert_eq!(
            help_row.source_asset_path.as_deref(),
            Some("Catalogs/Products/Forms/ItemForm/Ext/Help.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_template_metadata_xml_to_owner_or_common_template_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let owned_template_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let common_template_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},0,{owned_template_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let owned_template_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{2,0,\r\n{{3,\r\n{{1,0,{owned_template_uuid}}},\"Print\",{{1,\"en\",\"Print\"}},\"\"}}\r\n,0}}\r\n}}"
            )
            .as_bytes(),
        );
        let owned_template_body = deflate_for_test(
            "MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{8,1,2,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{0},{0},0,0,0,2,0,{16,0,{1,1,{\"ru\",\"Hello [Name]\"}},0},2,{0,1}}"
                .as_bytes(),
        );
        let common_template_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\r\n{{3,\r\n{{1,0,{common_template_uuid}}},\"SharedText\",{{1,\"en\",\"Shared text\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},4}},0}}"
            )
            .as_bytes(),
        );
        let common_template_body = deflate_for_test(b"\xef\xbb\xbfPlain text");
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: owned_template_uuid.to_string(),
                part_no: 0,
                data_size: owned_template_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&owned_template_metadata),
            },
            ConfigRow {
                file_name: format!("{owned_template_uuid}.0"),
                part_no: 0,
                data_size: owned_template_body.len() as i64,
                binary_hex: encode_hex_for_test(&owned_template_body),
            },
            ConfigRow {
                file_name: common_template_uuid.to_string(),
                part_no: 0,
                data_size: common_template_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&common_template_metadata),
            },
            ConfigRow {
                file_name: format!("{common_template_uuid}.0"),
                part_no: 0,
                data_size: common_template_body.len() as i64,
                binary_hex: encode_hex_for_test(&common_template_body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 3);
        assert_eq!(dumped.source_asset_rows, 2);
        let owned_xml =
            fs::read_to_string(root.join("Catalogs/Products/Templates/Print.xml")).unwrap();
        let common_xml = fs::read_to_string(root.join("CommonTemplates/SharedText.xml")).unwrap();
        let common_body =
            fs::read(root.join("CommonTemplates/SharedText/Ext/Template.txt")).unwrap();
        let template_body =
            fs::read_to_string(root.join("Catalogs/Products/Templates/Print/Ext/Template.xml"))
                .unwrap();
        assert!(owned_xml.contains("<Template uuid=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\">"));
        assert!(owned_xml.contains("<TemplateType>SpreadsheetDocument</TemplateType>"));
        assert!(template_body.contains("<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\""));
        assert!(template_body.contains("<v8:content>Hello [Name]</v8:content>"));
        assert!(template_body.contains("<i>2</i>"));
        assert!(
            common_xml.contains("<CommonTemplate uuid=\"cccccccc-cccc-4ccc-cccc-cccccccccccc\">")
        );
        assert!(common_xml.contains("<TemplateType>TextDocument</TemplateType>"));
        assert_eq!(common_body, b"\xef\xbb\xbfPlain text");
        let template_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == owned_template_uuid)
            .unwrap();
        assert_eq!(
            template_row.metadata_xml_path.as_deref(),
            Some("Catalogs/Products/Templates/Print.xml")
        );
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{common_template_uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("CommonTemplates/SharedText/Ext/Template.txt")
        );
        let spreadsheet_body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{owned_template_uuid}.0"))
            .unwrap();
        assert_eq!(
            spreadsheet_body_row.source_asset_path.as_deref(),
            Some("Catalogs/Products/Templates/Print/Ext/Template.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detects_data_composition_appearance_template_body() {
        let body = deflate_for_test(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<AppearanceTemplate xmlns="http://v8.1c.ru/8.1/data-composition-system/appearance-template">
	<dcscor:item xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core"/>
</AppearanceTemplate>
"#,
        );

        assert_eq!(
            infer_template_type_from_body(&body),
            Some("DataCompositionAppearanceTemplate")
        );
        let (path, kind) = template_body_source_asset("DataCompositionAppearanceTemplate").unwrap();
        assert_eq!(path, "Template.xml");
        assert!(matches!(kind, SourceAssetKind::InflatedBinary));
    }

    #[test]
    fn detects_graphical_schema_template_body() {
        let body = deflate_for_test(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/scheme" version="2.21">
	<Items/>
</GraphicalSchema>
"#,
        );

        assert_eq!(
            infer_template_type_from_body(&body),
            Some("GraphicalSchema")
        );
        let (path, kind) = template_body_source_asset("GraphicalSchema").unwrap();
        assert_eq!(path, "Template.xml");
        assert!(matches!(kind, SourceAssetKind::InflatedBinary));
    }

    #[test]
    fn formats_moxel_observed_columns_empty_rows_and_cell_formats() {
        let object_refs = BTreeMap::from([(
            "43d91051-d5a2-4d2a-8447-7fa917e5ea38".to_string(),
            "StyleItem.ЦветШтампаЭП".to_string(),
        )]);
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,7,0,0,0,1,0,3,0,{0,1},1,{16,2,{1,0},0},2,{16,3,{1,1,{\"ru\",\"ДОКУМЕНТ ПОДПИСАН\\nЭЛЕКТРОННОЙ ПОДПИСЬЮ\"}},0},2,0,2,0,{0,4},1,{16,5,{1,1,{\"\",\"ТекстШтампа\"}},0},{2,{1,1,1,2,0},{1,3,2,5,0}},{1,\"Штамп\",{1,{3,1,1,2,6,00000000-0000-0000-0000-000000000000},0}},{3,3,{-1}},{3,3,{-3}},{3,3,{0,43d91051-d5a2-4d2a-8447-7fa917e5ea38}},7,{719,0,0,0,0,45,72,0},{66985,0,1,2,219,6,2,0},{16769,0,90,6,3},{3221308845,1,1,1,2,139,6,2,2,0,0,0},{128,25},{128,85},{128,226},{7,0,575,60,0,0,0,400,0,0,0,0,0,0,0,0,\"Arial\",1,100},{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},1,{4,0,{0},\"\",-1,-1,1,0,\"\"}}",
            &object_refs,
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains("<size>3</size>"));
        assert!(xml.contains(
            "<index>2</index>\r\n\t\t\t<column>\r\n\t\t\t\t<formatIndex>3</formatIndex>"
        ));
        assert!(xml.contains("<empty>true</empty>"));
        assert!(!xml.contains("<formatIndex>1</formatIndex>\r\n\t\t\t<empty>true</empty>"));
        assert!(xml.contains("<f>4</f>"));
        assert!(xml.contains("<f>5</f>"));
        assert!(xml.contains("<f>6</f>"));
        assert!(xml.contains("<f>5</f>\r\n\t\t\t\t\t<tl/>"));
        assert!(xml.contains("ДОКУМЕНТ ПОДПИСАН\\nЭЛЕКТРОННОЙ ПОДПИСЬЮ"));
        assert!(xml.contains("<parameter>ТекстШтампа</parameter>"));
        assert!(!xml.contains("<v8:content>ТекстШтампа</v8:content>"));
        assert!(xml.contains("<templateMode>true</templateMode>"));
        assert!(xml.contains("<defaultFormatIndex>9</defaultFormatIndex>"));
        assert!(xml.contains("<height>7</height>"));
        assert!(xml.contains("<vgRows>7</vgRows>"));
        assert_eq!(
            xml.matches("\t<format>\r\n").count() + xml.matches("\t<format/>\r\n").count(),
            9
        );
        assert_eq!(xml.matches("\t<format/>\r\n").count(), 1);
        assert!(xml.contains("\t<format>\r\n\t\t<width>25</width>\r\n\t</format>"));
        assert!(xml.contains("\t<format>\r\n\t\t<width>85</width>\r\n\t</format>"));
        assert!(xml.contains("\t<format>\r\n\t\t<width>226</width>\r\n\t</format>"));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<leftBorder>0</leftBorder>\r\n\t\t<topBorder>0</topBorder>\r\n\t\t<rightBorder>0</rightBorder>\r\n\t\t<height>45</height>\r\n\t\t<width>72</width>\r\n\t\t<verticalAlignment>Top</verticalAlignment>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<rightBorder>1</rightBorder>\r\n\t\t<borderColor>style:ЦветШтампаЭП</borderColor>\r\n\t\t<width>219</width>\r\n\t\t<horizontalAlignment>Center</horizontalAlignment>\r\n\t\t<textColor>style:ЦветШтампаЭП</textColor>\r\n\t\t<protection>true</protection>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<width>90</width>\r\n\t\t<horizontalAlignment>Center</horizontalAlignment>\r\n\t\t<textPlacement>Wrap</textPlacement>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>1</font>\r\n\t\t<topBorder>1</topBorder>\r\n\t\t<rightBorder>1</rightBorder>\r\n\t\t<borderColor>style:ЦветШтампаЭП</borderColor>\r\n\t\t<width>139</width>\r\n\t\t<horizontalAlignment>Center</horizontalAlignment>\r\n\t\t<textColor>style:ЦветШтампаЭП</textColor>\r\n\t\t<textPlacement>Block</textPlacement>\r\n\t\t<protection>true</protection>\r\n\t\t<indent>0</indent>\r\n\t\t<autoIndent>0</autoIndent>\r\n\t</format>"
        ));
        assert!(xml.contains("\t<format>\r\n\t\t<width>72</width>\r\n\t</format>"));
        assert!(
            xml.contains("\t<picture>\r\n\t\t<index>0</index>\r\n\t\t<picture/>\r\n\t</picture>")
        );
        let default_format_index_pos = xml
            .find("<defaultFormatIndex>9</defaultFormatIndex>")
            .unwrap();
        let height_pos = xml.find("<height>7</height>").unwrap();
        let merge_pos = xml.find("<merge>").unwrap();
        assert!(default_format_index_pos < merge_pos);
        assert!(height_pos < merge_pos);
        assert!(
            xml.contains("<merge>\r\n\t\t<r>1</r>\r\n\t\t<c>1</c>\r\n\t\t<h>1</h>\r\n\t</merge>")
        );
        assert!(xml.contains(
            "<merge>\r\n\t\t<r>3</r>\r\n\t\t<c>1</c>\r\n\t\t<h>2</h>\r\n\t\t<w>1</w>\r\n\t</merge>"
        ));
        assert!(xml.contains("<namedItem xsi:type=\"NamedItemCells\">"));
        assert!(xml.contains("<name>Штамп</name>"));
        assert!(xml.contains("<type>Rectangle</type>"));
        assert!(xml.contains("<beginRow>1</beginRow>"));
        assert!(xml.contains("<endRow>6</endRow>"));
        assert!(xml.contains("<beginColumn>1</beginColumn>"));
        assert!(xml.contains("<endColumn>2</endColumn>"));
        assert!(xml.contains(
            "<line width=\"1\" gap=\"false\">\r\n\t\t<v8ui:style xsi:type=\"v8ui:SpreadsheetDocumentCellLineType\">None</v8ui:style>\r\n\t</line>"
        ));
        assert!(xml.contains(
            "<line width=\"1\" gap=\"false\">\r\n\t\t<v8ui:style xsi:type=\"v8ui:SpreadsheetDocumentCellLineType\">Solid</v8ui:style>\r\n\t</line>"
        ));
        assert!(xml.contains(
            "<font faceName=\"Arial\" height=\"6\" bold=\"false\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>"
        ));
        assert!(xml.contains(
            "<font faceName=\"Arial\" height=\"8\" bold=\"true\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>"
        ));
        let line_pos = xml.find("<line width=\"1\"").unwrap();
        let font_pos = xml.find("<font faceName=\"Arial\"").unwrap();
        let format_pos = xml.find("<format>").unwrap();
        let picture_pos = xml.find("<picture>").unwrap();
        assert!(line_pos < font_pos);
        assert!(font_pos < format_pos);
        assert!(format_pos < picture_pos);
    }

    #[test]
    fn formats_moxel_single_column_empty_row_without_prefix() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,3,0,0,1,0,{16,1,{1,1,{\"\",\"Описание\"}},0},1,0,0,2,0,1,0,{16,2,{1,1,{\"\",\"Название\"}},0},{1,0,00000000-0000-0000-0000-000000000000,1,0,3},3,0,0,0,0,0,0,0,0,{0},{0},{0},{3,\"Заголовок\",{1,{1,-1,0,-1,0,00000000-0000-0000-0000-000000000000},0},\"ПустаяСтрока\",{1,{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},\"СодержаниеОтчета\",{1,{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0}},\"\",{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}},{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,3,{32769,0,1},{32768,1},{128,509},1,{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},0,0,0,2,{3,3,{-1}},{3,3,{-3}},0,0,0,\"\",0,{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0},{0},0,0,0,1,0,0,0}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains("<size>1</size>"));
        assert_eq!(xml.matches("<columnsItem>").count(), 1);
        assert!(xml.contains("<index>1</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"));
        assert!(xml.contains("<f>2</f>\r\n\t\t\t\t\t<parameter>Описание</parameter>"));
        assert!(xml.contains("<f>3</f>\r\n\t\t\t\t\t<parameter>Название</parameter>"));
        assert!(!xml.contains("<i>3</i>"));
        assert!(xml.contains("<defaultFormatIndex>4</defaultFormatIndex>"));
        assert!(xml.contains("\t<format>\r\n\t\t<width>509</width>\r\n\t</format>"));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<fillType>Parameter</fillType>\r\n\t</format>"
        ));
        assert!(xml.contains("\t<format>\r\n\t\t<fillType>Parameter</fillType>\r\n\t</format>"));
    }

    #[test]
    fn formats_moxel_simple_template_empty_row_range_and_style_formats() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{1,1,{4,0,{0},1,1,0,f527dc88-1d39-40b3-bcbb-d98b690ead68,0},0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,6,0,0,1,0,{24,1,\"Наименование\",{1,1,{\"\",\"Заголовок\"}},0},1,0,0,2,0,0,3,0,0,4,0,1,0,{16,2,{1,1,{\"\",\"Группа\"}},0},5,0,1,0,{16,3,{1,1,{\"\",\"Заголовок\"}},0},{1,0,00000000-0000-0000-0000-000000000000,1,0,4},6,0,0,0,0,0,0,0,0,{0},{0},{0},{2,\"Заголовок\",{1,{3,0,0,0,0,00000000-0000-0000-0000-000000000000},0},\"Шапка2Строки\",{1,{3,0,4,0,5,00000000-0000-0000-0000-000000000000},0}},\"\",{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}},{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,4,{20402178753,0,66,319,24,2,1,0,1,0,0},{32799,1,0,0,0,0,1},{32798,0,0,0,0,1},{128,135},2,{7,2,60,{-31},700,0,0,0,1,100},{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},0,0,0,3,{3,3,{-1}},{3,3,{-3}},{3,3,{-7}},0,0,0,\"\",0,{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0},{0},1,{1,0},0,0,1,0,0,0}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains(
            "<index>1</index>\r\n\t\t<indexTo>3</indexTo>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"
        ));
        assert!(!xml.contains("<index>2</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"));
        assert!(!xml.contains(
            "<v8ui:style xsi:type=\"v8ui:SpreadsheetDocumentCellLineType\">None</v8ui:style>"
        ));
        assert!(xml.contains(
            "<font ref=\"style:NormalTextFont\" bold=\"true\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"StyleItem\"/>"
        ));
        assert!(xml.contains(
            "<font faceName=\"Arial\" height=\"8\" bold=\"true\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<height>66</height>\r\n\t\t<width>319</width>\r\n\t\t<verticalAlignment>Center</verticalAlignment>\r\n\t\t<backColor>style:ButtonBackColor</backColor>\r\n\t\t<fillType>Parameter</fillType>\r\n\t\t<bySelectedColumns>false</bySelectedColumns>\r\n\t\t<indent>1</indent>\r\n\t\t<autoIndent>0</autoIndent>\r\n\t\t<mask/>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>1</font>\r\n\t\t<border>0</border>\r\n\t\t<fillType>Parameter</fillType>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<border>0</border>\r\n\t\t<fillType>Parameter</fillType>\r\n\t</format>"
        ));
        assert!(xml.contains("\t<format>\r\n\t\t<width>135</width>\r\n\t</format>"));
        assert!(xml.contains("\t<format>\r\n\t\t<width>72</width>\r\n\t</format>"));
    }

    #[test]
    fn formats_moxel_standard_print_picture_ref() {
        let picture = parse_moxel_picture("{4,0,{-13}}", &BTreeMap::new()).unwrap();

        assert_eq!(picture.index, 0);
        assert_eq!(picture.ref_name.as_deref(), Some("v8ui:Print"));
    }

    #[test]
    fn formats_moxel_standard_picture_refs() {
        let cases = [
            ("{4,0,{-13}}", "v8ui:Print"),
            ("{4,1,{-6}}", "v8ui:InputFieldCalculator"),
            (
                "{4,2,{0,4b54770b-d069-4c0e-9b17-5cc2a01134d9}}",
                "v8ui:Information",
            ),
            (
                "{4,3,{0,818ab7d0-4654-4542-bd5e-fd9d1352b5a1}}",
                "v8ui:SaveFile",
            ),
        ];

        for (text, expected_ref) in cases {
            let picture = parse_moxel_picture(text, &BTreeMap::new()).unwrap();
            assert_eq!(picture.ref_name.as_deref(), Some(expected_ref));
        }
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_global_format_indexes() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>3</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
		<columnsItem>
			<index>1</index>
			<column>
				<formatIndex>2</formatIndex>
			</column>
		</columnsItem>
		<columnsItem>
			<index>2</index>
			<column>
				<formatIndex>3</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<formatIndex>5</formatIndex>
			<c>
				<c>
					<f>6</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<format>
		<width>40</width>
	</format>
	<format>
		<width>50</width>
	</format>
	<format>
		<width>60</width>
	</format>
	<format>
		<fillType>Parameter</fillType>
	</format>
	<format>
		<horizontalAlignment>Center</horizontalAlignment>
	</format>
	<format>
		<verticalAlignment>Center</verticalAlignment>
	</format>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(extracted.contains("<formatIndex>5</formatIndex>"));
        assert!(extracted.contains("<f>6</f>"));
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_text_entity_spacing() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>0</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>CatalogRef &quot;ReportsKinds&quot; -&amp;nbsp;the report type</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(
            extracted.contains("CatalogRef &quot;ReportsKinds&quot; -&amp;nbsp;the report type")
        );
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_empty_sheet() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>0</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<templateMode>true</templateMode>
	<defaultFormatIndex>1</defaultFormatIndex>
	<vgRows>0</vgRows>
	<format>
		<width>72</width>
	</format>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(extracted.contains("<empty>true</empty>"));
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_row_columns_id() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>2</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
		<columnsItem>
			<index>1</index>
			<column>
				<formatIndex>2</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<columns>
		<id>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</id>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>3</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>4</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>First</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<rowsItem>
		<index>1</index>
		<row>
			<formatIndex>4</formatIndex>
			<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>
			<c>
				<c>
					<f>5</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<format>
		<width>10</width>
	</format>
	<format>
		<width>20</width>
	</format>
	<format>
		<width>30</width>
	</format>
	<format>
		<width>40</width>
	</format>
	<format>
		<width>50</width>
	</format>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(extracted.contains("<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>"));
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_row_zero_columns_id() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<columns>
		<id>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</id>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>
			<c>
				<c>
					<f>2</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<format>
		<width>10</width>
	</format>
	<format>
		<width>20</width>
	</format>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(extracted.contains("<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>"));
    }

    #[test]
    fn spreadsheet_pack_extract_roundtrip_preserves_multiple_row_columns_id_from_zero() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<columns>
		<id>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</id>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<columns>
		<id>bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb</id>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>
			<c>
				<c>
					<f>2</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>First</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<rowsItem>
		<index>1</index>
		<row>
			<columnsID>bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb</columnsID>
			<c>
				<c>
					<f>2</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Second</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
	<format>
		<width>10</width>
	</format>
	<format>
		<width>20</width>
	</format>
</document>
"#;

        let first = pack_moxel_spreadsheet_blob_from_xml(xml).unwrap();
        let extracted =
            extract_moxel_spreadsheet_xml(&first.blob, &BTreeMap::new()).expect("first extract");
        let second = pack_moxel_spreadsheet_blob_from_xml(extracted.as_bytes()).unwrap();
        let extracted_again =
            extract_moxel_spreadsheet_xml(&second.blob, &BTreeMap::new()).expect("second extract");

        assert_eq!(extracted, extracted_again);
        assert!(extracted.contains("<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>"));
        assert!(extracted.contains("<columnsID>bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb</columnsID>"));
    }

    #[test]
    fn formats_moxel_receipt_columns_headers_style_font_and_default_format() {
        let object_refs = BTreeMap::from([(
            "757b547b-b79c-459a-a64a-eef19a09a38f".to_string(),
            "StyleItem.ГиперссылкаЦвет".to_string(),
        )]);
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{32,0},{0},0,{16,0,{1,0},1,{1,{1,0},1}},{16,0,{1,0},1,{1,{1,0},1}},{16,0,{1,0},1,{1,{1,0},1}},{16,0,{1,0},1,{1,{1,0},1}},{16,0,{1,0},1,{1,{1,0},1}},{16,0,{1,0},1,{1,{1,0},1}},1,2,4,0,0,2,0,{16,1,{1,1,{\"\",\"Наименование\"}},0},1,{16,2,{1,1,{\"\",\"Значение\"}},0},1,3,1,0,{16,4,{1,1,{\"\",\"Значение\"}},0},2,0,1,0,{24,5,\"Файл\",{1,1,{\"ru\",\"Открыть для просмотра\"}},0},3,6,1,0,{16,1,{1,1,{\"\",\"Значение\"}},0},{2,0,00000000-0000-0000-0000-000000000000,12,0,7,1,8,2,9,3,9,4,9,5,9,6,9,7,9,8,9,9,9,10,9,11,9},4,0,0,0,0,0,0,0,0,{2,{0,1,1,1,0},{0,3,1,3,0}},{0},{0},{4,\"Заголовок\",{1,{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},\"ОткрытьДляПросмотра\",{1,{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0},\"Строка\",{1,{1,-1,0,-1,0,00000000-0000-0000-0000-000000000000},0},\"Текст\",{1,{1,-1,3,-1,3,00000000-0000-0000-0000-000000000000},0}},\"\",{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}},{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,1,2,1,2,9,{49152,3,1},{49312,0,68,3,1},{1,0},{49569,0,0,68,6,3,1},{67109889,1,3,1},{16384,3},{128,403},{128,377},{128,68},2,{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},{7,2,60,{-32},400,0,1,0,1,100},0,0,0,4,{3,0,{8765644}},{3,3,{-1}},{3,3,{-3}},{3,3,{0,757b547b-b79c-459a-a64a-eef19a09a38f}},0,0,0,\"\",0,{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0},{1,0,{3,3,{-28}}},0,0,0,1,0,0,0}",
            &object_refs,
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains("\t<columns>\r\n\t\t<size>2</size>"));
        assert!(xml.contains(
            "<index>11</index>\r\n\t\t\t<column>\r\n\t\t\t\t<formatIndex>3</formatIndex>"
        ));
        assert_eq!(xml.matches("<columnsItem>").count(), 12);
        assert!(xml.contains("<leftHeader>\r\n\t\t<f>0</f>\r\n\t\t<tfl/>\r\n\t</leftHeader>"));
        assert!(xml.contains("<rightFooter>\r\n\t\t<f>0</f>\r\n\t\t<tfl/>\r\n\t</rightFooter>"));
        assert!(xml.contains("<defaultFormatIndex>10</defaultFormatIndex>"));
        assert!(xml.contains("<f>4</f>\r\n\t\t\t\t\t<parameter>Наименование</parameter>"));
        assert!(xml.contains("<formatIndex>6</formatIndex>"));
        assert!(xml.contains(
            "<font ref=\"style:LargeTextFont\" bold=\"false\" italic=\"false\" underline=\"true\" strikeout=\"false\" kind=\"StyleItem\"/>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<width>403</width>\r\n\t</format>\r\n\t<format>\r\n\t\t<width>377</width>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<borderColor>style:ReportLineColor</borderColor>\r\n\t\t<width>68</width>\r\n\t\t<textPlacement>Wrap</textPlacement>\r\n\t\t<fillType>Parameter</fillType>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>1</font>\r\n\t\t<textColor>style:ГиперссылкаЦвет</textColor>\r\n\t\t<hyperLink>true</hyperLink>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<borderColor>style:ReportLineColor</borderColor>\r\n\t</format>"
        ));
    }

    #[test]
    fn formats_moxel_trims_trailing_empty_rows_outside_named_areas() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,16,0,0,0,1,1,1,0,{16,2,{1,1,{\"ru\",\"Движения документа [СсылкаНаДокумент]\"}},0},2,3,0,3,0,0,4,4,0,5,0,0,6,0,0,7,0,0,8,0,0,9,0,0,10,0,0,11,0,0,12,0,0,13,0,0,14,0,0,15,0,0,{1,0,00000000-0000-0000-0000-000000000000,1,0,5},5,0,0,0,0,0,0,0,0,{0},{0},{0},{2,\"ОбластьЗаголовок\",{1,{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},\"ПустаяОбласть\",{1,{1,-1,4,-1,4,00000000-0000-0000-0000-000000000000},0}},\"\",{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}},{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,5,{1025,0,2},{558081,0,2,2,1},{1089,0,30,2},{64,30},{128,567},1,{7,0,575,180,0,0,0,400,0,0,0,0,0,0,0,0,\"Arial\",1,100},0,0,0,3,{3,3,{-1}},{3,3,{-3}},{3,0,{4625920}},0,0,0,\"\",0,{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0},{0},0,0,0,1,0,0,0}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains("<height>5</height>"));
        assert!(xml.contains("<vgRows>5</vgRows>"));
        assert!(xml.contains(
            "<index>5</index>\r\n\t\t<indexTo>15</indexTo>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"
        ));
        assert!(!xml.contains("<index>6</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"));
        assert!(!xml.contains("<index>15</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<textColor>#009646</textColor>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<textColor>#009646</textColor>\r\n\t\t<fillType>Template</fillType>\r\n\t\t<detailsUse>Row</detailsUse>\r\n\t</format>"
        ));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<font>0</font>\r\n\t\t<height>30</height>\r\n\t\t<textColor>#009646</textColor>\r\n\t</format>"
        ));
    }

    #[test]
    fn formats_moxel_additional_column_sets_and_columns_id() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,3,0,0,0,1,0,1,0,{16,1,{1,1,{\"\",\"Описание\"}},0},2,2,2,0,{16,3,{1,1,{\"\",\"Название\"}},0},1,{16,4,{1,1,{\"\",\"Ошибка\"}},0},{2,0,00000000-0000-0000-0000-000000000000,2,0,5,1,6},3,1,{1,0,9bb67b5f-5e3e-459e-98c5-618e04892d9b,1,0,7},1,1,0,0,0,0,0,0,0,{0},{0},{0},{2,\"Заголовок\",{1,{1,-1,1,-1,1,9bb67b5f-5e3e-459e-98c5-618e04892d9b},0},\"Строка\",{1,{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0}},\"\",{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}},{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,7,{32769,0,1},{64,96},{32768,1},{49152,3,1},{128,465},{128,491},{128,383},1,{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},0,0,0,2,{3,3,{-1}},{3,3,{-3}},0,0,0,\"\",0,{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0},{0},0,0,0,1,0,0,0}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(xml.contains("\t<columns>\r\n\t\t<size>2</size>"));
        assert!(xml.contains(
            "\t<columns>\r\n\t\t<id>9bb67b5f-5e3e-459e-98c5-618e04892d9b</id>\r\n\t\t<size>1</size>"
        ));
        assert!(xml.contains(
            "<index>0</index>\r\n\t\t\t<column>\r\n\t\t\t\t<formatIndex>3</formatIndex>"
        ));
        assert!(xml.contains("<index>1</index>\r\n\t\t<row>\r\n\t\t\t<columnsID>9bb67b5f-5e3e-459e-98c5-618e04892d9b</columnsID>"));
        assert!(xml.contains("<name>Заголовок</name>"));
        assert!(xml.contains("<columnsID>9bb67b5f-5e3e-459e-98c5-618e04892d9b</columnsID>"));
        assert!(xml.contains("<defaultFormatIndex>8</defaultFormatIndex>"));
        assert!(xml.contains("\t<format>\r\n\t\t<width>465</width>\r\n\t</format>"));
        assert!(xml.contains("<f>4</f>\r\n\t\t\t\t\t<parameter>Описание</parameter>"));
        assert!(xml.contains("<formatIndex>5</formatIndex>"));
        assert!(xml.contains("<f>6</f>\r\n\t\t\t\t\t<parameter>Название</parameter>"));
        assert!(xml.contains("<f>7</f>\r\n\t\t\t\t\t<parameter>Ошибка</parameter>"));
    }

    #[test]
    fn formats_moxel_row_column_ids_accept_pair_mapping() {
        let additional_sets = vec![
            MoxelColumnSet {
                id: Some("5c3926f2-4223-4ca7-a6a7-7160301c991d".to_string()),
                size: 1,
                columns: vec![],
            },
            MoxelColumnSet {
                id: Some("c00ea4cf-0123-4de2-9c91-0ec224c7b2e9".to_string()),
                size: 1,
                columns: vec![],
            },
        ];
        let fields = ["2", "11", "0", "12", "1", "0"];
        let row_column_ids = parse_moxel_row_column_set_ids(&fields, 0, &additional_sets).unwrap();

        assert_eq!(
            row_column_ids.get(&11).map(String::as_str),
            Some("5c3926f2-4223-4ca7-a6a7-7160301c991d")
        );
        assert_eq!(
            row_column_ids.get(&12).map(String::as_str),
            Some("c00ea4cf-0123-4de2-9c91-0ec224c7b2e9")
        );
    }

    #[test]
    fn formats_moxel_row_column_ids_accept_row_zero_pair_mapping() {
        let additional_sets = vec![
            MoxelColumnSet {
                id: Some("5c3926f2-4223-4ca7-a6a7-7160301c991d".to_string()),
                size: 1,
                columns: vec![],
            },
            MoxelColumnSet {
                id: Some("c00ea4cf-0123-4de2-9c91-0ec224c7b2e9".to_string()),
                size: 1,
                columns: vec![],
            },
        ];
        let fields = ["2", "0", "0", "1", "1", "0"];
        let row_column_ids = parse_moxel_row_column_set_ids(&fields, 0, &additional_sets).unwrap();

        assert_eq!(
            row_column_ids.get(&0).map(String::as_str),
            Some("5c3926f2-4223-4ca7-a6a7-7160301c991d")
        );
        assert_eq!(
            row_column_ids.get(&1).map(String::as_str),
            Some("c00ea4cf-0123-4de2-9c91-0ec224c7b2e9")
        );
    }

    #[test]
    fn formats_moxel_scanning_does_not_treat_column_mapping_as_empty_row() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{0},{0},0,1,1,5,{16,2,{1,1,{\"ru\",\"Hello\"}},0},{10,0,00000000-0000-0000-0000-000000000000,10,0,1,1,2,2,3,3,4,4,5,5,11,6,7,7,12,8,13,9,14},1,1,{10,0,cab491a9-17d5-47b2-96fc-7a31b2075a1c,10,0,1,1,2,2,3,3,4,4,5,5,11,6,7,7,12,8,13,9,14},1,0,0,{0}}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert_eq!(xml.matches("<rowsItem>").count(), 1);
        assert!(xml.contains("<index>0</index>"));
        assert!(!xml.contains("<index>1</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>"));
    }

    #[test]
    fn formats_moxel_accepts_sparse_column_sets() {
        let column_set = parse_moxel_column_set(
            "{11,0,00000000-0000-0000-0000-000000000000,8,0,7,2,8,3,9,4,10,5,10,7,11,9,12,10,7}",
        )
        .unwrap();
        let mut column_sets = vec![column_set];
        normalize_moxel_column_set_format_indices(&mut column_sets);

        assert_eq!(column_sets[0].size, 11);
        assert_eq!(column_sets[0].columns.len(), 8);
        assert_eq!(column_sets[0].columns[1].index, 2);
        assert_eq!(column_sets[0].columns[1].format_index, 2);
        assert_eq!(column_sets[0].columns[5].index, 7);
        assert_eq!(column_sets[0].columns[5].format_index, 5);
        assert_eq!(column_sets[0].columns[7].index, 10);
        assert_eq!(column_sets[0].columns[7].format_index, 1);
        assert_eq!(moxel_column_format_slots(&column_sets, 11), 6);
    }

    #[test]
    fn formats_moxel_alignment_and_text_placement_mappings() {
        assert_eq!(moxel_horizontal_alignment(0), Some("Left"));
        assert_eq!(moxel_horizontal_alignment(2), Some("Right"));
        assert_eq!(moxel_horizontal_alignment(6), Some("Center"));
        assert_eq!(moxel_horizontal_alignment(7), Some("Right"));
        assert_eq!(moxel_text_placement(0), Some("Auto"));
        assert_eq!(moxel_text_placement(2), Some("Block"));
        assert_eq!(moxel_text_placement(3), Some("Wrap"));
    }

    #[test]
    fn formats_moxel_line_style_mappings() {
        assert_eq!(parse_moxel_line("{3,3,{-1}}").unwrap().style, "None");
        assert_eq!(parse_moxel_line("{3,3,{-3}}").unwrap().style, "Solid");
        assert_eq!(parse_moxel_line("{3,3,{-10}}").unwrap().style, "Dotted");
    }

    #[test]
    fn formats_moxel_shifts_default_line_styles_for_two_used_indexes() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(1),
                ..MoxelFormat::default()
            },
        ];
        let lines = parse_moxel_lines(&["{3,3,{-1}}", "{3,3,{-3}}"], &formats, true);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].style, "Solid");
        assert_eq!(lines[1].style, "Dotted");
        assert_eq!(lines[0].width, 1);
        assert_eq!(lines[1].width, 1);
    }

    #[test]
    fn formats_moxel_shifts_three_default_line_styles_to_two_solid_widths() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(1),
                ..MoxelFormat::default()
            },
        ];
        let lines = parse_moxel_lines(&["{3,3,{-1}}", "{3,3,{-3}}", "{3,3,{-10}}"], &formats, true);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].style, "Solid");
        assert_eq!(lines[0].width, 2);
        assert_eq!(lines[1].style, "Solid");
        assert_eq!(lines[1].width, 1);
    }

    #[test]
    fn formats_moxel_web_colors_embedded_styles_and_details_use() {
        let style_refs = parse_moxel_style_refs(
            &[
                "{3,1,{4,0,{0},1,2,0,f527dc88-1d39-40b3-bcbb-d98b690ead68,0},0}",
                "{3,3,{-1}}",
                "{3,3,{-3}}",
                "{3,3,{-10}}",
                "{3,2,{121}}",
                "{3,2,{21}}",
            ],
            &BTreeMap::new(),
        );

        assert_eq!(style_refs[0].as_deref(), Some("style:FieldBackColor"));
        assert_eq!(style_refs[1], None);
        assert_eq!(style_refs[2], None);
        assert_eq!(style_refs[3].as_deref(), Some("style:FieldBackColor"));
        assert_eq!(style_refs[4].as_deref(), Some("d3p1:RoyalBlue"));
        assert_eq!(style_refs[5].as_deref(), Some("d3p1:Crimson"));
        assert_eq!(moxel_details_use(0), Some("Cell"));
        assert_eq!(moxel_details_use(1), Some("Row"));
    }

    #[test]
    fn formats_moxel_cell_zero_format_stays_zero() {
        let cell = parse_moxel_cell("{16,0,{0},0}", 0).unwrap();

        assert_eq!(cell.format_index, 0);
    }

    #[test]
    fn formats_moxel_print_settings_from_raw_pairs() {
        let settings = parse_moxel_print_settings_field(
            r#"{{0,18,0,{"N",9},1,{"N",2},2,{"N",100},3,{"N",1},4,{"N",1},5,{"N",1},6,{"N",1000},7,{"N",1000},8,{"N",1000},9,{"N",1000},10,{"N",1000},11,{"N",1000},12,{"N",0},13,{"N",0},14,{"S","HP LaserJet 5100 PCL 6"},15,{"N",7},16,{"N",0},17,{"N",0}}}"#,
        )
        .unwrap();
        let mut xml = String::new();
        push_moxel_print_settings_xml(&mut xml, &settings);

        assert!(xml.contains("<pageOrientation>Landscape</pageOrientation>"));
        assert!(xml.contains("<fitToPage>false</fitToPage>"));
        assert!(xml.contains("<blackAndWhite>false</blackAndWhite>"));
        assert!(xml.contains("<printerName>HP LaserJet 5100 PCL 6</printerName>"));
        assert!(xml.contains("<paper>9</paper>"));
        assert!(xml.contains("<paperSource>7</paperSource>"));
    }

    #[test]
    fn formats_moxel_print_settings_default_format_index_uses_first_extra_columns() {
        let column_sets = vec![
            MoxelColumnSet {
                id: None,
                size: 1,
                columns: vec![MoxelColumn {
                    index: 0,
                    format_index: 1,
                }],
            },
            MoxelColumnSet {
                id: Some("5c3926f2-4223-4ca7-a6a7-7160301c991d".to_string()),
                size: 4,
                columns: vec![
                    MoxelColumn {
                        index: 0,
                        format_index: 2,
                    },
                    MoxelColumn {
                        index: 4,
                        format_index: 6,
                    },
                ],
            },
        ];
        let settings = MoxelPrintSettings::default();

        assert_eq!(
            moxel_default_format_index(&column_sets, Some(&settings), false, 21),
            Some(6)
        );
        assert_eq!(
            moxel_default_format_index(&column_sets, Some(&settings), true, 21),
            Some(21)
        );
        assert_eq!(
            moxel_default_format_index(&column_sets, None, false, 21),
            None
        );
    }

    #[test]
    fn formats_moxel_picture_drawing_and_normalized_picture_index() {
        let drawing = parse_moxel_drawing("{{0,31},5,1,20,24,6,1,20,88,70,1,1,1,0}").unwrap();

        assert_eq!(drawing.format_index, 31);
        assert_eq!(drawing.begin_row, 20);
        assert_eq!(drawing.begin_row_offset, 6);
        assert_eq!(drawing.end_row, 20);
        assert_eq!(drawing.end_row_offset, 70);
        assert_eq!(drawing.begin_column, 1);
        assert_eq!(drawing.begin_column_offset, 24);
        assert_eq!(drawing.end_column, 1);
        assert_eq!(drawing.end_column_offset, 88);
        assert!(!drawing.auto_size);
        assert_eq!(drawing.picture_size, "Stretch");
        assert_eq!(drawing.z_order, 1);
        assert_eq!(drawing.picture_index, 1);

        let mut xml = String::new();
        push_moxel_drawing_xml(&mut xml, &drawing);
        assert!(xml.contains("<drawingType>Picture</drawingType>"));
        assert!(xml.contains("<formatIndex>31</formatIndex>"));
        assert!(xml.contains("<beginRow>20</beginRow>"));
        assert!(xml.contains("<beginRowOffset>6</beginRowOffset>"));
        assert!(xml.contains("<endRowOffset>70</endRowOffset>"));
        assert!(xml.contains("<beginColumn>1</beginColumn>"));
        assert!(xml.contains("<beginColumnOffset>24</beginColumnOffset>"));
        assert!(xml.contains("<endColumnOffset>88</endColumnOffset>"));
        assert!(xml.contains("<autoSize>false</autoSize>"));
        assert!(xml.contains("<pictureSize>Stretch</pictureSize>"));
        assert!(xml.contains("<zOrder>1</zOrder>"));
        assert!(xml.contains("<pictureIndex>1</pictureIndex>"));

        let pictures = parse_moxel_pictures(
            &[
                "1",
                "{4,1,{0,b5e73fbe-499c-4666-a482-0ef399c97c1e},\"\",-1,-1,0,0,\"\"}",
            ],
            &BTreeMap::from([(
                "b5e73fbe-499c-4666-a482-0ef399c97c1e".to_string(),
                "CommonPicture.Предупреждение32".to_string(),
            )]),
        );
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].index, 0);
        assert_eq!(
            pictures[0].ref_name.as_deref(),
            Some("v8ui:Предупреждение32")
        );
    }

    #[test]
    fn formats_moxel_detail_parameters_and_row_format_offsets() {
        let spreadsheet = parse_moxel_spreadsheet_text(
            "{8,1,12,{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0},{128,72},{0},0,{0,0},{0,0},{0,0},{0,0},{0,0},{0,0},1,2,3,0,0,3,0,{0,1},1,{0,1},2,{0,1},1,0,3,0,{16,1,{1,1,{\"ru\",\"Файл\"}},0},1,{16,1,{1,1,{\"ru\",\"Причина\"}},0},2,{16,1,{1,1,{\"ru\",\"Размещение\"}},0},2,2,3,0,{24,3,\"Версия\",{1,1,{\"\",\"Название\"}},0},1,{16,4,{1,1,{\"\",\"Ошибка\"}},0},2,{16,4,{1,1,{\"\",\"Размещение\"}},0},{2,\"Заголовок\",{1,{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},\"Строка\",{1,{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0}},7,{1,0},{64,54},{67158528,0,3,1,1},{49664,0,3,1},{128,318},{128,363},{128,324},1,{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,\"Arial\",1,100},2,{3,3,{-1}},{3,3,{-3}}}",
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(
            xml.contains("<index>2</index>\r\n\t\t<row>\r\n\t\t\t<formatIndex>5</formatIndex>")
        );
        assert!(xml.contains(
            "<f>6</f>\r\n\t\t\t\t\t<parameter>Название</parameter>\r\n\t\t\t\t\t<detailParameter>Версия</detailParameter>"
        ));
        assert!(xml.contains("\t<format>\r\n\t\t<font>0</font>\r\n\t</format>"));
        assert!(xml.contains("\t<format>\r\n\t\t<height>54</height>\r\n\t</format>"));
        assert!(xml.contains(
            "\t<format>\r\n\t\t<verticalAlignment>Top</verticalAlignment>\r\n\t\t<textPlacement>Wrap</textPlacement>\r\n\t\t<fillType>Parameter</fillType>\r\n\t\t<hyperLink>true</hyperLink>\r\n\t</format>"
        ));
        assert!(!xml.contains("<line width=\"1\""));
    }

    #[test]
    fn writes_xdto_package_body_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{1,\r\n{{3,\r\n{{1,0,{uuid}}},\"Exchange\",{{1,\"en\",\"Exchange\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\"http://example.com/exchange\"}},0}}"
            )
            .as_bytes(),
        );
        let package = b"\xef\xbb\xbf<package xmlns=\"http://v8.1c.ru/8.1/xdto\"/>";
        let body = deflate_for_test(package);
        let rows = vec![
            ConfigRow {
                file_name: uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{uuid}.0"),
                part_no: 0,
                data_size: body.len() as i64,
                binary_hex: encode_hex_for_test(&body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        assert_eq!(
            fs::read(root.join("XDTOPackages/Exchange/Ext/Package.bin")).unwrap(),
            package
        );
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("XDTOPackages/Exchange/Ext/Package.bin")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_role_rights_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let role_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let catalog_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let register_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let field_uuid = "77777777-7777-4777-8777-777777777777";
        let configuration_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let web_service_uuid = "99999999-9999-4999-9999-999999999999";
        let operation_uuid = "88888888-8888-4888-8888-888888888888";
        let role_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{6,\r\n{{3,\r\n{{1,0,{role_uuid}}},\"Editor\",{{1,\"en\",\"Editor\"}},\"\"}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let configuration_metadata = deflate_for_test(
            format!(
                "{{2,\r\n{{{configuration_uuid}}},7,\r\n{{9cd510cd-abfc-11d4-9434-004095e12fc7,\r\n{{1,\r\n{{68,\r\n{{0,\r\n{{3,\r\n{{1,0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee}},\"DemoApp\",{{1,\"en\",\"Demo app\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let register_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{33,dddddddd-dddd-4ddd-dddd-dddddddddddd,\r\n{{3,\r\n{{1,0,{register_uuid}}},\"Prices\",{{1,\"en\",\"Prices\"}},\"\"}},\r\n{{3,\r\n{{1,0,{field_uuid}}},\"ВерсияОбъекта\",{{1,\"ru\",\"Версия объекта\"}},\"\"}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let web_service_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{4,\"http://example.com/svc\",\r\n{{3,\r\n{{1,0,{web_service_uuid}}},\"RemoteApi\",{{1,\"en\",\"Remote api\"}},\"\"}},{{0,0}},\"RemoteApi.1cws\",{{0}},0,20}},1,\r\n{{11111111-1111-4111-8111-111111111111,1,\r\n{{\r\n{{1,\r\n{{3,\r\n{{1,0,{operation_uuid}}},\"Ping\",{{1,\"en\",\"Ping\"}},\"\"}},{{0,\"http://www.w3.org/2001/XMLSchema\",\"boolean\"}},0,0,\"Ping\",1}},1,\r\n{{22222222-2222-4222-8222-222222222222,0}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let rights_text = r##"{10,{4,
{{1,@catalog_uuid@,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,1,33200740-82b0-4de7-8556-d3fb25ca4328,1,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}},
{{1,@configuration_uuid@,0,0},{0,d066966a-ff6a-4a41-bd68-6191cab083bc,1}},
{{1,@operation_uuid@,0,0},{0,c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1,1}},
{{1,@register_uuid@,0,0},{1,4,1c87578f-9e09-4ec0-a991-5629c87b1588,1,287b74b8-3a66-4a76-ba27-4f1f6a93770e,1,24abfe06-289a-48c5-8bb4-032c733e45c5,1,c0028105-4cc1-41ca-aef1-bfbd8fc8f8c4,-1,3,
{{1c87578f-9e09-4ec0-a991-5629c87b1588,{1,{1,"#Если &Allowed #Тогда ""OK""",0}}},
{287b74b8-3a66-4a76-ba27-4f1f6a93770e,{1,{1,"ГДЕ Owner = &User",0}}},
{24abfe06-289a-48c5-8bb4-032c733e45c5,{2,{1,"",0},{1,"ГДЕ ЛОЖЬ",1,{{0},{0,@field_uuid@}}}}}}}}},
{1,{"OnlyAllowed","// Template & ""quoted"""}},4294967295,1,0,4294967295}"##
            .replace("@catalog_uuid@", catalog_uuid)
            .replace("@configuration_uuid@", configuration_uuid)
            .replace("@operation_uuid@", operation_uuid)
            .replace("@register_uuid@", register_uuid)
            .replace("@field_uuid@", field_uuid);
        let rights = deflate_for_test(rights_text.as_bytes());
        let rows = vec![
            ConfigRow {
                file_name: configuration_uuid.to_string(),
                part_no: 0,
                data_size: configuration_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&configuration_metadata),
            },
            ConfigRow {
                file_name: role_uuid.to_string(),
                part_no: 0,
                data_size: role_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&role_metadata),
            },
            ConfigRow {
                file_name: format!("{role_uuid}.0"),
                part_no: 0,
                data_size: rights.len() as i64,
                binary_hex: encode_hex_for_test(&rights),
            },
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: register_uuid.to_string(),
                part_no: 0,
                data_size: register_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&register_metadata),
            },
            ConfigRow {
                file_name: web_service_uuid.to_string(),
                part_no: 0,
                data_size: web_service_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&web_service_metadata),
            },
        ];
        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml = fs::read_to_string(root.join("Roles/Editor/Ext/Rights.xml")).unwrap();
        assert!(xml.contains(r#"<Rights xmlns="http://v8.1c.ru/8.2/roles""#));
        assert!(
            xml.find("<name>InformationRegister.Prices</name>").unwrap()
                < xml.find("<name>Catalog.Products</name>").unwrap()
        );
        assert!(xml.contains("<name>Configuration.DemoApp</name>"));
        assert!(xml.contains("<name>MainWindowModeNormal</name>"));
        assert!(xml.contains("<name>WebService.RemoteApi.Operation.Ping</name>"));
        assert_eq!(
            role_right_name("3b869658-ebc9-49ff-9bb3-e7c59686f538"),
            Some("InteractiveActivate")
        );
        assert_eq!(
            role_right_name("5e664189-f0ee-439c-bdc5-eb81cca41ddf"),
            Some("InteractiveExecute")
        );
        assert_eq!(
            role_right_name("84487e82-eb6c-4c51-ae16-3a6db17e886d"),
            Some("InteractiveStart")
        );
        assert!(xml.contains("<name>Read</name>"));
        assert!(xml.contains("<setForNewObjects>true</setForNewObjects>"));
        assert!(xml.contains("<restrictionByCondition>"));
        assert!(xml.contains("#Если &amp;Allowed #Тогда \"OK\""));
        assert!(xml.contains("<name>Update</name>"));
        assert!(xml.contains("ГДЕ Owner = &amp;User"));
        assert!(xml.contains("<name>TotalsControl</name>"));
        assert!(xml.contains("<field>ВерсияОбъекта</field>"));
        assert!(xml.contains("ГДЕ ЛОЖЬ"));
        assert!(xml.contains("<name>Delete</name>"));
        assert!(xml.contains("<value>false</value>"));
        assert!(xml.contains("<name>Insert</name>"));
        assert!(xml.contains("<name>View</name>"));
        assert!(xml.contains("<restrictionTemplate>"));
        assert!(xml.contains("<name>OnlyAllowed</name>"));
        assert!(xml.contains("// Template &amp; \"quoted\""));
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{role_uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("Roles/Editor/Ext/Rights.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_subsystem_command_interface_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let subsystem_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let catalog_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let process_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let processor_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let command_uuid = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let unknown_uuid = "ffffffff-ffff-4fff-ffff-ffffffffffff";
        let subsystem_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{22,\r\n{{3,\r\n{{1,0,{subsystem_uuid}}},\"Admin\",{{1,\"en\",\"Admin\"}},\"\"}},1}}\r\n}}"
            )
            .as_bytes(),
        );
        let catalog_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let process_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{30,\r\n{{3,\r\n{{1,0,{process_uuid}}},\"TaskFlow\",{{1,\"en\",\"Task flow\"}},\"\"}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let processor_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{17,835478cc-434a-480c-ad61-99801cd685ed,92b15a50-2234-40c9-af13-3d746d4b870f,\r\n{{0,\r\n{{3,\r\n{{1,0,{processor_uuid}}},\"Scanning\",{{1,\"en\",\"Scanning\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},00000000-0000-0000-0000-000000000000,1,0,86df3c66-2c45-49c1-9e7d-5d1892acb646,6ae2f4ed-a57a-49ed-a854-8795bf1e1519,00000000-0000-0000-0000-000000000000,\r\n{{0}},\r\n{{0}}\r\n}},5,\r\n{{45556acb-826a-4f73-898a-6025fc9536e1,1,\r\n{{\r\n{{0,\r\n{{1,\r\n{{2,{command_uuid},078a6af8-d22c-4248-9c33-7e90075a3d2c}},\r\n{{9,\r\n{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},3,\r\n{{1,\"en\",\"Scan sheet\"}},1,\r\n{{0,0,0}},0,\r\n{{1,bc80566a-86a5-4e87-acd4-872239385a2e}},\r\n{{\"Pattern\"}},\r\n{{3,\r\n{{1,0,{command_uuid}}},\"ScanSheet\",{{1,\"en\",\"Scan sheet\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0}}\r\n}}\r\n}},0}}\r\n}}\r\n}},\r\n{{d5b0e5ed-256d-401c-9c36-f630cafd8a62,3,0f193c89-b664-448e-bed3-2147430367f7,4c9b2506-75a8-47d3-a5d5-d946088ba14a,36eacaa1-2efd-49c0-82de-2f8972535bf2}},\r\n{{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,0}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let command_interface = deflate_for_test(
            format!(
                "{{7,1,5,{{0,{catalog_uuid}}},{{0,{{0,{{\"B\",0}},0}}}},{{1,{process_uuid}}},{{0,{{0,{{\"B\",1}},0}}}},{{0,{command_uuid}}},{{0,{{0,{{\"B\",1}},0}}}},{{100,{unknown_uuid}}},{{0,{{0,{{\"B\",0}},0}}}},{{0}},{{0,{{0,{{\"B\",0}},0}}}},0,0,0,0,0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: subsystem_uuid.to_string(),
                part_no: 0,
                data_size: subsystem_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&subsystem_metadata),
            },
            ConfigRow {
                file_name: format!("{subsystem_uuid}.1"),
                part_no: 0,
                data_size: command_interface.len() as i64,
                binary_hex: encode_hex_for_test(&command_interface),
            },
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_metadata),
            },
            ConfigRow {
                file_name: process_uuid.to_string(),
                part_no: 0,
                data_size: process_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&process_metadata),
            },
            ConfigRow {
                file_name: processor_uuid.to_string(),
                part_no: 0,
                data_size: processor_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&processor_metadata),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml =
            fs::read_to_string(root.join("Subsystems/Admin/Ext/CommandInterface.xml")).unwrap();
        assert!(xml.contains(r#"<Command name="Catalog.Products.StandardCommand.OpenList">"#));
        assert!(
            xml.contains(r#"<Command name="BusinessProcess.TaskFlow.StandardCommand.Create">"#)
        );
        assert!(xml.contains(r#"<Command name="DataProcessor.Scanning.Command.ScanSheet">"#));
        assert!(xml.contains(&format!(r#"<Command name="100:{unknown_uuid}">"#)));
        assert!(xml.contains(r#"<Command name="0">"#));
        assert!(xml.contains("<xr:Common>true</xr:Common>"));
        assert!(xml.contains("<xr:Common>false</xr:Common>"));
        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{subsystem_uuid}.1"))
            .unwrap();
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("Subsystems/Admin/Ext/CommandInterface.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_exchange_plan_content_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let plan_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let catalog_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let register_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let plan = deflate_for_test(
            format!(
                "{{1,\r\n{{37,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,\r\n{{3,\r\n{{1,0,{plan_uuid}}},\"Sync\",{{1,\"en\",\"Sync\"}},\"\"}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let catalog = deflate_for_test(
            format!(
                "{{1,\r\n{{57,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Customers\",{{1,\"en\",\"Customers\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let register = deflate_for_test(
            format!(
                "{{1,\r\n{{33,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,\r\n{{0,\r\n{{3,\r\n{{1,0,{register_uuid}}},\"Prices\",{{1,\"en\",\"Prices\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let content =
            deflate_for_test(format!("{{2,2,{catalog_uuid},0,{register_uuid},1}}").as_bytes());
        let rows = vec![
            ConfigRow {
                file_name: plan_uuid.to_string(),
                part_no: 0,
                data_size: plan.len() as i64,
                binary_hex: encode_hex_for_test(&plan),
            },
            ConfigRow {
                file_name: format!("{plan_uuid}.1"),
                part_no: 0,
                data_size: content.len() as i64,
                binary_hex: encode_hex_for_test(&content),
            },
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog.len() as i64,
                binary_hex: encode_hex_for_test(&catalog),
            },
            ConfigRow {
                file_name: register_uuid.to_string(),
                part_no: 0,
                data_size: register.len() as i64,
                binary_hex: encode_hex_for_test(&register),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml = fs::read_to_string(root.join("ExchangePlans/Sync/Ext/Content.xml")).unwrap();
        assert!(xml.contains("<Metadata>Catalog.Customers</Metadata>"));
        assert!(xml.contains("<AutoRecord>Deny</AutoRecord>"));
        assert!(xml.contains("<Metadata>InformationRegister.Prices</Metadata>"));
        assert!(xml.contains("<AutoRecord>Auto</AutoRecord>"));
        let content_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{plan_uuid}.1"))
            .unwrap();
        assert_eq!(
            content_row.source_asset_path.as_deref(),
            Some("ExchangePlans/Sync/Ext/Content.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_config_dump_info_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let versions = deflate_for_test(
            b"{1,2,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\",cccccccc-cccc-4ccc-cccc-cccccccccccc}",
        );
        let row = ConfigRow {
            file_name: "versions".to_string(),
            part_no: 0,
            data_size: versions.len() as i64,
            binary_hex: encode_hex_for_test(&versions),
        };

        let dumped = dump_table_rows(&root, "Config", vec![row], false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml = fs::read_to_string(root.join("ConfigDumpInfo.xml")).unwrap();
        assert!(xml.contains("<ConfigDumpInfo"));
        assert!(xml.contains(r#"format="Hierarchical""#));
        assert!(xml.contains("<ConfigVersions/>"));
        let versions_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == "versions")
            .unwrap();
        assert_eq!(
            versions_row.source_asset_path.as_deref(),
            Some("ConfigDumpInfo.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_business_process_flowchart_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let process_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let start_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let completion_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{30,\r\n{{3,\r\n{{1,0,{process_uuid}}},\"Approval\",{{1,\"en\",\"Approval\"}},\"\"}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let style = "{7,{3,4,{0}},{3,3,{-22}},{3,3,{-3}},{7,1,0,{0},1,100},{1,0},1,1,1,0,0,0,0,0}";
        let line_style =
            "{7,{3,0,{0}},{3,3,{-22}},{3,3,{-3}},{7,1,0,{0},1,100},{1,0},1,1,1,1,0,0,0,0}";
        let border = "{4,0,{0},1,1,0,e45c0cd8-a878-4bcb-8e1a-af934481e1cc,0}";
        let start_head = format!("{{{{4,1,{{1,0}},\"Start\",1}},4,{start_uuid},0}}");
        let completion_head = format!("{{{{4,3,{{1,0}},\"Done\",3}},4,{completion_uuid},0}}");
        let line_head = "{4,2,{1,0},\"Line\",2}";
        let start_geometry = format!("{{{style},5,10,20,50,60}}");
        let completion_geometry = format!("{{{style},5,70,80,110,120}}");
        let start_shape = format!("{{{{{start_geometry},1}}}}");
        let completion_shape = format!("{{{{{completion_geometry},1}}}}");
        let line_geometry = format!("{{{line_style},6,2,50,60,70,80,{border},0,4,2,0,0,1}}");
        let line_shape = format!("{{{line_geometry}}}");
        let start_item = format!("{{{start_head},2,{start_shape},{{0}}}}");
        let line_item = format!("{{{line_head},3,1,0,3,0,{line_shape}}}");
        let completion_item =
            format!("{{{completion_head},2,{completion_shape},{{1,{{0,\"OnDone\"}}}}}}");
        let flowchart = deflate_for_test(
            format!(
                "{{5,{{{{1,{style},1,20,20}}}},3,2,{start_item},1,{line_item},3,{completion_item},4}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: process_uuid.to_string(),
                part_no: 0,
                data_size: metadata.len() as i64,
                binary_hex: encode_hex_for_test(&metadata),
            },
            ConfigRow {
                file_name: format!("{process_uuid}.7"),
                part_no: 0,
                data_size: flowchart.len() as i64,
                binary_hex: encode_hex_for_test(&flowchart),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml =
            fs::read_to_string(root.join("BusinessProcesses/Approval/Ext/Flowchart.xml")).unwrap();
        assert!(xml.contains(r#"<Start id="1" uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">"#));
        assert!(xml.contains(r#"<ConnectionLine id="2">"#));
        assert!(xml.contains("<Item>Start</Item>"));
        assert!(xml.contains("<Item>Done</Item>"));
        assert!(xml.contains(r#"<Completion id="3" uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">"#));
        assert!(xml.contains(r#"<Event name="OnComplete">OnDone</Event>"#));
        let flowchart_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{process_uuid}.7"))
            .unwrap();
        assert_eq!(
            flowchart_row.source_asset_path.as_deref(),
            Some("BusinessProcesses/Approval/Ext/Flowchart.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_nested_subsystem_metadata_and_help_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let parent_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let child_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let parent_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{22,\r\n{{3,\r\n{{1,0,{parent_uuid}}},\"StandardSubsystems\",{{1,\"en\",\"Standard subsystems\"}},\"\"}},1,{child_uuid}}}\r\n}}"
            )
            .as_bytes(),
        );
        let child_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{22,\r\n{{3,\r\n{{1,0,{child_uuid}}},\"Users\",{{1,\"en\",\"Users\"}},\"\"}},1}}\r\n}}"
            )
            .as_bytes(),
        );
        let help = deflate_for_test(b"{5,1,\"ru\",{#base64:PGgxPlVzZXJzPC9oMT4=},0}");
        let rows = vec![
            ConfigRow {
                file_name: parent_uuid.to_string(),
                part_no: 0,
                data_size: parent_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&parent_metadata),
            },
            ConfigRow {
                file_name: child_uuid.to_string(),
                part_no: 0,
                data_size: child_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&child_metadata),
            },
            ConfigRow {
                file_name: format!("{child_uuid}.0"),
                part_no: 0,
                data_size: help.len() as i64,
                binary_hex: encode_hex_for_test(&help),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 2);
        assert_eq!(dumped.source_asset_rows, 1);
        assert!(root.join("Subsystems/StandardSubsystems.xml").exists());
        assert!(
            root.join("Subsystems/StandardSubsystems/Subsystems/Users.xml")
                .exists()
        );
        assert!(
            fs::read_to_string(
                root.join("Subsystems/StandardSubsystems/Subsystems/Users/Ext/Help.xml")
            )
            .unwrap()
            .contains("<Page>ru</Page>")
        );
        assert_eq!(
            fs::read(root.join("Subsystems/StandardSubsystems/Subsystems/Users/Ext/Help/ru.html"))
                .unwrap(),
            b"<h1>Users</h1>"
        );
        let child_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == child_uuid)
            .unwrap();
        assert_eq!(
            child_row.metadata_xml_path.as_deref(),
            Some("Subsystems/StandardSubsystems/Subsystems/Users.xml")
        );
        let help_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{child_uuid}.0"))
            .unwrap();
        assert_eq!(
            help_row.source_asset_path.as_deref(),
            Some("Subsystems/StandardSubsystems/Subsystems/Users/Ext/Help.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disambiguates_colliding_metadata_object_codes() {
        let enum_uuid = "11111111-1111-4111-8111-111111111111";
        let enum_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{20,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,cccccccc-cccc-4ccc-cccc-cccccccccccc,dddddddd-dddd-4ddd-dddd-dddddddddddd,\r\n{{0,\r\n{{3,\r\n{{1,0,{enum_uuid}}},\"Status\",{{1,\"en\",\"Status\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let report_uuid = "22222222-2222-4222-8222-222222222222";
        let report_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{20,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,\r\n{{0,\r\n{{3,\r\n{{1,0,{report_uuid}}},\"SalesReport\",{{1,\"en\",\"Sales report\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let subsystem_uuid = "33333333-3333-4333-8333-333333333333";
        let subsystem_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{22,\r\n{{3,\r\n{{1,0,{subsystem_uuid}}},\"Sales\",{{1,\"en\",\"Sales\"}},\"\"}},1}}\r\n}}"
            )
            .as_bytes(),
        );
        let accounting_uuid = "44444444-4444-4444-8444-444444444444";
        let accounting_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{22,22,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,cccccccc-cccc-4ccc-cccc-cccccccccccc,dddddddd-dddd-4ddd-dddd-dddddddddddd,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee,ffffffff-ffff-4fff-ffff-ffffffffffff,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,66666666-6666-4666-8666-666666666666,77777777-7777-4777-8777-777777777777,\r\n{{0,\r\n{{3,\r\n{{1,0,{accounting_uuid}}},\"Ledger\",{{1,\"en\",\"Ledger\"}},\"\"}}\r\n}},1}}\r\n}}"
            )
            .as_bytes(),
        );
        let task_uuid = "55555555-5555-4555-8555-555555555555";
        let task_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{33,\r\n{{3,\r\n{{1,0,{task_uuid}}},\"Task\",{{1,\"en\",\"Task\"}},\"\"}},0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa}}\r\n}}"
            )
            .as_bytes(),
        );

        assert_eq!(
            extract_metadata_source_xml(
                &enum_blob,
                enum_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("Enums").join("Status.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(
                &report_blob,
                report_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("Reports").join("SalesReport.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(
                &subsystem_blob,
                subsystem_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("Subsystems").join("Sales.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(
                &accounting_blob,
                accounting_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("AccountingRegisters").join("Ledger.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(
                &task_blob,
                task_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("Tasks").join("Task.xml")
        );

        let catalog_uuid = "66666666-6666-4666-8666-666666666666";
        let catalog_form_uuid = "77777777-7777-4777-8777-777777777777";
        let catalog_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\"}}\r\n}},2,1,{{0,0}},1,0,0,0,3,1,10,1,{catalog_form_uuid},{catalog_form_uuid},1,{{1,{{1,9,{{-13}},510405d3-2a0c-4fea-960a-7fee59b32f,{{14,25,1183c14f-f814-49c6-9233-a3c26b3f64cf}}}}}}}}\r\n}}"
            )
            .as_bytes(),
        );
        assert_eq!(
            extract_metadata_source_xml(
                &catalog_blob,
                catalog_uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap()
            .relative_path,
            PathBuf::from("Catalogs").join("Products.xml")
        );
    }

    #[test]
    fn ignores_report_and_indexes_task_rows_in_generated_type_index() {
        let report_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let report_object_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let report_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{20,{report_object_type_id},cccccccc-cccc-4ccc-cccc-cccccccccccc,\r\n{{0,\r\n{{3,\r\n{{1,0,{report_uuid}}},\"SalesReport\",{{1,\"en\",\"Sales report\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let task_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let task_object_type_id = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let task_ref_type_id = "ffffffff-ffff-4fff-ffff-ffffffffffff";
        let task_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{33,\r\n{{3,\r\n{{1,0,{task_uuid}}},\"Task\",{{1,\"en\",\"Task\"}},\"\"}},0,{task_object_type_id},11111111-1111-4111-8111-111111111111,{task_ref_type_id}}}\r\n}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: report_uuid.to_string(),
                part_no: 0,
                data_size: report_blob.len() as i64,
                binary_hex: encode_hex_for_test(&report_blob),
            },
            ConfigRow {
                file_name: task_uuid.to_string(),
                part_no: 0,
                data_size: task_blob.len() as i64,
                binary_hex: encode_hex_for_test(&task_blob),
            },
        ];

        let index = build_metadata_type_index(&rows);

        assert!(!index.contains_key(report_object_type_id));
        assert_eq!(
            index.get(task_object_type_id).map(String::as_str),
            Some("cfg:TaskObject.Task")
        );
        assert_eq!(
            index.get(task_ref_type_id).map(String::as_str),
            Some("cfg:TaskRef.Task")
        );
    }

    #[test]
    fn extracts_common_module_xml_from_metadata_blob() {
        let uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let blob = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{12,\r\n{{3,\r\n{{1,0,{uuid}}},\"SalesModule\",{{1,\"ru\",\"Модуль продаж\"}},\"Module comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0,1,0,1,1,1,2,0}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_common_module_xml_properties(&extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("CommonModules").join("SalesModule.xml")
        );
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "SalesModule");
        assert_eq!(properties.comment, "Module comment");
        assert_eq!(properties.synonyms[0].content, "Модуль продаж");
        assert!(properties.global);
        assert!(properties.client_managed_application);
        assert!(properties.server);
        assert!(!properties.external_connection);
        assert!(!properties.client_ordinary_application);
        assert!(!properties.server_call);
        assert!(properties.privileged);
        assert_eq!(
            properties.return_values_reuse,
            ReturnValuesReuse::DuringSession
        );
    }

    #[test]
    fn extracts_configuration_xml_from_root_metadata_blob() {
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let header_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let blob = deflate_for_test(
            format!(
                "{{2,\r\n{{{uuid}}},1,\r\n{{9cd510cd-abfc-11d4-9434-004095e12fc7,\r\n{{1,\r\n{{68,\r\n{{0,\r\n{{3,\r\n{{1,0,{header_uuid}}},\"DemoApp\",{{1,\"en\",\"Demo app\"}},\"Configuration comment\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();

        assert_eq!(extracted.relative_path, PathBuf::from("Configuration.xml"));
        assert_eq!(properties.kind, "Configuration");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "DemoApp");
        assert_eq!(properties.comment, "Configuration comment");
    }

    #[test]
    fn extracts_constant_xml_from_metadata_blob() {
        let uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{16,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"Feature flag\",0,0,00000000-0000-0000-0000-000000000000,0}},{{\"Pattern\",{{\"B\"}}}}\r\n}},0,\r\n{{0}},\r\n{{0}},0,\"\",0,\r\n{{\"U\"}},\r\n{{\"U\"}},0,00000000-0000-0000-0000-000000000000,2,0,\r\n{{5006,0}},\r\n{{3,0,0}},\r\n{{0,0}},0,\r\n{{0}},\r\n{{\"S\",\"\"}},0,0,0}},00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,1,\r\n{{0}},1,0}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("Constants").join("UseFeature.xml")
        );
        assert_eq!(properties.kind, "Constant");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "UseFeature");
        assert_eq!(properties.comment, "Feature flag");
        assert!(String::from_utf8_lossy(&extracted.xml).contains("xs:boolean"));
        assert!(String::from_utf8_lossy(&extracted.xml).contains("<UseStandardCommands>true"));
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn extracts_constant_xml_with_builtin_uuid_types() {
        for (uuid, name, type_uuid, expected_type) in [
            (
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                "StoredValue",
                "e199ca70-93cf-46ce-a54b-6edc88c3a296",
                "v8:ValueStorage",
            ),
            (
                "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                "VersionUuid",
                "fc01b5df-97fe-449b-83d4-218a090e681e",
                "v8:UUID",
            ),
            (
                "cccccccc-cccc-4ccc-cccc-cccccccccccc",
                "FixedStructureValue",
                "3ee983d7-ace7-40f9-bb7e-2e916fcddd56",
                "v8:FixedStructure",
            ),
            (
                "dddddddd-dddd-4ddd-dddd-dddddddddddd",
                "FixedArrayValue",
                "4500381b-db30-4a10-9db4-990038032acf",
                "v8:FixedArray",
            ),
            (
                "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee",
                "FixedMapValue",
                "220455ea-6c85-4513-996f-bbe79ed07774",
                "v8:FixedMap",
            ),
            (
                "ffffffff-ffff-4fff-ffff-ffffffffffff",
                "ExchangePlanNode",
                "0a52f9de-73ea-4507-81e8-66217bead73a",
                "cfg:ExchangePlanRef",
            ),
        ] {
            let blob = deflate_for_test(
                format!(
                    "{{1,\r\n{{16,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"{name}\",{{1,\"en\",\"{name}\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},{{\"Pattern\",{{\"#\",{type_uuid}}}}}\r\n}},0,\r\n{{0}},\r\n{{0}},0,\"\",0,\r\n{{\"U\"}},\r\n{{\"U\"}},0,00000000-0000-0000-0000-000000000000,2,0,\r\n{{5006,0}},\r\n{{3,0,0}},\r\n{{0,0}},0,\r\n{{0}},\r\n{{\"S\",\"\"}},0,0,0}},00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,1,\r\n{{0}},1,0}}\r\n}}\r\n}}"
                )
                .as_bytes(),
            );

            let extracted = extract_metadata_source_xml(
                &blob,
                uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap();
            let xml = String::from_utf8_lossy(&extracted.xml);

            assert_eq!(
                extracted.relative_path,
                PathBuf::from("Constants").join(format!("{name}.xml"))
            );
            assert!(xml.contains(&format!("<v8:Type>{expected_type}</v8:Type>")));
        }
    }

    #[test]
    fn extracts_session_parameter_xml_with_type_from_metadata_blob() {
        let uuid = "11111111-1111-4111-8111-111111111111";
        let catalog_ref_type_id = "22222222-2222-4222-8222-222222222222";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"CurrentUser\",{{1,\"en\",\"Current user\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{\"Pattern\",{{\"#\",{catalog_ref_type_id}}}}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let type_index = BTreeMap::from([(
            catalog_ref_type_id.to_string(),
            "cfg:CatalogRef.Users".to_string(),
        )]);

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &type_index,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();
        let xml = String::from_utf8_lossy(&extracted.xml);

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("SessionParameters").join("CurrentUser.xml")
        );
        assert_eq!(properties.kind, "SessionParameter");
        assert_eq!(properties.uuid, uuid);
        assert!(xml.contains("<v8:Type>cfg:CatalogRef.Users</v8:Type>"));
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn extracts_common_attribute_xml_with_type_from_metadata_blob() {
        let uuid = "33333333-3333-4333-8333-333333333333";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{5,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"ExternalCode\",{{1,\"en\",\"External code\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{\"Pattern\",{{\"S\",50,1}}}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();
        let xml = String::from_utf8_lossy(&extracted.xml);

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("CommonAttributes").join("ExternalCode.xml")
        );
        assert_eq!(properties.kind, "CommonAttribute");
        assert_eq!(properties.uuid, uuid);
        assert!(xml.contains("<v8:Type>xs:string</v8:Type>"));
        assert!(xml.contains("<v8:Length>50</v8:Length>"));
        assert!(xml.contains("<v8:AllowedLength>Fixed</v8:AllowedLength>"));
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn extracts_functional_option_xml_from_metadata_blob() {
        let uuid = "44444444-4444-4444-8444-444444444444";
        let location_uuid = "55555555-5555-4555-8555-555555555555";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"Feature flag\",0,0,00000000-0000-0000-0000-000000000000,0}},{location_uuid},\r\n{{0,0}},1}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("FunctionalOptions").join("UseFeature.xml")
        );
        assert_eq!(properties.kind, "FunctionalOption");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "UseFeature");
        assert_eq!(properties.comment, "Feature flag");
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn extracts_functional_options_parameter_xml_without_confusing_defined_type() {
        let uuid = "66666666-6666-4666-8666-666666666666";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"UseFeatureFor\",{{1,\"en\",\"Use feature for\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{0,1,\r\n{{\"#\",157fa490-4ce9-11d4-9415-008048da11f9,\r\n{{1,77777777-7777-4777-8777-777777777777}}\r\n}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("FunctionalOptionsParameters").join("UseFeatureFor.xml")
        );
        assert_eq!(properties.kind, "FunctionalOptionsParameter");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "UseFeatureFor");
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn extracts_additional_simple_service_metadata_xml_from_blobs() {
        let language_uuid = "11111111-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let language_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{language_uuid}}},\"English\",{{1,\"en\",\"English\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\"en\"}},0}}"
            )
            .as_bytes(),
        );
        let xdto_uuid = "22222222-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let xdto_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{1,\r\n{{3,\r\n{{1,0,{xdto_uuid}}},\"Exchange\",{{1,\"en\",\"Exchange\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\"http://example.com/exchange\"}},0}}"
            )
            .as_bytes(),
        );
        let http_uuid = "33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let http_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\"api\",\r\n{{3,\r\n{{1,0,{http_uuid}}},\"Api\",{{1,\"en\",\"API\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},2,20}},0}}"
            )
            .as_bytes(),
        );
        let storage_uuid = "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let storage_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{0,\r\n{{3,\r\n{{1,0,{storage_uuid}}},\"UserSettings\",{{1,\"en\",\"User settings\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}},2,\r\n{{0}},\r\n{{0}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let job_uuid = "55555555-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let job_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{job_uuid}}},\"LoadRates\",{{1,\"en\",\"Load rates\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\"\",\"Load rates\",1,1,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa,\"LoadRates\",3,10}},0}}"
            )
            .as_bytes(),
        );

        for (blob, uuid, expected_kind, expected_path) in [
            (
                &language_blob,
                language_uuid,
                "Language",
                PathBuf::from("Languages").join("English.xml"),
            ),
            (
                &xdto_blob,
                xdto_uuid,
                "XDTOPackage",
                PathBuf::from("XDTOPackages").join("Exchange.xml"),
            ),
            (
                &http_blob,
                http_uuid,
                "HTTPService",
                PathBuf::from("HTTPServices").join("Api.xml"),
            ),
            (
                &storage_blob,
                storage_uuid,
                "SettingsStorage",
                PathBuf::from("SettingsStorages").join("UserSettings.xml"),
            ),
            (
                &job_blob,
                job_uuid,
                "ScheduledJob",
                PathBuf::from("ScheduledJobs").join("LoadRates.xml"),
            ),
        ] {
            let extracted = extract_metadata_source_xml(
                blob,
                uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap();
            let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
            let repacked = pack_simple_metadata_blob_from_xml(blob, &extracted.xml).unwrap();

            assert_eq!(extracted.relative_path, expected_path);
            assert_eq!(properties.kind, expected_kind);
            assert_eq!(properties.uuid, uuid);
            assert!(!repacked.blob.is_empty());
        }
    }

    #[test]
    fn extracts_sfc_metadata_family_xml_from_blob_shapes() {
        let cases = vec![
            (
                "66666666-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                deflate_for_test(
                    b"{1,\r\n{0,\r\n{3,\r\n{1,0,66666666-aaaa-4aaa-8aaa-aaaaaaaaaaaa},\"MessageExchange\",{1,\"en\",\"Message exchange\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\"\"},0}",
                ),
                "IntegrationService",
                PathBuf::from("IntegrationServices").join("MessageExchange.xml"),
            ),
            (
                "77777777-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                deflate_for_test(
                    b"{1,\r\n{1,\r\n{3,\r\n{1,0,77777777-aaaa-4aaa-8aaa-aaaaaaaaaaaa},\"NotifyUsers\",{1,\"en\",\"Notify users\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},0,{4,0,{0},\"\",-1,-1,1,0,\"\"}},0}",
                ),
                "Bot",
                PathBuf::from("Bots").join("NotifyUsers.xml"),
            ),
            (
                "88888888-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                deflate_for_test(
                    b"{1,\r\n{2,\r\n{\"https://example.invalid/ws?wsdl\",0},\r\n{3,\r\n{1,0,88888888-aaaa-4aaa-8aaa-aaaaaaaaaaaa},\"UpdateFiles\",{1,\"en\",\"Update files\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222},0}",
                ),
                "WSReference",
                PathBuf::from("WSReferences").join("UpdateFiles.xml"),
            ),
            (
                "99999999-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                deflate_for_test(
                    b"{1,\r\n{3,\r\n{3,\r\n{1,0,99999999-aaaa-4aaa-8aaa-aaaaaaaaaaaa},\"Main\",{1,\"en\",\"Main\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},0}",
                ),
                "Style",
                PathBuf::from("Styles").join("Main.xml"),
            ),
            (
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                deflate_for_test(
                    b"{1,\r\n{3,\r\n{3,\r\n{1,0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},\"CashDocuments\",{1,\"en\",\"Cash documents\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},1,11,1,1,0},0}",
                ),
                "DocumentNumerator",
                PathBuf::from("DocumentNumerators").join("CashDocuments.xml"),
            ),
            (
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                deflate_for_test(
                    b"{1,\r\n{6,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{0,\r\n{3,\r\n{1,0,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb},\"CompanyDocuments\",{1,\"en\",\"Company documents\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},{0}},0}",
                ),
                "Sequence",
                PathBuf::from("Sequences").join("CompanyDocuments.xml"),
            ),
        ];

        for (uuid, blob, expected_kind, expected_path) in cases {
            let extracted = extract_metadata_source_xml(
                &blob,
                uuid,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap();
            let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();

            assert_eq!(extracted.relative_path, expected_path);
            assert_eq!(properties.kind, expected_kind);
            assert_eq!(properties.uuid, uuid);
        }
    }

    #[test]
    fn writes_ws_reference_body_asset_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let ws_uuid = "88888888-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let ws_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{\"https://example.invalid/ws?wsdl\",0}},\r\n{{3,\r\n{{1,0,{ws_uuid}}},\"UpdateFiles\",{{1,\"en\",\"Update files\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222}},0}}"
            )
            .as_bytes(),
        );
        let ws_body = deflate_for_test(b"<definitions/>");
        let rows = vec![
            ConfigRow {
                file_name: ws_uuid.to_string(),
                part_no: 0,
                data_size: ws_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&ws_metadata),
            },
            ConfigRow {
                file_name: format!("{ws_uuid}.0"),
                part_no: 0,
                data_size: ws_body.len() as i64,
                binary_hex: encode_hex_for_test(&ws_body),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(dumped.source_asset_rows, 1);
        assert_eq!(
            fs::read_to_string(root.join("WSReferences/UpdateFiles/Ext/WSDefinition.xml")).unwrap(),
            "<definitions/>"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_style_body_xml_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let style_uuid = "99999999-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let color_uuid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let font_uuid = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let style_metadata = deflate_for_test(
            format!(
                "{{1,\r\n{{3,\r\n{{3,\r\n{{1,0,{style_uuid}}},\"Main\",{{1,\"en\",\"Main\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let color_metadata = deflate_for_test(
            format!(
                "{{1,{{3,0,{{\"#\",9cd510c7-abfc-11d4-9434-004095e12fc7,2,{{3,2,{{37}}}}}},{{3,{{1,0,{color_uuid}}},\"ErrorBackColor\",{{1,\"en\",\"Error back color\"}},\"\"}}}},0}}"
            )
            .as_bytes(),
        );
        let font_metadata = deflate_for_test(
            format!(
                "{{1,{{3,1,{{\"#\",9cd510c8-abfc-11d4-9434-004095e12fc7,1,{{7,2,60,{{-31}},700,0,0,0,1,100}},0}},{{3,{{1,0,{font_uuid}}},\"StrikeFont\",{{1,\"en\",\"Strike font\"}},\"\"}}}},0}}"
            )
            .as_bytes(),
        );
        let style_body = deflate_for_test(
            format!(
                "{{2,5,{{{{-1}},0,{{4,2,{{20}},2}}}},{{{{-18}},2,{{3,1,{{-18}},0,0,0}}}},{{{{-20}},1,{{8,2,0,{{-20}},1,100}}}},{{{{0,{color_uuid}}},0,{{4,0,{{13158655}},0}}}},{{{{0,{font_uuid}}},1,{{8,2,60,{{-20}},400,0,0,1,1,100}}}},{{0}}}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: style_uuid.to_string(),
                part_no: 0,
                data_size: style_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&style_metadata),
            },
            ConfigRow {
                file_name: format!("{style_uuid}.0"),
                part_no: 0,
                data_size: style_body.len() as i64,
                binary_hex: encode_hex_for_test(&style_body),
            },
            ConfigRow {
                file_name: color_uuid.to_string(),
                part_no: 0,
                data_size: color_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&color_metadata),
            },
            ConfigRow {
                file_name: font_uuid.to_string(),
                part_no: 0,
                data_size: font_metadata.len() as i64,
                binary_hex: encode_hex_for_test(&font_metadata),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();

        assert_eq!(dumped.source_asset_rows, 1);
        let xml = fs::read_to_string(root.join("Styles/Main/Ext/Style.xml")).unwrap();
        assert!(xml.contains("<Style "));
        assert!(xml.contains("<Item name=\"FormBackColor\">"));
        assert!(xml.contains("<Color>web:Cream</Color>"));
        assert!(xml.contains("<Item name=\"ControlBorder\">"));
        assert!(xml.contains("<Border ref=\"style:ControlBorder\"/>"));
        assert!(xml.contains("<Item name=\"TextFont\">"));
        assert!(xml.contains("<Font ref=\"style:TextFont\" kind=\"StyleItem\"/>"));
        assert!(xml.contains("<Item name=\"StyleItem.ErrorBackColor\">"));
        assert!(xml.contains("<Color>#FFC8C8</Color>"));
        assert!(xml.contains("<Item name=\"StyleItem.StrikeFont\">"));
        assert!(xml.contains(
            "<Font ref=\"style:TextFont\" kind=\"StyleItem\" bold=\"false\" italic=\"false\" underline=\"false\" strikeout=\"true\"/>"
        ));

        let body_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{style_uuid}.0"))
            .unwrap();
        assert_eq!(
            body_row.source_asset_path.as_deref(),
            Some("Styles/Main/Ext/Style.xml")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_defined_type_xml_from_metadata_blob() {
        let uuid = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{3,\r\n{{1,0,{uuid}}},\"OwnerType\",{{1,\"en\",\"Owner type\"}},\"Defined comment\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{\"Pattern\",{{\"B\"}},{{\"S\",80,1}}}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &blob,
            uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let properties = parse_simple_metadata_xml_properties(&extracted.xml).unwrap();
        let repacked = pack_simple_metadata_blob_from_xml(&blob, &extracted.xml).unwrap();
        let xml = String::from_utf8_lossy(&extracted.xml);

        assert_eq!(
            extracted.relative_path,
            PathBuf::from("DefinedTypes").join("OwnerType.xml")
        );
        assert_eq!(properties.kind, "DefinedType");
        assert_eq!(properties.uuid, uuid);
        assert_eq!(properties.name, "OwnerType");
        assert!(xml.contains("<v8:Type>xs:boolean</v8:Type>"));
        assert!(xml.contains("<v8:Type>xs:string</v8:Type>"));
        assert!(xml.contains("<v8:Length>80</v8:Length>"));
        assert!(xml.contains("<v8:AllowedLength>Fixed</v8:AllowedLength>"));
        assert!(!repacked.blob.is_empty());
    }

    #[test]
    fn resolves_defined_type_reference_from_dumped_document_type_index() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let document_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let document_ref_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let document_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{40,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,{document_ref_type_id},33333333-3333-4333-8333-333333333333,\r\n{{0,\r\n{{3,\r\n{{1,0,{document_uuid}}},\"Invoice\",{{1,\"en\",\"Invoice\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let defined_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let defined_type_pattern = format!(r##"{{"Pattern",{{"#",{document_ref_type_id}}}}}"##);
        let defined_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,\r\n{{3,\r\n{{1,0,{defined_uuid}}},\"InvoiceOwner\",{{1,\"en\",\"Invoice owner\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{defined_type_pattern}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: document_uuid.to_string(),
                part_no: 0,
                data_size: document_blob.len() as i64,
                binary_hex: encode_hex_for_test(&document_blob),
            },
            ConfigRow {
                file_name: defined_uuid.to_string(),
                part_no: 0,
                data_size: defined_blob.len() as i64,
                binary_hex: encode_hex_for_test(&defined_blob),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, false, true).unwrap();
        let defined = dumped
            .rows
            .iter()
            .find(|row| row.file_name == defined_uuid)
            .unwrap();
        assert_eq!(
            defined.metadata_xml_path.as_deref(),
            Some("DefinedTypes/InvoiceOwner.xml")
        );
        let xml = fs::read_to_string(root.join("DefinedTypes").join("InvoiceOwner.xml")).unwrap();
        assert!(xml.contains("<v8:Type>cfg:DocumentRef.Invoice</v8:Type>"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builds_catalog_and_enum_reference_type_index_entries() {
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let catalog_object_type_id = "11111111-1111-4111-8111-111111111111";
        let catalog_ref_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let catalog_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{57,{catalog_object_type_id},22222222-2222-4222-8222-222222222222,{catalog_ref_type_id},33333333-3333-4333-8333-333333333333,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Customers\",{{1,\"en\",\"Customers\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let enum_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let enum_ref_type_id = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let enum_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{20,{enum_ref_type_id},eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee,ffffffff-ffff-4fff-ffff-ffffffffffff,11111111-2222-4333-8444-555555555555,\r\n{{0,\r\n{{3,\r\n{{1,0,{enum_uuid}}},\"Statuses\",{{1,\"en\",\"Statuses\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: catalog_uuid.to_string(),
                part_no: 0,
                data_size: catalog_blob.len() as i64,
                binary_hex: encode_hex_for_test(&catalog_blob),
            },
            ConfigRow {
                file_name: enum_uuid.to_string(),
                part_no: 0,
                data_size: enum_blob.len() as i64,
                binary_hex: encode_hex_for_test(&enum_blob),
            },
        ];

        let index = build_metadata_type_index(&rows);

        assert_eq!(
            index.get(catalog_object_type_id).map(String::as_str),
            Some("cfg:CatalogObject.Customers")
        );
        assert_eq!(
            index.get(catalog_ref_type_id).map(String::as_str),
            Some("cfg:CatalogRef.Customers")
        );
        assert_eq!(
            index.get(enum_ref_type_id).map(String::as_str),
            Some("cfg:EnumRef.Statuses")
        );
    }

    #[test]
    fn builds_object_family_generated_type_index_entries() {
        let defined_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let defined_type_id = "11111111-1111-4111-8111-111111111111";
        let defined_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,{defined_type_id},22222222-2222-4222-8222-222222222222,\r\n{{3,\r\n{{1,0,{defined_uuid}}},\"OwnerType\",{{1,\"en\",\"Owner type\"}},\"\"}},{{\"Pattern\",{{\"B\"}}}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let constant_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let constant_value_manager_type_id = "33333333-3333-4333-8333-333333333333";
        let constant_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{16,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{constant_uuid}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"\"}},{{\"Pattern\",{{\"B\"}}}}\r\n}}}},44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,{constant_value_manager_type_id},66666666-6666-4666-8666-666666666666}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let business_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let business_object_type_id = "77777777-7777-4777-8777-777777777777";
        let business_ref_type_id = "88888888-8888-4888-8888-888888888888";
        let business_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{30,\r\n{{3,\r\n{{1,0,{business_uuid}}},\"Approval\",{{1,\"en\",\"Approval\"}},\"\"}},1,{business_object_type_id},99999999-9999-4999-8999-999999999999,{business_ref_type_id},aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let document_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let document_object_type_id = "99999999-9999-4999-8999-999999999991";
        let document_ref_type_id = "99999999-9999-4999-8999-999999999992";
        let document_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{40,{document_object_type_id},99999999-9999-4999-8999-999999999993,{document_ref_type_id},99999999-9999-4999-8999-999999999994,\r\n{{0,\r\n{{3,\r\n{{1,0,{document_uuid}}},\"Invoice\",{{1,\"en\",\"Invoice\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let exchange_uuid = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let exchange_ref_type_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let exchange_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{37,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,cccccccc-cccc-4ccc-8ccc-cccccccccccc,{exchange_ref_type_id},dddddddd-dddd-4ddd-8ddd-dddddddddddd,\r\n{{3,\r\n{{1,0,{exchange_uuid}}},\"Sync\",{{1,\"en\",\"Sync\"}},\"\"}}\r\n}},0}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: defined_uuid.to_string(),
                part_no: 0,
                data_size: defined_blob.len() as i64,
                binary_hex: encode_hex_for_test(&defined_blob),
            },
            ConfigRow {
                file_name: constant_uuid.to_string(),
                part_no: 0,
                data_size: constant_blob.len() as i64,
                binary_hex: encode_hex_for_test(&constant_blob),
            },
            ConfigRow {
                file_name: business_uuid.to_string(),
                part_no: 0,
                data_size: business_blob.len() as i64,
                binary_hex: encode_hex_for_test(&business_blob),
            },
            ConfigRow {
                file_name: document_uuid.to_string(),
                part_no: 0,
                data_size: document_blob.len() as i64,
                binary_hex: encode_hex_for_test(&document_blob),
            },
            ConfigRow {
                file_name: exchange_uuid.to_string(),
                part_no: 0,
                data_size: exchange_blob.len() as i64,
                binary_hex: encode_hex_for_test(&exchange_blob),
            },
        ];

        let index = build_metadata_type_index(&rows);

        assert_eq!(
            index.get(defined_type_id).map(String::as_str),
            Some("cfg:DefinedType.OwnerType")
        );
        assert_eq!(
            index
                .get(constant_value_manager_type_id)
                .map(String::as_str),
            Some("cfg:ConstantValueManager.UseFeature")
        );
        assert_eq!(
            index.get(business_object_type_id).map(String::as_str),
            Some("cfg:BusinessProcessObject.Approval")
        );
        assert_eq!(
            index.get(business_ref_type_id).map(String::as_str),
            Some("cfg:BusinessProcessRef.Approval")
        );
        assert_eq!(
            index.get(document_object_type_id).map(String::as_str),
            Some("cfg:DocumentObject.Invoice")
        );
        assert_eq!(
            index.get(document_ref_type_id).map(String::as_str),
            Some("cfg:DocumentRef.Invoice")
        );
        assert_eq!(
            index.get(exchange_ref_type_id).map(String::as_str),
            Some("cfg:ExchangePlanRef.Sync")
        );
    }

    #[test]
    fn extracts_catalog_generated_types_to_metadata_xml() {
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let object_type_id = "11111111-1111-4111-8111-111111111111";
        let object_value_id = "11111111-1111-4111-8111-111111111112";
        let ref_type_id = "22222222-2222-4222-8222-222222222221";
        let ref_value_id = "22222222-2222-4222-8222-222222222222";
        let selection_type_id = "33333333-3333-4333-8333-333333333331";
        let selection_value_id = "33333333-3333-4333-8333-333333333332";
        let list_type_id = "44444444-4444-4444-8444-444444444441";
        let list_value_id = "44444444-4444-4444-8444-444444444442";
        let manager_type_id = "55555555-5555-4555-8555-555555555551";
        let manager_value_id = "55555555-5555-4555-8555-555555555552";
        let catalog_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{57,{object_type_id},{object_value_id},{ref_type_id},{ref_value_id},{selection_type_id},{selection_value_id},{list_type_id},{list_value_id},\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},2,1,{{0,0}},1,0,0,0,3,1,10,1,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,{{0,0}},1,{manager_type_id},{manager_value_id}}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(
            &catalog_blob,
            catalog_uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = String::from_utf8(extracted.xml).unwrap();

        assert!(xml.contains("<InternalInfo>"));
        assert!(
            xml.contains(r#"<xr:GeneratedType name="CatalogObject.Products" category="Object">"#)
        );
        assert!(xml.contains(&format!("<xr:TypeId>{object_type_id}</xr:TypeId>")));
        assert!(xml.contains(&format!("<xr:ValueId>{object_value_id}</xr:ValueId>")));
        assert!(xml.contains(r#"<xr:GeneratedType name="CatalogRef.Products" category="Ref">"#));
        assert!(xml.contains(
            r#"<xr:GeneratedType name="CatalogSelection.Products" category="Selection">"#
        ));
        assert!(xml.contains(r#"<xr:GeneratedType name="CatalogList.Products" category="List">"#));
        assert!(
            xml.contains(r#"<xr:GeneratedType name="CatalogManager.Products" category="Manager">"#)
        );
        assert!(xml.starts_with("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains(r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#));
        assert!(xml.contains("<Comment/>"));
        assert!(xml.contains("<Hierarchical>false</Hierarchical>"));
        assert!(xml.contains("<HierarchyType>HierarchyFoldersAndItems</HierarchyType>"));
        assert!(xml.contains("<LimitLevelCount>false</LimitLevelCount>"));
        assert!(xml.contains("<LevelCount>2</LevelCount>"));
        assert!(xml.contains("<FoldersOnTop>true</FoldersOnTop>"));
        assert!(xml.contains("<UseStandardCommands>true</UseStandardCommands>"));
        assert!(xml.contains("<Owners/>"));
        assert!(xml.contains("<SubordinationUse>ToItems</SubordinationUse>"));
        assert!(xml.contains("<CodeLength>3</CodeLength>"));
        assert!(xml.contains("<DescriptionLength>10</DescriptionLength>"));
        assert!(xml.contains("<CodeType>String</CodeType>"));
        assert!(xml.contains("<CodeAllowedLength>Variable</CodeAllowedLength>"));
        assert!(xml.contains("<CodeSeries>WholeCatalog</CodeSeries>"));
        assert!(xml.contains("<CheckUnique>false</CheckUnique>"));
        assert!(xml.contains("<Autonumbering>false</Autonumbering>"));
        assert!(xml.contains("<DefaultPresentation>AsDescription</DefaultPresentation>"));

        let rows = vec![ConfigRow {
            file_name: catalog_uuid.to_string(),
            part_no: 0,
            data_size: catalog_blob.len() as i64,
            binary_hex: encode_hex_for_test(&catalog_blob),
        }];
        let index = build_metadata_type_index(&rows);
        assert_eq!(
            index.get(selection_type_id).map(String::as_str),
            Some("cfg:CatalogSelection.Products")
        );
        assert_eq!(
            index.get(list_type_id).map(String::as_str),
            Some("cfg:CatalogList.Products")
        );
        assert_eq!(
            index.get(manager_type_id).map(String::as_str),
            Some("cfg:CatalogManager.Products")
        );
    }

    #[test]
    fn extracts_catalog_form_refs_and_presentations_to_metadata_xml() {
        let catalog_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let object_form_uuid = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let list_form_uuid = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        let object_type_id = "11111111-1111-4111-8111-111111111111";
        let object_value_id = "11111111-1111-4111-8111-111111111112";
        let ref_type_id = "22222222-2222-4222-8222-222222222221";
        let ref_value_id = "22222222-2222-4222-8222-222222222222";
        let selection_type_id = "33333333-3333-4333-8333-333333333331";
        let selection_value_id = "33333333-3333-4333-8333-333333333332";
        let list_type_id = "44444444-4444-4444-8444-444444444441";
        let list_value_id = "44444444-4444-4444-8444-444444444442";
        let manager_type_id = "55555555-5555-4555-8555-555555555551";
        let manager_value_id = "55555555-5555-4555-8555-555555555552";
        let zero_uuid = "00000000-0000-0000-0000-000000000000";
        let catalog_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{57,{object_type_id},{object_value_id},{ref_type_id},{ref_value_id},{selection_type_id},{selection_value_id},{list_type_id},{list_value_id},\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Products\",{{1,\"en\",\"Products\"}},\"\",0,0,{zero_uuid},0}}\r\n}},2,1,{{0,0}},1,0,0,0,3,1,10,1,{object_form_uuid},{zero_uuid},{list_form_uuid},{list_form_uuid},{zero_uuid},{zero_uuid},{zero_uuid},{zero_uuid},{zero_uuid},{zero_uuid},1,{{0,0}},1,{manager_type_id},{manager_value_id},0,0,0,0,2,1,{{1,{{0,2,{{\"#\",60ea359f-3a6e-48bb-8e71-d2a457572918,{{-3}}}},{{\"#\",60ea359f-3a6e-48bb-8e71-d2a457572918,{{-2}}}}}}}},1,1,{{0}},{{2,\"ru\",\"Товар\",\"en\",\"Product\"}},{{0}},{{0}},{{0}},{{2,\"ru\",\"Товары для продажи\",\"en\",\"Goods for sale\"}}}}\r\n}}"
            )
            .as_bytes(),
        );
        let form_refs = BTreeMap::from([
            (
                object_form_uuid.to_string(),
                FormSourceReference {
                    relative_path: PathBuf::from("Catalogs/Products/Forms/ItemForm.xml"),
                    kind: "Form",
                },
            ),
            (
                list_form_uuid.to_string(),
                FormSourceReference {
                    relative_path: PathBuf::from("Catalogs/Products/Forms/ListForm.xml"),
                    kind: "Form",
                },
            ),
        ]);

        let extracted = extract_metadata_source_xml_with_refs(
            &catalog_blob,
            catalog_uuid,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &form_refs,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let xml = String::from_utf8(extracted.xml).unwrap();

        assert!(
            xml.contains("<DefaultObjectForm>Catalog.Products.Form.ItemForm</DefaultObjectForm>")
        );
        assert!(xml.contains("<Characteristics/>"));
        assert!(xml.contains("<StandardAttributes>"));
        assert!(xml.contains(r#"<xr:StandardAttribute name="PredefinedDataName">"#));
        assert!(xml.contains(r#"<xr:StandardAttribute name="Owner">"#));
        assert!(xml.contains("<xr:FillChecking>ShowError</xr:FillChecking>"));
        assert!(xml.contains("<xr:FillFromFillingValue>true</xr:FillFromFillingValue>"));
        assert!(xml.contains("<xr:TypeReductionMode>Deny</xr:TypeReductionMode>"));
        assert!(xml.contains(r#"<xr:StandardAttribute name="Parent">"#));
        assert!(xml.contains(r#"<xr:StandardAttribute name="Description">"#));
        assert!(xml.contains(r#"<xr:FillValue xsi:type="xs:string"/>"#));
        assert!(xml.contains(r#"<xr:StandardAttribute name="Code">"#));
        assert!(xml.contains(r#"<xr:FillValue xsi:type="xs:string">   </xr:FillValue>"#));
        assert!(xml.contains("<PredefinedDataUpdate>Auto</PredefinedDataUpdate>"));
        assert!(xml.contains("<EditType>InDialog</EditType>"));
        assert!(xml.contains("<QuickChoice>true</QuickChoice>"));
        assert!(xml.contains("<ChoiceMode>BothWays</ChoiceMode>"));
        assert!(xml.contains("<InputByString>"));
        assert!(
            xml.contains("<xr:Field>Catalog.Products.StandardAttribute.Description</xr:Field>")
        );
        assert!(xml.contains("<xr:Field>Catalog.Products.StandardAttribute.Code</xr:Field>"));
        assert!(
            xml.contains(
                "<SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString>"
            )
        );
        assert!(
            xml.contains("<FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString>")
        );
        assert!(xml.contains(
            "<ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString>"
        ));
        assert!(xml.contains("<DefaultFolderForm/>"));
        assert!(xml.contains("<DefaultListForm>Catalog.Products.Form.ListForm</DefaultListForm>"));
        assert!(
            xml.contains("<DefaultChoiceForm>Catalog.Products.Form.ListForm</DefaultChoiceForm>")
        );
        assert!(xml.contains("<IncludeHelpInContents>true</IncludeHelpInContents>"));
        assert!(xml.contains("<ObjectPresentation>"));
        assert!(xml.contains("<v8:content>Товар</v8:content>"));
        assert!(xml.contains("<v8:content>Product</v8:content>"));
        assert!(xml.contains("<ExtendedObjectPresentation/>"));
        assert!(xml.contains("<ListPresentation/>"));
        assert!(xml.contains("<ExtendedListPresentation/>"));
        assert!(xml.contains("<Explanation>"));
        assert!(xml.contains("<v8:content>Товары для продажи</v8:content>"));
        assert!(xml.contains("<v8:content>Goods for sale</v8:content>"));
        assert!(xml.contains("<CreateOnInput>DontUse</CreateOnInput>"));
        assert!(xml.contains("<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>"));
        assert!(xml.contains("<DataHistory>DontUse</DataHistory>"));
        assert!(xml.contains(
            "<UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite>"
        ));
        assert!(xml.contains(
            "<ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing>"
        ));
    }

    #[test]
    fn builds_register_and_chart_reference_type_index_entries() {
        let info_register_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let record_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let record_set_type_id = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let info_register_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{33,{record_type_id},11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,66666666-6666-4666-8666-666666666666,77777777-7777-4777-8777-777777777777,{record_set_type_id},88888888-8888-4888-8888-888888888888,99999999-9999-4999-8999-999999999999,aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee,ffffffff-ffff-4fff-8fff-ffffffffffff,eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee,\r\n{{0,\r\n{{3,\r\n{{1,0,{info_register_uuid}}},\"Prices\",{{1,\"en\",\"Prices\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let chart_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let chart_object_type_id = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let chart_ref_type_id = "ffffffff-ffff-4fff-ffff-ffffffffffff";
        let chart_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{34,{chart_object_type_id},11111111-1111-4111-8111-111111111111,{chart_ref_type_id},22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,66666666-6666-4666-8666-666666666666,77777777-7777-4777-8777-777777777777,88888888-8888-4888-8888-888888888888,99999999-9999-4999-8999-999999999999,aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee,\r\n{{0,\r\n{{3,\r\n{{1,0,{chart_uuid}}},\"ExpenseItems\",{{1,\"en\",\"Expense items\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let accounts_uuid = "99999999-9999-4999-8999-999999999991";
        let accounts_object_type_id = "99999999-9999-4999-8999-999999999992";
        let accounts_ref_type_id = "99999999-9999-4999-8999-999999999993";
        let accounts_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{32,{accounts_object_type_id},11111111-1111-4111-8111-111111111111,{accounts_ref_type_id},22222222-2222-4222-8222-222222222222,33333333-3333-4333-8333-333333333333,44444444-4444-4444-8444-444444444444,55555555-5555-4555-8555-555555555555,66666666-6666-4666-8666-666666666666,77777777-7777-4777-8777-777777777777,\r\n{{0,\r\n{{3,\r\n{{1,0,{accounts_uuid}}},\"MainAccounts\",{{1,\"en\",\"Main accounts\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let rows = vec![
            ConfigRow {
                file_name: info_register_uuid.to_string(),
                part_no: 0,
                data_size: info_register_blob.len() as i64,
                binary_hex: encode_hex_for_test(&info_register_blob),
            },
            ConfigRow {
                file_name: chart_uuid.to_string(),
                part_no: 0,
                data_size: chart_blob.len() as i64,
                binary_hex: encode_hex_for_test(&chart_blob),
            },
            ConfigRow {
                file_name: accounts_uuid.to_string(),
                part_no: 0,
                data_size: accounts_blob.len() as i64,
                binary_hex: encode_hex_for_test(&accounts_blob),
            },
        ];

        let index = build_metadata_type_index(&rows);

        assert_eq!(
            index.get(record_type_id).map(String::as_str),
            Some("cfg:InformationRegisterRecord.Prices")
        );
        assert_eq!(
            index.get(record_set_type_id).map(String::as_str),
            Some("cfg:InformationRegisterRecordSet.Prices")
        );
        assert_eq!(
            index.get(chart_object_type_id).map(String::as_str),
            Some("cfg:ChartOfCharacteristicTypesObject.ExpenseItems")
        );
        assert_eq!(
            index.get(chart_ref_type_id).map(String::as_str),
            Some("cfg:ChartOfCharacteristicTypesRef.ExpenseItems")
        );
        assert_eq!(
            index.get(accounts_object_type_id).map(String::as_str),
            Some("cfg:ChartOfAccountsObject.MainAccounts")
        );
        assert_eq!(
            index.get(accounts_ref_type_id).map(String::as_str),
            Some("cfg:ChartOfAccountsRef.MainAccounts")
        );
    }

    #[test]
    fn detects_ext_picture_binary_file_names() {
        assert_eq!(
            ext_picture_file_name(b"\x89PNG\r\n\x1a\npayload"),
            "Picture.png"
        );
        assert_eq!(ext_picture_file_name(b"GIF87apayload"), "Picture.gif");
        assert_eq!(ext_picture_file_name(b"GIF89apayload"), "Picture.gif");
        assert_eq!(
            ext_picture_file_name(b"\xff\xd8\xff\xe0payload"),
            "Picture.jpg"
        );
        assert_eq!(ext_picture_file_name(b"BMpayload"), "Picture.bmp");
        assert_eq!(
            ext_picture_file_name(b"\x00\x00\x01\x00payload"),
            "Picture.ico"
        );
        assert_eq!(ext_picture_file_name(b"PK\x03\x04payload"), "Picture.zip");
    }

    #[test]
    fn writes_extracted_metadata_xml_to_source_layout() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{40,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"Invoice\",{{1,\"en\",\"Invoice\"}},\"\"}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );
        let row = ConfigRow {
            file_name: uuid.to_string(),
            part_no: 0,
            data_size: blob.len() as i64,
            binary_hex: encode_hex_for_test(&blob),
        };

        let dumped = dump_table_rows(&root, "Config", vec![row], false, false, true).unwrap();

        assert_eq!(dumped.metadata_xml_rows, 1);
        assert_eq!(
            dumped.rows[0].metadata_xml_path.as_deref(),
            Some("Documents/Invoice.xml")
        );
        let written = fs::read(root.join("Documents").join("Invoice.xml")).unwrap();
        let properties = parse_simple_metadata_xml_properties(&written).unwrap();
        assert_eq!(properties.kind, "Document");
        assert_eq!(properties.uuid, uuid);

        let _ = fs::remove_dir_all(root);
    }

    fn encode_hex_for_test(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn deflate_for_test(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }
}
