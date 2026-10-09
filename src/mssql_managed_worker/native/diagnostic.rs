//! ROOT-only ignored native experiment. No admission, replay, cleanup or Ready.
use super::*;
use authentication::{DiagnosticMeasurements, Receipt};
use std::any::Any;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
mod recovery;

// Exact diagnostic runtime/custody module family and its current compile
// selection. Full repository inheritance is separately closed by the freeze.
const REQUEST_SOURCE_ROLES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "src/mssql_managed_worker.rs",
    "src/mssql_platform_profile.rs",
    "src/profile_process.rs",
    "src/rac_process.rs",
    "src/mssql_managed_worker/child.rs",
    "src/mssql_managed_worker/child/diagnostic.rs",
    "src/mssql_managed_worker/command.rs",
    "src/mssql_managed_worker/identity.rs",
    "src/mssql_managed_worker/native.rs",
    "src/mssql_managed_worker/native/authentication.rs",
    "src/mssql_managed_worker/native/authentication/tests.rs",
    "src/mssql_managed_worker/native/census_observer_tests.rs",
    "src/mssql_managed_worker/native/diagnostic.rs",
    "src/mssql_managed_worker/native/diagnostic/recovery.rs",
    "src/mssql_managed_worker/native/diagnostic/recovery/current.rs",
    "src/mssql_managed_worker/cli.rs",
    "src/mssql_managed_worker/cli/tests.rs",
    "src/mssql_managed_worker/tests.rs",
    "src/mssql_managed_worker/undo.rs",
    "src/mssql_managed_worker/undo/tests.rs",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedFile {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootRequest {
    protocol: String,
    request_nonce: String,
    parent: PathBuf,
    platform_bin: PathBuf,
    powershell: PathBuf,
    agent_port: u16,
    cluster_port: u16,
    ras_port: u16,
    worker_first: u16,
    worker_last: u16,
    startup_seconds: u64,
    measurement_seconds: u64,
    recovery_seconds: u64,
    experiment_binary: SelectedFile,
    source_files: Vec<SelectedFile>,
    tools: Vec<SelectedFile>,
}

/// Retains selected original files, including request, before runtime effects.
struct RootSelection {
    request: RootRequest,
    originals: Vec<Tool>,
}

impl RootSelection {
    fn load(path: &Path, expected: &str) -> Result<Self> {
        require_digest(expected)?;
        if fs::metadata(path)?.len() > 64 * 1024 {
            bail!("bounded diagnostic request required");
        }
        let request_tool = Tool::pin(path.to_owned())?;
        if request_tool.digest != expected {
            bail!("ROOT diagnostic request pin mismatch");
        }
        let bytes = fs::read(path)?;
        if bytes.len() > 64 * 1024 {
            bail!("bounded diagnostic request required");
        }
        let request: RootRequest = serde_json::from_slice(&bytes)?;
        request_tool.check()?;
        request.validate_shape()?;
        let mut selected = Self {
            request,
            originals: vec![request_tool],
        };
        let binary = std::env::current_exe()?;
        if fs::canonicalize(&binary)? != fs::canonicalize(&selected.request.experiment_binary.path)?
        {
            bail!("exact ROOT-selected experiment binary required");
        }
        let files = std::iter::once(&selected.request.experiment_binary)
            .chain(selected.request.source_files.iter())
            .chain(selected.request.tools.iter());
        let mut seen = BTreeSet::new();
        for file in files {
            require_digest(&file.sha256)?;
            if !seen.insert(file.path.clone()) || file.bytes == 0 {
                bail!("distinct finite ROOT diagnostic inputs required");
            }
            let tool = Tool::pin(file.path.clone())?;
            if tool.digest != file.sha256 || fs::metadata(&file.path)?.len() != file.bytes {
                bail!("ROOT diagnostic input changed");
            }
            selected.originals.push(tool);
        }
        selected.require_current()?;
        Ok(selected)
    }
    fn require_current(&self) -> Result<()> {
        for tool in &self.originals {
            tool.check()?;
        }
        Ok(())
    }
}

fn require_digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    {
        bail!("uppercase exact SHA256 required");
    }
    Ok(())
}

impl RootRequest {
    fn validate_shape(&self) -> Result<()> {
        let nonce =
            Uuid::parse_str(&self.request_nonce).context("canonical ROOT request UUID required")?;
        if self.protocol != "native-auth409/current-observer-diagnostic-request-v3"
            || nonce.is_nil()
            || nonce.to_string() != self.request_nonce
            || self.startup_seconds != 120
            || self.measurement_seconds != 600
            || self.recovery_seconds != 120
            || self.source_files.len() < 8
            || self.source_files.len() > 128
            || self.tools.len() != 7
            || !self.parent.is_absolute()
            || !self.platform_bin.is_absolute()
            || !self.powershell.is_absolute()
            || !self
                .parent
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("f:\\")
        {
            bail!("ROOT diagnostic fixed policy/finite binding invalid");
        }
        let tools: BTreeSet<_> = self.tools.iter().map(|f| f.path.clone()).collect();
        let expected: BTreeSet<_> = [
            "rac.exe",
            "ragent.exe",
            "ras.exe",
            "rmngr.exe",
            "rphost.exe",
            "dbda.exe",
        ]
        .into_iter()
        .map(|name| self.platform_bin.join(name))
        .chain(std::iter::once(self.powershell.clone()))
        .collect();
        if tools != expected {
            bail!("exact platform6 and powershell tools required");
        }
        let names: BTreeSet<_> = self
            .source_files
            .iter()
            .filter_map(|p| p.path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        if ![
            "native.rs",
            "authentication.rs",
            "diagnostic.rs",
            "command.rs",
            "child.rs",
            "recovery.rs",
        ]
        .into_iter()
        .all(|name| names.contains(name))
        {
            bail!("actual producer/consumer/original custody Source pins required");
        }
        let roots: Vec<_> = self
            .source_files
            .iter()
            .filter(|f| f.path.file_name().is_some_and(|n| n == "native.rs"))
            .collect();
        if roots.len() != 1 {
            bail!("one actual current native Source root required");
        }
        let worker = roots[0]
            .path
            .parent()
            .context("current worker Source parent absent")?;
        let repository = worker
            .parent()
            .and_then(Path::parent)
            .context("current repository Source parent absent")?;
        let selected: BTreeSet<_> = self.source_files.iter().map(|f| f.path.clone()).collect();
        let expected: BTreeSet<_> = REQUEST_SOURCE_ROLES
            .iter()
            .map(|role| repository.join(role))
            .collect();
        if !repository.is_absolute()
            || self.source_files.len() != REQUEST_SOURCE_ROLES.len()
            || selected != expected
        {
            bail!("exact full current diagnostic Source family required; arbitrary min8 forbidden");
        }
        Ok(())
    }
    fn options(&self) -> CreatorOptions {
        CreatorOptions {
            parent: self.parent.clone(),
            platform_bin: self.platform_bin.clone(),
            powershell: self.powershell.clone(),
            agent_port: self.agent_port,
            cluster_port: self.cluster_port,
            ras_port: self.ras_port,
            worker_first: self.worker_first,
            worker_last: self.worker_last,
            // This diagnostic never issues IB/SQL operations or reads secrets
            // from environment/config. These inert fields satisfy allocation.
            database_server: "localhost".into(),
            database: "diagnostic_no_infobase".into(),
            database_user: String::new(),
            database_password: String::new(),
            infobase_user: String::new(),
            infobase_password: String::new(),
            timeout: Duration::from_secs(120),
        }
    }
}

/// A distinct phase, not renewal of any consumed command or positive creator.
struct DiagnosticTimeline {
    startup: Instant,
    outer: Instant,
    measurement: Option<Instant>,
    request_until: Instant,
    recovery: Option<Instant>,
}
impl DiagnosticTimeline {
    fn select(now: Instant) -> Result<Self> {
        Ok(Self {
            startup: now
                .checked_add(Duration::from_secs(120))
                .context("startup overflow")?,
            outer: now
                .checked_add(Duration::from_secs(840))
                .context("diagnostic outer overflow")?,
            request_until: now
                .checked_add(Duration::from_secs(720))
                .context("diagnostic request overflow")?,
            measurement: None,
            recovery: None,
        })
    }
    fn enter_measurement(&mut self, now: Instant) -> Result<Instant> {
        if self.measurement.is_some() || now >= self.startup {
            bail!("diagnostic phase already issued or startup expired");
        }
        let deadline = now
            .checked_add(Duration::from_secs(600))
            .context("measurement overflow")?
            .min(self.request_until);
        self.measurement = Some(deadline);
        Ok(deadline)
    }
    fn enter_recovery(&mut self, received: Instant) -> Result<Instant> {
        if self.recovery.is_some() || received >= self.request_until {
            bail!("diagnostic recovery slot consumed or request late");
        }
        let deadline = received
            .checked_add(Duration::from_secs(120))
            .context("recovery overflow")?
            .min(self.outer);
        self.recovery = Some(deadline);
        Ok(deadline)
    }
}

pub(super) struct RawMutation {
    family: AdminFamily,
    kind: AdminCredentials,
    action: AdminAction,
    credentials: AdminCredentials,
    pub(super) original_index: Option<usize>,
    pub(super) raw: Option<Receipt>,
    pub(super) known_direct_both: bool,
    pub(super) unproved: bool,
}
impl RawMutation {
    pub(super) fn pending(
        family: AdminFamily,
        kind: AdminCredentials,
        action: AdminAction,
        credentials: AdminCredentials,
    ) -> Self {
        Self {
            family,
            kind,
            action,
            credentials,
            original_index: None,
            raw: None,
            known_direct_both: false,
            unproved: false,
        }
    }
}

enum DiagnosticState {
    Selected,
    Running,
    Complete,
    Refused,
    Panicked,
}

/// No into_ready/ManagedWorker/cold/cleanup API. The ROOT caller owns this
/// entire object through measurement, failure, publication and recovery.
struct RetainedAuthenticationDiagnostic {
    selection: RootSelection,
    timeline: DiagnosticTimeline,
    runtime: NativeRuntime,
    journal: Journal,
    mutations: Vec<RawMutation>,
    measurements: DiagnosticMeasurements,
    wrong_password: String,
    throwaway_password: String,
    state: DiagnosticState,
    original_error: Option<anyhow::Error>,
    original_panic: Option<Box<dyn Any + Send>>,
    evidence_files: Vec<fs::File>,
    publication_clock_proved: bool,
    checkpoint: recovery::CheckpointAttempt,
    recovery: recovery::RecoveryCustody,
}

fn allocate_runtime(
    options: CreatorOptions,
    deadline: Instant,
) -> Result<(NativeRuntime, Journal)> {
    super::super::child::require_deadline(deadline)?;
    if !cfg!(windows)
        || options.timeout.is_zero()
        || options.timeout > Duration::from_secs(120)
        || options.worker_first == 0
        || options.worker_first > options.worker_last
        || options.worker_last - options.worker_first > 127
        || options.database.is_empty()
        || options.database_server.is_empty()
    {
        bail!("managed creator platform, deadline or target invalid");
    }
    let ports = [options.agent_port, options.cluster_port, options.ras_port];
    if ports.contains(&0)
        || BTreeSet::from(ports).len() != 3
        || ports
            .iter()
            .any(|p| (options.worker_first..=options.worker_last).contains(p))
    {
        bail!("managed private port families must be disjoint");
    }
    ordinary(&options.parent)?;
    let nonce = Uuid::new_v4();
    let root = options.parent.join(format!("managed-{nonce}"));
    // No caller may provide a previously existing lifetime root.
    fs::create_dir(&root)?;
    ordinary(&root)?;
    fs::create_dir(root.join("srvinfo"))?;
    let journal = Journal::create(&root.join("lifetime.jsonl"), nonce)?;
    let rac = Tool::pin(options.platform_bin.join("rac.exe"))?;
    let pwsh = Tool::pin(options.powershell.clone())?;
    // SystemDirectory comes from the OS API, never a caller path or mutable
    // SystemRoot/PATH environment. This Tool confers shutdown-only authority.
    let console_tool = Tool::pin(system_console_image()?)?;
    let anchor_tools = [
        "ragent.exe",
        "ras.exe",
        "rmngr.exe",
        "rphost.exe",
        "dbda.exe",
    ]
    .into_iter()
    .map(|name| Tool::pin(options.platform_bin.join(name)))
    .collect::<Result<Vec<_>>>()?;
    let empty_identity = ProcessIdentity {
        pid: 0,
        parent: 0,
        birth_100ns: 0,
        executable: PathBuf::new(),
        command_sha256: String::new(),
    };
    let binding = LifetimeBinding {
        nonce,
        root,
        cluster: Uuid::nil(),
        infobase: Uuid::nil(),
        database: options.database.clone(),
        agent: empty_identity.clone(),
        ras: empty_identity,
    };
    let runtime = NativeRuntime {
        options,
        rac,
        pwsh,
        agent: None,
        ras: None,
        collectors: Vec::new(),
        census_observers: Vec::new(),
        binding,
        administrator: format!("ibcmd_{nonce}"),
        password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
        barrier_filetime: 0,
        admitted_load: BTreeSet::new(),
        workers: BTreeMap::new(),
        failed: false,
        authenticated_boundary: false,
        startup_deadline: None,
        #[cfg(test)]
        diagnostic_recovery_deadline: None,
        anchor_tools,
        console_tool,
        shutdown_handles: BTreeMap::new(),
        cold: false,
    };
    super::super::child::require_deadline(deadline)?;
    Ok((runtime, journal))
}

impl RetainedAuthenticationDiagnostic {
    fn allocate(selection: RootSelection, timeline: DiagnosticTimeline) -> Result<Self> {
        selection.require_current()?;
        // Fixed timeline was selected before request pins/IO. No actor starts
        // before this whole resident owner is built. Retain control failures.
        let (mut runtime, journal) =
            allocate_runtime(selection.request.options(), timeline.startup)?;
        let control = recovery::OriginalRootControl::start(std::io::stdin());
        let control_error = control.require_started().err();
        runtime.failed = control_error.is_some();
        Ok(Self {
            selection,
            timeline,
            runtime,
            journal,
            mutations: vec![],
            measurements: DiagnosticMeasurements::default(),
            wrong_password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
            throwaway_password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
            state: if control_error.is_some() {
                DiagnosticState::Refused
            } else {
                DiagnosticState::Selected
            },
            original_error: control_error,
            original_panic: None,
            evidence_files: vec![],
            publication_clock_proved: false,
            checkpoint: recovery::CheckpointAttempt::default(),
            recovery: recovery::RecoveryCustody::new(control),
        })
    }
    fn run_once(&mut self) {
        if !matches!(self.state, DiagnosticState::Selected) {
            return;
        }
        self.state = DiagnosticState::Running;
        let outcome = catch_unwind(AssertUnwindSafe(|| self.run_inner()));
        match outcome {
            Ok(Ok(())) => self.state = DiagnosticState::Complete,
            Ok(Err(error)) => {
                self.runtime.failed = true;
                self.original_error = Some(error);
                self.state = DiagnosticState::Refused;
            }
            Err(panic) => {
                self.runtime.failed = true;
                self.original_panic = Some(panic);
                self.state = DiagnosticState::Panicked;
            }
        }
    }
    fn run_inner(&mut self) -> Result<()> {
        self.selection.require_current()?;
        super::super::child::require_deadline(self.timeline.startup)?;
        self.journal.append(
            "diagnostic_scope",
            serde_json::json!({
            "request_nonce": self.selection.request.request_nonce.to_string(),
            "startup_seconds":120, "measurement_seconds":600, "Ready":false,
            "admission":false, "automatic_cleanup":false, "IB_SQL_commands":0 }),
        )?;
        self.runtime
            .start_private_lifetime(&mut self.journal, self.timeline.startup)?;
        self.runtime.startup_remaining()?;
        self.selection.require_current()?;
        // Capture actual returned live kernel identities on originals BEFORE
        // administrator mutations; no reopen after a failure or later exit.
        self.prepare_original_recovery_handles(self.timeline.startup)?;
        let deadline = self.timeline.enter_measurement(Instant::now())?;
        self.runtime.startup_deadline = Some(deadline);
        self.journal.append(
            "diagnostic_measurement_phase",
            serde_json::json!({
            "once":true, "maximum_seconds":600, "positive_creator_budget_changed":false }),
        )?;
        self.runtime.startup_remaining()?;
        self.runtime
            .establish_password_administrators(&mut self.journal)?;
        let plan =
            authentication::Plan::new(&self.runtime.administrator, self.runtime.binding.cluster)?;
        let mut io = NativeAdminIo {
            runtime: &mut self.runtime,
            journal: &mut self.journal,
            plan: &plan,
            wrong_password: self.wrong_password.clone(),
            throwaway_password: self.throwaway_password.clone(),
            captures: Some(&mut self.mutations),
        };
        authentication::measure(&mut io, &plan, &mut self.measurements)?;
        self.runtime.startup_remaining()?;
        self.selection.require_current()?;
        self.runtime.require_private_endpoint()?;
        self.runtime.startup_remaining()?;
        if self.runtime.authenticated_boundary
            || self.measurements.controls.len() != 2
            || self.measurements.challenges.len() != 4
            || self.mutations.len() != 8
            || self.runtime.collectors.iter().any(|c| !c.terminal_proved())
            || self.mutations.iter().any(|m| {
                m.unproved
                    || !m.known_direct_both
                    || m.raw.is_none()
                    || !m
                        .original_index
                        .and_then(|i| self.runtime.collectors.get(i))
                        .is_some_and(OriginalChild::terminal_proved)
            })
        {
            bail!("complete original diagnostic evidence required without authority");
        }
        self.journal.append(
            "diagnostic_measurements_complete",
            serde_json::json!({
            "controls":2, "challenges":4, "original_mutations":8, "Ready":false,
            "original_collectors":self.runtime.collectors.len(), "admission":false }),
        )?;
        self.runtime.startup_remaining()?;
        Ok(())
    }
    fn publish_once(&mut self) {
        let outcome = catch_unwind(AssertUnwindSafe(|| self.publish_inner()));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                if self.original_error.is_none() {
                    self.original_error = Some(error);
                }
                self.runtime.failed = true;
                self.state = DiagnosticState::Refused;
            }
            Err(panic) => {
                if self.original_panic.is_none() {
                    self.original_panic = Some(panic);
                }
                self.runtime.failed = true;
                self.state = DiagnosticState::Panicked;
            }
        }
    }
    fn publish_inner(&mut self) -> Result<()> {
        let deadline = self
            .timeline
            .measurement
            .context("measurement not started; retain startup custody")?;
        super::super::child::require_deadline(deadline)?;
        // Publish raw bytes only when none contain any generated secret in the
        // original ASCII/UTF16 representations. Original raw always stays held
        // in memory/readers, including refused output and partial outcomes.
        for m in &self.mutations {
            if let Some(raw) = &m.raw {
                for bytes in [&raw.stdout, &raw.stderr] {
                    if [
                        &self.runtime.password,
                        &self.wrong_password,
                        &self.throwaway_password,
                    ]
                    .into_iter()
                    .any(|secret| contains_secret(bytes, secret))
                    {
                        bail!("native diagnostic raw secret detected; memory custody retained");
                    }
                }
            }
        }
        let mut raw_records = vec![];
        for index in 0..self.mutations.len() {
            let m = &self.mutations[index];
            let summary = serde_json::json!({
                "family":m.family, "kind":m.kind, "credentials":m.credentials,
                "action":match m.action { AdminAction::Register=>"register", AdminAction::Remove=>"remove" },
                "original_index":m.original_index,
                "pid":m.original_index.and_then(|i|self.runtime.collectors.get(i)).map(OriginalChild::pid),
                "direct_BOTH":m.known_direct_both, "unproved":m.unproved,
                "exit":m.raw.as_ref().map(|r|r.exit),
                "stdout_sha256":m.raw.as_ref().map(|r|format!("{:X}",Sha256::digest(&r.stdout))),
                "stderr_sha256":m.raw.as_ref().map(|r|format!("{:X}",Sha256::digest(&r.stderr))) });
            let raw = m.raw.as_ref().map(|r| (r.stdout.clone(), r.stderr.clone()));
            if let Some((stdout, stderr)) = raw {
                self.write_original_evidence(&format!("mutation-{index}-stdout.raw"), &stdout)?;
                self.write_original_evidence(&format!("mutation-{index}-stderr.raw"), &stderr)?;
            }
            raw_records.push(summary);
            super::super::child::require_deadline(deadline)?;
        }
        let challenges: Vec<_> = self.measurements.challenges.iter().map(|w|serde_json::json!({
            "family":w.family,"kind":w.kind,"exit":w.exit,"before":w.before,"after":w.after,"mutated":w.mutated,
            "stdout_bytes":w.stdout.len(),"stderr_bytes":w.stderr.len(),
            "stdout_sha256":format!("{:X}",Sha256::digest(&w.stdout)),
            "stderr_sha256":format!("{:X}",Sha256::digest(&w.stderr)) })).collect();
        let controls: Vec<_> = self
            .measurements
            .controls
            .iter()
            .map(|c| {
                serde_json::json!({
            "family":c.family,"before":c.before,"present":c.present,"after":c.after })
            })
            .collect();
        let originals: Vec<_> = self.runtime.collectors.iter().enumerate().map(|(index,c)|
            serde_json::json!({"index":index,"pid":c.pid(),"direct_BOTH":c.terminal_proved()})).collect();
        let report = serde_json::to_vec(&serde_json::json!({
            "protocol":"native-auth409/retained-diagnostic-observations-v1",
            "request_nonce":self.selection.request.request_nonce.to_string(),
            "binding":{"nonce":self.runtime.binding.nonce.to_string(),
                "root":self.runtime.binding.root,"cluster":self.runtime.binding.cluster.to_string(),
                "infobase":self.runtime.binding.infobase.to_string(),
                "agent":self.runtime.binding.agent,"ras":self.runtime.binding.ras},
            "controls":controls,"challenges":challenges,
            "retained_original_collectors":originals,
            "mutations":raw_records,"complete_diagnostic":matches!(self.state,DiagnosticState::Complete),
            "Ready":false,"authenticated_boundary":self.runtime.authenticated_boundary,
            "resident_custody":true,"record_is_provisional_until_resident_checkpoint":true,
            "whole_helper_terminal_proved":false,"cleanup_proved":false }))?;
        self.write_original_evidence("diagnostic-observations.json", &report)?;
        self.selection.require_current()?;
        super::super::child::require_deadline(deadline)?;
        // Separate completion marker after all writes/rehash/last clock. If
        // anything fails, unsealed observations are not accepted; no retry.
        self.write_original_evidence(
            "diagnostic-publication-final.json",
            br#"{"provisional_publication_written":true,"Ready":false,"cleanup_proved":false}"#,
        )?;
        super::super::child::require_deadline(deadline)?;
        self.publication_clock_proved = true;
        Ok(())
    }
    fn write_original_evidence(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        self.write_original_evidence_at(
            name,
            bytes,
            self.timeline
                .measurement
                .context("original publication deadline absent")?,
        )
    }
    fn write_original_evidence_at(
        &mut self,
        name: &str,
        bytes: &[u8],
        deadline: Instant,
    ) -> Result<()> {
        super::super::child::require_deadline(deadline)?;
        let path = self.runtime.binding.root.join(name);
        ordinary(&self.runtime.binding.root)?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        super::super::child::require_deadline(deadline)?;
        self.evidence_files.push(options.open(path)?);
        super::super::child::require_deadline(deadline)?;
        let original = self
            .evidence_files
            .last_mut()
            .context("original evidence file absent")?;
        original.write_all(bytes)?;
        super::super::child::require_deadline(deadline)?;
        original.sync_all()?;
        super::super::child::require_deadline(deadline)?;
        Ok(())
    }
    fn retain_resident(self) -> ! {
        // The ignored entry's sole owner must not unwind/return/drop after
        // effects. No Drop cleanup, reader wait retry, signals or lease release.
        let _same_original_owner = self;
        loop {
            std::thread::park();
        }
    }
}

fn contains_secret(bytes: &[u8], secret: &str) -> bool {
    let utf16: Vec<u16> = secret.encode_utf16().collect();
    let le: Vec<u8> = utf16.iter().flat_map(|n| n.to_le_bytes()).collect();
    let be: Vec<u8> = utf16.iter().flat_map(|n| n.to_be_bytes()).collect();
    [secret.as_bytes(), &le, &be].into_iter().any(|pattern| {
        !pattern.is_empty() && bytes.windows(pattern.len()).any(|part| part == pattern)
    })
}

#[test]
#[ignore = "ROOT-only actual native experiment; retains resident custody, no automatic cleanup"]
fn root_selected_native_authentication_diagnostic() -> Result<()> {
    let timeline = DiagnosticTimeline::select(Instant::now())?;
    let path = std::env::var_os("IBCMD_NATIVE_AUTH409_ROOT_REQUEST")
        .context("ROOT request required; no effects")?;
    let sha = std::env::var("IBCMD_NATIVE_AUTH409_ROOT_REQUEST_SHA256")
        .context("ROOT request SHA required; no effects")?;
    let selection = RootSelection::load(Path::new(&path), &sha)?;
    super::super::child::require_deadline(timeline.startup)?;
    let mut owner = RetainedAuthenticationDiagnostic::allocate(selection, timeline)?;
    // The entire post-allocation caller is protected: no JSON/error/panic path
    // can unwind through the sole original owner. Run with --test-threads=1.
    let caller = catch_unwind(AssertUnwindSafe(|| {
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        owner.run_once();
        owner.publish_once();
        let checkpoint = serde_json::json!({"protocol":"native-auth409/resident-custody-v2",
            "root":owner.runtime.binding.root,"request_nonce":owner.selection.request.request_nonce.to_string(),
            "controls":owner.measurements.controls.len(),"challenges":owner.measurements.challenges.len(),
            "provisional":true,
            "complete_diagnostic":matches!(owner.state,DiagnosticState::Complete),
            "Ready":false,"known_helper_terminal":false,"cleanup_proved":false});
        let mut out = std::io::stdout().lock();
        let deadline = owner.timeline.measurement;
        owner.checkpoint.run(
            &mut out,
            || super::super::child::require_deadline(deadline.context("measurement absent")?),
            &checkpoint,
        );
        if !owner.checkpoint.proved() {
            owner.runtime.failed = true;
            owner.publication_clock_proved = false;
            owner.state = DiagnosticState::Refused;
        }
        std::panic::set_hook(old_hook);
    }));
    if let Err(panic) = caller {
        owner.runtime.failed = true;
        owner.publication_clock_proved = false;
        if owner.original_panic.is_none() {
            owner.original_panic = Some(panic);
        }
        owner.state = DiagnosticState::Panicked;
    }
    if owner.serve_root_recovery_once() {
        // All original resources exited, BOTH captured, final durable proof and
        // final same clock. No Ready/ColdSession or lease/undo authority.
        return Ok(());
    }
    owner.retain_resident()
}

#[test]
fn diagnostic_phase_policy_is_once_fixed_and_does_not_renew() {
    let now = Instant::now();
    let mut t = DiagnosticTimeline::select(now).unwrap();
    assert_eq!(t.startup.duration_since(now), Duration::from_secs(120));
    assert_eq!(t.outer.duration_since(now), Duration::from_secs(840));
    let deadline = t.enter_measurement(now + Duration::from_secs(20)).unwrap();
    assert_eq!(deadline.duration_since(now), Duration::from_secs(620));
    assert!(t.enter_measurement(now + Duration::from_secs(30)).is_err());
    let mut late = DiagnosticTimeline::select(now).unwrap();
    assert!(
        late.enter_measurement(now + Duration::from_secs(120))
            .is_err()
    );
    assert!(late.measurement.is_none());
}

#[test]
fn diagnostic_secret_outputs_remain_memory_only() {
    for raw in [
        b"xsecretx".to_vec(),
        "secret".encode_utf16().flat_map(u16::to_le_bytes).collect(),
        "secret".encode_utf16().flat_map(u16::to_be_bytes).collect(),
    ] {
        assert!(contains_secret(&raw, "secret"));
    }
    assert!(!contains_secret(&[0xff, 0xfe, 0x80], "secret"));
}

#[test]
fn diagnostic_pending_slot_has_no_original_or_known_outcome() {
    let held = RawMutation::pending(
        AdminFamily::Agent,
        AdminCredentials::WrongPassword,
        AdminAction::Register,
        AdminCredentials::WrongPassword,
    );
    assert!(held.original_index.is_none());
    assert!(held.raw.is_none());
    assert!(!held.known_direct_both);
}

#[test]
fn diagnostic_request_rejects_nonexact_digest_without_effects() {
    assert!(require_digest(&"A".repeat(64)).is_ok());
    for bad in [
        "A".repeat(63),
        "A".repeat(65),
        "a".repeat(64),
        "G".repeat(64),
    ] {
        assert!(require_digest(&bad).is_err());
    }
}

#[cfg(windows)]
fn request_fixture() -> RootRequest {
    let repository = PathBuf::from("F:/source");
    let selected = |path: PathBuf| SelectedFile {
        path,
        bytes: 1,
        sha256: "A".repeat(64),
    };
    let platform = PathBuf::from("F:/platform");
    let powershell = PathBuf::from("C:/tools/pwsh.exe");
    RootRequest {
        protocol: "native-auth409/current-observer-diagnostic-request-v3".into(),
        request_nonce: "11111111-1111-4111-8111-111111111111".into(),
        parent: PathBuf::from("F:\\fresh"),
        platform_bin: platform.clone(),
        powershell: powershell.clone(),
        agent_port: 2541,
        cluster_port: 2542,
        ras_port: 2543,
        worker_first: 2550,
        worker_last: 2553,
        startup_seconds: 120,
        measurement_seconds: 600,
        recovery_seconds: 120,
        experiment_binary: selected(PathBuf::from("F:/candidate/test.exe")),
        source_files: REQUEST_SOURCE_ROLES
            .iter()
            .map(|r| selected(repository.join(r)))
            .collect(),
        tools: [
            "rac.exe",
            "ragent.exe",
            "ras.exe",
            "rmngr.exe",
            "rphost.exe",
            "dbda.exe",
        ]
        .into_iter()
        .map(|r| selected(platform.join(r)))
        .chain(std::iter::once(selected(powershell)))
        .collect(),
    }
}
#[test]
#[cfg(windows)]
fn diagnostic_current_request_requires_both_actual_nested_source_roles() {
    request_fixture().validate_shape().unwrap();
    for role in [
        "native/diagnostic/recovery/current.rs",
        "child/diagnostic.rs",
    ] {
        let mut request = request_fixture();
        let worker = PathBuf::from("F:/source/src/mssql_managed_worker");
        request
            .source_files
            .iter_mut()
            .find(|f| f.path == worker.join(role))
            .unwrap()
            .path = PathBuf::from("F:/foreign").join(role);
        assert!(request.validate_shape().is_err());
    }
}
#[test]
#[cfg(windows)]
fn diagnostic_current_request_refuses_old_protocol_late_policy_and_wrong_tools() {
    for fault in 0..5 {
        let mut request = request_fixture();
        match fault {
            0 => request.protocol = "native-auth409/retained-diagnostic-request-v2".into(),
            1 => request.startup_seconds += 1,
            2 => request.measurement_seconds += 1,
            3 => request.recovery_seconds += 1,
            _ => request.tools[0].path = PathBuf::from("F:/foreign/rac.exe"),
        };
        assert!(request.validate_shape().is_err());
    }
}
