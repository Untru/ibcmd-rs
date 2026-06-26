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

        let source_asset_relative =
            if module_text_relative.is_none() && metadata_xml_relative.is_none() {
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

#[derive(Clone, Copy)]
enum SourceAssetKind {
    Binary,
    ExtPicture,
    Schedule,
}

struct SourceAsset {
    primary_path: PathBuf,
    kind: SourceAssetKind,
}

fn source_asset_paths(rows: &[ConfigRow]) -> BTreeMap<String, SourceAsset> {
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
                        kind: *kind,
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
        let Some(asset) = source_asset_from_metadata_blob(&bytes, &row.file_name, &file_names)
        else {
            continue;
        };
        paths.insert(format!("{}.0", row.file_name), asset);
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

fn source_asset_from_metadata_blob(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
) -> Option<SourceAsset> {
    let body_id = format!("{uuid}.0");
    if !file_names.contains(body_id.as_str()) {
        return None;
    }
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let object_code = parse_metadata_object_code(text)?;
    let (kind, folder) = metadata_source_for_text(object_code, text, uuid)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
    match kind {
        "CommonPicture" => Some(SourceAsset {
            primary_path: PathBuf::from(folder)
                .join(sanitize_source_path_segment(&header.name))
                .join("Ext")
                .join("Picture.xml"),
            kind: SourceAssetKind::ExtPicture,
        }),
        "ScheduledJob" => Some(SourceAsset {
            primary_path: PathBuf::from(folder)
                .join(sanitize_source_path_segment(&header.name))
                .join("Ext")
                .join("Schedule.xml"),
            kind: SourceAssetKind::Schedule,
        }),
        _ => None,
    }
}

fn write_source_asset(output_dir: &Path, asset: &SourceAsset, bytes: &[u8]) -> Result<PathBuf> {
    match asset.kind {
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
    }

    Ok(asset.primary_path.clone())
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

fn nested_command_headers_from_text(text: &str, owner_uuid: &str) -> Vec<MetadataHeader> {
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
        if !is_offset_inside_metadata_object_code(text, marker_start, 9) {
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
        2 if header_index == Some(1)
            && field_is_quoted_string(fields.get(2))
            && field_is_quoted_string(fields.get(3)) =>
        {
            Some(("ScheduledJob", "ScheduledJobs"))
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
    fn extracts_common_command_xml_from_metadata_blob() {
        let uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let blob = deflate_for_test(
            format!(
                "{{1,\r\n{{2,\r\n{{1,\r\n{{2,{uuid},078a6af8-d22c-4248-9c33-7e90075a3d2c}},\r\n{{9,\r\n{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},3,\r\n{{1,\"en\",\"Open settings\"}},1,\r\n{{0,0,0}},0,\r\n{{1,aabb34e1-98c1-4bd0-bf7f-243f95437b44}},\r\n{{\"Pattern\"}},\r\n{{3,\r\n{{1,0,{uuid}}},\"OpenSettings\",{{1,\"en\",\"Open settings\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0}}\r\n}}\r\n}},0}}"
            )
            .as_bytes(),
        );

        let extracted = extract_metadata_source_xml(&blob, uuid, &BTreeMap::new()).unwrap();
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
                "{{1,\r\n{{4,\r\n{{3,\r\n{{1,0,{text_uuid}}},\"DocumentKinds\",{{1,\"en\",\"Document kinds\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},4}},0}}"
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
            let extracted = extract_metadata_source_xml(blob, uuid, &BTreeMap::new()).unwrap();
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
