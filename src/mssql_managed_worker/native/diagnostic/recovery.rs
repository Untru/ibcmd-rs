//! One ROOT request, SAME diagnostic owner. No positive creator authority.
use super::*;
use crate::mssql_managed_worker::child::{DiagnosticLiveProof, PendingShutdownHandle};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
mod current;

#[derive(Default)]
pub(super) struct CheckpointAttempt {
    attempted: bool,
    complete: bool,
    original_error: Option<anyhow::Error>,
    original_panic: Option<Box<dyn Any + Send>>,
    original_bytes: Vec<u8>,
    completed_at: Option<Instant>,
}
impl CheckpointAttempt {
    pub(super) fn run(
        &mut self,
        out: &mut impl Write,
        mut clock: impl FnMut() -> Result<()>,
        value: &serde_json::Value,
    ) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            clock()?;
            self.original_bytes = serde_json::to_vec(value)?;
            self.original_bytes.push(b'\n');
            if self.original_bytes.len() > 4096 {
                bail!("checkpoint bound");
            }
            clock()?;
            out.write_all(&self.original_bytes)?;
            clock()?;
            out.flush()?;
            clock()?;
            self.completed_at = Some(Instant::now());
            clock()?;
            self.complete = true;
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => self.original_error = Some(e),
            Err(p) => self.original_panic = Some(p),
        }
    }
    pub(super) fn proved(&self) -> bool {
        self.complete && self.original_error.is_none() && self.original_panic.is_none()
    }
}

struct OriginalControlResult {
    raw: Vec<u8>,
    received: Option<Instant>,
    original_error: Option<anyhow::Error>,
    original_panic: Option<Box<dyn Any + Send>>,
}
pub(super) struct OriginalRootControl {
    // Pending slots exist before reader thread allocation; no source owner is
    // sent to the reader. Original stream remains retained even on read panic.
    _original: Arc<Mutex<Box<dyn Read + Send>>>,
    original_thread: Option<JoinHandle<()>>,
    completion: Arc<Mutex<OriginalControlResult>>,
    receive: mpsc::Receiver<()>,
    attempted: bool,
}
impl OriginalRootControl {
    pub(super) fn start(reader: impl Read + Send + 'static) -> Self {
        let original: Arc<Mutex<Box<dyn Read + Send>>> = Arc::new(Mutex::new(Box::new(reader)));
        let completion = Arc::new(Mutex::new(OriginalControlResult {
            raw: vec![],
            received: None,
            original_error: None,
            original_panic: None,
        }));
        let (send, receive) = mpsc::channel();
        let mut held = Self {
            _original: original.clone(),
            original_thread: None,
            completion: completion.clone(),
            receive,
            attempted: false,
        };
        let launched = std::thread::Builder::new()
            .name("root-diagnostic-control-original".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
                    let mut input = original.lock().unwrap_or_else(|e| e.into_inner());
                    let mut chunk = [0u8; 256];
                    loop {
                        let saved_len = completion
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .raw
                            .len();
                        let room = 4097usize
                            .checked_sub(saved_len)
                            .context("original control bound")?
                            .min(chunk.len());
                        let n = input.read(&mut chunk[..room])?;
                        if n == 0 {
                            break;
                        }
                        let mut saved = completion.lock().unwrap_or_else(|e| e.into_inner());
                        saved.raw.extend_from_slice(&chunk[..n]);
                        if saved.raw.len() > 4096 {
                            bail!("original control bound exceeded");
                        }
                    }
                    Ok(())
                }));
                let mut record = completion.lock().unwrap_or_else(|e| e.into_inner());
                match result {
                    Ok(Ok(())) => {
                        record.received = Some(Instant::now());
                    }
                    Ok(Err(e)) => record.original_error = Some(e),
                    Err(p) => record.original_panic = Some(p),
                }
                drop(record);
                let _ = send.send(());
            });
        match launched {
            Ok(t) => held.original_thread = Some(t),
            Err(e) => {
                held.completion
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .original_error = Some(e.into())
            }
        }
        held
    }
    pub(super) fn require_started(&self) -> Result<()> {
        if self.original_thread.is_none() {
            bail!("original ROOT control reader not started; no native effects");
        }
        let result = self
            .completion
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if result.original_error.is_some()
            || result.original_panic.is_some()
            || result.received.is_some()
        {
            bail!("original ROOT pipe failed/closed before native allocation");
        }
        Ok(())
    }
    fn receive_once(&mut self, deadline: Instant) -> Result<(Vec<u8>, Instant)> {
        if self.attempted {
            bail!("ROOT control request cannot replay");
        }
        self.attempted = true;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        self.receive
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("original ROOT control completion unproved")?;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        while !self
            .original_thread
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            crate::mssql_managed_worker::child::require_deadline(deadline)?;
            std::thread::sleep(Duration::from_millis(1));
        }
        let r = self.completion.lock().unwrap_or_else(|e| e.into_inner());
        if r.original_error.is_some() || r.original_panic.is_some() {
            bail!("original ROOT control reader fault retained");
        }
        require_frame(&r.raw)?;
        let received = r
            .received
            .context("original control received time absent")?;
        if received >= deadline {
            bail!("original ROOT request late");
        }
        let raw = r.raw.clone();
        drop(r);
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        Ok((raw, received))
    }
}
fn require_frame(raw: &[u8]) -> Result<()> {
    if raw.is_empty() || raw.len() > 4096 {
        bail!("one bounded ROOT recovery frame required");
    }
    std::str::from_utf8(raw).context("ROOT recovery frame strict UTF-8")?;
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryRequest {
    protocol: String,
    action: String,
    request_nonce: String,
    request_sha256: String,
    root: PathBuf,
    lifetime_nonce: String,
    cluster: String,
    observations_sha256: String,
    recovery_nonce: String,
}
impl RecoveryRequest {
    fn require_binding(&self, owner: &RetainedAuthenticationDiagnostic) -> Result<()> {
        self.require_identity(
            &owner.selection.request.request_nonce,
            &owner.selection.originals[0].digest,
            &owner.runtime.binding,
        )?;
        let actual = tool_digest(
            &owner
                .runtime
                .binding
                .root
                .join("diagnostic-observations.json"),
        )?;
        if actual != self.observations_sha256 {
            bail!("original diagnostic observations changed");
        }
        Ok(())
    }
    fn require_identity(
        &self,
        original_nonce: &str,
        original_sha: &str,
        binding: &LifetimeBinding,
    ) -> Result<()> {
        if self.protocol != "native-auth409/recovery-request-v1"
            || self.action != "stop-original-diagnostic"
            || self.request_nonce != original_nonce
            || self.request_sha256 != original_sha
            || self.root != binding.root
            || self.lifetime_nonce != binding.nonce.to_string()
            || self.cluster != binding.cluster.to_string()
        {
            bail!("ROOT recovery exact original binding mismatch");
        }
        let nonce = Uuid::parse_str(&self.recovery_nonce)?;
        if nonce.is_nil()
            || nonce.to_string() != self.recovery_nonce
            || self.recovery_nonce == self.request_nonce
        {
            bail!("distinct canonical recovery nonce required");
        }
        require_digest(&self.observations_sha256)?;
        Ok(())
    }
}
enum RecoveryState {
    Selected,
    RequestPending,
    Guarded,
    Signalling,
    Exited,
    Unknown,
}
pub(super) struct RecoveryCustody {
    control: OriginalRootControl,
    state: RecoveryState,
    request: Option<RecoveryRequest>,
    raw_request: Option<Vec<u8>>,
    pending_handles: Vec<PendingShutdownHandle>,
    preparation_attempted: bool,
    live_anchors: BTreeMap<u32, DiagnosticLiveProof>,
    boundaries: Vec<DiagnosticBoundary>,
    natural_exits: BTreeMap<u32, u32>,
    signals: BTreeSet<u32>,
    anchor_outputs: Vec<(i32, Vec<u8>, Vec<u8>)>,
    original_error: Option<anyhow::Error>,
    original_panic: Option<Box<dyn Any + Send>>,
    receipt: Option<DiagnosticExited>,
}
struct DiagnosticBoundaryHalf {
    original_collector: usize,
    rows: Vec<CensusIdentity>,
    listeners: Vec<Listener>,
}
struct DiagnosticBoundary {
    first: DiagnosticBoundaryHalf,
    second: DiagnosticBoundaryHalf,
}
struct DiagnosticExited {
    deadline: Instant,
    original_collectors: usize,
}
impl RecoveryCustody {
    pub(super) fn new(control: OriginalRootControl) -> Self {
        Self {
            control,
            state: RecoveryState::Selected,
            request: None,
            raw_request: None,
            pending_handles: vec![],
            preparation_attempted: false,
            live_anchors: BTreeMap::new(),
            boundaries: vec![],
            natural_exits: BTreeMap::new(),
            signals: BTreeSet::new(),
            anchor_outputs: vec![],
            original_error: None,
            original_panic: None,
            receipt: None,
        }
    }
    fn settle(&mut self, outcome: std::thread::Result<Result<()>>) -> bool {
        match outcome {
            Ok(Ok(())) if matches!(self.state, RecoveryState::Exited) && self.receipt.is_some() => {
                true
            }
            Ok(Ok(())) => {
                self.original_error = Some(anyhow::anyhow!("typed diagnostic exit proof absent"));
                self.state = RecoveryState::Unknown;
                false
            }
            Ok(Err(error)) => {
                self.original_error = Some(error);
                self.state = RecoveryState::Unknown;
                false
            }
            Err(panic) => {
                self.original_panic = Some(panic);
                self.state = RecoveryState::Unknown;
                false
            }
        }
    }
}
/// Cannot be deserialized, built by JSON or used by ManagedWorker/Ready.
struct DiagnosticRecoveryPermit {
    deadline: Instant,
    root: PathBuf,
    nonce: Uuid,
}

fn require_measurements(
    m: &DiagnosticMeasurements,
    raws: &[RawMutation],
    plan: &authentication::Plan,
) -> Result<()> {
    if !m.attempted || m.controls.len() != 2 || m.challenges.len() != 4 || raws.len() != 8 {
        bail!("complete diagnostic original evidence required");
    }
    for (index, family) in [AdminFamily::Agent, AdminFamily::Cluster]
        .into_iter()
        .enumerate()
    {
        let control = &m.controls[index];
        if control.family != family || control.before != control.after {
            bail!("diagnostic complete control baseline drift");
        }
        let name = plan.name(family, AdminCredentials::Correct);
        let before = authentication::Inventory::from_rows(
            control.before.values().cloned().collect(),
            &[plan.administrator()],
        )?;
        let present = authentication::Inventory::from_rows(
            control.present.values().cloned().collect(),
            &[plan.administrator(), &name],
        )?;
        if before.complete() != &control.before || present.complete() != &control.present {
            bail!("control inventory original names drift");
        }
        // Reuse actual control inventory rule instead of count-only equality.
        present.require_control_present(&before, &name)?;
        for (offset, kind) in [
            AdminCredentials::WrongPassword,
            AdminCredentials::ImplicitOs,
        ]
        .into_iter()
        .enumerate()
        {
            let witness = &m.challenges[index * 2 + offset];
            if witness.family != family
                || witness.kind != kind
                || witness.exit == 0
                || witness.mutated
                || witness.before != control.after
                || witness.before != witness.after
            {
                bail!("diagnostic challenge is not complete known unchanged cell");
            }
        }
    }
    let mut indexes = BTreeSet::new();
    for (index, raw) in raws.iter().enumerate() {
        let family = if index < 4 {
            AdminFamily::Agent
        } else {
            AdminFamily::Cluster
        };
        let offset = index % 4;
        let expected_kind = match offset {
            0 | 1 => AdminCredentials::Correct,
            2 => AdminCredentials::WrongPassword,
            _ => AdminCredentials::ImplicitOs,
        };
        let expected_action = if offset == 1 {
            AdminAction::Remove
        } else {
            AdminAction::Register
        };
        if raw.family != family
            || raw.kind != expected_kind
            || raw.credentials != expected_kind
            || raw.action != expected_action
            || raw.unproved
            || !raw.known_direct_both
            || !indexes.insert(raw.original_index.context("mutation original absent")?)
        {
            bail!("diagnostic raw mapping/original custody drift");
        }
        let receipt = raw.raw.as_ref().context("diagnostic original raw absent")?;
        if offset < 2 {
            if receipt.exit != 0 || !receipt.stderr.is_empty() {
                bail!("correct control original failed");
            }
        } else {
            let witness = &m.challenges[(index / 4) * 2 + offset - 2];
            if receipt.exit != witness.exit
                || receipt.stdout != witness.stdout
                || receipt.stderr != witness.stderr
            {
                bail!("challenge raw not SAME original");
            }
        }
    }
    Ok(())
}

impl RetainedAuthenticationDiagnostic {
    pub(super) fn serve_root_recovery_once(&mut self) -> bool {
        if !matches!(self.recovery.state, RecoveryState::Selected) {
            return false;
        }
        self.recovery.state = RecoveryState::RequestPending;
        let outcome = catch_unwind(AssertUnwindSafe(|| self.recovery_inner()));
        let proved = self.recovery.settle(outcome);
        if !proved {
            self.runtime.failed = true;
        }
        proved
    }
    fn require_known_diagnostic(&self) -> Result<()> {
        if !matches!(self.state, DiagnosticState::Complete)
            || self.runtime.failed
            || self.original_error.is_some()
            || self.original_panic.is_some()
            || !self.publication_clock_proved
            || !self.checkpoint.proved()
            || self.runtime.authenticated_boundary
            || self.runtime.cold
            || !self.runtime.binding.infobase.is_nil()
            || !self.runtime.admitted_load.is_empty()
            || !self.runtime.workers.is_empty()
            || self.runtime.barrier_filetime != 0
            || self.runtime.collectors.iter().any(|c| !c.terminal_proved())
        {
            bail!("diagnostic known-only recovery prerequisite missing");
        }
        let plan =
            authentication::Plan::new(&self.runtime.administrator, self.runtime.binding.cluster)?;
        require_measurements(&self.measurements, &self.mutations, &plan)?;
        for raw in &self.mutations {
            if !raw
                .original_index
                .and_then(|i| self.runtime.collectors.get(i))
                .is_some_and(OriginalChild::terminal_proved)
            {
                bail!("SAME original terminal absent");
            }
        }
        self.selection.require_current()?;
        Ok(())
    }
    fn recovery_inner(&mut self) -> Result<()> {
        self.require_known_diagnostic()?;
        let (raw, received) = self
            .recovery
            .control
            .receive_once(self.timeline.request_until)?;
        self.recovery.raw_request = Some(raw);
        self.recovery.request = Some(serde_json::from_slice(
            self.recovery
                .raw_request
                .as_ref()
                .context("original frame absent")?,
        )?);
        self.recovery
            .request
            .as_ref()
            .context("ROOT request absent")?
            .require_binding(self)?;
        let deadline = self.timeline.enter_recovery(received)?;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        let permit = DiagnosticRecoveryPermit {
            deadline,
            root: self.runtime.binding.root.clone(),
            nonce: self.runtime.binding.nonce,
        };
        // Fixed recovery scope never overwrites the consumed measurement field.
        if self.runtime.diagnostic_recovery_deadline.is_some() {
            bail!("recovery scope already selected");
        }
        self.runtime.diagnostic_recovery_deadline = Some(permit.deadline);
        self.runtime.require_private_endpoint()?;
        self.fresh_empty_diagnostic_inventory(&permit)?;
        self.recovery.state = RecoveryState::Guarded;
        self.journal.append("diagnostic_recovery_authorized",serde_json::json!({"root":permit.root,"nonce":permit.nonce.to_string(),"once":true,"maximum_seconds":120,"Ready":false}))?;
        self.physical_stop(&permit)?;
        self.verify_diagnostic_exited(&permit)?;
        let raw_originals = self.persist_original_outputs(&permit)?;
        let raw = serde_json::to_vec(
            &serde_json::json!({"protocol":"native-auth409/diagnostic-exited-v1","Ready":false,"authenticated_boundary":false,"ColdSession":false,"cleanup_observations_complete":true,"cleanup_proved":false,"provisional_until_original_helper_terminal":true,"known_helper_terminal":false,
            "original_collectors":self.runtime.collectors.len(),"raw_originals":raw_originals,"anchor_outputs":self.recovery.anchor_outputs.iter().map(|r|serde_json::json!({"exit":r.0,"stdout_bytes":r.1.len(),"stderr_bytes":r.2.len(),"stdout_sha256":format!("{:X}",Sha256::digest(&r.1)),"stderr_sha256":format!("{:X}",Sha256::digest(&r.2))})).collect::<Vec<_>>() }),
        )?;
        self.write_original_evidence_at("diagnostic-exited.json", &raw, permit.deadline)?;
        self.journal.append("diagnostic_exited",serde_json::json!({"Ready":false,"leases_released":false,"original_collectors":self.runtime.collectors.len()}))?;
        self.selection.require_current()?;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        self.recovery.receipt = Some(DiagnosticExited {
            deadline,
            original_collectors: self.runtime.collectors.len(),
        });
        self.recovery.state = RecoveryState::Exited;
        // Same final fence after receipt/state construction. Late => resident.
        let receipt = self
            .recovery
            .receipt
            .as_ref()
            .context("typed exit absent")?;
        if receipt.original_collectors != self.runtime.collectors.len() {
            bail!("final original collection drift");
        }
        crate::mssql_managed_worker::child::require_deadline(receipt.deadline)?;
        Ok(())
    }
    fn persist_original_outputs(
        &mut self,
        permit: &DiagnosticRecoveryPermit,
    ) -> Result<Vec<serde_json::Value>> {
        let mut records = vec![];
        let count = self.runtime.collectors.len() + 2;
        for index in 0..count {
            let child = if index < self.runtime.collectors.len() {
                &self.runtime.collectors[index]
            } else if index == self.runtime.collectors.len() {
                self.runtime
                    .agent
                    .as_ref()
                    .context("original agent absent")?
            } else {
                self.runtime.ras.as_ref().context("original RAS absent")?
            };
            let pid = child.pid();
            let (exit, out, err) = child.terminal_raw()?;
            let (out, err) = (out.to_vec(), err.to_vec());
            for bytes in [&out, &err] {
                if [
                    &self.runtime.password,
                    &self.wrong_password,
                    &self.throwaway_password,
                ]
                .into_iter()
                .any(|secret| contains_secret(bytes, secret))
                {
                    bail!("diagnostic original raw secret detected; resident custody retained");
                }
            }
            let out_name = format!("original-{index}-stdout.raw");
            let err_name = format!("original-{index}-stderr.raw");
            self.write_original_evidence_at(&out_name, &out, permit.deadline)?;
            crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
            self.write_original_evidence_at(&err_name, &err, permit.deadline)?;
            records.push(serde_json::json!({"index":index,"pid":pid,"exit":exit,"direct_BOTH":true,
                "stdout":{"file":out_name,"bytes":out.len(),"sha256":format!("{:X}",Sha256::digest(&out))},
                "stderr":{"file":err_name,"bytes":err.len(),"sha256":format!("{:X}",Sha256::digest(&err))}}));
            crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
        }
        Ok(records)
    }
    fn fresh_empty_diagnostic_inventory(
        &mut self,
        permit: &DiagnosticRecoveryPermit,
    ) -> Result<()> {
        if permit.root != self.runtime.binding.root || permit.nonce != self.runtime.binding.nonce {
            bail!("recovery permit not SAME lifetime");
        }
        let plan =
            authentication::Plan::new(&self.runtime.administrator, self.runtime.binding.cluster)?;
        let administrator = self.runtime.administrator.clone();
        let mut io = NativeAdminIo {
            runtime: &mut self.runtime,
            journal: &mut self.journal,
            plan: &plan,
            wrong_password: self.wrong_password.clone(),
            throwaway_password: self.throwaway_password.clone(),
            captures: None,
        };
        use authentication::Io;
        for control in &self.measurements.controls {
            let observed = io.inventory(control.family, &[&administrator])?;
            if observed.complete() != &control.after {
                bail!("current full diagnostic admin inventory drift");
            }
        }
        self.runtime.require_admins()?;
        for family in ["infobase", "session", "connection"] {
            let mut args = vec![family.to_owned()];
            if family == "infobase" {
                args.push("summary".into());
            }
            args.extend([
                "list".into(),
                format!("--cluster={}", self.runtime.binding.cluster),
            ]);
            if !blocks(&self.runtime.rac(args, false, true)?)?.is_empty() {
                bail!("diagnostic current native database/session inventory not empty");
            }
        }
        crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
        Ok(())
    }
    fn signal_once(&mut self, pid: u32, deadline: Instant) -> Result<()> {
        if !self.recovery.signals.insert(pid) {
            bail!("diagnostic original signal cannot retry");
        }
        crate::mssql_managed_worker::child::require_deadline(deadline)
    }
    fn verify_diagnostic_exited(&mut self, permit: &DiagnosticRecoveryPermit) -> Result<()> {
        if self.runtime.failed
            || self.runtime.authenticated_boundary
            || self.runtime.cold
            || self.recovery.anchor_outputs.len() != 2
            || self.runtime.collectors.iter().any(|c| !c.terminal_proved())
            || [&self.runtime.agent, &self.runtime.ras]
                .iter()
                .any(|a| !a.as_ref().is_some_and(OriginalChild::terminal_proved))
        {
            bail!("diagnostic original direct/BOTH exit unproved");
        }
        for h in self.runtime.shutdown_handles.values() {
            if !h.diagnostic_exited(permit.deadline)? {
                bail!("SAME original diagnostic descendant exit unproved");
            }
        }
        let boundary = self.current_boundary(permit.deadline)?;
        let current = &self.recovery.boundaries[boundary];
        if !current.first.listeners.is_empty()
            || !current.second.listeners.is_empty()
            || current
                .first
                .rows
                .iter()
                .chain(&current.second.rows)
                .any(|r| self.runtime.is_shutdown_member(r))
        {
            bail!("final full current original/private/listener absence unproved");
        }
        if current.first.original_collector == current.second.original_collector {
            bail!("two distinct SAME original census collector slots required");
        }
        if self.runtime.collectors.iter().any(|c| !c.terminal_proved()) {
            bail!("final original observer direct/BOTH unproved");
        }
        crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error, ErrorKind};
    fn neutral() -> serde_json::Value {
        serde_json::json!({"protocol":"native-auth409/resident-custody-v2","provisional":true,"Ready":false,"known_helper_terminal":false,"cleanup_proved":false})
    }
    struct FaultWriter {
        bytes: Vec<u8>,
        write_error: bool,
        flush_error: bool,
    }
    impl Write for FaultWriter {
        fn write(&mut self, raw: &[u8]) -> std::io::Result<usize> {
            if self.write_error {
                return Err(Error::new(ErrorKind::BrokenPipe, "original write fault"));
            }
            self.bytes.extend_from_slice(raw);
            Ok(raw.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if self.flush_error {
                Err(Error::other("original flush fault"))
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn diagnostic_checkpoint_write_and_flush_faults_are_retained_sticky() {
        for (write_error, flush_error) in [(true, false), (false, true)] {
            let mut attempt = CheckpointAttempt::default();
            let mut out = FaultWriter {
                bytes: vec![],
                write_error,
                flush_error,
            };
            attempt.run(&mut out, || Ok(()), &neutral());
            assert!(!attempt.proved());
            assert!(attempt.original_error.is_some());
            assert!(!attempt.original_bytes.is_empty());
            let before = out.bytes.clone();
            out.write_error = false;
            out.flush_error = false;
            attempt.run(&mut out, || Ok(()), &neutral());
            assert_eq!(out.bytes, before);
            assert!(!attempt.proved());
        }
    }
    #[test]
    fn diagnostic_checkpoint_each_fallible_fence_expiry_is_not_wire_acceptance() {
        for fault in 1..=5 {
            let mut attempt = CheckpointAttempt::default();
            let mut out = Vec::new();
            let mut n = 0;
            attempt.run(
                &mut out,
                || {
                    n += 1;
                    if n == fault {
                        bail!("fixed original expired");
                    }
                    Ok(())
                },
                &neutral(),
            );
            assert!(!attempt.proved());
            assert!(attempt.original_error.is_some());
            if !out.is_empty() {
                let wire: serde_json::Value = serde_json::from_slice(&out).unwrap();
                assert_eq!(wire["provisional"], true);
                assert!(wire.get("publication_clock_proved").is_none());
                assert_eq!(wire["cleanup_proved"], false);
            }
        }
        let mut good = CheckpointAttempt::default();
        good.run(&mut Vec::new(), || Ok(()), &neutral());
        assert!(good.proved());
    }
    #[test]
    fn diagnostic_checkpoint_panic_retains_payload_and_original_panic() {
        let mut held = CheckpointAttempt::default();
        let mut calls = 0;
        held.run(
            &mut Vec::new(),
            || {
                calls += 1;
                if calls == 3 {
                    panic!("original checkpoint fault");
                }
                Ok(())
            },
            &neutral(),
        );
        assert!(held.original_panic.is_some());
        assert!(!held.original_bytes.is_empty());
        assert!(!held.proved());
    }
    #[test]
    fn diagnostic_original_control_bounded_eof_once_and_deadline() {
        let mut reader = OriginalRootControl::start(Cursor::new(b"{\"one\":true}".to_vec()));
        let (raw, _) = reader
            .receive_once(Instant::now() + Duration::from_secs(2))
            .unwrap();
        assert_eq!(raw, b"{\"one\":true}");
        assert!(
            reader
                .receive_once(Instant::now() + Duration::from_secs(2))
                .is_err()
        );
        assert!(reader.original_thread.is_some());
        for bad in [vec![], vec![b'x'; 4097], vec![0xff]] {
            let mut reader = OriginalRootControl::start(Cursor::new(bad));
            assert!(
                reader
                    .receive_once(Instant::now() + Duration::from_secs(2))
                    .is_err()
            );
            assert!(reader.original_thread.is_some());
        }
        let mut reader = OriginalRootControl::start(Cursor::new(b"{}".to_vec()));
        assert!(reader.receive_once(Instant::now()).is_err());
        assert!(reader.attempted);
    }
    struct PartialFault {
        step: u8,
        panic: bool,
    }
    impl Read for PartialFault {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.step == 0 {
                self.step = 1;
                out[..2].copy_from_slice(b"{x");
                return Ok(2);
            }
            if self.panic {
                panic!("original input fault");
            }
            Err(Error::other("original input fault"))
        }
    }
    #[test]
    fn diagnostic_original_control_partial_error_and_panic_keep_stream_raw() {
        for panic in [false, true] {
            let mut held = OriginalRootControl::start(PartialFault { step: 0, panic });
            assert!(
                held.receive_once(Instant::now() + Duration::from_secs(2))
                    .is_err()
            );
            let record = held.completion.lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(record.raw, b"{x");
            assert!(record.original_error.is_some() || record.original_panic.is_some());
            assert!(held.original_thread.is_some());
        }
    }
    #[test]
    fn diagnostic_recovery_timeline_has_distinct_once_slot_no_original_renewal() {
        let now = Instant::now();
        let mut t = DiagnosticTimeline::select(now).unwrap();
        let measurement = t.enter_measurement(now + Duration::from_secs(10)).unwrap();
        let recovery = t.enter_recovery(now + Duration::from_secs(719)).unwrap();
        assert_eq!(recovery.duration_since(now), Duration::from_secs(839));
        assert_eq!(t.measurement, Some(measurement));
        assert!(t.enter_recovery(now + Duration::from_secs(720)).is_err());
        let mut late = DiagnosticTimeline::select(now).unwrap();
        assert!(late.enter_recovery(now + Duration::from_secs(720)).is_err());
        assert!(late.recovery.is_none());
    }
    fn full_evidence() -> (
        DiagnosticMeasurements,
        Vec<RawMutation>,
        authentication::Plan,
    ) {
        let admin = "ibcmd_11111111-1111-4111-8111-111111111111";
        let plan = authentication::Plan::new(admin, Uuid::from_u128(99)).unwrap();
        let mut records = DiagnosticMeasurements::default();
        records.attempted = true;
        let mut raw = vec![];
        let baseline = BTreeMap::from([(
            admin.to_owned(),
            BTreeMap::from([
                ("name".into(), admin.to_owned()),
                ("auth".into(), "pwd".into()),
                ("description".into(), "whole original baseline".into()),
            ]),
        )]);
        for family in [AdminFamily::Agent, AdminFamily::Cluster] {
            let name = plan.name(family, AdminCredentials::Correct);
            let mut present = baseline.clone();
            present.insert(
                name.clone(),
                BTreeMap::from([("name".into(), name), ("auth".into(), "pwd".into())]),
            );
            records.controls.push(authentication::DiagnosticControl {
                family,
                before: baseline.clone(),
                present,
                after: baseline.clone(),
            });
            for kind in [
                AdminCredentials::WrongPassword,
                AdminCredentials::ImplicitOs,
            ] {
                records.challenges.push(authentication::DiagnosticWitness {
                    family,
                    kind,
                    exit: -1,
                    stdout: vec![],
                    stderr: vec![family as u8, kind as u8],
                    before: baseline.clone(),
                    after: baseline.clone(),
                    mutated: false,
                });
            }
            for offset in 0..4 {
                let kind = match offset {
                    0 | 1 => AdminCredentials::Correct,
                    2 => AdminCredentials::WrongPassword,
                    _ => AdminCredentials::ImplicitOs,
                };
                let action = if offset == 1 {
                    AdminAction::Remove
                } else {
                    AdminAction::Register
                };
                let index = raw.len();
                let mut entry = RawMutation::pending(family, kind, action, kind);
                entry.original_index = Some(index);
                entry.known_direct_both = true;
                entry.raw = Some(Receipt {
                    exit: if offset < 2 { 0 } else { -1 },
                    stdout: vec![],
                    stderr: if offset < 2 {
                        vec![]
                    } else {
                        vec![family as u8, kind as u8]
                    },
                });
                raw.push(entry);
            }
        }
        (records, raw, plan)
    }
    #[test]
    fn diagnostic_known_evidence_four_cells_full_controls_raw_original_mapping() {
        let (m, raw, plan) = full_evidence();
        require_measurements(&m, &raw, &plan).unwrap();
        for fault in 0..10 {
            let (mut m, mut raw, plan) = full_evidence();
            match fault {
                0 => m.challenges[3].kind = AdminCredentials::WrongPassword,
                1 => m.challenges[0].mutated = true,
                2 => m.challenges[0].exit = 0,
                3 => {
                    m.challenges[0]
                        .after
                        .values_mut()
                        .next()
                        .unwrap()
                        .insert("description".into(), "drift".into());
                }
                4 => {
                    m.controls[0]
                        .present
                        .values_mut()
                        .find(|r| r["name"] == plan.administrator())
                        .unwrap()
                        .insert("description".into(), "drift".into());
                }
                5 => raw[3].original_index = raw[2].original_index,
                6 => raw[2].raw.as_mut().unwrap().stderr.push(9),
                7 => raw[7].known_direct_both = false,
                8 => raw[7].unproved = true,
                _ => raw[5].action = AdminAction::Register,
            }
            assert!(
                require_measurements(&m, &raw, &plan).is_err(),
                "fault {fault}"
            );
        }
    }
    #[test]
    fn diagnostic_recovery_settlement_requires_typed_exit_and_retains_faults() {
        let mut held =
            RecoveryCustody::new(OriginalRootControl::start(Cursor::new(b"{}".to_vec())));
        held.state = RecoveryState::Signalling;
        held.signals.insert(42);
        assert!(!held.settle(Ok(Ok(()))));
        assert!(held.receipt.is_none());
        assert!(!held.settle(Ok(Err(anyhow::anyhow!("original final proof failed")))));
        assert!(held.original_error.is_some());
        assert!(matches!(held.state, RecoveryState::Unknown));
        assert!(held.signals.contains(&42));
        assert!(held.control.original_thread.is_some());
        assert!(!held.settle(Err(Box::new("original final panic"))));
        assert!(held.original_panic.is_some());
        assert!(held.signals.contains(&42));
    }
    #[test]
    fn diagnostic_pending_shutdown_expiry_keeps_pending_no_reopen() {
        let row = CensusIdentity {
            pid: 42,
            parent: 41,
            birth_filetime: 1,
            executable: PathBuf::from("F:/rmngr.exe"),
            command: "original".into(),
        };
        let mut held = PendingShutdownHandle::new(row);
        assert!(held.bind_once(Instant::now()).is_err());
        assert!(held.original_error.is_some());
        assert_eq!(held.row.pid, 42);
        assert!(
            held.bind_once(Instant::now() + Duration::from_secs(5))
                .is_err()
        );
    }
    fn graph_fixture() -> (Vec<CensusIdentity>, LifetimeBinding, PathBuf, PathBuf) {
        let platform = PathBuf::from("F:/platform");
        let console = PathBuf::from("C:/Windows/System32/conhost.exe");
        let rows = vec![
            CensusIdentity {
                pid: 10,
                parent: 1,
                birth_filetime: 100,
                executable: platform.join("ragent.exe"),
                command: "owned agent".into(),
            },
            CensusIdentity {
                pid: 11,
                parent: 1,
                birth_filetime: 100,
                executable: platform.join("ras.exe"),
                command: "owned ras".into(),
            },
            CensusIdentity {
                pid: 12,
                parent: 10,
                birth_filetime: 200,
                executable: platform.join("rmngr.exe"),
                command: "owned manager".into(),
            },
        ];
        let identity = |row: &CensusIdentity| ProcessIdentity {
            pid: row.pid,
            parent: row.parent,
            birth_100ns: row.birth_filetime + 6,
            executable: row.executable.clone(),
            command_sha256: format!("{:X}", Sha256::digest(row.command.as_bytes())),
        };
        let binding = LifetimeBinding {
            nonce: Uuid::from_u128(1),
            root: PathBuf::from("F:/private"),
            cluster: Uuid::from_u128(2),
            infobase: Uuid::nil(),
            database: "diagnostic_no_infobase".into(),
            agent: identity(&rows[0]),
            ras: identity(&rows[1]),
        };
        (rows, binding, platform, console)
    }
    #[test]
    fn diagnostic_recovery_exact_graph_full_identity_and_foreign_failures() {
        let (rows, binding, platform, console) = graph_fixture();
        let refs: Vec<_> = rows.iter().collect();
        require_shutdown_graph(&refs, &refs, &platform, &console, &binding).unwrap();
        for fault in 0..6 {
            let mut second = rows.clone();
            match fault {
                0 => second[2].command.push('x'),
                1 => second[2].parent = 999,
                2 => second[2].executable = platform.join("foreign.exe"),
                3 => second[2].birth_filetime = 50,
                4 => second[0].pid = 99,
                _ => {
                    second.pop();
                }
            }
            let after: Vec<_> = second.iter().collect();
            assert!(
                require_shutdown_graph(&refs, &after, &platform, &console, &binding).is_err(),
                "fault {fault}"
            );
            if (1..=4).contains(&fault) {
                assert!(
                    require_shutdown_graph(&after, &after, &platform, &console, &binding).is_err(),
                    "stable invalid graph {fault}"
                );
            }
        }
    }
    #[test]
    fn diagnostic_recovery_request_closed_fields_exact_binding_no_authority() {
        let (_, binding, _, _) = graph_fixture();
        let nonce = "11111111-1111-4111-8111-111111111111";
        let sha = "A".repeat(64);
        let json = serde_json::json!({"protocol":"native-auth409/recovery-request-v1","action":"stop-original-diagnostic","request_nonce":nonce,"request_sha256":sha,"root":binding.root,"lifetime_nonce":binding.nonce.to_string(),"cluster":binding.cluster.to_string(),"observations_sha256":"B".repeat(64),"recovery_nonce":"22222222-2222-4222-8222-222222222222"});
        let good: RecoveryRequest = serde_json::from_value(json.clone()).unwrap();
        good.require_identity(nonce, &sha, &binding).unwrap();
        for field in [
            "protocol",
            "action",
            "request_nonce",
            "request_sha256",
            "root",
            "lifetime_nonce",
            "cluster",
            "observations_sha256",
            "recovery_nonce",
        ] {
            let mut changed = json.clone();
            changed[field] = serde_json::json!("wrong");
            let read = serde_json::from_value::<RecoveryRequest>(changed).unwrap();
            assert!(
                read.require_identity(nonce, &sha, &binding).is_err(),
                "{field}"
            );
        }
        let mut extra = json;
        extra["authenticated_boundary"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RecoveryRequest>(extra).is_err());
    }
}
