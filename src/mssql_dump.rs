use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Serialize)]
pub struct MssqlDumpedTableReport {
    pub table: String,
    pub rows: usize,
    pub binary_bytes: usize,
    pub inflated_rows: usize,
    pub module_text_rows: usize,
    pub metadata_xml_rows: usize,
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
}

#[derive(Debug, Deserialize)]
struct ConfigRow {
    #[serde(rename = "file_name")]
    file_name: String,
    #[serde(rename = "part_no")]
    part_no: i32,
    #[serde(rename = "data_size")]
    data_size: i64,
    #[serde(rename = "binary_hex")]
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
        tables: reports,
    })
}

struct DumpedTable {
    rows: Vec<MssqlDumpRowManifest>,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
    metadata_xml_rows: usize,
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
        common_module_body_paths(&rows)
    } else {
        BTreeMap::new()
    };
    let type_index = if extract_metadata_xml {
        build_metadata_type_index(&rows)
    } else {
        BTreeMap::new()
    };

    let mut manifests = Vec::new();
    let mut binary_bytes = 0;
    let mut inflated_rows = 0;
    let mut module_text_rows = 0;
    let mut metadata_xml_rows = 0;
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
            match unpack_module_blob_text(&bytes) {
                Ok(text) => {
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
                Err(_) => None,
            }
        } else {
            None
        };

        let metadata_xml_relative = if extract_metadata_xml {
            match extract_metadata_source_xml(&bytes, &row.file_name, &type_index) {
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

        manifests.push(MssqlDumpRowManifest {
            file_name: row.file_name,
            part_no: row.part_no,
            data_size: row.data_size,
            binary_bytes: bytes.len(),
            binary_path: binary_relative.to_string_lossy().replace('\\', "/"),
            inflated_path: inflated_relative,
            module_text_path: module_text_relative,
            metadata_xml_path: metadata_xml_relative,
        });
    }

    Ok(DumpedTable {
        rows: manifests,
        binary_bytes,
        inflated_rows,
        module_text_rows,
        metadata_xml_rows,
    })
}

fn common_module_body_paths(rows: &[ConfigRow]) -> BTreeMap<String, PathBuf> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeMap::new();

    for row in rows {
        if row.file_name.contains('.') {
            continue;
        }
        let body_id = format!("{}.0", row.file_name);
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some(name) = parse_common_module_name_from_metadata_blob(&bytes, &row.file_name) else {
            continue;
        };
        paths.insert(body_id, common_module_source_path(&name));
    }

    paths
}

fn common_module_source_path(name: &str) -> PathBuf {
    PathBuf::from("CommonModules")
        .join(sanitize_source_path_segment(name))
        .join("Ext")
        .join("Module.bsl")
}

struct ExtractedMetadataSourceXml {
    relative_path: PathBuf,
    xml: Vec<u8>,
}

struct MetadataHeader {
    uuid: String,
    name: String,
    synonyms: Vec<(String, String)>,
    comment: String,
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

struct TypedMetadataProperties {
    value_types: Vec<ConstantValueType>,
}

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

    if object_code == 40 {
        if let Some(type_id) = fields.get(3).copied().and_then(parse_uuid_field) {
            entries.push((type_id, format!("cfg:DocumentRef.{}", header.name)));
        }
    }
    if object_code == 57 {
        if let Some(type_id) = fields.get(3).copied().and_then(parse_uuid_field) {
            entries.push((type_id, format!("cfg:CatalogRef.{}", header.name)));
        }
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

fn is_uuid_text(value: &str) -> bool {
    value.len() == 36 && value.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

fn extract_metadata_source_xml(
    blob: &[u8],
    uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<ExtractedMetadataSourceXml> {
    if uuid.contains('.') {
        return None;
    }
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
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
    if object_code == 0 && is_defined_type_metadata_text(text, uuid) {
        let header = parse_metadata_header_from_text(text, uuid)?;
        let defined_type = parse_defined_type_properties_from_text(text, uuid, type_index)?;
        let relative_path = PathBuf::from("DefinedTypes")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_defined_type_source_xml(&header, &defined_type).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    let (kind, folder) = metadata_source_for_text(object_code, text, uuid)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
    let relative_path = PathBuf::from(folder)
        .join(sanitize_source_path_segment(&header.name))
        .with_extension("xml");
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
        1 if header_index == Some(1) && field_starts_with(fields.get(2), r#"{"Pattern""#) => {
            Some(("EventSubscription", "EventSubscriptions"))
        }
        1 if header_index == Some(1) && field_starts_with(fields.get(1), "{2,") => {
            Some(("SessionParameter", "SessionParameters"))
        }
        2 if header_index == Some(1)
            && fields.get(2).copied().and_then(parse_uuid_field).is_some()
            && field_starts_with(fields.get(3), "{0,") =>
        {
            Some(("FunctionalOption", "FunctionalOptions"))
        }
        5 => Some(("CommonAttribute", "CommonAttributes")),
        6 => Some(("Role", "Roles")),
        14 => Some(("FilterCriterion", "FilterCriteria")),
        17 => Some(("DataProcessor", "DataProcessors")),
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
            let reference = type_index.get(&type_id)?.clone();
            Some(ConstantValueType::Reference { reference })
        }
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
    })
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

fn parse_common_module_name_from_metadata_blob(blob: &[u8], uuid: &str) -> Option<String> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let after_root = text.trim_start().strip_prefix("{1,")?.trim_start();
    if !after_root.starts_with("{12,") {
        return None;
    }

    let uuid_pos = text.find(uuid)?;
    let after_uuid = &text[uuid_pos + uuid.len()..];
    let name_start = after_uuid.find("},\"")? + 2;
    parse_1c_quoted_string(&after_uuid[name_start..])
}

fn parse_1c_quoted_string(input: &str) -> Option<String> {
    let mut chars = input.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut output = String::new();
    let mut previous_quote = false;
    for ch in chars {
        if ch == '"' {
            if previous_quote {
                output.push('"');
                previous_quote = false;
            } else {
                previous_quote = true;
            }
            continue;
        }
        if previous_quote {
            return Some(output);
        }
        output.push(ch);
    }
    None
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
    let sql = format!(
        "SET NOCOUNT ON; USE {db};\n\
         SELECT FileName AS file_name,\n\
                PartNo AS part_no,\n\
                DataSize AS data_size,\n\
                CONVERT(varchar(max), BinaryData, 2) AS binary_hex\n\
         FROM {table}\n\
         {filter}\
         ORDER BY FileName, PartNo\n\
         FOR JSON PATH;",
        db = quote_ident(database),
        table = quote_ident(table),
        filter = filter,
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("dump {table} rows from {database}"))?;
    let json = normalize_sqlcmd_json(&json);
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse {table} rows JSON for {database}"))
}

fn expand_selected_file_names(file_names: &[String]) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    for file_name in file_names {
        let file_name = file_name.trim();
        if file_name.is_empty() {
            continue;
        }
        selected.insert(file_name.to_string());
        if let Some(metadata_id) = file_name.strip_suffix(".0") {
            if !metadata_id.is_empty() {
                selected.insert(metadata_id.to_string());
            }
        } else {
            selected.insert(format!("{file_name}.0"));
        }
    }
    selected
}

fn run_sql_capture(sqlcmd: &Path, server: &str, sql: &str) -> Result<String> {
    let output = Command::new(sqlcmd)
        .arg("-C")
        .arg("-S")
        .arg(server)
        .arg("-y")
        .arg("0")
        .arg("-w")
        .arg("65535")
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
            "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0".to_string(),
            "".to_string(),
        ]);

        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(selected.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0"));
        assert!(selected.contains("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"));
        assert!(selected.contains("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0"));
        assert_eq!(selected.len(), 4);
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
    fn extracts_simple_metadata_xml_from_recognized_blob() {
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let blob = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{57,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"SalesCatalog\",{{2,\"ru\",\"Продажи\",\"en\",\"Sales\"}},\"Comment\"}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
    fn extracts_chart_of_characteristic_types_xml_from_metadata_blob() {
        let uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let blob = deflate_for_test(
            format!(
                "\u{feff}{{1,\r\n{{34,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{0,\r\n{{3,\r\n{{1,0,{uuid}}},\"ExpenseItems\",{{1,\"en\",\"Expense items\"}},\"\"}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
            extract_metadata_source_xml(&enum_blob, enum_uuid, &BTreeMap::new())
                .unwrap()
                .relative_path,
            PathBuf::from("Enums").join("Status.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(&report_blob, report_uuid, &BTreeMap::new())
                .unwrap()
                .relative_path,
            PathBuf::from("Reports").join("SalesReport.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(&subsystem_blob, subsystem_uuid, &BTreeMap::new())
                .unwrap()
                .relative_path,
            PathBuf::from("Subsystems").join("Sales.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(&accounting_blob, accounting_uuid, &BTreeMap::new())
                .unwrap()
                .relative_path,
            PathBuf::from("AccountingRegisters").join("Ledger.xml")
        );
        assert_eq!(
            extract_metadata_source_xml(&task_blob, task_uuid, &BTreeMap::new())
                .unwrap()
                .relative_path,
            PathBuf::from("Tasks").join("Task.xml")
        );
    }

    #[test]
    fn ignores_report_and_task_rows_in_generated_type_index() {
        let report_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let report_object_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let report_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{20,{report_object_type_id},cccccccc-cccc-4ccc-cccc-cccccccccccc,\r\n{{0,\r\n{{3,\r\n{{1,0,{report_uuid}}},\"SalesReport\",{{1,\"en\",\"Sales report\"}},\"\"}}\r\n}},0}}\r\n}}"
            )
            .as_bytes(),
        );
        let task_uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let task_generated_type_id = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let task_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{33,\r\n{{3,\r\n{{1,0,{task_uuid}}},\"Task\",{{1,\"en\",\"Task\"}},\"\"}},0,{task_generated_type_id}}}\r\n}}"
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
        assert!(!index.contains_key(task_generated_type_id));
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

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
    fn extracts_constant_xml_from_metadata_blob() {
        let uuid = "dddddddd-dddd-4ddd-dddd-dddddddddddd";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{16,\r\n{{27,\r\n{{2,\r\n{{3,\r\n{{1,0,{uuid}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"Feature flag\",0,0,00000000-0000-0000-0000-000000000000,0}},{{\"Pattern\",{{\"B\"}}}}\r\n}},0,\r\n{{0}},\r\n{{0}},0,\"\",0,\r\n{{\"U\"}},\r\n{{\"U\"}},0,00000000-0000-0000-0000-000000000000,2,0,\r\n{{5006,0}},\r\n{{3,0,0}},\r\n{{0,0}},0,\r\n{{0}},\r\n{{\"S\",\"\"}},0,0,0}},00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,1,\r\n{{0}},1,0}}\r\n}}\r\n}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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

        let extracted = extract_metadata_source_xml(&blob, uuid, &type_index).unwrap();
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

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
    fn extracts_defined_type_xml_from_metadata_blob() {
        let uuid = "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{0,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,\r\n{{3,\r\n{{1,0,{uuid}}},\"OwnerType\",{{1,\"en\",\"Owner type\"}},\"Defined comment\",0,0,00000000-0000-0000-0000-000000000000,0}},\r\n{{\"Pattern\",{{\"B\"}},{{\"S\",80,1}}}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
        let catalog_ref_type_id = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let catalog_blob = deflate_for_test(
            format!(
                "{{1,\r\n{{57,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,{catalog_ref_type_id},33333333-3333-4333-8333-333333333333,\r\n{{0,\r\n{{3,\r\n{{1,0,{catalog_uuid}}},\"Customers\",{{1,\"en\",\"Customers\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}\r\n}},0}}\r\n}}"
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
            index.get(catalog_ref_type_id).map(String::as_str),
            Some("cfg:CatalogRef.Customers")
        );
        assert_eq!(
            index.get(enum_ref_type_id).map(String::as_str),
            Some("cfg:EnumRef.Statuses")
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
