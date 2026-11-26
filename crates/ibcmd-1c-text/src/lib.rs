//! Primitives of the text 1C stores its records in: braced values
//! (`{field,field,...}`), the string literals inside them, a few scalar
//! fields, and the XML escaping the writers apply to what they read.
//!
//! Everything here moved unchanged from the root crate's `mssql_dump`
//! (`mod.rs`; `extract_base64_payload`, `decode_base64_mime`,
//! `format_1c_date_time` and the 2.21 palette namespace rule
//! `declare_palette_namespace_beside_style` from `source_assets.rs`; the UUID scan
//! `uuid_like_values` from `refs.rs`), which still reaches every item by its
//! old name through imports. The writers `encode_base64`,
//! `platform_base64_token`, `format_1c_string` and `crlf_line_breaks` are the
//! one copy of what `module_blob`, `mssql_dump::form::load_8_5_1`, the root's
//! asset, dynamic-list and editor-protocol writers and `ibcmd-form-native`
//! each spelled out, and `decode_base64_mime_lenient` is the one copy of the
//! base64 reader `module_blob` and `ibcmd-form-body`'s record reader each
//! held, which reads what the stricter `decode_base64_mime` refuses. The
//! root's
//! form-body, reference and metadata readers register their texts in this
//! crate's per-thread jump table ([`register_brace_jumps`]); the splitters
//! here read the same table, so it has one definition, in this crate.
//!
//! [`brace_list`], the top-level fields of a brace list, moved unchanged from
//! the root's external-object adapter (`external/brace.rs`), which was outside
//! the guard's slice; the adapter re-exports it, and the adopted-form compile
//! reads it.
//!
//! The code came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and their literals stay in the guard's baseline. A file added here is in
//! the slice at once and fails the guard until its record is reviewed.

pub mod brace_list;
mod braced;
mod fields;
mod namespaces;
mod paths;
mod strings;
mod xml;

pub use braced::{
    BRACE_JUMPS_MIN_TEXT, BraceJumpsGuard, expect_comma_at, for_each_1c_braced_field,
    register_brace_jumps, registered_header_marker, scan_1c_braced_value, skip_ascii_ws_at,
    split_1c_braced_fields, split_1c_braced_fields_bounded, split_1c_braced_sequence,
};
pub use fields::{
    base64_value, decode_base64_mime, decode_base64_mime_lenient, encode_base64,
    extract_base64_payload, format_1c_date_time, is_uuid_like_ascii, is_uuid_text,
    parse_1c_bool_field, parse_1c_bool_field_or_default, parse_1c_bool_flag, parse_1c_u32_field,
    parse_non_zero_uuid, parse_uuid_field, platform_base64_token, uuid_like_values,
    uuid_like_values_in_text_order,
};
pub use namespaces::declare_palette_namespace_beside_style;
pub use paths::sanitize_source_path_segment;
pub use strings::{
    crlf_line_breaks, format_1c_string, parse_1c_quoted_string, parse_1c_quoted_string_with_len,
    parse_1c_string, parse_1c_synonyms,
};
pub use xml::{escape_xml_character_data, escape_xml_element_text, escape_xml_text, xml_bool};
