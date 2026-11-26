//! The restructuring check of a configuration apply, formerly the root's
//! `apply_check` module. The crate mirrors the root's `apply_check::` tree,
//! so the moved code keeps its `crate::apply_check::…` and `super::…` paths;
//! the root's `apply_check` module re-exports it.

pub mod apply_check;

// The crates behind the root facades the moved code names by their root
// paths (`crate::metadata_model::…`, `crate::sql::…`, `crate::parallel::…`).
// Each root facade re-exports exactly these, so the paths name the same
// items here as they did in the root.
use ibcmd_metadata_model::metadata_model;
use ibcmd_parallel as parallel;
use ibcmd_sql::sql;

// The scratch-path guard of the moved unit tests; a copy of `ibcmd-filedb`'s.
#[cfg(test)]
mod test_scratch;
