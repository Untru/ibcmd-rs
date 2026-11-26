//! Own restructuring of the infobase on a configuration change (issues #341 and #391, track "ddl").
//!
//! When `config apply` changes a table's structure the platform rebuilds the table through a "new
//! generation" (`NG`) copy and records its progress in `SchemaStorage`. This module does that itself for
//! the cases of the minimal set (S1) it has reached: **new attributes of catalogs and documents**:
//!
//! - [`names`] -- `Params.DBNames`, the numbering of tables and fields, and `DBNamesVersion`;
//! - [`schema`] -- `DBSchema` (and `NewGenCreated`, the same grammar): a byte-exact model of the text and
//!   the SQL it stands for (columns, indexes, `create table`);
//! - [`storage`] -- the `SchemaStorage` states and the empty-generation marker;
//! - [`catalog`], [`object`] -- the descriptor row of a catalog or a document as the table structure needs
//!   it, and the mapping of an attribute's type to the field's type entries;
//! - [`xdto`], [`registry`] -- the derived caches (`Params` `*.si`) that a new attribute makes stale: the
//!   XDTO model and the object registry;
//! - [`plan`] -- the checks (fail closed) and the plan: new schema, new names, caches, the statements;
//! - [`reader`] -- the database side of the plan's input;
//! - [`exec`] -- the plan run in **one transaction**, verified before it commits;
//! - [`extensions`] -- the extensions of the infobase (S1-I): the objects they adopt, which refuse a
//!   restructure, and the proof that a restructure leaves their state as it was;
//! - [`script`] -- the same plan as T-SQL text with assertions, to run inside another transaction (the
//!   own apply's);
//! - the root `s1` adapter -- the structural gate of the own apply: which reasons of the restructuring check are S1
//!   operations, and the structure phase the apply runs in its transaction.
//!
//! The brace text is `metadata_model::brace`, shared with the model export. Findings and measurements:
//! `docs/apply/restructuring.md`.

pub mod caches;
pub mod catalog;
pub mod common;
pub mod create;
pub mod entry;
pub mod exec;
pub mod extensions;
pub mod names;
pub mod object;
pub mod plan;
pub mod reader;
pub mod registry;
pub mod schema;
pub mod script;
pub mod size_guard;
pub mod storage;
pub mod xdto;

#[cfg(test)]
mod tests_corpus;
#[cfg(test)]
mod tests_create;
#[cfg(test)]
mod tests_entry;
#[cfg(test)]
mod tests_plan;
#[cfg(test)]
mod tests_schema;
#[cfg(test)]
mod tests_sections;

#[cfg(test)]
#[path = "../../../../tests/support/restructure_corpus.rs"]
mod test_corpus_support;
#[cfg(test)]
#[path = "../../../../tests/support/restructure_create.rs"]
mod test_create_support;
#[cfg(test)]
#[path = "../../../../tests/support/restructure_plan.rs"]
mod test_plan_support;
#[cfg(test)]
#[path = "../../../../tests/support/restructure_sections.rs"]
mod test_sections_support;
