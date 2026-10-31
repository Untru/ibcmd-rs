//! The platform registry: the `ibcmd-platform-spec` crate's `platform`, at
//! its old path. The process-wide export platform
//! (`note_export_platform`, `export_writes_default_button_importance`)
//! keeps its one owner there.
//!
//! The crate made two of its crate-visible functions `#[doc(hidden)] pub`
//! for the root; the names below re-export each of them crate-visibly,
//! which shadows the glob, so neither joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_platform_spec::platform::*;

pub(crate) use ibcmd_platform_spec::platform::{clear_export_platform, of_xml_dialect};
