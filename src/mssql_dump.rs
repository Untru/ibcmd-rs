use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use serde::Serialize;

use crate::cli::MssqlDumpConfigArgs;
use crate::module_blob::unpack_module_blob_text;

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

    let mut manifests = Vec::new();
    let mut binary_bytes = 0;
    let mut inflated_rows = 0;
    let mut module_text_rows = 0;
    let mut metadata_xml_rows = 0;
    let mut source_asset_rows = 0;
    for row in rows {
        let bytes = decode_hex(&row.binary_hex)
            .with_context(|| format!("failed to decode {} row {}", table, row.file_name))?;
        if bytes.len() != row.data_size as usize {
            bail!(
                "{} row {} DataSize {} does not match BinaryData length {}",
                table,
                row.file_name,
                row.data_size,
                bytes.len()
            );
        }
        binary_bytes += bytes.len();

        let safe_name = safe_storage_file_name(&row.file_name, row.part_no);
        let binary_relative = PathBuf::from(table).join(format!("{safe_name}.bin"));
        let binary_path = output_dir.join(&binary_relative);
        fs::write(&binary_path, &bytes)
            .with_context(|| format!("failed to write {}", binary_path.display()))?;

        let inflated_relative = if inflate {
            match inflate_raw_deflate(&bytes) {
                Ok(inflated) => {
                    let relative =
                        PathBuf::from(format!("{table}_inflated")).join(format!("{safe_name}.txt"));
                    let path = output_dir.join(&relative);
                    fs::write(&path, inflated)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    inflated_rows += 1;
                    Some(relative.to_string_lossy().replace('\\', "/"))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let module_text_relative = if extract_module_text {
            let module_text = match unpack_module_blob_text(&bytes) {
                Ok(text) => Some(text),
                Err(_) if module_text_paths.contains_key(&row.file_name) => {
                    unpack_form_body_module_text(&bytes)
                }
                Err(_) => None,
            };
            match module_text {
                Some(text) => {
                    let relative = module_text_paths
                        .get(&row.file_name)
                        .cloned()
                        .unwrap_or_else(|| {
                            PathBuf::from(format!("{table}_module_text"))
                                .join(format!("{safe_name}.bsl"))
                        });
                    let path = output_dir.join(&relative);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("failed to create {}", parent.display()))?;
                    }
                    fs::write(&path, text)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    module_text_rows += 1;
                    Some(relative.to_string_lossy().replace('\\', "/"))
                }
                None => None,
            }
        } else {
            None
        };

        let metadata_xml_relative = if extract_metadata_xml {
            match extract_metadata_source_xml_with_refs(
                &bytes,
                &row.file_name,
                &type_index,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            ) {
                Some(extracted) => {
                    let path = output_dir.join(&extracted.relative_path);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("failed to create {}", parent.display()))?;
                    }
                    fs::write(&path, extracted.xml)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    metadata_xml_rows += 1;
                    Some(extracted.relative_path.to_string_lossy().replace('\\', "/"))
                }
                None => None,
            }
        } else {
            None
        };

        let source_asset_relative = if metadata_xml_relative.is_none() {
            match source_assets.get(&row.file_name) {
                Some(asset) => {
                    let relative = write_source_asset(output_dir, asset, &bytes)?;
                    source_asset_rows += 1;
                    Some(relative.to_string_lossy().replace('\\', "/"))
                }
                None => None,
            }
        } else {
            None
        };

        manifests.push(MssqlDumpRowManifest {
            file_name: row.file_name,
            part_no: row.part_no,
            data_size: row.data_size,
            binary_bytes: bytes.len(),
            binary_path: binary_relative.to_string_lossy().replace('\\', "/"),
            inflated_path: inflated_relative,
            module_text_path: module_text_relative,
            metadata_xml_path: metadata_xml_relative,
            source_asset_path: source_asset_relative,
        });
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
    Form,
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
    paths.extend(form_body_asset_paths(rows, &file_names));
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
        "DataCompositionSchema" => Some(("Template.xml", SourceAssetKind::InflatedBinary)),
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
                kind: SourceAssetKind::Form,
            },
        );
    }

    paths
}

const CONFIGURATION_SOURCE_ASSET_SUFFIXES: &[(&str, &str, SourceAssetKind)] = &[
    ("2", "Ext/Splash.xml", SourceAssetKind::ExtPicture),
    ("4", "Ext/ParentConfigurations.bin", SourceAssetKind::Binary),
    (
        "c",
        "Ext/MainSectionPicture.xml",
        SourceAssetKind::ExtPicture,
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
    for (body_id, body_row) in rows_by_file_name {
        if !body_id.starts_with(&object_row_prefix) || mapped_ids.contains(*body_id) {
            continue;
        }
        if let Ok(help_bytes) = decode_hex(&body_row.binary_hex)
            && parse_help_blob_pages(&help_bytes).is_some()
        {
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
        SourceAssetKind::Form => {
            let xml = extract_form_body_xml(bytes).with_context(|| {
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
    column_widths: Vec<usize>,
    default_format_width: Option<usize>,
    formats: Vec<MoxelFormat>,
    rows: Vec<MoxelRow>,
    merges: Vec<MoxelMerge>,
    areas: Vec<MoxelArea>,
    lines: Vec<MoxelLine>,
    fonts: Vec<MoxelFont>,
    pictures: Vec<MoxelPicture>,
    default_format_index: usize,
    height: usize,
}

struct MoxelRow {
    index: usize,
    index_to: Option<usize>,
    format_index: usize,
    columns_id: Option<String>,
    cells: Vec<MoxelCell>,
}

struct MoxelColumnSet {
    id: Option<String>,
    columns: Vec<MoxelColumn>,
}

struct MoxelColumn {
    index: usize,
    format_index: usize,
}

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
    face_name: String,
    height: usize,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    scale: usize,
}

struct MoxelLine {
    style: &'static str,
}

struct MoxelPicture {
    index: usize,
}

#[derive(Clone, Default)]
struct MoxelFormat {
    font: Option<usize>,
    left_border: Option<usize>,
    top_border: Option<usize>,
    right_border: Option<usize>,
    bottom_border: Option<usize>,
    height: Option<usize>,
    border_color: Option<String>,
    width: Option<usize>,
    horizontal_alignment: Option<&'static str>,
    vertical_alignment: Option<&'static str>,
    text_color: Option<String>,
    text_placement: Option<&'static str>,
    fill_type: Option<&'static str>,
    hyper_link: Option<bool>,
    protection: Option<bool>,
    indent: Option<usize>,
    auto_indent: Option<usize>,
    pic_index: Option<usize>,
    picture_size_mode: Option<&'static str>,
    pic_horizontal_alignment: Option<&'static str>,
    pic_vertical_alignment: Option<&'static str>,
}

impl MoxelFormat {
    fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.left_border.is_none()
            && self.top_border.is_none()
            && self.right_border.is_none()
            && self.bottom_border.is_none()
            && self.height.is_none()
            && self.border_color.is_none()
            && self.width.is_none()
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.text_color.is_none()
            && self.text_placement.is_none()
            && self.fill_type.is_none()
            && self.hyper_link.is_none()
            && self.protection.is_none()
            && self.indent.is_none()
            && self.auto_indent.is_none()
            && self.pic_index.is_none()
            && self.picture_size_mode.is_none()
            && self.pic_horizontal_alignment.is_none()
            && self.pic_vertical_alignment.is_none()
    }

    fn is_width_only(&self) -> bool {
        self.width.is_some()
            && self.font.is_none()
            && self.left_border.is_none()
            && self.top_border.is_none()
            && self.right_border.is_none()
            && self.bottom_border.is_none()
            && self.height.is_none()
            && self.border_color.is_none()
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.text_color.is_none()
            && self.text_placement.is_none()
            && self.fill_type.is_none()
            && self.hyper_link.is_none()
            && self.protection.is_none()
            && self.indent.is_none()
            && self.auto_indent.is_none()
            && self.pic_index.is_none()
            && self.picture_size_mode.is_none()
            && self.pic_horizontal_alignment.is_none()
            && self.pic_vertical_alignment.is_none()
    }

    fn uses_line(&self) -> bool {
        self.left_border.is_some()
            || self.top_border.is_some()
            || self.right_border.is_some()
            || self.bottom_border.is_some()
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
    if text.starts_with("<?xml") && text.contains("data-composition-system/schema") {
        Some("DataCompositionSchema")
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
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        "Picture.ico"
    } else if bytes.starts_with(b"PK\x03\x04") {
        "Picture.zip"
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        let text = text.trim_start_matches('\u{feff}').trim_start();
        if text.starts_with("<svg") || text.starts_with("<?xml") && text.contains("<svg") {
            "Picture.svg"
        } else if text.starts_with('<') {
            "Picture.xml"
        } else {
            "Picture.txt"
        }
    } else {
        "Picture.bin"
    }
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Help xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
    );
    for page in pages {
        xml.push_str("\t<Page>");
        xml.push_str(&escape_xml_text(&page.page));
        xml.push_str("</Page>\r\n");
    }
    xml.push_str("</Help>\r\n");
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

fn extract_form_body_xml(bytes: &[u8]) -> Option<String> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let form_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    if !form_fields
        .first()
        .is_some_and(|value| value.trim().chars().all(|ch| ch.is_ascii_digit()))
    {
        return None;
    }

    Some(format_form_body_xml())
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

fn format_form_body_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
</Form>\r\n"
        .to_string()
}

fn extract_moxel_spreadsheet_xml(
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
    trim_moxel_trailing_empty_rows(&mut rows, &areas, &merges);
    let (column_sets, row_column_ids) = parse_moxel_column_sets(&fields);
    let parsed_lines = parse_moxel_lines(&fields);
    let fonts = parse_moxel_fonts(&fields);
    let pictures = parse_moxel_pictures(&fields);
    let style_refs = parse_moxel_style_refs(&fields, object_refs);
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
    let column_format_slots = column_sets
        .iter()
        .map(|column_set| column_set.columns.len())
        .sum::<usize>()
        .max(column_count);
    let format_offset = column_format_slots.saturating_sub(1);
    for row in &mut rows {
        if let Some(columns_id) = row_column_ids.get(&row.index) {
            row.columns_id = Some(columns_id.clone());
        }
        if row.format_index > 1 {
            row.format_index += format_offset;
        }
        for cell in &mut row.cells {
            cell.format_index += format_offset;
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
    let height = moxel_spreadsheet_height(&rows, &merges, &areas);
    let formats = parse_moxel_formats(&fields, column_format_slots, &style_refs);
    let lines = if formats.iter().any(MoxelFormat::uses_line) {
        parsed_lines
    } else {
        Vec::new()
    };
    Some(MoxelSpreadsheet {
        column_count,
        column_sets,
        column_widths: parse_moxel_column_widths(&fields, column_format_slots),
        default_format_width: parse_moxel_default_format_width(&fields, column_format_slots),
        formats,
        rows,
        merges,
        areas,
        lines,
        fonts,
        pictures,
        default_format_index: max_format_index + 1,
        height,
    })
}

fn default_moxel_column_sets(column_count: usize) -> Vec<MoxelColumnSet> {
    vec![MoxelColumnSet {
        id: None,
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
    if count == 0 || count > 2048 || declared_count != count || fields.len() != count * 2 + 4 {
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
    Some(MoxelColumnSet { id, columns })
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
    while index < fields.len() {
        if let Some((row, next_index)) =
            parse_moxel_row_at_for_scanning(fields, index, expected_row_index)
        {
            rows.push(row);
            expected_row_index += 1;
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
            allow_empty: false,
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
        .unwrap_or(0)
        + 1;
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

fn parse_moxel_fonts(fields: &[&str]) -> Vec<MoxelFont> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_font(field))
        .collect()
}

fn parse_moxel_font(text: &str) -> Option<MoxelFont> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "7" || fields.len() < 19 {
        return None;
    }
    let height_raw = fields.get(3)?.trim().parse::<usize>().ok()?;
    let weight = fields.get(7)?.trim().parse::<usize>().ok()?;
    Some(MoxelFont {
        face_name: parse_1c_string(fields.get(16)?)?,
        height: height_raw / 10,
        bold: weight >= 700,
        italic: fields.get(4)?.trim() != "0",
        underline: fields.get(5)?.trim() != "0",
        strikeout: fields.get(6)?.trim() != "0",
        scale: fields.get(18)?.trim().parse::<usize>().ok()?,
    })
}

fn parse_moxel_lines(fields: &[&str]) -> Vec<MoxelLine> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_line(field))
        .collect()
}

fn parse_moxel_pictures(fields: &[&str]) -> Vec<MoxelPicture> {
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
        for field in &fields[index + 1..=index + count] {
            let Some(picture) = parse_moxel_picture(field) else {
                pictures.clear();
                break;
            };
            pictures.push(picture);
        }
        if pictures.len() == count {
            return pictures;
        }
    }
    Vec::new()
}

fn parse_moxel_picture(text: &str) -> Option<MoxelPicture> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    Some(MoxelPicture {
        index: fields.get(1)?.trim().parse::<usize>().ok()?,
    })
}

fn parse_moxel_column_widths(fields: &[&str], column_count: usize) -> Vec<usize> {
    let widths = fields
        .iter()
        .filter_map(|field| parse_moxel_column_width(field))
        .collect::<Vec<_>>();
    if widths.len() < column_count {
        return Vec::new();
    }
    widths[widths.len() - column_count..].to_vec()
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

fn parse_moxel_formats(
    fields: &[&str],
    column_count: usize,
    style_refs: &[Option<String>],
) -> Vec<MoxelFormat> {
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
        for field in &fields[index + 1..=index + count] {
            let Some(format) = parse_moxel_format(field, style_refs) else {
                formats.clear();
                break;
            };
            formats.push(format);
        }
        if formats.len() == count
            && formats
                .iter()
                .rev()
                .take(column_count)
                .all(MoxelFormat::is_width_only)
        {
            formats.truncate(count - column_count);
            return formats;
        }
    }
    Vec::new()
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
    Some(MoxelFormat {
        font: parse_moxel_format_usize(&values, 0),
        left_border: parse_moxel_format_usize(&values, 1),
        top_border: parse_moxel_format_usize(&values, 2),
        right_border: parse_moxel_format_usize(&values, 3),
        bottom_border: parse_moxel_format_usize(&values, 4),
        height: parse_moxel_format_usize(&values, 6),
        border_color: parse_moxel_format_style_ref(&values, 5, style_refs),
        width: parse_moxel_format_usize(&values, 7),
        horizontal_alignment: parse_moxel_format_usize(&values, 8)
            .and_then(moxel_horizontal_alignment),
        vertical_alignment: parse_moxel_format_usize(&values, 9).and_then(moxel_vertical_alignment),
        text_color: parse_moxel_format_style_ref(&values, 10, style_refs),
        text_placement: parse_moxel_format_usize(&values, 14).and_then(moxel_text_placement),
        fill_type: parse_moxel_format_usize(&values, 15).and_then(moxel_fill_type),
        hyper_link: parse_moxel_format_usize(&values, 26).and_then(moxel_hyper_link),
        protection: parse_moxel_format_usize(&values, 16).and_then(moxel_protection),
        indent: parse_moxel_format_usize(&values, 30),
        auto_indent: parse_moxel_format_usize(&values, 31),
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
                | 14
                | 15
                | 16
                | 26
                | 30
                | 31
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
    fields
        .iter()
        .filter_map(|field| parse_moxel_style_ref_slot(field, object_refs))
        .collect()
}

fn parse_moxel_style_ref_slot(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" || fields.get(1)?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    match payload.first()?.trim() {
        "-1" | "-3" => Some(None),
        "0" => {
            let uuid = parse_uuid_field(payload.get(1)?.trim())?;
            let style_ref = object_refs
                .get(&uuid)
                .and_then(|reference| reference.strip_prefix("StyleItem."))
                .map(|name| format!("style:{name}"));
            Some(style_ref)
        }
        _ => None,
    }
}

fn moxel_horizontal_alignment(value: usize) -> Option<&'static str> {
    match value {
        6 => Some("Center"),
        _ => None,
    }
}

fn moxel_vertical_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Top"),
        _ => None,
    }
}

fn moxel_text_placement(value: usize) -> Option<&'static str> {
    match value {
        0 | 2 => Some("Block"),
        3 => Some("Wrap"),
        _ => None,
    }
}

fn moxel_fill_type(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Text"),
        1 => Some("Parameter"),
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
        _ => return None,
    };
    Some(MoxelLine { style })
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
    xml.push_str("\t<templateMode>true</templateMode>\r\n");
    xml.push_str(&format!(
        "\t<defaultFormatIndex>{}</defaultFormatIndex>\r\n",
        spreadsheet.default_format_index
    ));
    xml.push_str(&format!("\t<height>{}</height>\r\n", spreadsheet.height));
    xml.push_str(&format!("\t<vgRows>{}</vgRows>\r\n", spreadsheet.height));
    for merge in &spreadsheet.merges {
        push_moxel_merge_xml(&mut xml, merge);
    }
    for area in &spreadsheet.areas {
        push_moxel_area_xml(&mut xml, area);
    }
    for line in &spreadsheet.lines {
        push_moxel_line_xml(&mut xml, line);
    }
    for font in &spreadsheet.fonts {
        push_moxel_font_xml(&mut xml, font);
    }
    for format_index in 1..=spreadsheet.default_format_index.max(1) {
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
    xml.push_str(&format!(
        "\t\t<size>{}</size>\r\n",
        column_set.columns.len()
    ));
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

fn push_moxel_format_xml(xml: &mut String, spreadsheet: &MoxelSpreadsheet, format_index: usize) {
    let format = moxel_format_for_index(spreadsheet, format_index);
    if format.is_empty() {
        xml.push_str("\t<format/>\r\n");
        return;
    };
    xml.push_str("\t<format>\r\n");
    push_moxel_format_usize(xml, "font", format.font);
    push_moxel_format_usize(xml, "leftBorder", format.left_border);
    push_moxel_format_usize(xml, "topBorder", format.top_border);
    push_moxel_format_usize(xml, "rightBorder", format.right_border);
    push_moxel_format_usize(xml, "bottomBorder", format.bottom_border);
    push_moxel_format_usize(xml, "height", format.height);
    push_moxel_format_text(xml, "borderColor", format.border_color.as_deref());
    push_moxel_format_usize(xml, "width", format.width);
    push_moxel_format_text(xml, "horizontalAlignment", format.horizontal_alignment);
    push_moxel_format_text(xml, "verticalAlignment", format.vertical_alignment);
    push_moxel_format_text(xml, "textColor", format.text_color.as_deref());
    push_moxel_format_text(xml, "textPlacement", format.text_placement);
    push_moxel_format_text(xml, "fillType", format.fill_type);
    if let Some(hyper_link) = format.hyper_link {
        xml.push_str(&format!("\t\t<hyperLink>{hyper_link}</hyperLink>\r\n"));
    }
    if let Some(protection) = format.protection {
        xml.push_str(&format!("\t\t<protection>{protection}</protection>\r\n"));
    }
    push_moxel_format_usize(xml, "indent", format.indent);
    push_moxel_format_usize(xml, "autoIndent", format.auto_indent);
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
        .column_widths
        .len()
        .max(
            spreadsheet
                .column_sets
                .iter()
                .map(|column_set| column_set.columns.len())
                .sum::<usize>(),
        )
        .max(spreadsheet.column_count);
    if let Some(width) = spreadsheet
        .column_widths
        .get(format_index.saturating_sub(1))
        .copied()
    {
        return MoxelFormat {
            width: Some(width),
            ..MoxelFormat::default()
        };
    }
    if format_index == spreadsheet.default_format_index {
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
    xml.push_str("\t\t<picture/>\r\n");
    xml.push_str("\t</picture>\r\n");
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
    xml.push_str("\t<line width=\"1\" gap=\"false\">\r\n");
    xml.push_str(&format!(
        "\t\t<v8ui:style xsi:type=\"v8ui:SpreadsheetDocumentCellLineType\">{}</v8ui:style>\r\n",
        line.style
    ));
    xml.push_str("\t</line>\r\n");
}

fn push_moxel_font_xml(xml: &mut String, font: &MoxelFont) {
    xml.push_str(&format!(
        "\t<font faceName=\"{}\" height=\"{}\" bold=\"{}\" italic=\"{}\" underline=\"{}\" strikeout=\"{}\" kind=\"Absolute\" scale=\"{}\"/>\r\n",
        escape_xml_text(&font.face_name),
        font.height,
        font.bold,
        font.italic,
        font.underline,
        font.strikeout,
        font.scale
    ));
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
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let (module_text, _) = parse_1c_quoted_string_with_len(fields.get(2)?.trim())?;
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
        ("CommonCommand", "2") => Some("CommandModule.bsl"),
        ("Constant", "0") => Some("ValueManagerModule.bsl"),
        ("Constant", "1") => Some("ManagerModule.bsl"),
        ("SettingsStorage", "8") => Some("ManagerModule.bsl"),
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

struct TypedMetadataProperties {
    value_types: Vec<ConstantValueType>,
}

#[derive(Clone)]
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
    let xml = if is_typed_metadata_source(kind) {
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
        1 if header_index == Some(1) && field_starts_with(fields.get(2), r#"{"Pattern""#) => {
            Some(("EventSubscription", "EventSubscriptions"))
        }
        1 if header_index == Some(1) && field_starts_with(fields.get(1), "{2,") => {
            Some(("SessionParameter", "SessionParameters"))
        }
        1 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("XDTOPackage", "XDTOPackages"))
        }
        2 if contains_wrapped_metadata_object_code(text, 9, uuid) => {
            Some(("CommonCommand", "CommonCommands"))
        }
        2 if header_index == Some(2) && field_is_quoted_string(fields.get(1)) => {
            Some(("HTTPService", "HTTPServices"))
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
        6 => Some(("Role", "Roles")),
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
    contains_wrapped_metadata_object_code(text, 13, uuid)
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
        46 => Some("web:ForestGreen"),
        48 => Some("web:Gainsboro"),
        52 => Some("web:Gray"),
        53 => Some("web:Green"),
        55 => Some("web:HoneyDew"),
        67 => Some("web:LightCyan"),
        72 => Some("web:LightPink"),
        79 => Some("web:LightYellow"),
        84 => Some("web:Maroon"),
        98 => Some("web:MistyRose"),
        119 => Some("web:Red"),
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
        -3 => Some("style:FormTextColor"),
        -16 => Some("style:SpecialTextColor"),
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
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
    xml.push_str(&format!(
        "\t\t\t<Comment>{}</Comment>\r\n\
\t\t\t<FormType>Managed</FormType>\r\n\
\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\
\t\t\t<UsePurposes>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value>\r\n\
\t\t\t</UsePurposes>\r\n",
        escape_xml_text(&header.comment)
    ));
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
</MetaDataObject>\r\n"
    ));
    xml
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
        ReturnValuesReuse, pack_module_blob_bytes, pack_simple_metadata_blob_from_xml,
        parse_common_module_xml_properties, parse_simple_metadata_xml_properties,
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
        ];

        for (kind, folder, name, suffix, expected) in cases {
            assert_eq!(
                module_owner_source_path(kind, folder, name, suffix),
                Some(expected)
            );
        }
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
        let main_picture_blob = deflate_for_test(b"{1,{0,0,-1,-1},{{#base64:iVBORw0KGgo=}}}");
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
                file_name: format!("{uuid}.c"),
                part_no: 0,
                data_size: main_picture_blob.len() as i64,
                binary_hex: encode_hex_for_test(&main_picture_blob),
            },
        ];

        let dumped = dump_table_rows(&root, "Config", rows, false, true, false).unwrap();

        assert_eq!(dumped.module_text_rows, 4);
        assert_eq!(dumped.source_asset_rows, 3);
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
        let main_picture_row = dumped
            .rows
            .iter()
            .find(|row| row.file_name == format!("{uuid}.c"))
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
            main_picture_row.source_asset_path.as_deref(),
            Some("Ext/MainSectionPicture.xml")
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
                "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{owned_form_uuid}}},\"ListForm\",{{1,\"en\",\"List form\"}},\"\"}},0,1,{{2,{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}},{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}}\r\n}}\r\n}},0}}"
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
        let owned_xml =
            fs::read_to_string(root.join("Catalogs/Products/Forms/ListForm.xml")).unwrap();
        let common_xml = fs::read_to_string(root.join("CommonForms/SharedForm.xml")).unwrap();
        assert!(owned_xml.contains("<Form uuid=\"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb\">"));
        assert!(owned_xml.contains("<FormType>Managed</FormType>"));
        assert!(common_xml.contains("<CommonForm uuid=\"cccccccc-cccc-4ccc-cccc-cccccccccccc\">"));
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
        assert!(
            fs::read_to_string(root.join("Catalogs/Products/Ext/Help.xml"))
                .unwrap()
                .contains("<Page>ru</Page>")
        );
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
