//! Does applying the staged configuration change the data structure?
//!
//! The check is the `ibcmd-apply-check` crate's `apply_check` module, and
//! this module keeps its paths: [`check_staged`], [`check_tree_against_db`]
//! and [`check_trees`], the [`Verdict`] they give, and the public modules
//! `cli`, `roles`, `rules` and `s1`.

pub use ibcmd_apply_check::apply_check::*;
