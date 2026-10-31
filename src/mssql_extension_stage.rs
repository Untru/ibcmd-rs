//! Transactional staging primitives for 8.3.27 MSSQL configuration
//! extensions: the `ibcmd-mssql-extensions` crate's `mssql_extension_stage`,
//! at its old path.
//!
//! The crate made two of its crate-visible functions `#[doc(hidden)] pub`
//! for the root; the names below re-export each of them crate-visibly,
//! which shadows the glob, so neither joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_mssql_extensions::mssql_extension_stage::*;

pub(crate) use ibcmd_mssql_extensions::mssql_extension_stage::{
    generate_container_configinfo_manifest, validate_container_configinfo_manifest,
};
