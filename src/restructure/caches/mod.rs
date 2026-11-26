//! Compatibility exports for the native derived caches.

pub mod change;
pub mod facts;
pub mod help_props;
pub mod members;
pub mod names_tables;
pub mod order;
pub mod owner_map;
pub mod plan;
pub mod registry;
pub mod root;
pub mod slots;
pub mod synonyms;
pub mod trace_diagnostic;
pub mod type_index;
pub mod type_sets;
pub mod xdto_types;

#[cfg(test)]
pub(crate) mod tests_corpus;
