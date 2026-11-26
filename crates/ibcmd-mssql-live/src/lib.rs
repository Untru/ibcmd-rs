//! MSSQL main activation and its live track, formerly root modules of the
//! same names: the native platform profile and its bounded RAC/RAS probes,
//! the activation plan and SQL, the live gate and continuation, the recovery
//! and live artifacts, the dedicated-worker switch, and the managed worker's
//! lifetime authority (the managed worker's command glue stays in the root).
//! The moved code keeps its `crate::…` paths; the root re-exports each module
//! at its old path.

pub mod mssql_artifact;
pub mod mssql_live_artifact;
pub mod mssql_live_continue;
pub mod mssql_live_gate;
pub mod mssql_main_activation;
// The process identities and the system directory of `child` and `native`
// are read through Windows FFI.
#[allow(unsafe_code)]
pub mod mssql_managed_worker;
pub mod mssql_platform_profile;
pub mod mssql_recovery_artifact;
pub mod mssql_worker_switch;
mod profile_process;
mod rac_process;

// The database clients, at the path the root names them.
use ibcmd_sql::sql;

// The scratch-path guard of the moved unit tests; a copy of `ibcmd-filedb`'s.
#[cfg(test)]
mod test_scratch;
