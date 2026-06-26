use anyhow::Result;
use rayon::ThreadPoolBuilder;

const MAX_WORKERS: usize = 8;

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
    let pool = ThreadPoolBuilder::new()
        .num_threads(bounded_worker_count())
        .build()?;
    Ok(pool.install(work))
}

fn bounded_worker_count_from(override_value: Option<usize>, available: usize) -> usize {
    let requested = override_value.unwrap_or(available);
    requested.clamp(1, MAX_WORKERS)
}

#[cfg(test)]
mod tests {
    use super::bounded_worker_count_from;

    #[test]
    fn clamps_worker_count_to_supported_bounds() {
        assert_eq!(bounded_worker_count_from(None, 1), 1);
        assert_eq!(bounded_worker_count_from(None, 4), 4);
        assert_eq!(bounded_worker_count_from(None, 16), 8);
        assert_eq!(bounded_worker_count_from(Some(0), 16), 1);
        assert_eq!(bounded_worker_count_from(Some(2), 16), 2);
        assert_eq!(bounded_worker_count_from(Some(64), 16), 8);
    }
}
