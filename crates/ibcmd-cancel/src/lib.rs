//! Cooperative cancellation of a long operation, and the lock under which
//! one such operation runs at a time.
//!
//! A caller that can be asked to stop (the editor server, `serve --stdio`,
//! on `$/cancelRequest`) installs a flag for the duration of one operation
//! with [`scope`]; the long loops of the engine call [`check`] at their
//! natural steps (a batch of rows read, a row converted, a file handed to
//! the writers) and fail with [`Cancelled`] once the flag is set. Nothing
//! else changes: without a scope -- every command-line run -- [`check`] is
//! one uncontended read and never fails.
//!
//! The flag is process-wide, not per thread: the export converts rows on a
//! pool of worker threads, and every one of them must see it. One operation
//! runs at a time anyway: the offline rows and the storage views of the
//! export are process-wide too, so the operations of the root's
//! `stored_objects` hold [`engine`] and whatever else reads or writes through
//! that state runs through [`exclusive`]. The lock moved here from
//! `stored_objects`, which re-exports `exclusive` at its old path, so that
//! the export below it takes the same one.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

static CURRENT: RwLock<Option<Arc<AtomicBool>>> = RwLock::new(None);

static ENGINE: Mutex<()> = Mutex::new(());

/// Holds the engine's lock until the guard drops; a panic that poisoned it
/// does not keep the next operation out.
pub fn engine() -> MutexGuard<'static, ()> {
    ENGINE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `operation` while no other operation holds [`engine`]: what else reads
/// or writes through the export's process-wide state (a stage, whose guard
/// exports in memory; a check of ConfigSave) takes its turn with them.
pub fn exclusive<T>(operation: impl FnOnce() -> T) -> T {
    let _engine = engine();
    operation()
}

/// The error an operation stopped by its caller fails with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the operation was cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Keeps a flag installed; the one it replaced comes back on drop.
pub struct CancelScope {
    previous: Option<Arc<AtomicBool>>,
}

impl Drop for CancelScope {
    fn drop(&mut self) {
        let mut current = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
        *current = self.previous.take();
    }
}

/// Makes `flag` the one [`check`] reads until the scope drops.
pub fn scope(flag: Arc<AtomicBool>) -> CancelScope {
    let mut current = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    CancelScope {
        previous: current.replace(flag),
    }
}

/// Fails with [`Cancelled`] when the installed flag is set.
pub fn check() -> Result<(), Cancelled> {
    let current = CURRENT.read().unwrap_or_else(PoisonError::into_inner);
    match current.as_ref() {
        Some(flag) if flag.load(Ordering::Relaxed) => Err(Cancelled),
        _ => Ok(()),
    }
}

/// Whether `error` (or anything it was caused by) is a cancellation.
pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.is::<Cancelled>())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The flag itself is process-wide and the library tests run in
    // parallel, so a set flag would fail other tests' exports: only the
    // error side is tested here (`tests/editor_server.rs` cancels through
    // the server).
    #[test]
    fn a_cancellation_is_found_under_context() {
        let error = anyhow::Error::from(Cancelled).context("export failed");
        assert!(is_cancelled(&error));
        assert!(!is_cancelled(&anyhow::anyhow!("another failure")));
    }
}
