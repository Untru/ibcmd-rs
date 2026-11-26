//! Original child handles for the managed creator. An observation failure
//! never kills a guessed process or turns a failed command into success.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::ProcessIdentity;

const OUTPUT_BOUND: usize = 1024 * 1024;

/// The handle and pipe receivers remain owned on every failure. Dropping this
/// value sends no signal; the caller must retain its unproved lifetime journal.
pub(crate) struct OriginalChild {
    terminal_exit: Option<i32>,
    child: Child,
    executable: PathBuf,
    birth_100ns: u64,
    pipes: OriginalPipes,
    identity: Option<ProcessIdentity>,
    outcome_unproved: bool,
    terminal_proved: bool,
    direct_exit_proved: bool,
    command_deadline: Option<Instant>,
}

struct OriginalReader {
    // The waiting owner never locks the stream or joins its reader.
    _original: Option<Arc<Mutex<Box<dyn Read + Send>>>>,
    _thread: Option<JoinHandle<()>>,
    receive: Receiver<Result<Vec<u8>>>,
    obtained: Option<Result<Vec<u8>>>,
}

impl OriginalReader {
    fn new(reader: impl Read + Send + 'static) -> Self {
        Self::start_with(reader, |task| std::thread::Builder::new().spawn(task))
    }

    fn start_with(
        reader: impl Read + Send + 'static,
        start: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> Self {
        let original: Arc<Mutex<Box<dyn Read + Send>>> = Arc::new(Mutex::new(Box::new(reader)));
        let source = Arc::clone(&original);
        let (send, receive) = mpsc::channel();
        let error_send = send.clone();
        let thread = start(Box::new(move || {
            let result = (|| -> Result<Vec<u8>> {
                let mut stream = source.lock().unwrap_or_else(|error| error.into_inner());
                let mut bytes = Vec::new();
                stream
                    .by_ref()
                    .take((OUTPUT_BOUND + 1) as u64)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > OUTPUT_BOUND {
                    bail!("managed child pipe exceeds bound");
                }
                Ok(bytes)
            })();
            let _ = send.send(result);
        }));
        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(error) => {
                let _ = error_send.send(Err(error).context("managed original reader start failed"));
                None
            }
        };
        Self {
            _original: Some(original),
            _thread: thread,
            receive,
            obtained: None,
        }
    }

    fn startup_proved(&self) -> bool {
        self._original.is_some() && self._thread.is_some()
    }

    fn missing() -> Self {
        let (send, receive) = mpsc::channel();
        let _ = send.send(Err(anyhow::anyhow!("original managed pipe handle missing")));
        Self {
            _original: None,
            _thread: None,
            receive,
            obtained: None,
        }
    }

    fn complete_at(&mut self, deadline: Instant, label: &str) -> Result<&[u8]> {
        require_deadline(deadline)?;
        if self.obtained.is_none() {
            self.obtained = Some(
                self.receive
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .with_context(|| format!("original {label} unproved"))?,
            );
        }
        // Even queued EOF cannot turn an expired attempt into known completion.
        require_deadline(deadline)?;
        match self
            .obtained
            .as_ref()
            .context("original reader result absent")?
        {
            Ok(bytes) => Ok(bytes),
            Err(_) => bail!("original managed {label} capture unproved; output redacted"),
        }
    }
}

struct OriginalPipes {
    stdout: OriginalReader,
    stderr: OriginalReader,
}

impl OriginalPipes {
    fn finish_at(&mut self, deadline: Instant) -> Result<(Vec<u8>, Vec<u8>)> {
        self.stdout.complete_at(deadline, "stdout")?;
        self.stderr.complete_at(deadline, "stderr")?;
        require_deadline(deadline)?;
        // Retain the original obtained results through the caller policy gate.
        Ok((
            self.stdout.complete_at(deadline, "stdout")?.to_vec(),
            self.stderr.complete_at(deadline, "stderr")?.to_vec(),
        ))
    }
}

pub(crate) fn require_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        bail!("managed child deadline; no signal sent");
    }
    Ok(())
}

fn await_direct_exit(
    mut poll: impl FnMut() -> Result<Option<std::process::ExitStatus>>,
    deadline: Instant,
) -> Result<std::process::ExitStatus> {
    loop {
        require_deadline(deadline)?;
        if let Some(status) = poll()? {
            require_deadline(deadline)?;
            return Ok(status);
        }
        require_deadline(deadline)?;
        std::thread::sleep(
            Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(windows)]
fn original_birth(child: &Child) -> Result<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    let mut creation: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
    let mut user: FILETIME = unsafe { std::mem::zeroed() };
    // The std Child's retained handle, never an OpenProcess on a recycled PID.
    if unsafe {
        GetProcessTimes(
            child.as_raw_handle() as _,
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .context("original managed child creation time");
    }
    Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

#[cfg(not(windows))]
fn original_birth(_child: &Child) -> Result<u64> {
    bail!("managed worker creator requires Windows original-handle identity");
}

impl OriginalChild {
    pub(crate) fn spawn(executable: &Path, arguments: &[String]) -> Result<Self> {
        Self::spawn_original(executable, arguments, None)
    }

    pub(crate) fn spawn_at(
        executable: &Path,
        arguments: &[String],
        deadline: Instant,
    ) -> Result<Self> {
        require_deadline(deadline)?;
        Self::spawn_original(executable, arguments, Some(deadline))
    }

    fn spawn_original(
        executable: &Path,
        arguments: &[String],
        deadline: Option<Instant>,
    ) -> Result<Self> {
        Self::spawn_original_with_readers(executable, arguments, deadline, OriginalReader::new)
    }

    fn spawn_original_with_readers(
        executable: &Path,
        arguments: &[String],
        deadline: Option<Instant>,
        mut reader: impl FnMut(Box<dyn Read + Send>) -> OriginalReader,
    ) -> Result<Self> {
        if !cfg!(windows) || !executable.is_absolute() {
            bail!("managed creator requires a fully qualified Windows executable");
        }
        if let Some(deadline) = deadline {
            require_deadline(deadline)?;
        }
        let mut child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("managed original child launch failed; arguments redacted")?;
        // Establish handle ownership and both readers before closing input.
        // After Start, capture failures must still return original custody.
        // An Err here would discard the only handle to a potentially live child.
        let out = child.stdout.take();
        let err = child.stderr.take();
        let stdout = out.map_or_else(OriginalReader::missing, |out| reader(Box::new(out)));
        let stderr = err.map_or_else(OriginalReader::missing, |err| reader(Box::new(err)));
        // Both retained reader threads are required before this child can bind
        // a long-lived census anchor, even when both pipe handles exist.
        let readers_unproved = !stdout.startup_proved() || !stderr.startup_proved();
        let birth = original_birth(&child);
        drop(child.stdin.take()); // Valid pipe EOF, rather than inherited invalid stdin.
        let mut owned = Self {
            terminal_exit: None,
            child,
            executable: executable.to_owned(),
            birth_100ns: 0,
            pipes: OriginalPipes { stdout, stderr },
            identity: None,
            outcome_unproved: readers_unproved,
            terminal_proved: false,
            direct_exit_proved: false,
            command_deadline: deadline,
        };
        match birth {
            Ok(value) => owned.birth_100ns = value,
            Err(_) => {
                owned.outcome_unproved = true;
            }
        }
        if deadline.is_some_and(|deadline| require_deadline(deadline).is_err()) {
            owned.outcome_unproved = true;
        }
        Ok(owned)
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn alive(&mut self) -> Result<bool> {
        let alive = self.child.try_wait()?.is_none();
        if !alive {
            self.direct_exit_proved = true;
        }
        Ok(alive)
    }

    /// Full CIM identity augments the original handle. CIM's microsecond birth
    /// interval must contain the handle time; printed seven digits are not
    /// proof of 100ns precision.
    pub(crate) fn bind_census(&mut self, row: &CensusIdentity) -> Result<ProcessIdentity> {
        let result = (|| -> Result<ProcessIdentity> {
            if self.birth_100ns == 0 || self.outcome_unproved {
                bail!("original spawned handle identity unproved; retain child");
            }
            let end = row
                .birth_filetime
                .checked_add(9)
                .context("CIM birth overflow")?;
            if row.pid != self.pid()
                || row.parent != std::process::id()
                || row.birth_filetime > self.birth_100ns
                || self.birth_100ns > end
                || !same_executable(&row.executable, &self.executable)
                || row.command.is_empty()
            {
                bail!("original managed child census identity unproved");
            }
            let identity = ProcessIdentity {
                pid: row.pid,
                parent: row.parent,
                birth_100ns: self.birth_100ns,
                executable: self.executable.clone(),
                command_sha256: format!("{:X}", Sha256::digest(row.command.as_bytes())),
            };
            if self.identity.as_ref().is_some_and(|old| old != &identity) {
                bail!("original managed child command identity drift");
            }
            self.identity = Some(identity.clone());
            Ok(identity)
        })();
        if result.is_err() {
            self.outcome_unproved = true;
        }
        result
    }

    pub(crate) fn completed(&mut self, timeout: Duration) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("child deadline overflow")?;
        self.completed_at(deadline)
    }

    pub(crate) fn completed_at(&mut self, deadline: Instant) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let result = (|| -> Result<_> {
            require_deadline(deadline)?;
            if self
                .command_deadline
                .is_some_and(|original| original != deadline)
            {
                bail!("managed original command deadline changed");
            }
            if self.outcome_unproved {
                bail!("managed child outcome already unproved");
            }
            let exit = await_direct_exit(|| Ok(self.child.try_wait()?), deadline)?;
            self.direct_exit_proved = true;
            let (stdout, stderr) = self.pipes.finish_at(deadline)?;
            require_deadline(deadline)?;
            Ok((
                exit.code().context("managed child exit code unavailable")?,
                stdout,
                stderr,
            ))
        })();
        if result.is_err() {
            self.outcome_unproved = true;
        } else {
            self.terminal_proved = true;
            self.terminal_exit = result.as_ref().ok().map(|output| output.0);
        }
        result
    }

    pub(crate) fn mark_unproved(&mut self) {
        self.outcome_unproved = true;
    }

    pub(crate) fn terminal_proved(&self) -> bool {
        self.terminal_proved && !self.outcome_unproved
    }

    /// Already captured original bytes; no new wait, pipe read or deadline.
    pub(crate) fn terminal_raw(&self) -> Result<(i32, &[u8], &[u8])> {
        if !self.terminal_proved() {
            bail!("original terminal snapshot unproved");
        }
        let stdout = self
            .pipes
            .stdout
            .obtained
            .as_ref()
            .context("original stdout absent")?
            .as_ref()
            .map_err(|_| anyhow::anyhow!("original stdout fault retained"))?;
        let stderr = self
            .pipes
            .stderr
            .obtained
            .as_ref()
            .context("original stderr absent")?
            .as_ref()
            .map_err(|_| anyhow::anyhow!("original stderr fault retained"))?;
        Ok((
            self.terminal_exit.context("original typed exit absent")?,
            stdout,
            stderr,
        ))
    }

    /// A census utility observes itself while running, but its row is bound
    /// only after the SAME retained command has proved direct exit and BOTH.
    /// No PID reopen, synthetic row or replacement process is accepted.
    pub(crate) fn bind_completed_census(
        &mut self,
        row: &CensusIdentity,
        original_stdout: &[u8],
    ) -> Result<ProcessIdentity> {
        let result = (|| {
            self.require_command_current()?;
            if !self.direct_exit_proved() {
                bail!("original census direct exit unproved");
            }
            let (exit, stdout, stderr) = self.terminal_raw()?;
            if exit != 0 || !stderr.is_empty() || stdout != original_stdout {
                bail!("original census command exit/BOTH/raw unproved");
            }
            let identity = self.bind_census(row)?;
            self.require_command_current()?;
            Ok(identity)
        })();
        if result.is_err() {
            self.outcome_unproved = true;
        }
        result
    }

    /// Use the immutable deadline selected before this original command's
    /// spawn. In particular, recovery must not use the spent startup phase.
    pub(crate) fn require_command_current(&mut self) -> Result<()> {
        let result = self
            .command_deadline
            .context("original finite command deadline absent")
            .and_then(require_deadline);
        if result.is_err() {
            self.outcome_unproved = true;
        }
        result
    }

    /// Current original kernel exit permits subsequent descendant cleanup;
    /// it does not establish EOF or undo authority.
    pub(crate) fn direct_exit_proved(&self) -> bool {
        self.direct_exit_proved && !self.outcome_unproved
    }

    /// Only the original std Child handle receives the direct signal. The
    /// caller must retain every descendant before stopping a spawning anchor.
    pub(crate) fn stop_exact(
        &mut self,
        row: &CensusIdentity,
        expected: &ProcessIdentity,
        timeout: Duration,
    ) -> Result<()> {
        if timeout.is_zero() || timeout > Duration::from_secs(5) {
            bail!("bounded direct original stop deadline required");
        }
        if &self.bind_census(row)? != expected {
            bail!("original shutdown anchor identity drift; no signal");
        }
        if !self.alive()? {
            bail!("shutdown anchor exited before owned stop intent");
        }
        self.child.kill().context("direct original anchor stop")?;
        // Descendants can inherit these pipes. Retain both readers and prove
        // only original direct exit now; drain EOF after every descendant.
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("direct stop deadline overflow")?;
        if let Err(error) = await_direct_exit(|| Ok(self.child.try_wait()?), deadline) {
            self.outcome_unproved = true;
            return Err(error);
        }
        self.direct_exit_proved = true;
        Ok(())
    }
}

fn same_executable(left: &Path, right: &Path) -> bool {
    left.is_absolute()
        && right.is_absolute()
        && left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
}

/// Query-only handle acquired for a post-barrier descendant. It intentionally
/// has no PROCESS_TERMINATE right; a turn-off is addressed to the independently
/// bound RAC process UUID, never a Windows PID signal.
pub(crate) struct KernelProcess {
    #[cfg(windows)]
    handle: std::os::windows::io::OwnedHandle,
    pub identity: ProcessIdentity,
    terminate_authorized: bool,
}

impl KernelProcess {
    #[cfg(windows)]
    pub(crate) fn bind(row: &CensusIdentity) -> Result<Self> {
        Self::bind_with_rights(row, false)
    }

    #[cfg(windows)]
    pub(crate) fn bind_for_shutdown(row: &CensusIdentity) -> Result<Self> {
        Self::bind_with_rights(row, true)
    }

    #[cfg(windows)]
    fn bind_with_rights(row: &CensusIdentity, terminate_authorized: bool) -> Result<Self> {
        use std::os::windows::io::{FromRawHandle, OwnedHandle};
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::{
            GetProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        };
        // SYNCHRONIZE permits exit observation, but no termination.
        let rights = PROCESS_QUERY_LIMITED_INFORMATION
            | 0x00100000
            | if terminate_authorized { 1 } else { 0 }; // PROCESS_TERMINATE
        let raw = unsafe { OpenProcess(rights, 0, row.pid) };
        if raw.is_null() {
            return Err(std::io::Error::last_os_error())
                .context("bind managed descendant original handle");
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw as _) };
        let mut birth: FILETIME = unsafe { std::mem::zeroed() };
        let mut exit: FILETIME = unsafe { std::mem::zeroed() };
        let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
        let mut user: FILETIME = unsafe { std::mem::zeroed() };
        let mut path = vec![0u16; 32768];
        let mut count = path.len() as u32;
        if unsafe { GetProcessId(raw) } != row.pid
            || unsafe { GetProcessTimes(raw, &mut birth, &mut exit, &mut kernel, &mut user) } == 0
            || unsafe { QueryFullProcessImageNameW(raw, 0, path.as_mut_ptr(), &mut count) } == 0
        {
            bail!("managed descendant handle identity unavailable");
        }
        let birth = (u64::from(birth.dwHighDateTime) << 32) | u64::from(birth.dwLowDateTime);
        let executable = PathBuf::from(String::from_utf16(&path[..count as usize])?);
        let end = row
            .birth_filetime
            .checked_add(9)
            .context("descendant CIM interval overflow")?;
        if birth < row.birth_filetime
            || birth > end
            || row.parent == 0
            || row.command.is_empty()
            || !same_executable(&executable, &row.executable)
        {
            bail!("managed descendant PID reused or complete identity drift");
        }
        Ok(Self {
            handle,
            identity: ProcessIdentity {
                pid: row.pid,
                parent: row.parent,
                birth_100ns: birth,
                executable,
                command_sha256: format!("{:X}", Sha256::digest(row.command.as_bytes())),
            },
            terminate_authorized,
        })
    }

    #[cfg(not(windows))]
    pub(crate) fn bind(_row: &CensusIdentity) -> Result<Self> {
        bail!("managed descendant handle binding requires Windows");
    }

    #[cfg(not(windows))]
    pub(crate) fn bind_for_shutdown(_row: &CensusIdentity) -> Result<Self> {
        bail!("managed shutdown handle binding requires Windows");
    }

    pub(crate) fn require_exited(&self) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::WaitForSingleObject;
            if unsafe { WaitForSingleObject(self.handle.as_raw_handle() as _, 0) } == 0 {
                return Ok(());
            }
        }
        bail!("original shutdown descendant exit unproved");
    }

    pub(crate) fn stop_exact(&self, row: &CensusIdentity, timeout: Duration) -> Result<()> {
        if !self.terminate_authorized {
            bail!("query-only working handle cannot authorize shutdown");
        }
        if timeout.is_zero() || timeout > Duration::from_secs(5) {
            bail!("bounded descendant stop deadline required");
        }
        self.require_current(row)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};
            let handle = self.handle.as_raw_handle() as _;
            if unsafe { TerminateProcess(handle, 1) } == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("direct owned descendant stop");
            }
            let millis = u32::try_from(timeout.as_millis())?;
            if unsafe { WaitForSingleObject(handle, millis) } == 0 {
                return Ok(());
            }
        }
        bail!("direct owned descendant stop completion unproved");
    }

    pub(crate) fn require_current(&self, row: &CensusIdentity) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::WaitForSingleObject;
            if unsafe { WaitForSingleObject(self.handle.as_raw_handle() as _, 0) } != 258 {
                bail!("original managed descendant exited or handle wait unproved");
            }
        }
        let end = row
            .birth_filetime
            .checked_add(9)
            .context("current CIM interval overflow")?;
        if row.pid != self.identity.pid
            || row.parent != self.identity.parent
            || self.identity.birth_100ns < row.birth_filetime
            || self.identity.birth_100ns > end
            || !same_executable(&row.executable, &self.identity.executable)
            || format!("{:X}", Sha256::digest(row.command.as_bytes()))
                != self.identity.command_sha256
        {
            bail!("original descendant identity changed; no process signal");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CensusIdentity {
    pub pid: u32,
    pub parent: u32,
    pub birth_filetime: u64,
    pub executable: PathBuf,
    pub command: String,
}

#[cfg(test)]
mod shutdown_pipe_tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn original_child_survives_both_reader_creation_failures() {
        // The actual test binary runs an empty libtest selection, never 1C/native.
        let executable = std::env::current_exe().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut original = OriginalChild::spawn_original_with_readers(
            &executable,
            &[
                "--exact".into(),
                "managed_deadline_no_such_test".into(),
                "--quiet".into(),
            ],
            Some(deadline),
            |stream| {
                OriginalReader::start_with(stream, |_| {
                    Err(std::io::Error::other("reader allocation refused"))
                })
            },
        )
        .unwrap();
        assert!(original.pid() > 0);
        assert!(original.pipes.stdout._original.is_some());
        assert!(original.pipes.stderr._original.is_some());
        assert!(original.pipes.stdout._thread.is_none());
        assert!(original.pipes.stderr._thread.is_none());
        assert!(original.outcome_unproved);
        assert!(original.completed_at(deadline).is_err());
        assert!(!original.terminal_proved());
        // Immediate refusal leaves both original queued errors untouched. Cache
        // those exact results only for inspecting their retained error custody.
        for reader in [&mut original.pipes.stdout, &mut original.pipes.stderr] {
            assert!(reader.obtained.is_none());
            let result = reader.receive.try_recv().unwrap();
            assert!(
                format!("{:#}", result.as_ref().unwrap_err()).contains("reader allocation refused")
            );
            reader.obtained = Some(result);
            assert!(reader.obtained.as_ref().unwrap().is_err());
        }
        assert!(original.outcome_unproved);
        assert!(original.identity.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn census_anchor_requires_both_original_readers_to_start() {
        let executable = std::env::current_exe().unwrap();
        let arguments = [
            "--exact".into(),
            "managed_deadline_no_such_test".into(),
            "--quiet".into(),
        ];
        for failed in [[false, false], [true, false], [false, true], [true, true]] {
            let mut index = 0;
            // This is the actual long-lived spawn path: no command deadline.
            // Only thread startup is substituted; the original Child and pipes
            // are the actual finite, empty-selection test process.
            let mut original = OriginalChild::spawn_original_with_readers(
                &executable,
                &arguments,
                None,
                |stream| {
                    let refuse = failed[index];
                    index += 1;
                    if refuse {
                        OriginalReader::start_with(stream, |_| {
                            Err(std::io::Error::other("reader allocation refused"))
                        })
                    } else {
                        OriginalReader::new(stream)
                    }
                },
            )
            .unwrap();
            assert_eq!(index, 2);
            assert!(original.birth_100ns > 0);
            assert!(original.command_deadline.is_none());
            let pid = original.pid();
            let streams = [
                Arc::clone(original.pipes.stdout._original.as_ref().unwrap()),
                Arc::clone(original.pipes.stderr._original.as_ref().unwrap()),
            ];
            let threads = [
                original
                    .pipes
                    .stdout
                    ._thread
                    .as_ref()
                    .map(|thread| thread.thread().id()),
                original
                    .pipes
                    .stderr
                    ._thread
                    .as_ref()
                    .map(|thread| thread.thread().id()),
            ];
            // The seam supplies a census row from the actual retained spawn
            // identity; it is not native/CIM acceptance evidence.
            let row = CensusIdentity {
                pid,
                parent: std::process::id(),
                birth_filetime: original.birth_100ns / 10 * 10,
                executable: executable.clone(),
                command: format!(
                    "{} --exact managed_deadline_no_such_test --quiet",
                    executable.display()
                ),
            };
            let unproved = failed.into_iter().any(|failed| failed);
            assert_eq!(original.outcome_unproved, unproved);
            let bound = original.bind_census(&row);
            if unproved {
                assert_eq!(
                    bound.unwrap_err().to_string(),
                    "original spawned handle identity unproved; retain child"
                );
                assert!(original.identity.is_none());
            } else {
                assert_eq!(bound.unwrap().pid, pid);
                assert!(original.identity.is_some());
            }
            assert_eq!(original.pid(), pid);
            assert_eq!(original.outcome_unproved, unproved);
            assert!(!original.terminal_proved());
            for (index, reader) in [&mut original.pipes.stdout, &mut original.pipes.stderr]
                .into_iter()
                .enumerate()
            {
                assert!(Arc::ptr_eq(
                    reader._original.as_ref().unwrap(),
                    &streams[index]
                ));
                assert_eq!(
                    reader._thread.as_ref().map(|thread| thread.thread().id()),
                    threads[index]
                );
                assert_eq!(reader.startup_proved(), !failed[index]);
                assert!(reader.obtained.is_none());
                if failed[index] {
                    let result = reader.receive.try_recv().unwrap();
                    assert!(
                        format!("{:#}", result.as_ref().unwrap_err())
                            .contains("reader allocation refused")
                    );
                    reader.obtained = Some(result);
                    assert!(reader.obtained.as_ref().unwrap().is_err());
                }
            }
            assert_eq!(original.outcome_unproved, unproved);
            if unproved {
                assert!(original.bind_census(&row).is_err());
                assert!(original.identity.is_none());
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn post_start_reader_setup_cannot_reset_original_command_budget() {
        let executable = std::env::current_exe().unwrap();
        let deadline = Instant::now() + Duration::from_millis(10);
        let mut original = OriginalChild::spawn_original_with_readers(
            &executable,
            &[
                "--exact".into(),
                "managed_deadline_no_such_test".into(),
                "--quiet".into(),
            ],
            Some(deadline),
            |stream| {
                std::thread::sleep(Duration::from_millis(20));
                OriginalReader::new(stream)
            },
        )
        .unwrap();
        assert!(original.pipes.stdout._original.is_some());
        assert!(original.pipes.stderr._original.is_some());
        assert!(original.completed_at(deadline).is_err());
        assert!(
            original
                .completed_at(Instant::now() + Duration::from_secs(5))
                .is_err()
        );
        assert!(!original.terminal_proved());
    }

    #[test]
    fn queued_original_reader_does_not_rescue_expired_deadline() {
        let (send, receive) = mpsc::channel();
        send.send(Ok(b"queued EOF".to_vec())).unwrap();
        let mut reader = OriginalReader {
            _original: None,
            _thread: None,
            receive,
            obtained: None,
        };
        assert!(reader.complete_at(Instant::now(), "stdout").is_err());
        assert!(reader.obtained.is_none());
        assert_eq!(reader.receive.try_recv().unwrap().unwrap(), b"queued EOF");
    }

    #[test]
    fn fallible_reader_start_keeps_the_original_stream_and_error() {
        let mut reader = OriginalReader::start_with(std::io::Cursor::new(b"original"), |_| {
            Err(std::io::Error::other("reader allocation refused"))
        });
        assert!(reader._original.is_some());
        assert!(reader._thread.is_none());
        assert!(
            reader
                .complete_at(Instant::now() + Duration::from_secs(1), "stderr")
                .is_err()
        );
        assert!(reader.obtained.as_ref().unwrap().is_err());
        assert_eq!(Arc::strong_count(reader._original.as_ref().unwrap()), 1);
    }

    #[test]
    fn late_direct_status_and_poll_are_not_terminal_authority() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        let mut polled = false;
        assert!(
            await_direct_exit(
                || {
                    polled = true;
                    Ok(Some(std::process::ExitStatus::from_raw(0)))
                },
                Instant::now()
            )
            .is_err()
        );
        assert!(!polled);
        let deadline = Instant::now() + Duration::from_millis(10);
        assert!(
            await_direct_exit(
                || {
                    std::thread::sleep(Duration::from_millis(20));
                    Ok(Some(std::process::ExitStatus::from_raw(0)))
                },
                deadline
            )
            .is_err()
        );
    }

    #[test]
    fn each_original_stream_checks_same_deadline_before_queued_eof() {
        for late_stream in ["stdout", "stderr"] {
            let queued = || {
                let (send, receive) = mpsc::channel();
                send.send(Ok(Vec::new())).unwrap();
                OriginalReader {
                    _original: None,
                    _thread: None,
                    receive,
                    obtained: None,
                }
            };
            let mut pipes = OriginalPipes {
                stdout: queued(),
                stderr: queued(),
            };
            if late_stream == "stderr" {
                pipes
                    .stdout
                    .complete_at(Instant::now() + Duration::from_secs(1), "stdout")
                    .unwrap();
            }
            assert!(pipes.finish_at(Instant::now()).is_err());
            assert!(pipes.stderr.obtained.is_none());
        }
    }

    #[test]
    fn direct_exit_precedes_descendant_owned_pipe_completion() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        let (out_send, stdout) = mpsc::channel();
        let (err_send, stderr) = mpsc::channel();
        let reader = |receive| OriginalReader {
            _original: None,
            _thread: None,
            receive,
            obtained: None,
        };
        let mut pipes = OriginalPipes {
            stdout: reader(stdout),
            stderr: reader(stderr),
        };
        // Direct exit remains independent of inherited pipe EOF, but must be timely.
        let status = await_direct_exit(
            || Ok(Some(std::process::ExitStatus::from_raw(1))),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(!status.success());
        out_send.send(Ok(b"anchor output".to_vec())).unwrap();
        assert!(
            pipes
                .finish_at(Instant::now() + Duration::from_millis(20))
                .is_err()
        );
        assert_eq!(
            pipes.stdout.obtained.as_ref().unwrap().as_ref().unwrap(),
            b"anchor output"
        );
        err_send.send(Ok(b"descendant EOF".to_vec())).unwrap();
        // This is the shutdown owner retrying its distinct pipe phase, not a utility command.
        let (out, err) = pipes
            .finish_at(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert_eq!(out, b"anchor output");
        assert_eq!(err, b"descendant EOF");
        assert!(pipes.finish_at(Instant::now()).is_err());
        assert!(await_direct_exit(|| Ok(None), Instant::now()).is_err());
        assert!(
            await_direct_exit(|| Err(anyhow::anyhow!("unproved handle")), Instant::now()).is_err()
        );
    }
}
