//! The platform an infobase command runs for, formerly the root's modules of
//! the same names:
//!
//! - `platform`: the `--platform` spec, `platform::PlatformSpec`, which
//!   wraps the portable registry of `ibcmd-platform` with what the commands
//!   add to it (the XML selector and the MSSQL capabilities of an exact
//!   build), and the process-wide export platform the form writer reads
//!   (`note_export_platform`, `export_writes_default_button_importance`),
//!   whose one owner this crate is;
//! - `legacy_version`: the historical XML selector of the command line,
//!   `InfobaseConfigSourceVersion`, and the independent version axes it
//!   converts to;
//! - `adapters::mssql_legacy`: the descriptor of the legacy MSSQL store
//!   (`Config`/`ConfigSave`) those axes configure.
//!
//! The moved code keeps its `crate::…` paths; the root re-exports each module
//! at its old path.
