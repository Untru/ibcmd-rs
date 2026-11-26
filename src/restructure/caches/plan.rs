//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::caches::plan::{
    CacheRow, NewObject, NewSection, NewSections, catalog_shape, document_shape, new_object,
    new_tabular_section, new_tabular_sections, rows, section_traversal,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::caches::plan::reference_type_slots;
