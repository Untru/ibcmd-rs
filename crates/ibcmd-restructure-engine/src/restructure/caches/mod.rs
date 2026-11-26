//! The derived caches of a new object (issue #403, part of S1 = #391): the `Params` `*.si` rows the
//! platform rewrites when a configuration change adds a catalog, a document, a tabular section or an
//! attribute. [`change::rewrite`] gives the final text of every rewritten row for a staged change
//! ([`plan::new_object`], [`plan::new_tabular_section`], the registry [`registry`], the attribute lines
//! of [`xdto_types::attribute_property`]).
//!
//! See `docs/apply/derived-caches.md` for the measurements behind every rule here; the lab kit of the
//! proofs is `scripts/apply-trace/lab/s1g-caches/`.

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
mod tests_cases;
#[cfg(test)]
mod tests_change;
#[cfg(test)]
pub(crate) mod tests_corpus;
#[cfg(test)]
mod tests_registry;
#[cfg(test)]
mod tests_unit;
#[cfg(test)]
mod tests_xdto_lines;
