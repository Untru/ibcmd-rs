//! The own configuration apply over SQL Server, formerly the root's
//! `mssql_config_apply` module, with the S1 structural gate
//! (`restructure::s1`) and its size guard (`restructure::size_guard::guard_phase`).
//! The crate mirrors the root's `mssql_config_apply::` and `restructure::`
//! trees, so the moved code keeps its `crate::…` and `super::…` paths; the
//! root re-exports each at its old path. The developer command,
//! `run_command`, stays in the root.

pub mod mssql_config_apply;
pub mod restructure;

// The crates behind the root facades the moved code names by their root
// paths (`crate::sql::…`, `crate::metadata_model::…`, `crate::apply_check::…`,
// `crate::cancel::…`, `crate::mssql_main_activation::…`,
// `crate::mssql_platform_profile::…`, `crate::v8_container::…`). Each root
// facade re-exports these, so the paths name the same items here as they did
// in the root.
use ibcmd_apply_check::apply_check;
use ibcmd_cancel as cancel;
use ibcmd_metadata_model::metadata_model;
use ibcmd_module_blob::v8_container;
use ibcmd_mssql_live::{mssql_main_activation, mssql_platform_profile};
use ibcmd_sql::sql;

// `brace_list!`, which the root exports at its crate root.
pub use ibcmd_metadata_model::brace_list;

// The scratch-path guard of the moved unit tests; a copy of `ibcmd-filedb`'s.
#[cfg(test)]
mod test_scratch;
