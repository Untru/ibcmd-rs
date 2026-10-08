//! One original RAC process and both original readers share a single deadline.
//! An unproved lifetime is retained for this process's lifetime, and prevents
//! another readiness attempt. No PID lookup, signal, retry or blocking join.

use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, TryLockError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};

const OUTPUT_BOUND: usize = 1024 * 1024;
static UNPROVED: Mutex<Option<Attempt>> = Mutex::new(None);

struct Reader {
    // The original reader remains reachable even if thread creation fails or
    // the drain is still blocked. Never lock this mutex from the waiting side.
    _original: Arc<Mutex<Box<dyn Read + Send>>>,
    _thread: Option<JoinHandle<()>>,
    receive: Receiver<Result<Vec<u8>>>,
    completed: Option<Vec<u8>>,
}

impl Reader {
    fn new(stream: impl Read + Send + 'static) -> Self {
        let original: Arc<Mutex<Box<dyn Read + Send>>> = Arc::new(Mutex::new(Box::new(stream)));
        let source = Arc::clone(&original);
        let (send, receive) = mpsc::channel();
        let error_send = send.clone();
        let thread = std::thread::Builder::new().spawn(move || {
            let result = (|| -> Result<Vec<u8>> {
                let mut stream = source.lock().unwrap_or_else(|e| e.into_inner());
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer)?;
                    if n == 0 {
                        return Ok(bytes);
                    }
                    ensure!(bytes.len() + n <= OUTPUT_BOUND, "rac output exceeded bound");
                    bytes.extend_from_slice(&buffer[..n]);
                }
            })();
            let _ = send.send(result);
        });
        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(error) => {
                let _ = error_send.send(Err(error).context("rac reader could not start"));
                None
            }
        };
        Self {
            _original: original,
            _thread: thread,
            receive,
            completed: None,
        }
    }

    fn complete(&mut self, deadline: Instant, label: &str) -> Result<()> {
        ensure!(Instant::now() < deadline, "rac {label} deadline");
        self.completed = Some(
            self.receive
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .with_context(|| format!("rac {label} completion unproved"))??,
        );
        ensure!(Instant::now() < deadline, "rac {label} deadline");
        Ok(())
    }
}

struct Attempt {
    child: Child,
    stdout: Option<Reader>,
    stderr: Option<Reader>,
    direct_exit: Option<ExitStatus>,
    deadline: Instant,
}

impl Attempt {
    fn complete(&mut self) -> Result<()> {
        let deadline = self.deadline;
        loop {
            ensure!(Instant::now() < deadline, "rac direct-process deadline");
            if let Some(status) = self.child.try_wait()? {
                self.direct_exit = Some(status);
                break;
            }
            std::thread::sleep(
                Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        self.stdout
            .as_mut()
            .context("rac original stdout missing")?
            .complete(deadline, "stdout")?;
        self.stderr
            .as_mut()
            .context("rac original stderr missing")?
            .complete(deadline, "stderr")?;
        Ok(())
    }
}

/// A successful return proves direct exit and EOF of BOTH original streams.
/// The output bound belongs to the small RAC control response, not metadata.
pub(crate) fn bounded_bytes(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    // Serialize readiness attempts so a concurrent attempt cannot escape the
    // sticky unknown outcome of an earlier original process.
    let mut unproved = match UNPROVED.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::Poisoned(error)) => error.into_inner(),
        Err(TryLockError::WouldBlock) => {
            bail!(
                "RAS session readiness already has an active original process; cycle 2 was not started"
            );
        }
    };
    ensure!(
        unproved.is_none(),
        "RAS session readiness has a retained unproved original process; cycle 2 was not started"
    );
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("rac deadline is not representable")?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("rac launch failed; arguments redacted")?;
    let stdout = child.stdout.take().map(Reader::new);
    let stderr = child.stderr.take().map(Reader::new);
    let mut attempt = Attempt {
        child,
        stdout,
        stderr,
        direct_exit: None,
        deadline,
    };
    if let Err(error) = attempt.complete() {
        // Keep the exact Child, both original readers, thread handles,
        // receivers and any already captured bytes. Even later EOF does not
        // retroactively authorize continuation or release this unknown.
        *unproved = Some(attempt);
        bail!(
            "RAS session readiness timed out or completion is unproved; cycle 2 was not started: {error:#}"
        );
    }
    let status = attempt.direct_exit.context("rac direct exit absent")?;
    let stdout = attempt
        .stdout
        .as_mut()
        .and_then(|reader| reader.completed.take())
        .context("rac stdout result absent")?;
    let stderr = attempt
        .stderr
        .as_mut()
        .and_then(|reader| reader.completed.take())
        .context("rac stderr result absent")?;
    ensure!(
        status.success() && stderr.is_empty(),
        "RAS session readiness failed: {}",
        String::from_utf8_lossy(&stderr)
    );
    Ok(stdout)
}
