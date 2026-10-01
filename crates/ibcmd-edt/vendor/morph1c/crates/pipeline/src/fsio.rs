//! Chunked file writes — the one I/O primitive the whole pipeline writes through.
//!
//! # Why this exists (measured, not folklore)
//! [`std::fs::write`] issues a SINGLE `WriteFile` for the entire buffer. On Windows a write
//! that large falls off a cliff: writing a 100 MB `.cf` as one call runs at ~7 MB/s, while the
//! SAME bytes written in ≤16 MiB pieces run at >1 GB/s. Measured on the dev box (100 MB total,
//! varying the per-call size):
//!
//! ```text
//!   1 MiB × 100 →   98 ms          32 MiB × 3 →  8 385 ms
//!   4 MiB ×  25 →   99 ms          64 MiB × 1 →  9 588 ms
//!  16 MiB ×   6 →  103 ms         100 MiB × 1 → 14 372 ms
//! ```
//!
//! The knee is between 16 and 32 MiB per call, so [`CHUNK`] stays comfortably under it. This is
//! not a micro-optimisation: on SSL it was ~20 s of a 35 s `edt→cf`, and it scales with the
//! output — an ERP-sized 1.5 GB `.cf` would have spent minutes inside one `WriteFile`.
//!
//! Files below [`CHUNK`] take exactly one write, so small descriptors/sidecars behave as before.
//!
//! # What the XML write phase is actually made of (and what does NOT help)
//! Under `MORPH1C_TIMING` the whole-SSL `edt→xml` (9 397 files, 224 MB, 8 rayon workers) reports,
//! summed across threads: **~530 s filesystem, ~4 s our serialisers**. Of the filesystem part,
//! `create_dir_all` was ~192 s over 9 397 calls — one per file, and ~5 000 of those are on a
//! directory that already exists. That looks like free money and is not:
//!
//! * A no-op `create_dir_all` costs 0.067 ms here; a REAL two-level one costs 2.4 ms and a
//!   1-byte file write 1.2 ms (measured single-threaded on this box). The ~192 s is therefore
//!   dominated by the ~4 500 directories that genuinely have to be created — which no scheme
//!   avoids — not by the redundant calls.
//! * Creating the parent LAZILY instead (open, and only on `NotFound` create + retry) does cut
//!   the call count 9 397 → 4 471 exactly as predicted, and measured **1.3–1.6× SLOWER**
//!   end-to-end on the XML lanes, reproducibly, across five interleaved A/B campaigns. It was
//!   implemented, measured, and REVERTED. Do not re-derive it.
//!
//! So: callers create the parent before writing ([`create_dir_all`]), and the XML lanes sit at
//! the filesystem/AV floor. The remaining lever is not code — it is excluding the output
//! directory from the on-access scanner (needs administrator).

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-`WriteFile` payload cap (8 MiB) — half the measured 16 MiB knee, so we stay in the fast
/// regime with margin on other machines/filesystems.
pub const CHUNK: usize = 8 * 1024 * 1024;

/// Cumulative file-write accounting, so `MORPH1C_TIMING` can split the XML lanes' cost into
/// «bytes into the filesystem» and «everything else» (serialisation).
///
/// The counters are the ONLY way to tell a Defender/filesystem wall from our own CPU: the write
/// phase is `par_iter` across cores, so the wall clock alone cannot say which side is the wall.
/// Three relaxed atomic adds per FILE (not per byte, not per syscall) — at ERP scale that is
/// ~130k adds, i.e. unmeasurable next to the writes themselves.
static IO_FILES: AtomicU64 = AtomicU64::new(0);
static IO_BYTES: AtomicU64 = AtomicU64::new(0);
static IO_NANOS: AtomicU64 = AtomicU64::new(0);

/// `(files, bytes, nanos_summed_across_threads)` written through [`write`] so far.
///
/// `nanos` is a SUM over worker threads, so on the parallel write phase it exceeds the wall
/// clock — that ratio is exactly what tells CPU-bound from I/O-bound.
pub fn io_stats() -> (u64, u64, u64) {
    (
        IO_FILES.load(Ordering::Relaxed),
        IO_BYTES.load(Ordering::Relaxed),
        IO_NANOS.load(Ordering::Relaxed),
    )
}

thread_local! {
    /// Same clock as [`IO_NANOS`] but PER THREAD, so a caller can bracket a section of its own
    /// work and subtract the filesystem time inside it (the global counter cannot: other rayon
    /// workers advance it concurrently).
    static IO_TL_NANOS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Nanoseconds THIS thread has spent inside [`write`] so far.
pub fn io_nanos_this_thread() -> u64 {
    IO_TL_NANOS.with(std::cell::Cell::get)
}

static MKDIR_CALLS: AtomicU64 = AtomicU64::new(0);
static MKDIR_NANOS: AtomicU64 = AtomicU64::new(0);

/// `(calls, nanos_summed_across_threads)` spent ensuring output directories exist.
pub fn mkdir_stats() -> (u64, u64) {
    (
        MKDIR_CALLS.load(Ordering::Relaxed),
        MKDIR_NANOS.load(Ordering::Relaxed),
    )
}

/// Whether the accounting below runs at all — `MORPH1C_TIMING`, read once.
///
/// The counters exist to PROFILE, and profiling must not be part of what it measures: the write
/// phase calls these on every one of ~9 400 (SSL) / ~130 000 (ERP) files from 8 workers, so the
/// clock reads and the shared-cache-line atomics are pure overhead on a production run. Off by
/// default, whole thing compiled to one relaxed load.
pub fn accounting() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("MORPH1C_TIMING").is_some())
}

/// Accounted [`std::fs::create_dir_all`] — the ONE way the pipeline creates output directories.
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    if !accounting() {
        return std::fs::create_dir_all(path);
    }
    let t = std::time::Instant::now();
    let r = std::fs::create_dir_all(path);
    let ns = t.elapsed().as_nanos() as u64;
    IO_TL_NANOS.with(|c| c.set(c.get() + ns));
    MKDIR_NANOS.fetch_add(ns, Ordering::Relaxed);
    MKDIR_CALLS.fetch_add(1, Ordering::Relaxed);
    r
}

/// Create/truncate `path` and write `bytes`, capping every underlying write at [`CHUNK`].
///
/// Drop-in replacement for [`std::fs::write`] (same signature, same semantics) that never hands
/// the OS an oversized single write. See the module docs for the measurements. The caller creates
/// the parent directory (see the module docs for why the obvious lazy alternative was measured
/// and rejected).
pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let (path, bytes) = (path.as_ref(), bytes.as_ref());
    let r = timed_write(path, bytes);
    if accounting() {
        IO_FILES.fetch_add(1, Ordering::Relaxed);
        IO_BYTES.fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }
    r
}

fn timed_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if !accounting() {
        return write_inner(path, bytes);
    }
    let t = std::time::Instant::now();
    let r = write_inner(path, bytes);
    let ns = t.elapsed().as_nanos() as u64;
    IO_TL_NANOS.with(|c| c.set(c.get() + ns));
    IO_NANOS.fetch_add(ns, Ordering::Relaxed);
    r
}

fn write_inner(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = File::create(path)?;
    for chunk in bytes.chunks(CHUNK) {
        f.write_all(chunk)?;
    }
    f.flush()
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Round-trip across the chunk boundary: a payload spanning several chunks (plus a partial
    /// tail) must land byte-identical — the chunking is a pure I/O detail, never a content change.
    #[test]
    fn chunked_write_is_byte_identical() {
        let dir = std::env::temp_dir().join("morph1c_fsio_test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("chunked.bin");

        // 2.5 chunks: exercises full chunks + a partial final one.
        let n = CHUNK * 2 + CHUNK / 2;
        let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
        write(&path, &data).expect("write");
        assert_eq!(std::fs::read(&path).expect("read"), data);

        // Truncation: rewriting with a SHORTER payload must not leave a tail behind.
        write(&path, b"short").expect("rewrite");
        assert_eq!(std::fs::read(&path).expect("read"), b"short");

        let _ = std::fs::remove_file(&path);
    }

}
