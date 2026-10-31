//! The active dynamic (online) configuration generation a storage table
//! holds: the `ibcmd-dynamic-generation` crate, at its old path. The registry
//! of installed overlays and the `StorageViewScope` over it have one owner,
//! the crate's; this module copies nothing.

pub use ibcmd_dynamic_generation::*;

/// The overlay the export installs, run against a real server: the rows the
/// derived table publishes and the headers the export lists must be what the
/// overlay describes, row by row (SHA-256 of the stored bytes).
///
/// Set `IBCMD_RS_DYNGEN_DB` to a lab database (it is only read). With
/// `IBCMD_RS_DYNGEN_MAIN=1` the run publishes the main configuration, as the
/// platform's export does, staged `ConfigSave` rows included:
///
/// ```text
/// IBCMD_RS_DYNGEN_DB=ibcmd_rs_04_rcheck_bsp_a IBCMD_RS_DYNGEN_MAIN=1 cargo test --locked \
///     -p ibcmd-rs --lib --no-default-features --features mssql-live-tests \
///     dynamic_generation::live -- --ignored --nocapture
/// ```
#[cfg(all(test, feature = "mssql-live-tests"))]
mod live;
