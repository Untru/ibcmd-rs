//! The codec of the bodies a 1C configuration stores: raw deflate and the
//! uppercase hex SHA-256 the reports print, and a raw-deflated body packed or
//! digested (`raw.rs`); the format-15 V8 container read and written through
//! `ibcmd-v8` with `anyhow` errors ([`v8_container`]); the module blob
//! (`module.rs`); the versions blob's generation UUIDs patched
//! (`versions.rs`); and the `deleted` row of the platform's import
//! (`deleted.rs`).
//!
//! Everything here moved from the root crate (`module_blob.rs`,
//! `v8_container.rs` and, for `deleted_row`, `mssql/override_stage.rs`), and
//! `inflate_raw` from `ibcmd-1c-vocab`, which now
//! imports it from here. The root still reaches every item by its old path
//! through re-exports (`module_blob::…`, `crate::v8_container::…`). The bodies
//! are unchanged but for visibility (what the root reads is `pub`) and one
//! name: the lenient base64 decoder the root re-exports as
//! `decode_base64_mime` is called `decode_base64_mime_lenient` here, its own
//! name in `ibcmd-1c-text`. The tests moved with them; their fixture paths
//! start from this crate and one clippy finding is fixed. The
//! command-line wrappers that read and write files and their reports
//! (`pack_module_blob`, `patch_versions_blob`), and the Help-content digest,
//! which reads a braced record through `ibcmd_form_body::record_text`, stay in
//! the root.
//!
//! The code came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and their literals stay in the guard's baseline. A file added here is in
//! the slice at once and fails the guard until its record is reviewed.

mod deleted;
mod module;
mod raw;
/// The format-15 V8 container through `ibcmd-v8`'s reader and writer, its
/// errors as `anyhow` errors.
pub mod v8_container;
mod versions;

pub use deleted::deleted_row;
pub use module::{
    PackedModuleBlob, module_blob_text_sha256, pack_module_blob_bytes,
    pack_module_blob_bytes_base_free, pack_module_blob_container_bytes, unpack_module_blob_text,
    unpack_module_container_text, unpack_plain_text_module_body,
};
pub use raw::{
    PackedRawDeflatedBlob, deflate_raw, hex_sha256, inflate_raw, pack_raw_deflated_blob_from_bytes,
    raw_deflated_first_base64_payload_sha256, raw_deflated_plain_sha256,
};
pub use versions::{
    PatchedVersionsBlob, VersionReplacement, patch_versions_blob_bytes,
    patch_versions_blob_bytes_allowing_additions, versions_base_free_blockers,
};
