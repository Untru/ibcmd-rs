//! The base-free metadata descriptor model, formerly the root's
//! `metadata_model` module. The crate mirrors the root's `metadata_model::`
//! tree, so the moved code keeps its `crate::metadata_model::…` paths; the
//! root's `metadata_model` module re-exports it.

pub mod metadata_model;

// The scratch-path guard of the moved unit tests; a copy of `ibcmd-filedb`'s.
#[cfg(test)]
mod test_scratch;
