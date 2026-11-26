//! The own `config apply`: the `ibcmd-config-apply` crate's
//! `mssql_config_apply`, at its old path, with the root's developer command
//! (`run_command`, `crate::mssql_config_apply_command`).
//!
//! The crate made a few of its crate-visible items `#[doc(hidden)] pub` for
//! the root; the names below re-export each of them crate-visibly, which
//! shadows the glob, so none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_config_apply::mssql_config_apply::*;

pub use crate::mssql_config_apply_command::run_command;

pub(crate) use ibcmd_config_apply::mssql_config_apply::refusal_report;

/// The dynamic apply: the crate's `mssql_config_apply::dynamic`.
pub mod dynamic {
    pub use ibcmd_config_apply::mssql_config_apply::dynamic::*;

    pub(crate) use ibcmd_config_apply::mssql_config_apply::dynamic::SourceOwnerPreimages;
}
