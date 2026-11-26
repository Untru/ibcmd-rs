//! What the decoder took along from the root crate's `mssql_dump`.
//!
//! The two standard-picture identifiers moved here with the decoder, their
//! only reader. The helpers it shares with the root (`parse_uuid_field`,
//! `parse_non_zero_uuid`, `xml_bool`, `extract_base64_payload`) are the
//! `ibcmd-1c-text` crate's.
//!
//! The code came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file in this module's directory
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and its literals stay in the guard's baseline; a file added here is in the
//! slice at once. The rest of the crate was outside that slice before the
//! move and stays outside it.

// Platform-level 1C standard pictures, not metadata UUIDs from one database.
pub(crate) const STD_PICTURE_INFORMATION_UUID: &str = "4b54770b-d069-4c0e-9b17-5cc2a01134d9";
pub(crate) const STD_PICTURE_SAVE_FILE_UUID: &str = "818ab7d0-4654-4542-bd5e-fd9d1352b5a1";
