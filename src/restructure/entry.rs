//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::entry::{EntryInput, Kind, RefTables, main_entry};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::entry::{attribute_types, subtable};
