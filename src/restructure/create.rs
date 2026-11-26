//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::create::{
    Context, Created, check_count, context, context_uuids, extend_caches, find_created,
    in_platform_order, new_descriptor_names, without_created,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::create::{
    allocate, plan_created, walk_positions,
};
