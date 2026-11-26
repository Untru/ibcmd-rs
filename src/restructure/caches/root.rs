//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::caches::root::{
    CATALOG_CLASS, Collection, DOCUMENT_CLASS, class_of, class_of_kind, collection_of, collections,
    flat_order, kind_of_class,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::caches::root::is_uuid;
