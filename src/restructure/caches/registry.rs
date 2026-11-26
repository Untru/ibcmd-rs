//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::caches::registry::{
    REGISTRY_ROW, add_attributes, add_object, add_section, names, object_records, set_synonyms,
};

#[cfg(test)]
#[path = "../../../tests/support/restructure_registry.rs"]
#[expect(
    dead_code,
    reason = "Keep the original test helper path after moving its consumers"
)]
mod test_support;
#[cfg(test)]
#[expect(
    unused_imports,
    reason = "Retain the original crate-internal test helper path"
)]
pub(crate) use test_support::subtree_text;
