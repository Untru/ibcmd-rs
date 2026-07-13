use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{
    MssqlDumpConfigReport, MssqlDumpedTableReport, SQLCMD_DUMP_BATCH_MAX_DATA_BYTES,
    SourceAssetKind,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MssqlDumpTimingReport {
    pub fetch_headers_ms: u64,
    pub fetch_headers_sqlcmd_ms: u64,
    pub prepare_indexes_ms: u64,
    pub prepare_metadata_fetch_ms: u64,
    pub prepare_metadata_fetch_sqlcmd_ms: u64,
    pub prepare_metadata_fetch_bcp_ms: u64,
    pub prepare_metadata_texts_ms: u64,
    pub prepare_reference_indexes_ms: u64,
    pub prepare_command_refs_ms: u64,
    pub prepare_metadata_refs_ms: u64,
    pub prepare_type_index_ms: u64,
    pub prepare_form_refs_ms: u64,
    pub prepare_template_refs_ms: u64,
    pub prepare_subsystem_refs_ms: u64,
    pub prepare_object_refs_ms: u64,
    pub prepare_field_refs_ms: u64,
    pub prepare_functional_option_refs_ms: u64,
    pub prepare_source_assets_ms: u64,
    pub prepare_help_refs_ms: u64,
    pub prepare_standalone_refs_ms: u64,
    pub prepare_body_owners_ms: u64,
    pub fetch_rows_ms: u64,
    pub fetch_rows_bcp_ms: u64,
    pub fetch_row_batches: u64,
    pub fetch_row_batch_max_rows: u64,
    pub fetch_row_batch_max_binary_bytes: u64,
    pub process_rows_wall_ms: u64,
    pub binary_write_cpu_ms: u64,
    pub inflate_cpu_ms: u64,
    pub form_body_parse_cpu_ms: u64,
    pub module_text_cpu_ms: u64,
    pub metadata_xml_cpu_ms: u64,
    pub source_asset_cpu_ms: u64,
    pub source_asset_form_cpu_ms: u64,
    pub source_asset_form_xml_cpu_ms: u64,
    pub source_asset_form_split_cpu_ms: u64,
    pub source_asset_form_properties_cpu_ms: u64,
    pub source_asset_form_events_cpu_ms: u64,
    pub source_asset_form_attributes_cpu_ms: u64,
    pub source_asset_form_parameters_cpu_ms: u64,
    pub source_asset_form_commands_cpu_ms: u64,
    pub source_asset_form_auto_command_bar_cpu_ms: u64,
    pub source_asset_form_child_items_cpu_ms: u64,
    pub source_asset_form_command_interface_cpu_ms: u64,
    pub source_asset_form_format_cpu_ms: u64,
    pub source_asset_form_items_cpu_ms: u64,
    pub source_asset_help_cpu_ms: u64,
    pub source_asset_moxel_cpu_ms: u64,
    pub source_asset_inflated_cpu_ms: u64,
    pub source_asset_command_interface_cpu_ms: u64,
    pub source_asset_ext_picture_cpu_ms: u64,
    pub source_asset_predefined_data_cpu_ms: u64,
    pub source_asset_role_rights_cpu_ms: u64,
    pub source_asset_standalone_content_cpu_ms: u64,
    pub source_asset_style_body_cpu_ms: u64,
    pub source_asset_exchange_plan_cpu_ms: u64,
    pub source_asset_business_process_cpu_ms: u64,
    pub source_asset_schedule_cpu_ms: u64,
    pub source_asset_config_dump_info_cpu_ms: u64,
    pub source_asset_other_cpu_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MssqlDumpTimingSummary {
    pub report_path: Option<PathBuf>,
    pub database: String,
    pub output_dir: PathBuf,
    pub total_rows: usize,
    pub total_binary_bytes: usize,
    pub total_source_asset_rows: usize,
    pub batch_cap_binary_bytes: u64,
    pub fetch_row_batches: u64,
    pub fetch_row_batch_max_rows: u64,
    pub fetch_row_batch_max_binary_bytes: u64,
    pub fetch_row_batch_max_binary_mib: u64,
    pub prepare_indexes_ms: u64,
    pub prepare_metadata_fetch_ms: u64,
    pub prepare_metadata_fetch_sqlcmd_ms: u64,
    pub prepare_metadata_fetch_bcp_ms: u64,
    pub prepare_metadata_texts_ms: u64,
    pub prepare_reference_indexes_ms: u64,
    pub prepare_command_refs_ms: u64,
    pub fetch_rows_ms: u64,
    pub fetch_rows_bcp_ms: u64,
    pub process_rows_wall_ms: u64,
    pub source_asset_cpu_ms: u64,
    pub source_asset_cpu_ms_per_row: Option<u64>,
    pub source_asset_cpu_breakdown: Vec<MssqlDumpCpuTimingSummary>,
    pub form_cpu_breakdown: Vec<MssqlDumpCpuTimingSummary>,
    pub fetch_rows_ms_per_gib: Option<u64>,
    pub process_rows_wall_ms_per_gib: Option<u64>,
    pub tables: Vec<MssqlDumpTableTimingSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MssqlDumpCpuTimingSummary {
    pub metric: &'static str,
    pub cpu_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MssqlDumpTableTimingSummary {
    pub table: String,
    pub rows: usize,
    pub binary_bytes: usize,
    pub source_asset_rows: usize,
    pub fetch_row_batches: u64,
    pub fetch_row_batch_max_rows: u64,
    pub fetch_row_batch_max_binary_bytes: u64,
    pub fetch_row_batch_max_binary_mib: u64,
    pub prepare_indexes_ms: u64,
    pub prepare_metadata_fetch_ms: u64,
    pub prepare_metadata_texts_ms: u64,
    pub prepare_reference_indexes_ms: u64,
    pub prepare_command_refs_ms: u64,
    pub fetch_rows_ms: u64,
    pub fetch_rows_bcp_ms: u64,
    pub process_rows_wall_ms: u64,
    pub source_asset_cpu_ms: u64,
    pub source_asset_cpu_ms_per_row: Option<u64>,
    pub source_asset_cpu_breakdown: Vec<MssqlDumpCpuTimingSummary>,
    pub form_cpu_breakdown: Vec<MssqlDumpCpuTimingSummary>,
    pub fetch_rows_ms_per_gib: Option<u64>,
    pub process_rows_wall_ms_per_gib: Option<u64>,
}

pub fn read_dump_timing_summaries(paths: &[PathBuf]) -> Result<Vec<MssqlDumpTimingSummary>> {
    paths
        .iter()
        .map(|path| read_dump_timing_summary(path))
        .collect()
}

pub fn read_dump_timing_summary(path: &Path) -> Result<MssqlDumpTimingSummary> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read dump timing report {}", path.display()))?;
    parse_dump_timing_summary(&json, Some(path.to_path_buf()))
}

pub fn parse_dump_timing_summary(
    json: &str,
    report_path: Option<PathBuf>,
) -> Result<MssqlDumpTimingSummary> {
    let report: MssqlDumpConfigReport =
        serde_json::from_str(json).context("failed to parse mssql-dump-config JSON report")?;
    Ok(MssqlDumpTimingSummary::from_report(&report, report_path))
}

impl MssqlDumpTimingSummary {
    fn from_report(report: &MssqlDumpConfigReport, report_path: Option<PathBuf>) -> Self {
        Self {
            report_path,
            database: report.database.clone(),
            output_dir: report.output_dir.clone(),
            total_rows: report.total_rows,
            total_binary_bytes: report.total_binary_bytes,
            total_source_asset_rows: report.total_source_asset_rows,
            batch_cap_binary_bytes: SQLCMD_DUMP_BATCH_MAX_DATA_BYTES,
            fetch_row_batches: report.timings.fetch_row_batches,
            fetch_row_batch_max_rows: report.timings.fetch_row_batch_max_rows,
            fetch_row_batch_max_binary_bytes: report.timings.fetch_row_batch_max_binary_bytes,
            fetch_row_batch_max_binary_mib: bytes_to_mib_ceil(
                report.timings.fetch_row_batch_max_binary_bytes,
            ),
            prepare_indexes_ms: report.timings.prepare_indexes_ms,
            prepare_metadata_fetch_ms: report.timings.prepare_metadata_fetch_ms,
            prepare_metadata_fetch_sqlcmd_ms: report.timings.prepare_metadata_fetch_sqlcmd_ms,
            prepare_metadata_fetch_bcp_ms: report.timings.prepare_metadata_fetch_bcp_ms,
            prepare_metadata_texts_ms: report.timings.prepare_metadata_texts_ms,
            prepare_reference_indexes_ms: report.timings.prepare_reference_indexes_ms,
            prepare_command_refs_ms: report.timings.prepare_command_refs_ms,
            fetch_rows_ms: report.timings.fetch_rows_ms,
            fetch_rows_bcp_ms: report.timings.fetch_rows_bcp_ms,
            process_rows_wall_ms: report.timings.process_rows_wall_ms,
            source_asset_cpu_ms: report.timings.source_asset_cpu_ms,
            source_asset_cpu_ms_per_row: ms_per_row(
                report.timings.source_asset_cpu_ms,
                report.total_source_asset_rows as u64,
            ),
            source_asset_cpu_breakdown: source_asset_cpu_breakdown(&report.timings),
            form_cpu_breakdown: form_cpu_breakdown(&report.timings),
            fetch_rows_ms_per_gib: ms_per_gib(
                report.timings.fetch_rows_ms,
                report.total_binary_bytes as u64,
            ),
            process_rows_wall_ms_per_gib: ms_per_gib(
                report.timings.process_rows_wall_ms,
                report.total_binary_bytes as u64,
            ),
            tables: report
                .tables
                .iter()
                .map(MssqlDumpTableTimingSummary::from_table_report)
                .collect(),
        }
    }
}

impl MssqlDumpTableTimingSummary {
    fn from_table_report(table: &MssqlDumpedTableReport) -> Self {
        Self {
            table: table.table.clone(),
            rows: table.rows,
            binary_bytes: table.binary_bytes,
            source_asset_rows: table.source_asset_rows,
            fetch_row_batches: table.timings.fetch_row_batches,
            fetch_row_batch_max_rows: table.timings.fetch_row_batch_max_rows,
            fetch_row_batch_max_binary_bytes: table.timings.fetch_row_batch_max_binary_bytes,
            fetch_row_batch_max_binary_mib: bytes_to_mib_ceil(
                table.timings.fetch_row_batch_max_binary_bytes,
            ),
            prepare_indexes_ms: table.timings.prepare_indexes_ms,
            prepare_metadata_fetch_ms: table.timings.prepare_metadata_fetch_ms,
            prepare_metadata_texts_ms: table.timings.prepare_metadata_texts_ms,
            prepare_reference_indexes_ms: table.timings.prepare_reference_indexes_ms,
            prepare_command_refs_ms: table.timings.prepare_command_refs_ms,
            fetch_rows_ms: table.timings.fetch_rows_ms,
            fetch_rows_bcp_ms: table.timings.fetch_rows_bcp_ms,
            process_rows_wall_ms: table.timings.process_rows_wall_ms,
            source_asset_cpu_ms: table.timings.source_asset_cpu_ms,
            source_asset_cpu_ms_per_row: ms_per_row(
                table.timings.source_asset_cpu_ms,
                table.source_asset_rows as u64,
            ),
            source_asset_cpu_breakdown: source_asset_cpu_breakdown(&table.timings),
            form_cpu_breakdown: form_cpu_breakdown(&table.timings),
            fetch_rows_ms_per_gib: ms_per_gib(
                table.timings.fetch_rows_ms,
                table.binary_bytes as u64,
            ),
            process_rows_wall_ms_per_gib: ms_per_gib(
                table.timings.process_rows_wall_ms,
                table.binary_bytes as u64,
            ),
        }
    }
}

fn source_asset_cpu_breakdown(timings: &MssqlDumpTimingReport) -> Vec<MssqlDumpCpuTimingSummary> {
    sorted_nonzero_cpu_timings([
        ("form", timings.source_asset_form_cpu_ms),
        ("help", timings.source_asset_help_cpu_ms),
        ("moxel", timings.source_asset_moxel_cpu_ms),
        ("inflated", timings.source_asset_inflated_cpu_ms),
        (
            "command_interface",
            timings.source_asset_command_interface_cpu_ms,
        ),
        ("ext_picture", timings.source_asset_ext_picture_cpu_ms),
        (
            "predefined_data",
            timings.source_asset_predefined_data_cpu_ms,
        ),
        ("role_rights", timings.source_asset_role_rights_cpu_ms),
        (
            "standalone_content",
            timings.source_asset_standalone_content_cpu_ms,
        ),
        ("style_body", timings.source_asset_style_body_cpu_ms),
        ("exchange_plan", timings.source_asset_exchange_plan_cpu_ms),
        (
            "business_process",
            timings.source_asset_business_process_cpu_ms,
        ),
        ("schedule", timings.source_asset_schedule_cpu_ms),
        (
            "config_dump_info",
            timings.source_asset_config_dump_info_cpu_ms,
        ),
        ("other", timings.source_asset_other_cpu_ms),
    ])
}

fn form_cpu_breakdown(timings: &MssqlDumpTimingReport) -> Vec<MssqlDumpCpuTimingSummary> {
    sorted_nonzero_cpu_timings([
        ("form_xml", timings.source_asset_form_xml_cpu_ms),
        ("split", timings.source_asset_form_split_cpu_ms),
        ("properties", timings.source_asset_form_properties_cpu_ms),
        ("events", timings.source_asset_form_events_cpu_ms),
        ("attributes", timings.source_asset_form_attributes_cpu_ms),
        ("parameters", timings.source_asset_form_parameters_cpu_ms),
        ("commands", timings.source_asset_form_commands_cpu_ms),
        (
            "auto_command_bar",
            timings.source_asset_form_auto_command_bar_cpu_ms,
        ),
        ("child_items", timings.source_asset_form_child_items_cpu_ms),
        (
            "command_interface",
            timings.source_asset_form_command_interface_cpu_ms,
        ),
        ("format", timings.source_asset_form_format_cpu_ms),
        ("items", timings.source_asset_form_items_cpu_ms),
    ])
}

fn sorted_nonzero_cpu_timings<const N: usize>(
    timings: [(&'static str, u64); N],
) -> Vec<MssqlDumpCpuTimingSummary> {
    let mut timings: Vec<_> = timings
        .into_iter()
        .filter(|(_, cpu_ms)| *cpu_ms > 0)
        .map(|(metric, cpu_ms)| MssqlDumpCpuTimingSummary { metric, cpu_ms })
        .collect();
    timings.sort_by(|left, right| {
        right
            .cpu_ms
            .cmp(&left.cpu_ms)
            .then_with(|| left.metric.cmp(right.metric))
    });
    timings
}

fn bytes_to_mib_ceil(bytes: u64) -> u64 {
    if bytes == 0 {
        0
    } else {
        bytes.div_ceil(1024 * 1024)
    }
}

fn ms_per_row(elapsed_ms: u64, rows: u64) -> Option<u64> {
    if rows == 0 {
        return None;
    }
    Some(elapsed_ms / rows)
}

fn ms_per_gib(elapsed_ms: u64, bytes: u64) -> Option<u64> {
    if bytes == 0 {
        return None;
    }
    let scaled = u128::from(elapsed_ms) * 1024 * 1024 * 1024;
    Some((scaled / u128::from(bytes)).min(u128::from(u64::MAX)) as u64)
}

impl MssqlDumpTimingReport {
    pub(crate) fn add_assign(&mut self, other: &Self) {
        self.fetch_headers_ms += other.fetch_headers_ms;
        self.fetch_headers_sqlcmd_ms += other.fetch_headers_sqlcmd_ms;
        self.prepare_indexes_ms += other.prepare_indexes_ms;
        self.prepare_metadata_fetch_ms += other.prepare_metadata_fetch_ms;
        self.prepare_metadata_fetch_sqlcmd_ms += other.prepare_metadata_fetch_sqlcmd_ms;
        self.prepare_metadata_fetch_bcp_ms += other.prepare_metadata_fetch_bcp_ms;
        self.prepare_metadata_texts_ms += other.prepare_metadata_texts_ms;
        self.prepare_reference_indexes_ms += other.prepare_reference_indexes_ms;
        self.prepare_command_refs_ms += other.prepare_command_refs_ms;
        self.prepare_metadata_refs_ms += other.prepare_metadata_refs_ms;
        self.prepare_type_index_ms += other.prepare_type_index_ms;
        self.prepare_form_refs_ms += other.prepare_form_refs_ms;
        self.prepare_template_refs_ms += other.prepare_template_refs_ms;
        self.prepare_subsystem_refs_ms += other.prepare_subsystem_refs_ms;
        self.prepare_object_refs_ms += other.prepare_object_refs_ms;
        self.prepare_field_refs_ms += other.prepare_field_refs_ms;
        self.prepare_functional_option_refs_ms += other.prepare_functional_option_refs_ms;
        self.prepare_source_assets_ms += other.prepare_source_assets_ms;
        self.prepare_help_refs_ms += other.prepare_help_refs_ms;
        self.prepare_standalone_refs_ms += other.prepare_standalone_refs_ms;
        self.prepare_body_owners_ms += other.prepare_body_owners_ms;
        self.fetch_rows_ms += other.fetch_rows_ms;
        self.fetch_rows_bcp_ms += other.fetch_rows_bcp_ms;
        self.fetch_row_batches += other.fetch_row_batches;
        self.fetch_row_batch_max_rows = self
            .fetch_row_batch_max_rows
            .max(other.fetch_row_batch_max_rows);
        self.fetch_row_batch_max_binary_bytes = self
            .fetch_row_batch_max_binary_bytes
            .max(other.fetch_row_batch_max_binary_bytes);
        self.process_rows_wall_ms += other.process_rows_wall_ms;
        self.binary_write_cpu_ms += other.binary_write_cpu_ms;
        self.inflate_cpu_ms += other.inflate_cpu_ms;
        self.form_body_parse_cpu_ms += other.form_body_parse_cpu_ms;
        self.module_text_cpu_ms += other.module_text_cpu_ms;
        self.metadata_xml_cpu_ms += other.metadata_xml_cpu_ms;
        self.source_asset_cpu_ms += other.source_asset_cpu_ms;
        self.source_asset_form_cpu_ms += other.source_asset_form_cpu_ms;
        self.source_asset_form_xml_cpu_ms += other.source_asset_form_xml_cpu_ms;
        self.source_asset_form_split_cpu_ms += other.source_asset_form_split_cpu_ms;
        self.source_asset_form_properties_cpu_ms += other.source_asset_form_properties_cpu_ms;
        self.source_asset_form_events_cpu_ms += other.source_asset_form_events_cpu_ms;
        self.source_asset_form_attributes_cpu_ms += other.source_asset_form_attributes_cpu_ms;
        self.source_asset_form_parameters_cpu_ms += other.source_asset_form_parameters_cpu_ms;
        self.source_asset_form_commands_cpu_ms += other.source_asset_form_commands_cpu_ms;
        self.source_asset_form_auto_command_bar_cpu_ms +=
            other.source_asset_form_auto_command_bar_cpu_ms;
        self.source_asset_form_child_items_cpu_ms += other.source_asset_form_child_items_cpu_ms;
        self.source_asset_form_command_interface_cpu_ms +=
            other.source_asset_form_command_interface_cpu_ms;
        self.source_asset_form_format_cpu_ms += other.source_asset_form_format_cpu_ms;
        self.source_asset_form_items_cpu_ms += other.source_asset_form_items_cpu_ms;
        self.source_asset_help_cpu_ms += other.source_asset_help_cpu_ms;
        self.source_asset_moxel_cpu_ms += other.source_asset_moxel_cpu_ms;
        self.source_asset_inflated_cpu_ms += other.source_asset_inflated_cpu_ms;
        self.source_asset_command_interface_cpu_ms += other.source_asset_command_interface_cpu_ms;
        self.source_asset_ext_picture_cpu_ms += other.source_asset_ext_picture_cpu_ms;
        self.source_asset_predefined_data_cpu_ms += other.source_asset_predefined_data_cpu_ms;
        self.source_asset_role_rights_cpu_ms += other.source_asset_role_rights_cpu_ms;
        self.source_asset_standalone_content_cpu_ms += other.source_asset_standalone_content_cpu_ms;
        self.source_asset_style_body_cpu_ms += other.source_asset_style_body_cpu_ms;
        self.source_asset_exchange_plan_cpu_ms += other.source_asset_exchange_plan_cpu_ms;
        self.source_asset_business_process_cpu_ms += other.source_asset_business_process_cpu_ms;
        self.source_asset_schedule_cpu_ms += other.source_asset_schedule_cpu_ms;
        self.source_asset_config_dump_info_cpu_ms += other.source_asset_config_dump_info_cpu_ms;
        self.source_asset_other_cpu_ms += other.source_asset_other_cpu_ms;
    }

    pub(crate) fn add_source_asset_kind(&mut self, kind: &SourceAssetKind, elapsed_ms: u64) {
        match kind {
            SourceAssetKind::Form { .. } => self.source_asset_form_cpu_ms += elapsed_ms,
            SourceAssetKind::Help => self.source_asset_help_cpu_ms += elapsed_ms,
            SourceAssetKind::MoxelSpreadsheet => self.source_asset_moxel_cpu_ms += elapsed_ms,
            SourceAssetKind::DataCompositionSchema
            | SourceAssetKind::ClientApplicationInterface
            | SourceAssetKind::InflatedBinary
            | SourceAssetKind::InflatedBase64OrBinary
            | SourceAssetKind::HomePageWorkArea
            | SourceAssetKind::WsDefinition => {
                self.source_asset_inflated_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::CommandInterface => {
                self.source_asset_command_interface_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::ExtPicture => self.source_asset_ext_picture_cpu_ms += elapsed_ms,
            SourceAssetKind::PredefinedData { .. } => {
                self.source_asset_predefined_data_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::RoleRights => self.source_asset_role_rights_cpu_ms += elapsed_ms,
            SourceAssetKind::StandaloneContent => {
                self.source_asset_standalone_content_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::StyleBody => self.source_asset_style_body_cpu_ms += elapsed_ms,
            SourceAssetKind::ExchangePlanContent => {
                self.source_asset_exchange_plan_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::BusinessProcessFlowchart => {
                self.source_asset_business_process_cpu_ms += elapsed_ms;
            }
            SourceAssetKind::Schedule => self.source_asset_schedule_cpu_ms += elapsed_ms,
            SourceAssetKind::AccumulationRegisterAggregates { .. } => {
                self.source_asset_other_cpu_ms += elapsed_ms;
            }
        }
    }
}
