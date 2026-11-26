//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::reader::{
    ClientSource, RowSource, database_name, other_sessions, read_inputs,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::reader::rows;
