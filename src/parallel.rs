//! The process-wide worker pools. Their one owner, with the pool and
//! requested-worker state, is the `ibcmd-parallel` crate; this module keeps
//! the root paths.

#[expect(
    unused_imports,
    reason = "the root keeps the path; its one reader, the exporter, names the crate"
)]
pub(crate) use ibcmd_parallel::install_memory_bound_or_inline;
pub use ibcmd_parallel::{bounded_worker_count, io_bound_worker_count, request_workers};
pub(crate) use ibcmd_parallel::{install, install_io_bound};
