use anyhow::Result;
use rayon::ThreadPoolBuilder;
use std::sync::OnceLock;

// The default pool is at most 16 workers: the export's heavy rows are memory
// bound, and on a 24-thread workstation 24 workers made ERP УХ's export slower
// (every row cost 3-4x more). IBCMD_RS_WORKERS may ask for up to 64.
const DEFAULT_MAX_WORKERS: usize = 16;
const MAX_WORKERS: usize = 64;
const MAX_MEMORY_BOUND_WORKERS: usize = 4;
static THREAD_POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
static MEMORY_BOUND_THREAD_POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();

pub fn bounded_worker_count() -> usize {
    bounded_worker_count_from(
        std::env::var("IBCMD_RS_WORKERS")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok()),
        std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
    )
}

pub(crate) fn install<F, R>(work: F) -> Result<R>
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    Ok(thread_pool()?.install(work))
}

/// Runs expansion-heavy work in a narrower pool so multiple large decoded
/// payloads cannot multiply the retained-memory budget by the CPU pool width.
/// Pool creation remains an optimization rather than a correctness dependency.
pub(crate) fn install_memory_bound_or_inline<F, R>(work: F) -> R
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

fn bounded_worker_count_from(override_value: Option<usize>, available: usize) -> usize {
    match override_value {
        Some(requested) => requested.clamp(1, MAX_WORKERS),
        None => available.clamp(1, DEFAULT_MAX_WORKERS),
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_worker_count_from;

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
