//! Test-only originals for the current observer AUTH episode. Never Ready.
use super::*;

/// Returned live kernel facts, bound to the actual retained std Child handle.
/// No serde constructor, PID reopen, or expected-JSON image cache exists.
pub(crate) struct DiagnosticLiveProof {
    handle: usize,
    pub(crate) identity: ProcessIdentity,
}
impl DiagnosticLiveProof {
    pub(crate) fn require_selected(&self, expected: &ProcessIdentity) -> Result<()> {
        if self.identity.pid != expected.pid
            || self.identity.parent != expected.parent
            || self.identity.birth_100ns != expected.birth_100ns
            || self.identity.command_sha256 != expected.command_sha256
            || !same_executable(&self.identity.executable, &expected.executable)
        {
            bail!("actual returned original proof differs from selected full identity");
        }
        Ok(())
    }
    pub(crate) fn require_same(&self, original: &Self) -> Result<()> {
        if self.handle != original.handle || self.identity != original.identity {
            bail!("SAME original returned live kernel proof changed");
        }
        Ok(())
    }
}

#[cfg(windows)]
fn pid_birth(raw: windows_sys::Win32::Foundation::HANDLE, deadline: Instant) -> Result<(u32, u64)> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetProcessId, GetProcessTimes};
    require_deadline(deadline)?;
    let pid = unsafe { GetProcessId(raw) };
    require_deadline(deadline)?;
    if pid == 0 {
        return Err(std::io::Error::last_os_error()).context("original diagnostic kernel PID");
    }
    let mut birth: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
    let mut user: FILETIME = unsafe { std::mem::zeroed() };
    if unsafe { GetProcessTimes(raw, &mut birth, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(std::io::Error::last_os_error()).context("original diagnostic kernel birth");
    }
    require_deadline(deadline)?;
    Ok((
        pid,
        (u64::from(birth.dwHighDateTime) << 32) | u64::from(birth.dwLowDateTime),
    ))
}

#[cfg(windows)]
fn live_image(raw: windows_sys::Win32::Foundation::HANDLE, deadline: Instant) -> Result<PathBuf> {
    use windows_sys::Win32::System::Threading::{QueryFullProcessImageNameW, WaitForSingleObject};
    require_deadline(deadline)?;
    if unsafe { WaitForSingleObject(raw, 0) } != 258 {
        bail!("diagnostic live original not running");
    }
    require_deadline(deadline)?;
    let mut path = vec![0u16; 32768];
    let mut size = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(raw, 0, path.as_mut_ptr(), &mut size) } == 0 {
        return Err(std::io::Error::last_os_error()).context("diagnostic actual live kernel image");
    }
    require_deadline(deadline)?;
    let image = PathBuf::from(String::from_utf16(&path[..usize::try_from(size)?])?);
    if unsafe { WaitForSingleObject(raw, 0) } != 258 {
        bail!("diagnostic live image transition remains unproved");
    }
    require_deadline(deadline)?;
    Ok(image)
}

fn require_saved_kernel(identity: &ProcessIdentity, pid: u32, birth: u64) -> Result<()> {
    if identity.pid != pid || identity.birth_100ns != birth || !identity.executable.is_absolute() {
        bail!("SAME original diagnostic saved kernel identity changed");
    }
    Ok(())
}

// The common consumer is exercised with memory endpoints; the live adapter
// only borrows an ALREADY retained handle. It cannot open/adopt another PID.
trait OriginalKernelEndpoint {
    fn facts(&mut self, deadline: Instant) -> Result<(u32, u64)>;
    fn wait(&mut self, deadline: Instant) -> Result<u32>;
    fn image(&mut self, deadline: Instant) -> Result<PathBuf>;
}
#[derive(Debug, PartialEq, Eq)]
enum OriginalKernelObservation {
    Exited,
    Live,
}
fn observe_original_kernel(
    identity: &ProcessIdentity,
    endpoint: &mut impl OriginalKernelEndpoint,
    deadline: Instant,
    require_live_image: bool,
) -> Result<OriginalKernelObservation> {
    require_deadline(deadline)?;
    let (pid, birth) = endpoint.facts(deadline)?;
    require_deadline(deadline)?;
    require_saved_kernel(identity, pid, birth)?;
    let status = endpoint.wait(deadline)?;
    require_deadline(deadline)?;
    match status {
        0 if !require_live_image => Ok(OriginalKernelObservation::Exited),
        0 => bail!("live signal original already exited; no Image or signal"),
        258 => {
            if require_live_image {
                let image = endpoint.image(deadline)?;
                require_deadline(deadline)?;
                if !same_executable(&identity.executable, &image) {
                    bail!("actual original live image changed");
                }
                if endpoint.wait(deadline)? != 258 {
                    bail!("original transitioned during live image; unknown");
                }
                require_deadline(deadline)?;
            }
            Ok(OriginalKernelObservation::Live)
        }
        _ => bail!("original wait status unknown"),
    }
}
#[cfg(windows)]
struct BorrowedOriginalKernel<'a>(&'a std::os::windows::io::OwnedHandle);
#[cfg(windows)]
impl OriginalKernelEndpoint for BorrowedOriginalKernel<'_> {
    fn facts(&mut self, deadline: Instant) -> Result<(u32, u64)> {
        use std::os::windows::io::AsRawHandle;
        pid_birth(self.0.as_raw_handle() as _, deadline)
    }
    fn wait(&mut self, deadline: Instant) -> Result<u32> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        require_deadline(deadline)?;
        let result = unsafe { WaitForSingleObject(self.0.as_raw_handle() as _, 0) };
        require_deadline(deadline)?;
        Ok(result)
    }
    fn image(&mut self, deadline: Instant) -> Result<PathBuf> {
        use std::os::windows::io::AsRawHandle;
        live_image(self.0.as_raw_handle() as _, deadline)
    }
}

impl OriginalChild {
    pub(crate) fn diagnostic_lifetime_spawn_at(
        executable: &Path,
        arguments: &[String],
        startup: Instant,
    ) -> Result<Self> {
        require_deadline(startup)?;
        // Agent/RAS are long-lived originals, not utility commands whose EOF
        // must occur during startup. Their command_deadline remains None.
        let mut original = Self::spawn_original(executable, arguments, None)?;
        if require_deadline(startup).is_err() {
            original.mark_unproved();
        }
        Ok(original)
    }
    pub(crate) fn diagnostic_live_proof(
        &mut self,
        row: &CensusIdentity,
        deadline: Instant,
    ) -> Result<DiagnosticLiveProof> {
        require_deadline(deadline)?;
        let identity = self.bind_census(row)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let raw = self.child.as_raw_handle() as _;
            let (pid, birth) = pid_birth(raw, deadline)?;
            let image = live_image(raw, deadline)?;
            require_saved_kernel(&identity, pid, birth)?;
            if !same_executable(&identity.executable, &image) {
                bail!("original diagnostic anchor image drift");
            }
            let mut actual = identity;
            actual.executable = image;
            require_deadline(deadline)?;
            return Ok(DiagnosticLiveProof {
                handle: raw as usize,
                identity: actual,
            });
        }
        #[cfg(not(windows))]
        {
            let _ = identity;
            bail!("original diagnostic proof requires Windows");
        }
    }
    pub(crate) fn diagnostic_natural_exit(
        &mut self,
        proof: &DiagnosticLiveProof,
        deadline: Instant,
    ) -> Result<bool> {
        require_deadline(deadline)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let raw = self.child.as_raw_handle() as _;
            if raw as usize != proof.handle {
                bail!("SAME original anchor handle required");
            }
            let (pid, birth) = pid_birth(raw, deadline)?;
            require_saved_kernel(&proof.identity, pid, birth)?;
            // Exited proof deliberately never queries Image again.
            let exited = !self.alive()?;
            require_deadline(deadline)?;
            return Ok(exited);
        }
        #[cfg(not(windows))]
        {
            let _ = proof;
            bail!("original diagnostic exit requires Windows");
        }
    }
    pub(crate) fn stop_exact_at(
        &mut self,
        row: &CensusIdentity,
        expected: &ProcessIdentity,
        proof: &DiagnosticLiveProof,
        deadline: Instant,
    ) -> Result<()> {
        let result = (|| -> Result<()> {
            let current = self.diagnostic_live_proof(row, deadline)?;
            current.require_selected(expected)?;
            current.require_same(proof)?;
            let wait = (Instant::now() + Duration::from_secs(5)).min(deadline);
            require_deadline(wait)?;
            self.child
                .kill()
                .context("diagnostic direct original stop")?;
            await_direct_exit(|| Ok(self.child.try_wait()?), wait)?;
            require_deadline(deadline)?;
            self.direct_exit_proved = true;
            Ok(())
        })();
        if result.is_err() {
            self.outcome_unproved = true;
        }
        result
    }
}

impl KernelProcess {
    fn diagnostic_kernel(&self, deadline: Instant) -> Result<()> {
        require_deadline(deadline)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let (pid, birth) = pid_birth(self.handle.as_raw_handle() as _, deadline)?;
            require_saved_kernel(&self.identity, pid, birth)?;
            require_deadline(deadline)?;
            return Ok(());
        }
        #[cfg(not(windows))]
        bail!("diagnostic kernel proof requires Windows");
    }
    pub(crate) fn diagnostic_exited(&self, deadline: Instant) -> Result<bool> {
        #[cfg(windows)]
        {
            let observed = observe_original_kernel(
                &self.identity,
                &mut BorrowedOriginalKernel(&self.handle),
                deadline,
                false,
            )?;
            Ok(observed == OriginalKernelObservation::Exited)
        }
        #[cfg(not(windows))]
        bail!("diagnostic exited proof requires Windows");
    }
    pub(crate) fn diagnostic_live_current(
        &self,
        row: &CensusIdentity,
        deadline: Instant,
    ) -> Result<()> {
        require_deadline(deadline)?;
        self.require_current(row)?;
        #[cfg(windows)]
        {
            observe_original_kernel(
                &self.identity,
                &mut BorrowedOriginalKernel(&self.handle),
                deadline,
                true,
            )?;
            require_deadline(deadline)?;
            return Ok(());
        }
        #[cfg(not(windows))]
        bail!("diagnostic live proof requires Windows");
    }
    pub(crate) fn wait_natural_at(&self, deadline: Instant) -> Result<u32> {
        // SAME previously validated original, no signal, no exited Image call.
        self.diagnostic_kernel(deadline)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
            let raw = self.handle.as_raw_handle() as _;
            let millis = u32::try_from(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
            )?;
            if millis == 0 {
                bail!("original natural wait expired");
            }
            require_deadline(deadline)?;
            if unsafe { WaitForSingleObject(raw, millis) } != 0 {
                bail!("diagnostic natural exit unknown");
            }
            require_deadline(deadline)?;
            self.diagnostic_kernel(deadline)?;
            let mut code = 0;
            if unsafe { GetExitCodeProcess(raw, &mut code) } == 0 {
                return Err(std::io::Error::last_os_error()).context("original natural exit code");
            }
            require_deadline(deadline)?;
            return Ok(code);
        }
        #[cfg(not(windows))]
        bail!("diagnostic natural wait requires Windows");
    }
    pub(crate) fn stop_exact_at(&self, row: &CensusIdentity, deadline: Instant) -> Result<()> {
        if !self.terminate_authorized {
            bail!("diagnostic shutdown handle not selected");
        }
        self.diagnostic_live_current(row, deadline)?;
        let wait = (Instant::now() + Duration::from_secs(5)).min(deadline);
        require_deadline(wait)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};
            let raw = self.handle.as_raw_handle() as _;
            if unsafe { TerminateProcess(raw, 1) } == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("diagnostic original descendant stop");
            }
            require_deadline(wait)?;
            let millis = u32::try_from(wait.saturating_duration_since(Instant::now()).as_millis())?;
            if millis == 0 || unsafe { WaitForSingleObject(raw, millis) } != 0 {
                bail!("diagnostic descendant direct exit unknown");
            }
            require_deadline(wait)?;
            self.diagnostic_kernel(deadline)?;
            return Ok(());
        }
        #[cfg(not(windows))]
        bail!("diagnostic stop requires Windows");
    }
}

/// Pending registry slot is allocated by the owner before OpenProcess. Its
/// original handle stays in this slot through every fallible identity getter.
#[cfg(test)]
pub(crate) struct PendingShutdownHandle {
    pub(crate) row: CensusIdentity,
    #[cfg(windows)]
    original: Option<std::os::windows::io::OwnedHandle>,
    attempted: bool,
    pub(crate) original_error: Option<anyhow::Error>,
}
#[cfg(test)]
impl PendingShutdownHandle {
    pub(crate) fn new(row: CensusIdentity) -> Self {
        Self {
            row,
            #[cfg(windows)]
            original: None,
            attempted: false,
            original_error: None,
        }
    }
    pub(crate) fn bind_once(&mut self, deadline: Instant) -> Result<KernelProcess> {
        if self.attempted {
            bail!("pending shutdown handle cannot reopen");
        }
        self.attempted = true;
        let result = self.bind_inner(deadline);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.original_error = Some(error);
                bail!("pending original shutdown binding unproved; retained")
            }
        }
    }
    fn bind_inner(&mut self, deadline: Instant) -> Result<KernelProcess> {
        require_deadline(deadline)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
            use windows_sys::Win32::Foundation::FILETIME;
            use windows_sys::Win32::System::Threading::{
                GetProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            };
            let raw = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | 0x00100000 | 1,
                    0,
                    self.row.pid,
                )
            };
            if raw.is_null() {
                return Err(std::io::Error::last_os_error())
                    .context("diagnostic pending original OpenProcess");
            }
            self.original = Some(unsafe { OwnedHandle::from_raw_handle(raw as _) });
            require_deadline(deadline)?;
            let raw = self
                .original
                .as_ref()
                .context("pending original absent")?
                .as_raw_handle() as _;
            let mut birth: FILETIME = unsafe { std::mem::zeroed() };
            let mut exit: FILETIME = unsafe { std::mem::zeroed() };
            let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
            let mut user: FILETIME = unsafe { std::mem::zeroed() };
            let id = unsafe { GetProcessId(raw) };
            require_deadline(deadline)?;
            if unsafe { GetProcessTimes(raw, &mut birth, &mut exit, &mut kernel, &mut user) } == 0 {
                return Err(std::io::Error::last_os_error()).context("pending original times");
            }
            require_deadline(deadline)?;
            let executable = live_image(raw, deadline)?;
            let precise = (u64::from(birth.dwHighDateTime) << 32) | u64::from(birth.dwLowDateTime);
            let end = self
                .row
                .birth_filetime
                .checked_add(9)
                .context("pending birth bound")?;
            if id != self.row.pid
                || precise < self.row.birth_filetime
                || precise > end
                || self.row.parent == 0
                || self.row.command.is_empty()
                || !same_executable(&executable, &self.row.executable)
            {
                bail!("pending original complete identity drift");
            }
            let identity = ProcessIdentity {
                pid: id,
                parent: self.row.parent,
                birth_100ns: precise,
                executable,
                command_sha256: format!("{:X}", Sha256::digest(self.row.command.as_bytes())),
            };
            require_deadline(deadline)?;
            Ok(KernelProcess {
                handle: self
                    .original
                    .take()
                    .context("pending original ownership absent")?,
                identity,
                terminate_authorized: true,
            })
        }
        #[cfg(not(windows))]
        bail!("diagnostic binding requires Windows");
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    struct MemoryKernel {
        pid: u32,
        birth: u64,
        statuses: std::collections::VecDeque<u32>,
        image: Option<PathBuf>,
        image_calls: usize,
        facts_calls: usize,
    }
    impl OriginalKernelEndpoint for MemoryKernel {
        fn facts(&mut self, _: Instant) -> Result<(u32, u64)> {
            self.facts_calls += 1;
            Ok((self.pid, self.birth))
        }
        fn wait(&mut self, _: Instant) -> Result<u32> {
            self.statuses.pop_front().context("memory wait exhausted")
        }
        fn image(&mut self, _: Instant) -> Result<PathBuf> {
            self.image_calls += 1;
            self.image.clone().context("memory Image Win32 error31")
        }
    }
    fn original_fixture(statuses: Vec<u32>) -> (ProcessIdentity, MemoryKernel, Instant) {
        let identity = ProcessIdentity {
            pid: 42,
            parent: 41,
            birth_100ns: 1006,
            executable: PathBuf::from("F:/platform/rmngr.exe"),
            command_sha256: "A".repeat(64),
        };
        let endpoint = MemoryKernel {
            pid: 42,
            birth: 1006,
            statuses: statuses.into(),
            image: None,
            image_calls: 0,
            facts_calls: 0,
        };
        (identity, endpoint, Instant::now() + Duration::from_secs(5))
    }
    #[test]
    fn diagnostic_exited_original_image31_is_never_queried() {
        let (identity, mut endpoint, deadline) = original_fixture(vec![0]);
        assert_eq!(
            observe_original_kernel(&identity, &mut endpoint, deadline, false).unwrap(),
            OriginalKernelObservation::Exited
        );
        assert_eq!(endpoint.image_calls, 0);
        assert_eq!(endpoint.facts_calls, 1);
    }
    #[test]
    fn diagnostic_signal_live_consumer_keeps_image31_refusal() {
        let (identity, mut endpoint, deadline) = original_fixture(vec![258]);
        assert!(observe_original_kernel(&identity, &mut endpoint, deadline, true).is_err());
        assert_eq!(endpoint.image_calls, 1);
    }
    #[test]
    fn diagnostic_console_natural_consumer_accepts_live_or_already_exited_without_image() {
        for status in [0, 258] {
            let (identity, mut endpoint, deadline) = original_fixture(vec![status]);
            let observed =
                observe_original_kernel(&identity, &mut endpoint, deadline, false).unwrap();
            assert_eq!(
                observed,
                if status == 0 {
                    OriginalKernelObservation::Exited
                } else {
                    OriginalKernelObservation::Live
                }
            );
            assert_eq!(endpoint.image_calls, 0);
        }
    }
    #[test]
    fn diagnostic_signal_already_exited_refuses_before_image() {
        let (identity, mut endpoint, deadline) = original_fixture(vec![0]);
        assert!(observe_original_kernel(&identity, &mut endpoint, deadline, true).is_err());
        assert_eq!(endpoint.image_calls, 0);
    }
    #[test]
    fn diagnostic_current_live_image_and_wait_transition_remain_exact() {
        for statuses in [vec![258, 258], vec![258, 0], vec![999]] {
            let (identity, mut endpoint, deadline) = original_fixture(statuses.clone());
            endpoint.image = Some(identity.executable.clone());
            assert_eq!(
                observe_original_kernel(&identity, &mut endpoint, deadline, true).is_ok(),
                statuses == vec![258, 258]
            );
        }
        let (identity, mut endpoint, deadline) = original_fixture(vec![258]);
        endpoint.image = Some(PathBuf::from("F:/foreign.exe"));
        assert!(observe_original_kernel(&identity, &mut endpoint, deadline, true).is_err());
    }
    #[test]
    fn diagnostic_original_identity_and_expired_clock_precede_endpoint() {
        for changed_pid in [true, false] {
            let (identity, mut endpoint, deadline) = original_fixture(vec![0]);
            if changed_pid {
                endpoint.pid += 1;
            } else {
                endpoint.birth += 1;
            }
            assert!(observe_original_kernel(&identity, &mut endpoint, deadline, false).is_err());
            assert_eq!(endpoint.image_calls, 0);
            assert_eq!(endpoint.statuses.len(), 1);
        }
        let (identity, mut endpoint, _) = original_fixture(vec![0]);
        assert!(observe_original_kernel(&identity, &mut endpoint, Instant::now(), false).is_err());
        assert_eq!(endpoint.facts_calls, 0);
        assert_eq!(endpoint.image_calls, 0);
    }
    #[test]
    fn diagnostic_actual_saved_kernel_refuses_pid_birth_and_unselected_image() {
        let identity = ProcessIdentity {
            pid: 42,
            parent: 41,
            birth_100ns: 1006,
            executable: PathBuf::from("F:/platform/rmngr.exe"),
            command_sha256: "A".repeat(64),
        };
        require_saved_kernel(&identity, 42, 1006).unwrap();
        assert!(require_saved_kernel(&identity, 43, 1006).is_err());
        assert!(require_saved_kernel(&identity, 42, 1007).is_err());
        let mut missing = identity;
        missing.executable = PathBuf::from("relative.exe");
        assert!(require_saved_kernel(&missing, 42, 1006).is_err());
    }
    #[test]
    fn diagnostic_actual_live_proof_identity_and_handle_swaps_are_refused() {
        let identity = ProcessIdentity {
            pid: 42,
            parent: 41,
            birth_100ns: 1006,
            executable: PathBuf::from("F:/platform/rmngr.exe"),
            command_sha256: "A".repeat(64),
        };
        let original = DiagnosticLiveProof {
            handle: 99,
            identity: identity.clone(),
        };
        DiagnosticLiveProof {
            handle: 99,
            identity: identity.clone(),
        }
        .require_same(&original)
        .unwrap();
        assert!(
            DiagnosticLiveProof {
                handle: 100,
                identity: identity.clone()
            }
            .require_same(&original)
            .is_err()
        );
        let mut changed = identity;
        changed.birth_100ns += 1;
        assert!(
            DiagnosticLiveProof {
                handle: 99,
                identity: changed
            }
            .require_same(&original)
            .is_err()
        );
    }
    #[test]
    fn diagnostic_returned_live_proof_requires_selected_full_identity() {
        let (identity, _, _) = original_fixture(vec![]);
        let proof = DiagnosticLiveProof {
            handle: 99,
            identity: identity.clone(),
        };
        proof.require_selected(&identity).unwrap();
        for field in 0..5 {
            let mut changed = identity.clone();
            match field {
                0 => changed.pid += 1,
                1 => changed.parent += 1,
                2 => changed.birth_100ns += 1,
                3 => changed.command_sha256 = "B".repeat(64),
                _ => changed.executable = PathBuf::from("F:/foreign.exe"),
            }
            assert!(proof.require_selected(&changed).is_err());
        }
    }
}
