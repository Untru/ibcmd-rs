//! Vocabularies of the 1C native records that the root's readers share:
//! platform type identifiers and the XML names they are written as, the
//! stored type patterns (`{"Pattern",...}`) and the `<Type>` blocks written
//! from them, standard attributes, style codes, command shortcuts, pictures
//! and standard commands, the standard pictures, role rights and a few small
//! property values, with the payload helpers the form reader uses beside
//! them.
//!
//! Everything here moved from the root crate's `mssql_dump` (`mod.rs`, the
//! picture-payload helpers from `source_assets.rs`, the standard command
//! of a metadata kind, the standard command of a stored command code and
//! the stored command-interface codes (placements, home page work-area
//! templates, the platform's standard panel definitions) from
//! `command_interface.rs` and the role-right
//! vocabulary from `role_rights.rs`), and the standard
//! pictures and the compiler's built-in type table (`builtin_type_id`,
//! `builtin_type_qname`) from its `metadata_model`; `inflate_raw_deflate`
//! names in its context the raw inflater of `ibcmd-module-blob` (formerly
//! `module_blob`'s, then this crate's), and `module_blob`'s SVG check is
//! `is_svg_content`. The root still
//! reaches every item by its old name through imports. The bodies are unchanged but
//! for one argument: the two pattern readers take
//! `keeps_unresolved_type_id`, which answers whether a type id nothing
//! resolves is kept. The root answers it from its extension-export state,
//! which stays in the root.
//!
//! The code came from the physical-adapter guard's slice (the standard
//! pictures and the built-in type table joined it here), so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and their literals stay in the guard's baseline. A file added here is in
//! the slice at once and fails the guard until its record is reviewed.

mod builtin_type_ids;
mod command_interface;
mod commands;
mod patterns;
mod pictures;
mod role_rights;
mod standard_attributes;
mod style;
mod support;
mod type_xml;
mod types;
mod values;

pub use builtin_type_ids::{FIXED_ARRAY_TYPE, builtin_type_id, builtin_type_qname};
pub use command_interface::{
    CLIENT_APPLICATION_STANDARD_PANEL_DEFS, COMMAND_INTERFACE_PLACEMENTS,
    HOME_PAGE_WORK_AREA_TEMPLATES, ONE_COLUMN_TEMPLATE, client_application_panel_def_is_standard,
};
pub use commands::{
    ShortcutModifierStyle, command_interface_standard_command,
    command_interface_standard_command_for_code, parse_common_command_picture_value,
    parse_common_command_shortcut_value, parse_common_command_shortcut_value_with_style,
};
pub use patterns::{
    metadata_type_pattern_field_type_id, parse_form_metadata_type_pattern,
    parse_metadata_type_pattern_element_with_builtin,
};
pub use pictures::{standard_picture_name, standard_picture_uuid};
pub use role_rights::{
    ROLE_RIGHTS_EXT_DIMENSION_FAMILY, ROLE_RIGHTS_EXT_DIMENSION_TYPE_FAMILY,
    ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY, ROLE_RIGHTS_STANDARD_TABULAR_SECTION_FAMILY,
    role_right_name, role_right_uuid, role_standard_attribute_descriptor,
    role_standard_attribute_slot, role_standard_tabular_section,
    role_standard_tabular_section_attribute, role_standard_tabular_section_attribute_slot,
    role_standard_tabular_section_slot,
};
pub use standard_attributes::{
    ACCUMULATION_REGISTER_BALANCE_STANDARD_ATTRIBUTES,
    ACCUMULATION_REGISTER_TURNOVERS_STANDARD_ATTRIBUTES,
    CHART_OF_ACCOUNTS_STANDARD_ATTRIBUTE_DEFINITIONS,
    CHART_OF_ACCOUNTS_STANDARD_TABULAR_SECTION_DEFINITIONS, ChartStandardTabularSectionDefinition,
    DOCUMENT_STANDARD_ATTRIBUTES, accumulation_register_record_set_standard_attribute_name,
    chart_of_accounts_sole_standard_tabular_section_name,
    chart_of_accounts_standard_attribute_definitions, chart_of_accounts_standard_tabular_section,
};
pub use style::{
    standard_style_item_code_for_name, standard_style_item_for_code, style_system_color_name,
    style_web_color_name,
};
pub use support::{elapsed_ms, ext_picture_file_name, inflate_raw_deflate, is_svg_content};
pub use type_xml::{
    DATA_COMPOSITION_SETTINGS_NAMESPACE_ATTRIBUTE, form_metadata_type_xml_tag,
    format_form_metadata_types_xml, format_form_metadata_types_xml_with_indent,
    format_type_description_value_types_xml, metadata_type_xml_name,
    metadata_type_xml_namespace_attr, number_allowed_sign_xml, push_binary_data_qualifiers_xml,
    scoped_value_type, string_allowed_length_xml,
};
pub use types::{
    CONSTANTS_SET_TYPE_UUID, ConstantValueType, DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME,
    DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_UUID, DCS_BUILTIN_REFERENCE_TYPE_SETS,
    GANTT_CHART_TYPE_REFERENCE, GANTT_CHART_TYPE_UUID, TYPE_DESCRIPTION_TYPE_UUID,
    builtin_type_reference, data_processor_builtin_type_reference, metadata_reference_is_type_set,
    platform_reference_family_type_reference, unique_metadata_type_reference,
};
pub use values::{
    information_register_decimal_is_valid, metadata_choice_folders_and_items_xml,
    parse_design_time_reference,
};

mod command_groups;
pub use command_groups::COMMON_COMMAND_GROUPS;
