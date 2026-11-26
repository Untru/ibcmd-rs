//! Where the time of a base-free stage goes.
//!
//! Set `IBCMD_RS_STAGE_TIMING=1` and `audit-empty-stage` (or
//! `mssql-stage-source-objects --base-free`) prints, after the stage, the
//! time of every (phase, kind) the stage spent: how many rows or objects,
//! their summed wall time on the worker threads, the CPU time those threads
//! actually used meanwhile (user and kernel), and the slowest one. Wall time
//! well above CPU time is time a worker spent waiting. The file operations
//! (opens, attribute queries, closes and reads, from the process's I/O
//! counters) are the process's, so they are only a phase's own with
//! `IBCMD_RS_WORKERS=1`.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static ENABLED: AtomicBool = AtomicBool::new(false);
static PHASES: Mutex<BTreeMap<(String, String), Phase>> = Mutex::new(BTreeMap::new());
/// The slowest single items of any phase: (wall, cpu, when it ended since the
/// first record, phase, source).
static SLOWEST: Mutex<Vec<(Duration, Duration, Duration, String, String)>> = Mutex::new(Vec::new());
static FIRST: Mutex<Option<Instant>> = Mutex::new(None);
const SLOWEST_KEPT: usize = 15;

#[derive(Default)]
struct Phase {
    count: u64,
    wall: Duration,
    cpu: Duration,
    kernel: Duration,
    file_ops: u64,
    max: Duration,
    max_source: String,
}

/// A started phase: its wall clock and the thread's CPU times.
pub(crate) struct Mark {
    wall: Instant,
    user: Duration,
    kernel: Duration,
    file_ops: u64,
}

/// Reads `IBCMD_RS_STAGE_TIMING` once per stage and clears what an earlier
/// stage of the same process gathered.
pub(crate) fn reset_from_env() {
    let enabled = std::env::var_os("IBCMD_RS_STAGE_TIMING").is_some_and(|value| value != "0");
    ENABLED.store(enabled, Ordering::Relaxed);
    if let Ok(mut phases) = PHASES.lock() {
        phases.clear();
    }
    if let Ok(mut slowest) = SLOWEST.lock() {
        slowest.clear();
    }
    if let Ok(mut first) = FIRST.lock() {
        *first = enabled.then(Instant::now);
    }
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// A started phase; `None` when timing is off.
pub(crate) fn start() -> Option<Mark> {
    if !enabled() {
        return None;
    }
    let (user, kernel) = thread_times();
    Some(Mark {
        wall: Instant::now(),
        user,
        kernel,
        file_ops: process_file_ops(),
    })
}

/// Adds the time since `started` to (phase, kind).
pub(crate) fn record(started: Option<Mark>, phase: &str, kind: &str, source: &str) {
    let Some(started) = started else {
        return;
    };
    let wall = started.wall.elapsed();
    let (user, kernel) = thread_times();
    let kernel = kernel.saturating_sub(started.kernel);
    let cpu = user.saturating_sub(started.user) + kernel;
    let file_ops = process_file_ops().saturating_sub(started.file_ops);
    let Ok(mut phases) = PHASES.lock() else {
        return;
    };
    let entry = phases
        .entry((phase.to_string(), kind.to_string()))
        .or_default();
    entry.count += 1;
    entry.wall += wall;
    entry.cpu += cpu;
    entry.kernel += kernel;
    entry.file_ops += file_ops;
    if wall > entry.max {
        entry.max = wall;
        entry.max_source = source.to_string();
    }
    drop(phases);
    let ended = FIRST
        .lock()
        .ok()
        .and_then(|first| first.map(|first| first.elapsed()))
        .unwrap_or_default();
    if let Ok(mut slowest) = SLOWEST.lock()
        && (slowest.len() < SLOWEST_KEPT || slowest.last().is_some_and(|last| wall > last.0))
    {
        slowest.push((
            wall,
            cpu,
            ended,
            format!("{phase} {kind}"),
            source.to_string(),
        ));
        slowest.sort_by(|left, right| right.0.cmp(&left.0));
        slowest.truncate(SLOWEST_KEPT);
    }
}

/// The table, heaviest first, with the parallel part's wall time.
pub(crate) fn report(wall: Duration, workers: usize) -> String {
    let Ok(phases) = PHASES.lock() else {
        return String::new();
    };
    let mut rows = phases.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.wall.cmp(&left.1.wall));
    let total = rows.iter().map(|(_, phase)| phase.wall).sum::<Duration>();
    let cpu = rows.iter().map(|(_, phase)| phase.cpu).sum::<Duration>();
    let mut lines = vec![format!(
        "stage timing: wall {:.1} s on {workers} workers; worker time {:.1} s ({:.1} busy workers), of it CPU {:.1} s",
        wall.as_secs_f64(),
        total.as_secs_f64(),
        total.as_secs_f64() / wall.as_secs_f64().max(1e-9),
        cpu.as_secs_f64(),
    )];
    lines.push(format!(
        "{:<28} {:<34} {:>7} {:>9} {:>6} {:>8} {:>8} {:>10} {:>8} {:>8}  slowest",
        "phase",
        "kind",
        "count",
        "worker s",
        "share",
        "cpu s",
        "kernel s",
        "file ops",
        "mean ms",
        "max ms"
    ));
    for ((phase, kind), item) in rows {
        lines.push(format!(
            "{:<28} {:<34} {:>7} {:>9.1} {:>5.1}% {:>8.1} {:>8.1} {:>10} {:>8.2} {:>8.0}  {}",
            phase,
            kind,
            item.count,
            item.wall.as_secs_f64(),
            100.0 * item.wall.as_secs_f64() / total.as_secs_f64().max(1e-9),
            item.cpu.as_secs_f64(),
            item.kernel.as_secs_f64(),
            item.file_ops,
            1000.0 * item.wall.as_secs_f64() / item.count.max(1) as f64,
            1000.0 * item.max.as_secs_f64(),
            item.max_source,
        ));
    }
    if let Ok(slowest) = SLOWEST.lock() {
        lines.push(
            "slowest items: wall s, cpu s, ended at s (since the stage began), phase, source"
                .to_string(),
        );
        for (wall, cpu, ended, phase, source) in slowest.iter() {
            lines.push(format!(
                "  {:>8.1} {:>8.1} {:>8.1}  {phase}  {source}",
                wall.as_secs_f64(),
                cpu.as_secs_f64(),
                ended.as_secs_f64()
            ));
        }
    }
    lines.join("\n")
}

/// The process's I/O operations so far: reads plus "other" operations (an
/// open, an attribute query and a close each count as one).
#[cfg(windows)]
fn process_file_ops() -> u64 {
    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operations: u64,
        write_operations: u64,
        other_operations: u64,
        read_transfer: u64,
        write_transfer: u64,
        other_transfer: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn GetProcessIoCounters(process: isize, counters: *mut IoCounters) -> i32;
    }
    let mut counters = IoCounters::default();
    // SAFETY: the pseudo-handle of the current process and an owned,
    // writable IO_COUNTERS structure.
    let ok = unsafe { GetProcessIoCounters(GetCurrentProcess(), &mut counters) };
    if ok == 0 {
        return 0;
    }
    counters.read_operations + counters.write_operations + counters.other_operations
}

#[cfg(not(windows))]
fn process_file_ops() -> u64 {
    0
}

/// The calling thread's user and kernel CPU time.
#[cfg(windows)]
fn thread_times() -> (Duration, Duration) {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn GetThreadTimes(
            thread: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    let mut creation = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    // SAFETY: the pseudo-handle of the current thread and four owned,
    // writable FILETIME structures.
    let ok = unsafe {
        GetThreadTimes(
            GetCurrentThread(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    if ok == 0 {
        return (Duration::ZERO, Duration::ZERO);
    }
    let ticks = |time: &FileTime| (u64::from(time.high) << 32) | u64::from(time.low);
    (
        Duration::from_nanos(ticks(&user) * 100),
        Duration::from_nanos(ticks(&kernel) * 100),
    )
}

#[cfg(not(windows))]
fn thread_times() -> (Duration, Duration) {
    (Duration::ZERO, Duration::ZERO)
}
