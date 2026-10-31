//! The configuration exporter, formerly the root's `src/mssql_dump/`: a
//! configuration's stored rows, read from SQL Server, PostgreSQL, a file
//! infobase or saved rows, written as the native XML source tree, with its
//! incremental state, ConfigDumpInfo, the extension export and the export's
//! audits and reports. The root re-exports the module at its old path.
//! The moved code keeps its `crate::…` paths: the module keeps its name, and
//! the root modules it read through single-crate facades are aliased below.

// The atomic file replace, the UI language and the process CPU time are read
// through Windows FFI.
#[allow(unsafe_code)]
pub mod mssql_dump;

// The modules the exporter names at the root's paths, each one crate's.
use ibcmd_cancel as cancel;
use ibcmd_dump_readers::metadata_owner_graph;
use ibcmd_form_schema as form_schema;
use ibcmd_module_blob::v8_container;
use ibcmd_mssql_extensions::mssql_extension_stage;
use ibcmd_platform_spec::legacy_version;
use ibcmd_sql::{runtime_evidence_schema, sql};

// The scratch-path guard of the moved unit tests; a copy of the root's.
#[cfg(test)]
mod test_scratch;
