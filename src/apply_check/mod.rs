//! Does applying the staged configuration change the data structure?
//!
//! After `ibcmd infobase config import` the new configuration sits in the
//! ConfigSave table. Applying it changes rows of Config; whether it also has
//! to change tables, columns, indexes or data the platform derives from the
//! configuration (predefined items, route points, the content of an
//! exchange plan) only the platform's own `config apply` can do until the
//! restructuring is implemented. This module tells the two apart, and fails
//! closed: a change the rules do not list as harmless is a restructuring,
//! and so is a row the check cannot read.
//!
//! * [`check_staged`]: the ConfigSave of a database against its active
//!   Config (read-only). This is the check an apply runs.
//! * [`check_tree_against_db`]: a source tree against the active Config of
//!   a database: which descriptors differ and whether each difference is a
//!   restructuring. It sees what a stage may have dropped, so it is the check
//!   an import runs on its input.
//! * [`check_trees`]: two XML trees, an export and the tree that would
//!   replace it.
//!
//! All three give a [`Verdict`]: `needs_restructuring`, the reasons (the
//! object, the Config row, the property, the change), the harmless changes
//! as notes, and `objects`, every object whose descriptor differs with the
//! most serious class among its changes. The rules are in `rules`, the row
//! roles in `roles`, the design and the evidence in
//! `docs/apply/restructuring-check.md`.
//!
//! The whitelist of harmless properties is the first cut. A stronger test,
//! the table entries the platform derives from the metadata (`DBSchema`),
//! plugs into the same place: `Verdict::objects` names the table-owning
//! objects that changed, and a reason of class `structure` or `unknown` on
//! such an object is what the stronger test may confirm or withdraw. The
//! generator of those entries is not part of this module.

mod check;
pub mod cli;
mod dbtree;
mod descriptor;
mod model;
mod plan;
pub mod roles;
mod root_row;
mod rule_id;
pub mod rules;
pub mod s1;
mod sql;
mod tree_diff;
mod trees;
mod upgrade;

#[cfg(test)]
mod corpus_tests;
#[cfg(test)]
mod record_format_tests;
#[cfg(test)]
mod s1_matrix_tests;
#[cfg(test)]
mod s1h_corpus_tests;

pub use check::{Inputs, RowProvider, check};
pub use dbtree::check_tree_against_db;
pub(crate) use dbtree::check_tree_against_db_with_source;
pub use model::{Note, ObjectChange, ObjectOp, Reason, ReasonClass, Stats, Verdict};
pub use rule_id::RuleId;
pub use sql::check_staged;
pub use tree_diff::{ChangeOp, Seg};
pub use trees::check_trees;
