//! Original child handles for the managed creator. An observation failure
//! never kills a guessed process or turns a failed command into success.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::ProcessIdentity;

const OUTPUT_BOUND: usize = 1024 * 1024;

/// The handle and pipe receivers remain owned on every failure. Dropping this
/// value sends no signal; the caller must retain its unproved lifetime journal.
pub(crate) struct OriginalChild {
    child: Child,
    executable: PathBuf,
    birth_100ns: u64,
    pipes: OriginalPipes,
    identity: Option<ProcessIdentity>,
    outcome_unproved: bool,
    terminal_proved: bool,
    direct_exit_proved: bool,
}

struct OriginalPipes {
    stdout: Receiver<Result<Vec<u8>>>,
    stderr: Receiver<Result<Vec<u8>>>,
    retained_stdout: Option<Vec<u8>>,
    retained_stderr: Option<Vec<u8>>,
}

impl OriginalPipes {
    fn finish(&mut self, deadline: Instant) -> Result<(Vec<u8>, Vec<u8>)> {
        if self.retained_stdout.is_none() {
            self.retained_stdout = Some(
                self.stdout
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .context("original stdout unproved")??,
            );
        }
        if self.retained_stderr.is_none() {
            self.retained_stderr = Some(
                self.stderr
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .context("original stderr unproved")??,
            );
        }
        Ok((
            self.retained_stdout
                .take()
                .context("original stdout absent")?,
            self.retained_stderr
                .take()
                .context("original stderr absent")?,
        ))
    }
}

fn await_direct_exit(
    mut poll: impl FnMut() -> Result<Option<std::process::ExitStatus>>,
    deadline: Instant,
) -> Result<std::process::ExitStatus> {
    loop {
        if let Some(status) = poll()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            bail!("managed child deadline; no signal sent");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn pipe(reader: impl Read + Send + 'static) -> Receiver<Result<Vec<u8>>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<Vec<u8>> {
            let mut bytes = Vec::new();
            reader
                .take((OUTPUT_BOUND + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes.len() > OUTPUT_BOUND {
                bail!("managed child pipe exceeds bound");
            }
            Ok(bytes)
        })();
        let _ = send.send(result);
    });
    receive
}

fn missing_pipe() -> Receiver<Result<Vec<u8>>> {
    let (send, receive) = mpsc::channel();
    let _ = send.send(Err(anyhow::anyhow!("original managed pipe handle missing")));
    receive
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
        if !cfg!(windows) || !executable.is_absolute() {
            bail!("managed creator requires a fully qualified Windows executable");
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
        let pipes_missing = out.is_none() || err.is_none();
        let stdout = out.map_or_else(missing_pipe, pipe);
        let stderr = err.map_or_else(missing_pipe, pipe);
        let birth = original_birth(&child);
        drop(child.stdin.take()); // Valid pipe EOF, rather than inherited invalid stdin.
        let mut owned = Self {
            child,
            executable: executable.to_owned(),
            birth_100ns: 0,
            pipes: OriginalPipes {
                stdout,
                stderr,
                retained_stdout: None,
                retained_stderr: None,
            },
            identity: None,
            outcome_unproved: pipes_missing,
            terminal_proved: false,
            direct_exit_proved: false,
        };
        match birth {
            Ok(value) => owned.birth_100ns = value,
            Err(_) => {
                owned.outcome_unproved = true;
            }
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
        let result = (|| -> Result<_> {
            if self.outcome_unproved {
                bail!("managed child outcome already unproved");
            }
            let deadline = Instant::now()
                .checked_add(timeout)
                .context("child deadline overflow")?;
            let exit = await_direct_exit(|| Ok(self.child.try_wait()?), deadline)?;
            self.direct_exit_proved = true;
            let (stdout, stderr) = self.pipes.finish(deadline)?;
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
        }
        result
    }

    pub(crate) fn terminal_proved(&self) -> bool {
        self.terminal_proved && !self.outcome_unproved
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

#[derive(Debug, Deserialize)]
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

    #[test]
    fn direct_exit_precedes_descendant_owned_pipe_completion() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        let (out_send, stdout) = mpsc::channel();
        let (err_send, stderr) = mpsc::channel();
        let mut pipes = OriginalPipes {
            stdout,
            stderr,
            retained_stdout: None,
            retained_stderr: None,
        };
        // The real direct-exit poll primitive never touches inherited pipes.
        let status = await_direct_exit(
            || Ok(Some(std::process::ExitStatus::from_raw(1))),
            Instant::now(),
        )
        .unwrap();
        assert!(!status.success());
        out_send.send(Ok(b"anchor output".to_vec())).unwrap();
        // A descendant still owns stderr. This is not full terminal proof.
        assert!(pipes.finish(Instant::now()).is_err());
        assert_eq!(
            pipes.retained_stdout.as_deref(),
            Some(b"anchor output".as_slice())
        );
        err_send.send(Ok(b"descendant EOF".to_vec())).unwrap();
        let (out, err) = pipes.finish(Instant::now()).unwrap();
        assert_eq!(out, b"anchor output");
        assert_eq!(err, b"descendant EOF");
        assert!(await_direct_exit(|| Ok(None), Instant::now()).is_err());
        assert!(
            await_direct_exit(|| Err(anyhow::anyhow!("unproved handle")), Instant::now()).is_err()
        );
    }
}
