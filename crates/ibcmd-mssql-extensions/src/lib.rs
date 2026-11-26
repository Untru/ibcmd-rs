//! Configuration extensions in an MSSQL infobase.
//!
//! `mssql_extension_stage`, formerly the root's module of the same name,
//! holds the transactional staging primitives of an 8.3.27 extension: its
//! rows and their `configinfo` manifest, and the SQL that stages them into
//! `ConfigCASSave` (through the built-in client or `sqlcmd`). It stops there:
//! the activation, the export, the load and the restructuring of an
//! extension (the root's other `mssql_extension_*` modules) still reach the
//! command line or the exporter and stay in the root. The moved code keeps
//! its `crate::…` paths; the root re-exports the module at its old path.

pub mod mssql_extension_stage;

// The crate behind the root facade the moved code names by its root path
// (`crate::sql::…`); the root re-exports it, so the path names the same
// items here as it did in the root.
use ibcmd_sql::sql;
