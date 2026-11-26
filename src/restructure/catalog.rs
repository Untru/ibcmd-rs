//! Compatibility exports of the native restructuring engine.

pub use ibcmd_restructure_engine::restructure::catalog::{
    ATTRIBUTES, AttributeFacts, CatalogFacts, DOCUMENT_ATTRIBUTES, DOCUMENT_TABULAR_SECTIONS,
    SectionFacts, TABULAR_SECTIONS, type_entries,
};

#[expect(
    unused_imports,
    reason = "Retain the original crate-internal compatibility paths"
)]
pub(crate) use ibcmd_restructure_engine::restructure::catalog::{
    OwnerRecord, attribute_body, md_base, md_synonyms, number_at, record_positions, section,
};
