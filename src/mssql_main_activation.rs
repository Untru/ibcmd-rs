//! Fail-closed main-configuration publication plans for MSSQL 8.3.27: the
//! `ibcmd-mssql-live` crate's `mssql_main_activation`, at its old path.
//!
//! The crate made a few of its crate-visible items `#[doc(hidden)] pub` for
//! the root; the names below re-export each of them crate-visibly, which
//! shadows the glob, so none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_mssql_live::mssql_main_activation::*;

pub(crate) use ibcmd_mssql_live::mssql_main_activation::{
    ActivationInputs, allowed_activation_targets, exactly_one_optional_marker,
    read_activation_inputs,
};
// Read only by the exporter, which names the crate; it still hides the item from the glob.
#[expect(
    unused_imports,
    reason = "the re-export hides a widened item from the public glob"
)]
pub(crate) use ibcmd_mssql_live::mssql_main_activation::MainActivationRowSource;
// Read only by the exporter, which names the crate; it still hides the item from the glob.
#[expect(
    unused_imports,
    reason = "the re-export hides a widened item from the public glob"
)]
pub(crate) use ibcmd_mssql_live::mssql_main_activation::{MAX_PLAN_BYTES, MAX_ROW_BYTES, MAX_ROWS};
// Read only by a root test; outside tests it still hides the item from the glob.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use ibcmd_mssql_live::mssql_main_activation::quote_string;
