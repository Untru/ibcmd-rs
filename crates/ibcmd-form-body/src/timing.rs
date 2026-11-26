//! The dump's timing report (from `src/mssql_dump/timing.rs`, which still
//! reads, sums and summarizes it). The form reader adds its parts to it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// The parts of `prepare_field_refs_ms`: child names, child types, type
    /// set leaves, information register fields, master dimensions, the
    /// dynamic-list field declarations.
    pub prepare_field_names_ms: u64,
    pub prepare_field_types_ms: u64,
    pub prepare_type_set_leaves_ms: u64,
    pub prepare_register_fields_ms: u64,
    pub prepare_master_dimensions_ms: u64,
    pub prepare_field_declarations_ms: u64,
    pub prepare_functional_option_refs_ms: u64,
    pub prepare_source_assets_ms: u64,
    pub prepare_help_refs_ms: u64,
    pub prepare_standalone_refs_ms: u64,
    pub prepare_body_owners_ms: u64,
    /// The recalculation indexes (a recalculation's and a calculation
    /// register's own XML read them).
    pub prepare_recalculation_refs_ms: u64,
    /// The module body paths.
    pub prepare_module_paths_ms: u64,
    /// The predefined items the metadata values and flowcharts name, with
    /// the bodies they are read from.
    pub prepare_value_predefined_items_ms: u64,
    /// `--model-export`: building the name index from the rows.
    pub prepare_model_index_ms: u64,
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
    /// `--model-export`: the descriptors written through the model, and
    /// their share of `metadata_xml_cpu_ms`.
    pub model_metadata_xml_rows: u64,
    pub model_metadata_xml_cpu_ms: u64,
    /// Rows of a modelled kind the model failed on, written by the legacy
    /// converter instead.
    pub model_fallback_rows: u64,
    /// Producing the descriptor XML in memory, without the file write, in
    /// microseconds: every descriptor, and the model's share.
    pub metadata_xml_convert_us: u64,
    pub model_metadata_xml_convert_us: u64,
    /// `IBCMD_RS_MODEL_EXPORT_SHADOW=1`: the legacy converter also runs on
    /// every modelled row; its time and the rows whose bytes differ.
    pub model_shadow_rows: u64,
    pub model_shadow_legacy_convert_us: u64,
    pub model_shadow_differing_rows: u64,
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
    /// Creating the form's folder and writing its `Form.xml`: file-system
    /// time, which waits on the disk and the other writers rather than
    /// working the CPU.
    pub source_asset_form_write_cpu_ms: u64,
    /// The export's writer threads (`0`: every file written by its worker).
    pub output_write_threads: u64,
    pub output_write_files: u64,
    pub output_write_bytes: u64,
    /// Folders the writers created, and the time that took.
    pub output_write_folders: u64,
    pub output_write_folder_ms: u64,
    /// Time the writer threads spent writing, summed over them.
    pub output_write_busy_ms: u64,
    /// Time the converting workers waited for room in the write queue.
    pub output_write_wait_ms: u64,
    /// Time the end of a table waited for its queued files to land.
    pub output_write_drain_ms: u64,
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
    /// Finer parts of the index phase, by name (`object_refs.index`, ...).
    pub prepare_detail_ms: BTreeMap<String, u64>,
    /// CPU time of the whole process (every thread) spent in each phase, by
    /// name (`prepare`, `prepare.object_refs`, `process_rows`, ...): the
    /// measure a shared machine does not distort.
    pub cpu_ms: BTreeMap<String, u64>,
}
