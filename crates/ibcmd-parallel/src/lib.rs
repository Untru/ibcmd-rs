//! The process-wide worker pools: the CPU pool, the narrower memory-bound
//! pool and the wider file-bound pool, sized from a command's own option,
//! `IBCMD_RS_WORKERS` and `IBCMD_RS_IO_WORKERS`. Each pool is built once, on
//! first use, and this crate is its only owner; the root's `parallel` module
//! re-exports these functions.

use anyhow::Result;
use rayon::ThreadPoolBuilder;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

// The default pool is at most 16 workers: the export's heavy rows are memory
// bound, and on a 24-thread workstation 24 workers made ERP УХ's export slower
// (every row cost 3-4x more). IBCMD_RS_WORKERS may ask for up to 64.
const DEFAULT_MAX_WORKERS: usize = 16;
const MAX_WORKERS: usize = 64;
const MAX_MEMORY_BOUND_WORKERS: usize = 4;
// Work that mostly waits for the file system gets a wider pool of its own.
// The first read of a file on the lab workstation passes through the
// on-access scanner and costs milliseconds of waiting, not of CPU: first-read
// throughput there is 245 files/s on 8 threads, 740 on 16, 1 333 on 32 and
// about 1 550 on 64 or 96, while a repeat read runs at 38 000 files/s. ERP
// УХ's base-free stage spent 10 586 worker-seconds of which 967 were CPU on
// 16 workers; on 48 the same stage took 402 s instead of 884 s.
// IBCMD_RS_IO_WORKERS may ask for up to 128.
const MIN_IO_BOUND_WORKERS: usize = 16;
const MAX_DEFAULT_IO_BOUND_WORKERS: usize = 64;
const MAX_IO_BOUND_WORKERS: usize = 128;
static THREAD_POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
static MEMORY_BOUND_THREAD_POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
static IO_BOUND_THREAD_POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
/// Workers a command asked for (`infobase config export --threads`), ahead
/// of `IBCMD_RS_WORKERS`; 0 when none was asked.
static REQUESTED_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Sizes the worker pool from a command's own option; takes effect only
/// before the pool's first use.
pub fn request_workers(count: usize) {
    REQUESTED_WORKERS.store(count, Ordering::Relaxed);
}

pub fn bounded_worker_count() -> usize {
    let requested = REQUESTED_WORKERS.load(Ordering::Relaxed);
    bounded_worker_count_from(
        (requested > 0).then_some(requested).or_else(|| {
            std::env::var("IBCMD_RS_WORKERS")
                .ok()
                .and_then(|value| value.trim().parse::<usize>().ok())
        }),
        std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
    )
}

pub fn install<F, R>(work: F) -> Result<R>
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    Ok(thread_pool()?.install(work))
}

/// Workers of the file-bound pool: `IBCMD_RS_IO_WORKERS`, else an explicit
/// `IBCMD_RS_WORKERS`, else twice the hardware threads within 16..=64.
pub fn io_bound_worker_count() -> usize {
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
    };
    io_bound_worker_count_from(
        read("IBCMD_RS_IO_WORKERS"),
        read("IBCMD_RS_WORKERS"),
        std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
    )
}

/// Runs work whose threads mostly wait for the file system -- one task per
/// source file, the file read first -- on the wider file-bound pool.
pub fn install_io_bound<F, R>(work: F) -> Result<R>
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    Ok(io_bound_thread_pool()?.install(work))
}

/// Runs expansion-heavy work in a narrower pool so multiple large decoded
/// payloads cannot multiply the retained-memory budget by the CPU pool width.
/// Pool creation remains an optimization rather than a correctness dependency.
pub fn install_memory_bound_or_inline<F, R>(work: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    match memory_bound_thread_pool() {
        Ok(pool) => pool.install(work),
        Err(_) => work(),
    }
}

fn thread_pool() -> Result<&'static rayon::ThreadPool> {
    THREAD_POOL
        .get_or_init(|| {
            ThreadPoolBuilder::new()
                .num_threads(bounded_worker_count())
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

fn memory_bound_thread_pool() -> Result<&'static rayon::ThreadPool> {
    MEMORY_BOUND_THREAD_POOL
        .get_or_init(|| {
            ThreadPoolBuilder::new()
                .num_threads(bounded_worker_count().min(MAX_MEMORY_BOUND_WORKERS))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

fn io_bound_thread_pool() -> Result<&'static rayon::ThreadPool> {
    IO_BOUND_THREAD_POOL
        .get_or_init(|| {
            ThreadPoolBuilder::new()
                .num_threads(io_bound_worker_count())
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

fn io_bound_worker_count_from(
    io_override: Option<usize>,
    workers_override: Option<usize>,
    available: usize,
) -> usize {
    match (io_override, workers_override) {
        (Some(requested), _) => requested.clamp(1, MAX_IO_BOUND_WORKERS),
        (None, Some(requested)) => requested.clamp(1, MAX_WORKERS),
        (None, None) => available
            .saturating_mul(2)
            .clamp(MIN_IO_BOUND_WORKERS, MAX_DEFAULT_IO_BOUND_WORKERS),
    }
}

fn bounded_worker_count_from(override_value: Option<usize>, available: usize) -> usize {
    match override_value {
        Some(requested) => requested.clamp(1, MAX_WORKERS),
        None => available.clamp(1, DEFAULT_MAX_WORKERS),
    }
}

#[cfg(test)]
mod tests {
    use super::{bounded_worker_count_from, io_bound_worker_count_from};

    #[test]
    fn io_bound_pool_is_twice_the_hardware_threads_within_bounds() {
        assert_eq!(io_bound_worker_count_from(None, None, 1), 16);
        assert_eq!(io_bound_worker_count_from(None, None, 24), 48);
        assert_eq!(io_bound_worker_count_from(None, None, 64), 64);
        assert_eq!(io_bound_worker_count_from(None, Some(4), 24), 4);
        assert_eq!(io_bound_worker_count_from(Some(96), Some(4), 24), 96);
        assert_eq!(io_bound_worker_count_from(Some(0), None, 24), 1);
        assert_eq!(io_bound_worker_count_from(Some(500), None, 24), 128);
    }

    #[test]
    fn clamps_worker_count_to_supported_bounds() {
        assert_eq!(bounded_worker_count_from(None, 1), 1);
        assert_eq!(bounded_worker_count_from(None, 4), 4);
        assert_eq!(bounded_worker_count_from(None, 16), 16);
        assert_eq!(bounded_worker_count_from(None, 24), 16);
        assert_eq!(bounded_worker_count_from(Some(24), 24), 24);
        assert_eq!(bounded_worker_count_from(Some(0), 16), 1);
        assert_eq!(bounded_worker_count_from(Some(2), 16), 2);
        assert_eq!(bounded_worker_count_from(Some(64), 16), 64);
        assert_eq!(bounded_worker_count_from(Some(500), 16), 64);
    }
}
