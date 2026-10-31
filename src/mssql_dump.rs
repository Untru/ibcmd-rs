//! The configuration exporter: the `ibcmd-dump` crate's `mssql_dump` module,
//! at its old path.
//!
//! The crate made the items the root reads `#[doc(hidden)] pub`; the names
//! below re-export each of them crate-visibly, which shadows the glob, so
//! none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_dump::mssql_dump::*;

// The command line's export, which fills the `--object` selector from the
// stored objects; it shadows the crate's own `dump_config`, which has no
// selector and refuses `--object`.
pub use crate::stored_objects::dump_config;

pub(crate) use ibcmd_dump::mssql_dump::{
    ConfigTable, DumpHooks, FileDbReadScope, FileInfobaseConfig, FileSink, StagedRow, StateBase,
    StateExportReport, StoredActivationRows, active_generation_history, constant_row_always_used,
    dump_config_with, dump_config_with_descriptor_validation, dump_config_with_selected_validation,
    dump_config_with_sql, dynamic_generation_aliases, export_staged_state, extract_form_body_xml,
    extract_inflated_moxel_spreadsheet_xml_with_line_trace, extract_moxel_spreadsheet_xml,
    extract_moxel_spreadsheet_xml_with_line_trace, extract_standalone_metadata_source_xml,
    fetch_config_part0_rows, fetch_config_row_whole, fetch_extension_activation_rows,
    fetch_main_activation_rows, file_db_rows_from_reader, file_extension_base_provider,
    file_infobase_config, file_infobase_effective, help_link_marker_is_anchor,
    help_standard_picture_negative_index, read_config_dump_info_with_sql, rewrite_help_links,
    role_rights_xml_from_blob, root_family_folder, trace_form_body_with_context,
    try_extract_moxel_spreadsheet_xml_with_generated_types, unpack_form_body_module_text,
    with_database_table, with_file_db_table, with_main_database_table, with_memory_rows_table,
    with_rows_dir_table, write_source_xml_file,
};

// The exporter's crate-visible re-exports of other crates' items, from their
// crates.
pub(crate) use ibcmd_1c_text::parse_1c_quoted_string;
pub(crate) use ibcmd_dynamic_generation::stored_row_name;
pub(crate) use ibcmd_form_body::form_body::{
    FormItemSchemaTraceEvent, FormItemTraceEvent, FormItemTraceSink,
};
pub(crate) use ibcmd_form_body::normalize_data_composition_schema_template_documents_with_profiles;
pub(crate) use ibcmd_form_writer::form::load_8_5_1::{
    down_convert_xml_2_21_form, is_xml_2_21_form, up_convert_chart_records_8_5_1,
    up_convert_primitives_8_5_1_in_place,
};
pub(crate) use ibcmd_moxel::{MoxelLineTraceEvent, MoxelLineTraceSink};

// The type of `DumpHooks.select_objects`, which the root fills by a function;
// it still hides the widened type from the glob.
#[expect(
    unused_imports,
    reason = "the re-export hides a widened item from the public glob"
)]
pub(crate) use ibcmd_dump::mssql_dump::ObjectSelector;
// Read by the root's `module_blob` tests.
#[cfg(test)]
pub(crate) use ibcmd_dump::mssql_dump::extract_style_body_xml;
// The exporter's probes, read by the root's tests of the stored objects and
// the form writer.
#[cfg(test)]
pub(crate) use ibcmd_dump::test_fixtures::{
    file_db_fresh_load_count, file_db_read_scope_active, render_form_body_xml_offline,
};

// The command-line argument structs, which `cli` re-exports, and the
// extension export.
pub(crate) use ibcmd_dump::mssql_dump::{args, extension};

/// The incremental export's state: `ibcmd-dump`'s `mssql_dump::incremental`.
pub mod incremental {
    pub use ibcmd_dump::mssql_dump::incremental::*;

    pub(crate) use ibcmd_dump::mssql_dump::incremental::{
        DumpInfo, NativeFullExportRequired, Ownership, prepare_output_dir,
    };
    // Read by the root's `infobase` tests; outside tests it still hides the
    // widened item from the glob.
    #[cfg_attr(
        not(test),
        expect(
            unused_imports,
            reason = "the re-export hides a widened item from the public glob"
        )
    )]
    pub(crate) use ibcmd_dump::mssql_dump::incremental::CONFIG_DUMP_INFO;
    // The root reads it only through `DumpInfo::entries`; it still hides the
    // widened type from the glob.
    #[expect(
        unused_imports,
        reason = "the re-export hides a widened item from the public glob"
    )]
    pub(crate) use ibcmd_dump::mssql_dump::incremental::DumpInfoEntry;
}

/// The saved configuration's `.cf`: `ibcmd-dump`'s `mssql_dump::config_save`.
pub mod config_save {
    pub use ibcmd_dump::mssql_dump::config_save::*;

    pub(crate) use ibcmd_dump::mssql_dump::config_save::write_rows_cf;
    // The root reads it only through `write_rows_cf`; it still hides the
    // widened type from the glob.
    #[expect(
        unused_imports,
        reason = "the re-export hides a widened item from the public glob"
    )]
    pub(crate) use ibcmd_dump::mssql_dump::config_save::RowsCf;
}

/// The stored CAS images: `ibcmd-dump`'s `mssql_dump::cas`.
pub mod cas {
    pub use ibcmd_dump::mssql_dump::cas::*;

    pub(crate) use ibcmd_dump::mssql_dump::cas::resolve_cas_storage_image_with_origin;
}

/// The model export and its compatibility probe:
/// `ibcmd-dump`'s `mssql_dump::model_export`.
pub mod model_export {
    pub use ibcmd_dump::mssql_dump::model_export::*;

    #[cfg(test)]
    pub(crate) use ibcmd_dump::mssql_dump::model_export::configuration_compatibility_from_storage;
    pub(crate) use ibcmd_dump::mssql_dump::model_export::configuration_compatibility_with_snapshot;
}

// What `root_tests` reads of the exporter; where it reads nothing they still
// hide the widened modules from the glob.
#[cfg_attr(
    not(all(
        test,
        any(feature = "research-corpus-tests", feature = "mssql-live-tests")
    )),
    expect(
        unused_imports,
        reason = "the re-export hides a widened module from the public glob"
    )
)]
pub(crate) use ibcmd_dump::mssql_dump::moxel_entry;
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "the re-export hides a widened module from the public glob"
    )
)]
pub(crate) use ibcmd_dump::mssql_dump::role_rights;
#[cfg(all(test, feature = "research-corpus-tests"))]
use moxel_entry::parse_moxel_spreadsheet_text;

// The spreadsheet probes of the root's live blob comparisons (`mssql`).
#[cfg(all(test, feature = "mssql-live-tests"))]
mod live_moxel_probes;
#[cfg(all(test, feature = "mssql-live-tests"))]
pub(crate) use live_moxel_probes::{
    debug_moxel_number_format_usage, debug_moxel_spreadsheet_summary_from_blob,
};

#[cfg(test)]
mod root_tests;
