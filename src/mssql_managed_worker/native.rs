//! Product creator: fresh private registry, retained original process handles
//! and ephemeral password-only administration. There is no endpoint-adoption
//! constructor and no authority-file reader.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::child::{CensusIdentity, KernelProcess, OriginalChild};
use super::{Journal, LifetimeBinding, ManagedWorker, Observation, OwnedRuntime, ProcessIdentity};
use crate::mssql_platform_profile::ManagedRacReadAuth;

mod authentication;
use authentication::{
    Action as AdminAction, Credentials as AdminCredentials, Family as AdminFamily,
};

pub(crate) struct CreatorOptions {
    pub parent: PathBuf,
    pub platform_bin: PathBuf,
    pub powershell: PathBuf,
    pub agent_port: u16,
    pub cluster_port: u16,
    pub ras_port: u16,
    pub worker_first: u16,
    pub worker_last: u16,
    pub database_server: String,
    pub database: String,
    pub database_user: String,
    pub database_password: String,
    pub infobase_user: String,
    pub infobase_password: String,
    pub timeout: Duration,
}

#[derive(Clone)]
struct Tool {
    path: PathBuf,
    digest: String,
    // Windows denies executable writes/deletion while this lifetime is held.
    _original: Arc<fs::File>,
}

impl Tool {
    fn pin(path: PathBuf) -> Result<Self> {
        ordinary(&path)?;
        let mut open = fs::OpenOptions::new();
        open.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            open.share_mode(1); // FILE_SHARE_READ only
        }
        let original = Arc::new(open.open(&path)?);
        let digest = tool_digest(&path)?;
        Ok(Self {
            path,
            digest,
            _original: original,
        })
    }
    fn check(&self) -> Result<()> {
        ordinary(&self.path)?;
        if tool_digest(&self.path)? != self.digest {
            bail!("managed executable drift");
        }
        Ok(())
    }
}

fn tool_digest(path: &Path) -> Result<String> {
    let file = fs::File::open(path)?;
    let expected = file.metadata()?.len();
    if expected == 0 || expected > 256 * 1024 * 1024 {
        bail!("managed executable byte bound");
    }
    let mut bytes = Vec::new();
    file.take(expected + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected {
        bail!("managed executable opened size drift");
    }
    Ok(format!("{:X}", Sha256::digest(&bytes)))
}

fn ordinary(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("managed canonical absolute path required");
    }
    for p in path.ancestors() {
        let m = fs::symlink_metadata(p)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                bail!("managed reparse ancestry refused");
            }
        }
        if m.file_type().is_symlink() {
            bail!("managed symlink ancestry refused");
        }
    }
    Ok(())
}

#[cfg(windows)]
fn system_console_image() -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buffer = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        bail!("OS SystemDirectory unproved");
    }
    let path = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length])).join("conhost.exe");
    ordinary(&path)?;
    Ok(path)
}

#[cfg(not(windows))]
fn system_console_image() -> Result<PathBuf> {
    bail!("Windows lifecycle console authority unavailable")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForeignProcessFact {
    pid: u32,
    parent: u32,
    birth_filetime: Option<u64>,
    executable_present: bool,
    executable_path_sha256: Option<String>,
    command_present: bool,
    command_sha256: Option<String>,
    private_root_marker: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShutdownCensus {
    owned: Vec<CensusIdentity>,
    foreign: Vec<ForeignProcessFact>,
    listeners: Vec<Listener>,
}

fn require_shutdown_census(census: &ShutdownCensus, seeds: &BTreeSet<u32>) -> Result<()> {
    let mut all = BTreeSet::new();
    if census.owned.len() + census.foreign.len() > 4096 || census.listeners.len() > 512 {
        bail!("bounded shutdown census required");
    }
    for row in &census.owned {
        if !all.insert(row.pid)
            || row.pid == 0
            || row.birth_filetime == 0
            || !row.executable.is_absolute()
            || row.command.is_empty()
            || row.command.len() > 32768
        {
            bail!("complete relevant shutdown identity required");
        }
    }
    for row in &census.foreign {
        let digest = |present: bool, value: &Option<String>| match value {
            Some(hash) => {
                present && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
            }
            None => !present,
        };
        if !all.insert(row.pid)
            || row.private_root_marker == Some(true)
            || row.private_root_marker.is_some() != row.command_present
            || !digest(row.command_present, &row.command_sha256)
            || !digest(row.executable_present, &row.executable_path_sha256)
            || seeds.contains(&row.pid)
            || seeds.contains(&row.parent)
            || (row.pid != 0 && row.birth_filetime == Some(0))
        {
            bail!("foreign observation cannot confer owned authority");
        }
    }
    let mut owned = seeds.clone();
    loop {
        let previous = owned.len();
        for row in &census.owned {
            if owned.contains(&row.parent) {
                owned.insert(row.pid);
            }
        }
        if previous == owned.len() {
            break;
        }
    }
    if census.owned.iter().any(|row| !owned.contains(&row.pid))
        || census
            .foreign
            .iter()
            .any(|row| owned.contains(&row.pid) || owned.contains(&row.parent))
        || census.listeners.iter().any(|listener| {
            !owned.contains(&listener.pid)
                || !census.owned.iter().any(|row| row.pid == listener.pid)
        })
    {
        bail!("unknown private ancestry or selected listener; no signal");
    }
    Ok(())
}

fn shutdown_census_script(root: &str, seeds: &str, ports: &str) -> String {
    r#"$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$all=@(Get-CimInstance Win32_Process -OperationTimeoutSec 10 | Select-Object -First 4097); if($all.Count -gt 4096){throw 'cold census bound'}
$listeners=@(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object {$_.LocalPort -in @(__PORTS__)} | Select-Object -First 513); if($listeners.Count -gt 512){throw 'cold listener bound'}
$ids=[Collections.Generic.HashSet[uint32]]::new(); foreach($id in @(__SEEDS__)){[void]$ids.Add([uint32]$id)}
do{$n=$ids.Count; foreach($p in $all){if($ids.Contains([uint32]$p.ParentProcessId)){[void]$ids.Add([uint32]$p.ProcessId)}}}while($n -ne $ids.Count)
$owned=@();$foreign=@()
foreach($p in $all){
 if(($p.CommandLine -and $p.CommandLine.Length -gt 32768) -or ($p.ExecutablePath -and $p.ExecutablePath.Length -gt 32768)){throw 'cold field bound'}
 $marker=if($p.CommandLine){$p.CommandLine.IndexOf('__ROOT__',[StringComparison]::OrdinalIgnoreCase) -ge 0}else{$null}
 $selected=$ids.Contains([uint32]$p.ProcessId) -or $marker -eq $true -or @($listeners|Where-Object{$_.OwningProcess -eq $p.ProcessId}).Count -gt 0
 if($selected){
  if(!$p.ExecutablePath -or !$p.CommandLine -or !$p.CreationDate){throw 'cold relevant complete identity absent'}
  $owned+=@{pid=[uint32]$p.ProcessId;parent=[uint32]$p.ParentProcessId;birth_filetime=[uint64]$p.CreationDate.ToFileTimeUtc();executable=$p.ExecutablePath;command=$p.CommandLine}
 }else{
  $foreign+=@{pid=[uint32]$p.ProcessId;parent=[uint32]$p.ParentProcessId;birth_filetime=$(if($p.CreationDate){[uint64]$p.CreationDate.ToFileTimeUtc()}else{$null});executable_present=([bool]$p.ExecutablePath);executable_path_sha256=$(if($p.ExecutablePath){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($p.ExecutablePath)))}else{$null});command_present=([bool]$p.CommandLine);command_sha256=$(if($p.CommandLine){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($p.CommandLine)))}else{$null});private_root_marker=$marker}
 }
}
$ls=@($listeners|ForEach-Object{@{port=[uint16]$_.LocalPort;pid=[uint32]$_.OwningProcess}})
ConvertTo-Json -InputObject @{owned=$owned;foreign=$foreign;listeners=$ls} -Depth 5 -Compress"#
    .replace("__ROOT__", root).replace("__SEEDS__", seeds).replace("__PORTS__", ports)
}

/// Even failed creation returns its original children/journal. The caller
/// cannot silently discard uncertain ownership or restart a second attempt.
pub(crate) enum Creation {
    Ready(ManagedWorker<NativeRuntime>),
    Retained {
        runtime: NativeRuntime,
        journal: Journal,
        diagnostic: String,
    },
}

pub(crate) struct NativeRuntime {
    options: CreatorOptions,
    rac: Tool,
    pwsh: Tool,
    agent: Option<OriginalChild>,
    ras: Option<OriginalChild>,
    collectors: Vec<OriginalChild>,
    binding: LifetimeBinding,
    administrator: String,
    password: String,
    barrier_filetime: u64,
    admitted_load: BTreeSet<Uuid>,
    workers: BTreeMap<Uuid, KernelProcess>,
    failed: bool,
    authenticated_boundary: bool,
    startup_deadline: Option<Instant>,
    anchor_tools: Vec<Tool>,
    console_tool: Tool,
    shutdown_handles: BTreeMap<u32, KernelProcess>,
    cold: bool,
}

pub(crate) fn create(options: CreatorOptions) -> Result<Creation> {
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
    let mut runtime = NativeRuntime {
        options,
        rac,
        pwsh,
        agent: None,
        ras: None,
        collectors: Vec::new(),
        binding,
        administrator: format!("ibcmd_{nonce}"),
        password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
        barrier_filetime: 0,
        admitted_load: BTreeSet::new(),
        workers: BTreeMap::new(),
        failed: false,
        authenticated_boundary: false,
        startup_deadline: None,
        anchor_tools,
        console_tool,
        shutdown_handles: BTreeMap::new(),
        cold: false,
    };
    let mut journal = journal;
    let result = (|| -> Result<()> {
        runtime.bootstrap(&mut journal)?;
        let o = runtime.observe()?;
        if o.binding != runtime.binding
            || !o.registrations.is_empty()
            || !o.loaded.is_empty()
            || o.worker.is_some()
            || !o.password_only_admins_exact
            || !o.authenticated_inventory_exact
            || !o.lease_original_handle_exact
            || o.unknown_administration
        {
            bail!("fresh managed authority barrier drift");
        }
        journal.append("creator_barrier", &runtime.binding.nonce.to_string())?;
        runtime.startup_remaining()?;
        runtime.startup_deadline = None;
        Ok(())
    })();
    match result {
        Ok(()) => {
            let binding = runtime.binding.clone();
            Ok(Creation::Ready(ManagedWorker {
                runtime,
                binding,
                journal,
                ever_loaded: BTreeSet::new(),
                registered: false,
                admitted_worker: None,
                tainted: false,
                stopped: false,
            }))
        }
        Err(error) => {
            runtime.failed = true;
            Ok(Creation::Retained {
                runtime,
                journal,
                diagnostic: format!("managed creation unproved: {}", error),
            })
        }
    }
}

impl OwnedRuntime for NativeRuntime {
    fn observe(&mut self) -> Result<Observation> {
        self.rac.check()?;
        self.pwsh.check()?;
        self.require_admins()?;
        if !self.authenticated_boundary {
            bail!("managed administration boundary is not independently proved");
        }
        let first = self.census()?;
        let second = self.census()?;
        for (anchor, expected) in [
            (&mut self.agent, &self.binding.agent),
            (&mut self.ras, &self.binding.ras),
        ] {
            let anchor = anchor.as_mut().context("original managed anchor absent")?;
            if !anchor.alive()? {
                bail!("original managed anchor exited");
            }
            for rows in [&first, &second] {
                let row = rows
                    .iter()
                    .find(|p| p.pid == anchor.pid())
                    .context("original anchor census missing")?;
                if &anchor.bind_census(row)? != expected {
                    bail!("managed anchor identity drift");
                }
            }
        }
        let owned_first = self.descendants(&first)?;
        let owned = self.descendants(&second)?;
        if owned_first.len() != owned.len() {
            bail!("managed ancestry changed during observation");
        }
        for p in &owned {
            let old = owned_first
                .iter()
                .find(|old| old.pid == p.pid)
                .context("managed ancestry process drift")?;
            if old.parent != p.parent
                || old.birth_filetime != p.birth_filetime
                || old.executable != p.executable
                || old.command != p.command
            {
                bail!("managed ancestry complete identity drift");
            }
        }
        let cluster = self.binding.cluster;
        let args = |what: &str| {
            vec![
                what.to_owned(),
                "list".into(),
                format!("--cluster={cluster}"),
            ]
        };
        let registrations = blocks(&self.rac(
            vec![
                "infobase".into(),
                "summary".into(),
                "list".into(),
                format!("--cluster={}", self.binding.cluster),
            ],
            false,
            true,
        )?)?
        .iter()
        .map(|r| {
            Uuid::parse_str(r.get("infobase").context("registration UUID missing")?)
                .map_err(Into::into)
        })
        .collect::<Result<BTreeSet<_>>>()?;
        let mut loaded = self.admitted_load.clone();
        for family in ["session", "connection"] {
            for r in blocks(&self.rac(args(family), false, true)?)? {
                loaded.insert(Uuid::parse_str(
                    r.get("infobase").context("loaded inventory UUID missing")?,
                )?);
            }
        }
        let processes = blocks(&self.rac(args("process"), false, true)?)?;
        if processes.len() > 1 {
            bail!("managed single working process required");
        }
        let worker = processes
            .first()
            .map(|r| -> Result<_> {
                let id = Uuid::parse_str(r.get("process").context("working UUID missing")?)?;
                let pid: u32 = r.get("pid").context("working PID missing")?.parse()?;
                let p = owned
                    .iter()
                    .find(|p| p.pid == pid)
                    .context("working process is not owned descendant")?;
                let image = self
                    .anchor_tools
                    .iter()
                    .find(|tool| tool.path == self.options.platform_bin.join("rphost.exe"))
                    .context("original pinned working executable absent")?;
                image.check()?;
                require_working_image(p, &image.path, self.barrier_filetime)?;
                if let Some(handle) = self.workers.get(&id) {
                    handle.require_current(p)?;
                } else {
                    self.workers.insert(id, KernelProcess::bind(p)?);
                }
                Ok((
                    id,
                    self.workers
                        .get(&id)
                        .context("working handle binding missing")?
                        .identity
                        .clone(),
                ))
            })
            .transpose()?;
        Ok(Observation {
            binding: self.binding.clone(),
            registrations,
            loaded,
            worker,
            password_only_admins_exact: true,
            authenticated_inventory_exact: true,
            lease_original_handle_exact: true,
            unknown_administration: false,
        })
    }

    fn register(&mut self) -> Result<Uuid> {
        let raw = self.rac(
            vec![
                "infobase".into(),
                "create".into(),
                format!("--cluster={}", self.binding.cluster),
                format!("--name={}", self.binding.database),
                "--dbms=MSSQLServer".into(),
                format!("--db-server={}", self.options.database_server),
                format!("--db-name={}", self.binding.database),
                format!("--db-user={}", self.options.database_user),
                format!("--db-pwd={}", self.options.database_password),
                "--locale=ru_RU".into(),
                "--scheduled-jobs-deny=on".into(),
            ],
            false,
            true,
        )?;
        let rows = blocks(&raw)?;
        if rows.len() != 1 {
            bail!("new managed registration response shape");
        }
        let id = Uuid::parse_str(
            rows[0]
                .get("infobase")
                .context("new registration UUID missing")?,
        )?;
        if id.is_nil() {
            bail!("new registration nil UUID");
        }
        self.binding.infobase = id;
        Ok(id)
    }

    fn load(&mut self, infobase: Uuid) -> Result<()> {
        if infobase != self.binding.infobase {
            bail!("managed load target drift");
        }
        // The intent is retained even if the native response is lost.
        self.admitted_load.insert(infobase);
        self.rac(
            vec![
                "infobase".into(),
                "info".into(),
                format!("--cluster={}", self.binding.cluster),
                format!("--infobase={infobase}"),
                format!("--infobase-user={}", self.options.infobase_user),
                format!("--infobase-pwd={}", self.options.infobase_password),
            ],
            false,
            true,
        )?;
        Ok(())
    }

    fn turn_off(&mut self, worker_id: Uuid, identity: &ProcessIdentity) -> Result<()> {
        let current = self.observe()?;
        if current.worker != Some((worker_id, identity.clone())) {
            bail!("managed handoff identity drift");
        }
        self.rac(
            vec![
                "process".into(),
                "turn-off".into(),
                format!("--cluster={}", self.binding.cluster),
                format!("--process={worker_id}"),
            ],
            false,
            true,
        )?;
        Ok(())
    }

    fn stop_owned(&mut self, journal: &mut Journal) -> Result<()> {
        if self.failed
            || self.cold
            || !self.authenticated_boundary
            || self.collectors.iter().any(|child| !child.terminal_proved())
        {
            bail!("unproved original administration/collector forbids owned shutdown");
        }
        self.startup_deadline = Some(
            Instant::now()
                .checked_add(self.options.timeout)
                .context("owned shutdown shared deadline overflow")?,
        );
        // A complete authenticated barrier and both endpoint censuses precede
        // acquisition of any PROCESS_TERMINATE right.
        let observation = self.observe()?;
        require_shutdown_inventory(&observation, &self.binding, &self.admitted_load)?;
        self.require_private_endpoint()?;
        let first = self.shutdown_census()?;
        let second = self.shutdown_census()?;
        let a = self.descendants(&first)?;
        let b = self.descendants(&second)?;
        self.console_tool.check()?;
        require_shutdown_graph(
            &a,
            &b,
            &self.options.platform_bin,
            &self.console_tool.path,
            &self.binding,
        )?;
        journal.append(
            "cold_console_lifecycle_tool",
            (&self.console_tool.path, &self.console_tool.digest),
        )?;
        for rows in [&first, &second] {
            if rows.iter().any(|row| {
                row.command
                    .to_ascii_lowercase()
                    .contains(&self.binding.root.to_string_lossy().to_ascii_lowercase())
                    && !b.iter().any(|owned| owned.pid == row.pid)
            }) {
                bail!("foreign process names shutdown root; no signal");
            }
        }
        for row in &b {
            if row.pid != self.binding.agent.pid && row.pid != self.binding.ras.pid {
                self.shutdown_handles
                    .insert(row.pid, KernelProcess::bind_for_shutdown(row)?);
            }
        }
        journal.append(
            "cold_descendants_retained",
            self.shutdown_handles
                .values()
                .map(|handle| &handle.identity)
                .collect::<Vec<_>>(),
        )?;

        // Stop the spawning anchor first, after retaining the complete union.
        // Every signal uses an original handle, never a fresh PID lookup.
        for (agent, expected) in [
            (true, self.binding.agent.clone()),
            (false, self.binding.ras.clone()),
        ] {
            let rows = self.shutdown_census()?;
            self.require_shutdown_members(&rows)?;
            self.console_tool.check()?;
            let row = rows
                .iter()
                .find(|row| row.pid == expected.pid)
                .context("original shutdown anchor census absent")?;
            journal.append("cold_direct_stop_intent", &expected)?;
            let remaining = self.startup_remaining()?.min(Duration::from_secs(5));
            let anchor = if agent {
                &mut self.agent
            } else {
                &mut self.ras
            };
            anchor
                .as_mut()
                .context("original shutdown anchor handle absent")?
                .stop_exact(row, &expected, remaining)?;
            journal.append("cold_direct_stop_confirmed", &expected)?;
        }
        // The agent cannot spawn replacements now. Descendants that exited
        // naturally are proved through their retained kernel handles.
        let pids: Vec<_> = self.shutdown_handles.keys().copied().collect();
        for pid in pids {
            let rows = self.shutdown_census()?;
            self.require_shutdown_members(&rows)?;
            self.console_tool.check()?;
            let remaining = self.startup_remaining()?.min(Duration::from_secs(5));
            let handle = self
                .shutdown_handles
                .get(&pid)
                .context("retained shutdown handle absent")?;
            if let Some(row) = rows.iter().find(|row| row.pid == pid) {
                journal.append("cold_direct_stop_intent", &handle.identity)?;
                handle.stop_exact(row, remaining)?;
                journal.append("cold_direct_stop_confirmed", &handle.identity)?;
            } else {
                handle.require_exited()?;
                journal.append("cold_natural_exit_confirmed", &handle.identity)?;
            }
        }
        // No descendant can still hold an inherited anchor pipe now. Only at
        // this point prove EOF; a failure remains unproved, never cold/undo.
        for anchor in [&mut self.agent, &mut self.ras] {
            let remaining = self
                .startup_deadline
                .context("shutdown deadline missing")?
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(5));
            if remaining.is_zero() {
                bail!("owned shutdown deadline before pipe proof");
            }
            anchor
                .as_mut()
                .context("original shutdown anchor absent")?
                .completed(remaining)?;
        }
        self.cold = true;
        self.require_cold()?;
        self.startup_remaining()?;
        self.startup_deadline = None;
        Ok(())
    }

    fn require_cold(&mut self) -> Result<()> {
        self.console_tool.check()?;
        if !self.cold
            || self.failed
            || !self.authenticated_boundary
            || self.collectors.iter().any(|child| !child.terminal_proved())
            || !self
                .agent
                .as_ref()
                .is_some_and(OriginalChild::terminal_proved)
            || !self
                .ras
                .as_ref()
                .is_some_and(OriginalChild::terminal_proved)
        {
            bail!("owned original exit/pipe/admin proof incomplete");
        }
        for handle in self.shutdown_handles.values() {
            handle.require_exited()?;
        }
        let rows = self.shutdown_census()?;
        self.require_shutdown_members(&rows)?;
        if rows.iter().any(|row| self.is_shutdown_member(row)) {
            bail!("owned shutdown process still present or replaced; no undo");
        }
        let ports = self.selected_ports();
        let raw = self.shell(format!("$ErrorActionPreference='Stop'; if(@(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object {{ $_.LocalPort -in @({ports}) }}).Count) {{throw 'cold private listener present'}}; 'EMPTY'"))?;
        if raw.trim() != "EMPTY" {
            bail!("cold private listener absence unproved");
        }
        Ok(())
    }
}

impl NativeRuntime {
    fn selected_ports(&self) -> String {
        [
            self.options.agent_port,
            self.options.cluster_port,
            self.options.ras_port,
        ]
        .into_iter()
        .chain(self.options.worker_first..=self.options.worker_last)
        .map(|port| port.to_string())
        .collect::<Vec<_>>()
        .join(",")
    }

    fn shutdown_census(&mut self) -> Result<Vec<CensusIdentity>> {
        let root = self.binding.root.to_string_lossy().replace('\'', "''");
        let seeds: BTreeSet<_> = self
            .agent
            .as_ref()
            .map(OriginalChild::pid)
            .into_iter()
            .chain(self.ras.as_ref().map(OriginalChild::pid))
            .chain(self.shutdown_handles.keys().copied())
            .collect();
        let text = seeds
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let script = shutdown_census_script(&root, &text, &self.selected_ports());
        let raw = self.shell(script)?;
        let census: ShutdownCensus =
            serde_json::from_str(&raw).context("bounded owned/opaque-foreign census shape")?;
        require_shutdown_census(&census, &seeds)?;
        Ok(census.owned)
    }
    fn is_shutdown_member(&self, row: &CensusIdentity) -> bool {
        row.pid == self.binding.agent.pid
            || row.pid == self.binding.ras.pid
            || self.shutdown_handles.contains_key(&row.pid)
            || row
                .command
                .to_ascii_lowercase()
                .contains(&self.binding.root.to_string_lossy().to_ascii_lowercase())
            || row.parent == self.binding.agent.pid
            || row.parent == self.binding.ras.pid
            || self.shutdown_handles.contains_key(&row.parent)
    }

    fn require_shutdown_members(&self, rows: &[CensusIdentity]) -> Result<()> {
        for (child, expected) in [
            (&self.agent, &self.binding.agent),
            (&self.ras, &self.binding.ras),
        ] {
            match rows.iter().find(|row| row.pid == expected.pid) {
                Some(row) => require_shutdown_identity(row, expected)?,
                None if child
                    .as_ref()
                    .is_some_and(OriginalChild::direct_exit_proved) => {}
                None => bail!("original shutdown anchor absent without original direct exit proof"),
            }
        }
        for handle in self.shutdown_handles.values() {
            match rows.iter().find(|row| row.pid == handle.identity.pid) {
                Some(row) => handle.require_current(row)?,
                None => handle.require_exited()?,
            }
        }
        for row in rows.iter().filter(|row| self.is_shutdown_member(row)) {
            if row.pid != self.binding.agent.pid
                && row.pid != self.binding.ras.pid
                && !self.shutdown_handles.contains_key(&row.pid)
            {
                bail!("new or unknown descendant during shutdown; retain lifetime");
            }
        }
        Ok(())
    }

    pub(super) fn require_endpoint(&self, rac: &Path, endpoint: &str, server: &str) -> Result<()> {
        if rac != self.rac.path
            || endpoint != format!("localhost:{}", self.options.ras_port)
            || server != self.options.database_server
        {
            bail!("managed runtime endpoint does not match original creator");
        }
        Ok(())
    }

    pub(super) fn require_cold_sql_target(&self, sql: &crate::sql::SqlExec) -> Result<()> {
        if sql.server() != self.options.database_server || sql.client().is_none() {
            bail!("cold undo SQL endpoint/backend differs from original creator");
        }
        Ok(())
    }

    pub(super) fn verify_target_profile(
        &mut self,
        claimed: crate::mssql_platform_profile::MssqlNativePlatformProfile,
        options: crate::mssql_platform_profile::MssqlNativeProfileVerificationOptions<'_>,
    ) -> Result<crate::mssql_platform_profile::MssqlNativeProfileVerification> {
        self.require_endpoint(options.rac, options.ras_endpoint, options.server)?;
        if options.database != self.binding.database
            || options.cluster_id != Some(self.binding.cluster)
            || options.infobase_id != Some(self.binding.infobase)
        {
            bail!("managed profile target identity drift");
        }
        let agent_version = self.rac(vec!["agent".into(), "version".into()], true, true)?;
        let mut args = vec![
            "infobase".into(),
            "info".into(),
            format!("--cluster={}", self.binding.cluster),
            format!("--infobase={}", self.binding.infobase),
        ];
        if let Some(user) = options.infobase_user {
            args.extend([
                format!("--infobase-user={user}"),
                format!(
                    "--infobase-pwd={}",
                    options.infobase_pwd.unwrap_or_default()
                ),
            ]);
        } else if options.infobase_pwd.is_some() {
            bail!("infobase password requires user");
        }
        let registration = self.rac(args, false, true)?;
        crate::mssql_platform_profile::verify_mssql_native_profile_managed_observation(
            claimed,
            options,
            &self.credentials(),
            &agent_version,
            &registration,
        )
    }

    fn credentials(&self) -> ManagedRacReadAuth<'_> {
        ManagedRacReadAuth {
            agent_user: &self.administrator,
            agent_password: &self.password,
            cluster_user: &self.administrator,
            cluster_password: &self.password,
        }
    }

    fn execute(&mut self, tool: &Tool, argv: &[String]) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        tool.check()?;
        if self.failed {
            bail!("managed native lifetime already unproved");
        }
        let timeout = self.startup_remaining()?;
        if self.collectors.len() >= 1024 {
            bail!("managed original command custody bound; retain lifetime");
        }
        self.collectors
            .push(OriginalChild::spawn(&tool.path, argv)?);
        let index = self.collectors.len() - 1;
        let result = self.collectors[index].completed(timeout);
        if result.is_err() {
            self.failed = true;
        }
        let value = result?;
        self.startup_remaining()?;
        Ok(value)
    }

    fn startup_remaining(&self) -> Result<Duration> {
        remaining_budget(self.startup_deadline, Instant::now(), self.options.timeout)
    }

    fn shell(&mut self, script: String) -> Result<String> {
        let tool = self.pwsh.clone();
        let argv = vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            script,
        ];
        let (exit, out, err) = self.execute(&tool, &argv)?;
        if exit != 0 || !err.is_empty() {
            bail!("managed observation utility failed; output redacted");
        }
        String::from_utf8(out).context("managed observation must be strict UTF-8")
    }

    fn census(&mut self) -> Result<Vec<CensusIdentity>> {
        // Full identities are required only for the original graph, root
        // markers and selected listeners. Opaque foreign service facts grant
        // no registration, worker or signal authority.
        self.shutdown_census()
    }
    /// No command is addressed to a port merely because it was vacant at
    /// creation. Bind every current listener between two complete censuses.
    fn require_private_endpoint(&mut self) -> Result<()> {
        for tool in &self.anchor_tools {
            tool.check()?;
        }
        let first = self.census()?;
        let ports: Vec<_> = [
            self.options.agent_port,
            self.options.cluster_port,
            self.options.ras_port,
        ]
        .into_iter()
        .chain(self.options.worker_first..=self.options.worker_last)
        .collect();
        let selected = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let raw = self.shell(format!("$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $p=@(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object {{ $_.LocalPort -in @({selected}) }} | Select-Object -First 513); if($p.Count -gt 512){{throw 'listener bound'}}; $r=@($p | ForEach-Object {{ @{{port=[uint16]$_.LocalPort;pid=[uint32]$_.OwningProcess}} }}); ConvertTo-Json -InputObject $r -Compress"))?;
        let listeners: Vec<Listener> =
            serde_json::from_str(&raw).context("managed listener shape")?;
        let second = self.census()?;
        let result = (|| -> Result<bool> {
            for (anchor, expected) in [
                (&mut self.agent, &self.binding.agent),
                (&mut self.ras, &self.binding.ras),
            ] {
                let anchor = anchor
                    .as_mut()
                    .context("managed original endpoint handle absent")?;
                if !anchor.alive()? {
                    bail!("original endpoint anchor exited");
                }
                for rows in [&first, &second] {
                    let row = rows
                        .iter()
                        .find(|r| r.pid == anchor.pid())
                        .context("endpoint census anchor absent")?;
                    if anchor.bind_census(row)? != *expected {
                        bail!("endpoint anchor drift");
                    }
                }
            }
            let a = self.descendants(&first)?;
            let b = self.descendants(&second)?;
            listener_authority(
                &listeners,
                &a,
                &b,
                &self.options.platform_bin,
                &self.binding.root,
                &first,
                &second,
                (self.options.agent_port, self.binding.agent.pid),
                (self.options.ras_port, self.binding.ras.pid),
            )
        })();
        match result {
            Ok(true) => Ok(()),
            Ok(false) => bail!("managed private endpoint is not ready"),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }

    fn rac(
        &mut self,
        mut argv: Vec<String>,
        agent_auth: bool,
        authenticated: bool,
    ) -> Result<String> {
        self.require_private_endpoint()?;
        if authenticated {
            let prefix = if agent_auth { "agent" } else { "cluster" };
            argv.extend([
                format!("--{prefix}-user={}", self.administrator),
                format!("--{prefix}-pwd={}", self.password),
            ]);
        }
        argv.push(format!("localhost:{}", self.options.ras_port));
        let tool = self.rac.clone();
        let (exit, out, err) = self.execute(&tool, &argv)?;
        // Never propagate native text containing credentials or command lines.
        if exit != 0 || !err.is_empty() {
            bail!("managed RAC command failed; arguments/output redacted");
        }
        String::from_utf8(out).context("managed RAC response must be strict UTF-8")
    }

    fn bootstrap(&mut self, journal: &mut Journal) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(self.options.timeout)
            .context("startup deadline overflow")?;
        self.startup_deadline = Some(deadline);
        let ports: Vec<_> = [
            self.options.agent_port,
            self.options.cluster_port,
            self.options.ras_port,
        ]
        .into_iter()
        .chain(self.options.worker_first..=self.options.worker_last)
        .collect();
        let port_list = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let raw = self.shell(format!("$ErrorActionPreference='Stop'; if(@(Get-NetTCPConnection -State Listen -ErrorAction Stop | Where-Object {{ $_.LocalPort -in @({port_list}) }}).Count) {{throw 'private ports occupied'}}; 'EMPTY'"))?;
        if raw.trim() != "EMPTY" {
            bail!("fresh private ports not proved");
        }
        journal.append("agent_spawn_intent", self.binding.root.to_string_lossy())?;
        let agent = Tool::pin(self.options.platform_bin.join("ragent.exe"))?;
        self.startup_remaining()?;
        self.agent = Some(OriginalChild::spawn(
            &agent.path,
            &[
                "-agent".into(),
                "-port".into(),
                self.options.agent_port.to_string(),
                "-regport".into(),
                self.options.cluster_port.to_string(),
                "-range".into(),
                format!("{}:{}", self.options.worker_first, self.options.worker_last),
                "-d".into(),
                self.binding
                    .root
                    .join("srvinfo")
                    .to_string_lossy()
                    .into_owned(),
            ],
        )?);
        self.anchor_tools.push(agent);
        let rows = self.census()?;
        let anchor = self
            .agent
            .as_mut()
            .context("agent original handle absent")?;
        let row = rows
            .iter()
            .find(|p| p.pid == anchor.pid())
            .context("spawned agent census absent")?;
        self.binding.agent = anchor.bind_census(row)?;
        journal.append("agent_spawn_confirmed", &self.binding.agent)?;
        journal.append("ras_spawn_intent", self.binding.root.to_string_lossy())?;
        let ras = Tool::pin(self.options.platform_bin.join("ras.exe"))?;
        self.startup_remaining()?;
        self.ras = Some(OriginalChild::spawn(
            &ras.path,
            &[
                "cluster".into(),
                format!("--port={}", self.options.ras_port),
                format!("localhost:{}", self.options.agent_port),
            ],
        )?);
        self.anchor_tools.push(ras);
        let rows = self.census()?;
        let anchor = self.ras.as_mut().context("RAS original handle absent")?;
        let row = rows
            .iter()
            .find(|p| p.pid == anchor.pid())
            .context("spawned RAS census absent")?;
        self.binding.ras = anchor.bind_census(row)?;
        journal.append("ras_spawn_confirmed", &self.binding.ras)?;
        // Bounded startup only retries read-only version probes, never writes.
        loop {
            match self.rac(vec!["agent".into(), "version".into()], false, false) {
                Ok(build)
                    if crate::mssql_platform_profile::parse_rac_agent_build(&build)
                        .is_ok_and(|build| build == "8.3.27.2214") =>
                {
                    self.startup_remaining()?;
                    break;
                }
                _ if self.failed || Instant::now() >= deadline => {
                    bail!("managed agent startup/profile unproved")
                }
                _ => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let clusters = blocks(&self.rac(vec!["cluster".into(), "list".into()], false, false)?)?;
        if clusters.len() != 1 {
            bail!("fresh managed cluster count must be exactly one");
        }
        self.binding.cluster =
            Uuid::parse_str(clusters[0].get("cluster").context("cluster UUID absent")?)?;
        if self.binding.cluster.is_nil() {
            bail!("nil managed cluster");
        }
        journal.append("cluster_confirmed", self.binding.cluster.to_string())?;
        journal.append("agent_admin_intent", &self.administrator)?;
        self.rac(
            vec![
                "agent".into(),
                "admin".into(),
                "register".into(),
                format!("--name={}", self.administrator),
                format!("--pwd={}", self.password),
                "--auth=pwd".into(),
            ],
            true,
            false,
        )?;
        journal.append("agent_admin_confirmed", &self.administrator)?;
        journal.append("cluster_admin_intent", &self.administrator)?;
        self.rac(
            vec![
                "cluster".into(),
                "admin".into(),
                "register".into(),
                format!("--cluster={}", self.binding.cluster),
                format!("--name={}", self.administrator),
                format!("--pwd={}", self.password),
                "--auth=pwd".into(),
            ],
            true,
            true,
        )?;
        journal.append("cluster_admin_confirmed", &self.administrator)?;
        self.require_admins()?;
        self.challenge_authentication(journal)?;
        // Pre-barrier workers cannot acquire positive lifetime authority.
        let rows = self.census()?;
        if self.descendants(&rows)?.iter().any(|p| {
            p.executable
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("rphost.exe"))
        }) {
            bail!("pre-authentication working process exists; history cannot be inferred");
        }
        self.barrier_filetime = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .checked_div(100)
            .context("barrier clock")? as u64
            + 116444736000000000;
        Ok(())
    }

    fn require_admins(&mut self) -> Result<()> {
        for agent in [true, false] {
            let mut args = vec![
                if agent {
                    "agent".into()
                } else {
                    "cluster".into()
                },
                "admin".into(),
                "list".into(),
            ];
            if !agent {
                args.push(format!("--cluster={}", self.binding.cluster));
            }
            let admins = blocks(&self.rac(args, agent, true)?)?;
            if admins.len() != 1
                || admins[0].get("name") != Some(&self.administrator)
                || admins[0].get("auth").map(String::as_str) != Some("pwd")
                || admins[0].get("os-user").is_some_and(|v| !v.is_empty())
            {
                bail!("exact password-only managed administration not proved");
            }
        }
        Ok(())
    }

    fn challenge_authentication(&mut self, journal: &mut Journal) -> Result<()> {
        let plan = authentication::Plan::new(&self.administrator, self.binding.cluster)?;
        authentication::verify(
            &mut NativeAdminIo {
                runtime: self,
                journal,
                plan: &plan,
                wrong_password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
                throwaway_password: format!("{}{}", Uuid::new_v4(), Uuid::new_v4()),
            },
            &plan,
        )?;
        self.authenticated_boundary = true;
        Ok(())
    }

    fn descendants<'a>(&self, rows: &'a [CensusIdentity]) -> Result<Vec<&'a CensusIdentity>> {
        let mut admitted = BTreeSet::from([self.binding.agent.pid, self.binding.ras.pid]);
        loop {
            let old = admitted.len();
            for p in rows {
                if admitted.contains(&p.parent) {
                    admitted.insert(p.pid);
                }
            }
            if old == admitted.len() {
                break;
            }
        }
        let result: Vec<_> = rows.iter().filter(|p| admitted.contains(&p.pid)).collect();
        if result.len() > 64 {
            bail!("managed descendant bound");
        }
        Ok(result)
    }
}

struct NativeAdminIo<'a> {
    runtime: &'a mut NativeRuntime,
    journal: &'a mut Journal,
    plan: &'a authentication::Plan,
    wrong_password: String,
    throwaway_password: String,
}

impl authentication::Io for NativeAdminIo<'_> {
    fn inventory(
        &mut self,
        family: AdminFamily,
        allowed: &[&str],
    ) -> Result<authentication::Inventory> {
        let mut args = vec![family.name().into(), "admin".into(), "list".into()];
        if !family.is_agent() {
            args.push(format!("--cluster={}", self.runtime.binding.cluster));
        }
        let raw = self.runtime.rac(args, family.is_agent(), true)?;
        if raw.len() > 65536 {
            bail!("bounded authenticated admin inventory required");
        }
        authentication::Inventory::from_rows(blocks(&raw)?, allowed)
    }
    fn mutation(
        &mut self,
        family: AdminFamily,
        challenge: AdminCredentials,
        action: AdminAction,
        credentials: AdminCredentials,
    ) -> Result<authentication::Receipt> {
        let mut argv = self.plan.mutation_arguments(
            family,
            challenge,
            action,
            credentials,
            [
                &self.runtime.password,
                &self.wrong_password,
                &self.throwaway_password,
            ],
        );
        argv.push(format!("localhost:{}", self.runtime.options.ras_port));
        self.runtime.require_private_endpoint()?;
        let tool = self.runtime.rac.clone();
        let (exit, stdout, stderr) = self.runtime.execute(&tool, &argv)?;
        Ok(authentication::Receipt {
            exit,
            stdout,
            stderr,
        })
    }
    fn record(&mut self, event: &'static str, value: serde_json::Value) -> Result<()> {
        self.journal.append(event, value)
    }
    fn denial_is_measured(
        &self,
        family: AdminFamily,
        kind: AdminCredentials,
        receipt: &authentication::Receipt,
    ) -> bool {
        let fingerprint = DenialFingerprint {
            family,
            kind,
            exit: receipt.exit,
            stdout: format!("{:X}", Sha256::digest(&receipt.stdout)),
            stderr: format!("{:X}", Sha256::digest(&receipt.stderr)),
        };
        denial_admitted(&fingerprint, measured_denials())
    }
}

fn require_shutdown_graph(
    first: &[&CensusIdentity],
    second: &[&CensusIdentity],
    platform: &Path,
    console: &Path,
    binding: &LifetimeBinding,
) -> Result<()> {
    if first.len() != second.len() || second.len() < 2 || second.len() > 64 {
        bail!("complete bounded original shutdown graph required");
    }
    for row in second {
        let old = first
            .iter()
            .find(|old| old.pid == row.pid)
            .context("shutdown graph drift")?;
        if row.parent != old.parent
            || row.birth_filetime != old.birth_filetime
            || row.command != old.command
            || row.executable != old.executable
            || row.command.is_empty()
            || row.birth_filetime == 0
        {
            bail!("shutdown graph full identity drift");
        }
        let image = row
            .executable
            .file_name()
            .context("shutdown image absent")?
            .to_string_lossy();
        let server_image = [
            "ragent.exe",
            "ras.exe",
            "rmngr.exe",
            "rphost.exe",
            "dbda.exe",
        ]
        .iter()
        .any(|name| image.eq_ignore_ascii_case(name))
            && row
                .executable
                .to_string_lossy()
                .eq_ignore_ascii_case(&platform.join(image.as_ref()).to_string_lossy());
        let lifecycle_console = image.eq_ignore_ascii_case("conhost.exe")
            && console.is_absolute()
            && row
                .executable
                .to_string_lossy()
                .eq_ignore_ascii_case(&console.to_string_lossy());
        if !server_image && !lifecycle_console {
            bail!("unknown shutdown descendant executable; no signal");
        }
        if row.pid != binding.agent.pid && row.pid != binding.ras.pid {
            let parent = second
                .iter()
                .find(|p| p.pid == row.parent)
                .context("shutdown descendant parent outside retained graph")?;
            if row.pid == row.parent || parent.birth_filetime > row.birth_filetime {
                bail!("shutdown descendant genesis unproved");
            }
        }
    }
    for expected in [&binding.agent, &binding.ras] {
        let row = second
            .iter()
            .find(|row| row.pid == expected.pid)
            .context("shutdown anchor missing")?;
        require_shutdown_identity(row, expected)?;
    }
    Ok(())
}

fn require_shutdown_identity(row: &CensusIdentity, expected: &ProcessIdentity) -> Result<()> {
    if row.pid != expected.pid
        || row.parent != expected.parent
        || row.birth_filetime > expected.birth_100ns
        || expected.birth_100ns
            > row
                .birth_filetime
                .checked_add(9)
                .context("shutdown birth interval overflow")?
        || !row
            .executable
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.executable.to_string_lossy())
        || format!("{:X}", Sha256::digest(row.command.as_bytes())) != expected.command_sha256
    {
        bail!("original shutdown anchor identity drift");
    }
    Ok(())
}

fn require_shutdown_inventory(
    observation: &Observation,
    binding: &LifetimeBinding,
    retained: &BTreeSet<Uuid>,
) -> Result<()> {
    let sole = BTreeSet::from([binding.infobase]);
    if binding.infobase.is_nil()
        || observation.binding != *binding
        || retained != &sole
        || observation.registrations != sole
        || observation.loaded != sole
        || !observation.password_only_admins_exact
        || !observation.authenticated_inventory_exact
        || !observation.lease_original_handle_exact
        || observation.unknown_administration
    {
        bail!("fresh sole target/history/administration changed before owned shutdown; no signal");
    }
    Ok(())
}

fn remaining_budget(
    deadline: Option<Instant>,
    now: Instant,
    ordinary: Duration,
) -> Result<Duration> {
    match deadline {
        Some(deadline) => deadline
            .checked_duration_since(now)
            .filter(|value| !value.is_zero())
            .context("managed startup deadline expired; no admission or next command"),
        None => Ok(ordinary),
    }
}

fn require_working_image(row: &CensusIdentity, expected: &Path, barrier: u64) -> Result<()> {
    // RAC can expose a worker before it has a selected TCP listener. Listener
    // ownership is therefore not an executable check for this PID.
    if row.birth_filetime <= barrier
        || !row.executable.is_absolute()
        || !expected.is_absolute()
        || !row
            .executable
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        bail!(
            "working executable differs from original pinned platform or predates authentication barrier"
        );
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Listener {
    port: u16,
    pid: u32,
}

#[allow(clippy::too_many_arguments)]
fn listener_authority(
    listeners: &[Listener],
    first: &[&CensusIdentity],
    second: &[&CensusIdentity],
    platform: &Path,
    root: &Path,
    census_a: &[CensusIdentity],
    census_b: &[CensusIdentity],
    agent: (u16, u32),
    ras: (u16, u32),
) -> Result<bool> {
    if listeners.len() > 512 {
        bail!("managed listener inventory bound");
    }
    for row in census_a.iter().chain(census_b) {
        if row
            .command
            .to_ascii_lowercase()
            .contains(&root.to_string_lossy().to_ascii_lowercase())
            && !first.iter().any(|p| p.pid == row.pid)
        {
            bail!("foreign process names managed root");
        }
    }
    for listener in listeners {
        let a = first
            .iter()
            .find(|r| r.pid == listener.pid)
            .context("foreign or missing listener in first census")?;
        let b = second
            .iter()
            .find(|r| r.pid == listener.pid)
            .context("foreign or missing listener in second census")?;
        if a.parent != b.parent
            || a.birth_filetime != b.birth_filetime
            || a.executable != b.executable
            || a.command != b.command
        {
            bail!("listener identity changed between censuses");
        }
        let name = a
            .executable
            .file_name()
            .context("listener executable absent")?
            .to_string_lossy();
        if ![
            "ragent.exe",
            "ras.exe",
            "rmngr.exe",
            "rphost.exe",
            "dbda.exe",
        ]
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
            || !a
                .executable
                .to_string_lossy()
                .eq_ignore_ascii_case(&platform.join(name.as_ref()).to_string_lossy())
        {
            bail!("listener executable is outside exact owned platform");
        }
        for (port, pid) in [agent, ras] {
            if listener.port == port && listener.pid != pid {
                bail!("private endpoint listener belongs to another process");
            }
        }
    }
    Ok([agent, ras]
        .iter()
        .all(|(port, pid)| listeners.iter().any(|l| l.port == *port && l.pid == *pid)))
}

#[derive(Debug, PartialEq, Eq)]
struct DenialFingerprint {
    family: AdminFamily,
    kind: AdminCredentials,
    exit: i32,
    stdout: String,
    stderr: String,
}

fn denial_admitted(observed: &DenialFingerprint, measured: &[DenialFingerprint]) -> bool {
    observed.exit != 0 && measured.iter().any(|known| known == observed)
}

fn measured_denials() -> &'static [DenialFingerprint] {
    // No denial/OS-bypass measurement exists in the accepted native corpus.
    // Populate only from a reviewed exact native creator experiment. Neither
    // user JSON nor a current admin list can extend this closed admission set.
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_foreign_census_is_opaque_and_cannot_own_listener_or_descendant() {
        let full = |pid, parent, name: &str| CensusIdentity {
            pid,
            parent,
            birth_filetime: 100,
            executable: std::env::temp_dir().join(name),
            command: format!("{name} original"),
        };
        let mut census = ShutdownCensus {
            owned: vec![
                full(10, 1, "ragent.exe"),
                full(11, 1, "ras.exe"),
                full(12, 10, "conhost.exe"),
            ],
            foreign: vec![ForeignProcessFact {
                pid: 900,
                parent: 800,
                birth_filetime: None,
                executable_present: false,
                executable_path_sha256: None,
                command_present: false,
                command_sha256: None,
                private_root_marker: None,
            }],
            listeners: vec![Listener {
                pid: 10,
                port: 7540,
            }],
        };
        let seeds = BTreeSet::from([10, 11]);
        assert!(require_shutdown_census(&census, &seeds).is_ok());
        census.listeners.push(Listener {
            pid: 900,
            port: 7545,
        });
        assert!(require_shutdown_census(&census, &seeds).is_err());
        census.listeners.pop();
        census.foreign[0].parent = 12;
        assert!(require_shutdown_census(&census, &seeds).is_err());
        census.foreign[0].parent = 800;
        census.foreign[0].private_root_marker = Some(true);
        assert!(require_shutdown_census(&census, &seeds).is_err());
        census.foreign[0].private_root_marker = None;
        census.owned.push(full(901, 800, "unknown-root-marker.exe"));
        assert!(require_shutdown_census(&census, &seeds).is_err());
        census.owned.pop();
        census.owned[2].command.clear();
        assert!(require_shutdown_census(&census, &seeds).is_err());
        let script = shutdown_census_script("C:\\owned", "10,11", "7540,7545");
        assert!(script.contains("if($selected)"));
        assert!(script.contains("command_sha256="));
        assert!(!script.contains("Name -in"));
        assert!(!script.contains("__ROOT__"));
    }

    #[test]
    fn shutdown_graph_requires_every_complete_original_and_pinned_descendant() {
        let platform = std::env::temp_dir().join("owned-shutdown-platform");
        let console = std::env::temp_dir().join("trusted-os/System32/conhost.exe");
        let make = |pid, parent, name: &str| CensusIdentity {
            pid,
            parent,
            birth_filetime: 100,
            executable: platform.join(name),
            command: format!("{name} exact original command"),
        };
        let agent = make(10, 1, "ragent.exe");
        let ras = make(11, 1, "ras.exe");
        let worker = make(12, 10, "rphost.exe");
        let identity = |row: &CensusIdentity| ProcessIdentity {
            pid: row.pid,
            parent: row.parent,
            birth_100ns: 106,
            executable: row.executable.clone(),
            command_sha256: format!("{:X}", Sha256::digest(row.command.as_bytes())),
        };
        let binding = LifetimeBinding {
            nonce: Uuid::new_v4(),
            root: std::env::temp_dir(),
            cluster: Uuid::new_v4(),
            infobase: Uuid::new_v4(),
            database: "one".into(),
            agent: identity(&agent),
            ras: identity(&ras),
        };
        let clean = [&agent, &ras, &worker];
        assert!(require_shutdown_graph(&clean, &clean, &platform, &console, &binding).is_ok());
        let console_row = CensusIdentity {
            pid: 13,
            parent: 10,
            birth_filetime: 100,
            executable: console.clone(),
            command: "exact inherited lifecycle console".into(),
        };
        let with_console = [&agent, &ras, &worker, &console_row];
        assert!(
            require_shutdown_graph(&with_console, &with_console, &platform, &console, &binding)
                .is_ok()
        );
        let foreign_console = CensusIdentity {
            executable: platform.join("conhost.exe"),
            ..console_row
        };
        let foreign = [&agent, &ras, &worker, &foreign_console];
        assert!(require_shutdown_graph(&foreign, &foreign, &platform, &console, &binding).is_err());
        assert!(require_working_image(&foreign_console, &platform.join("rphost.exe"), 99).is_err());
        let make_observation = || Observation {
            binding: binding.clone(),
            registrations: BTreeSet::from([binding.infobase]),
            loaded: BTreeSet::from([binding.infobase]),
            worker: Some((Uuid::new_v4(), identity(&worker))),
            password_only_admins_exact: true,
            authenticated_inventory_exact: true,
            lease_original_handle_exact: true,
            unknown_administration: false,
        };
        let retained = BTreeSet::from([binding.infobase]);
        assert!(require_shutdown_inventory(&make_observation(), &binding, &retained).is_ok());
        for drift in [
            "second_registration",
            "second_load",
            "unregistered",
            "admin",
            "lease",
            "uuid",
        ] {
            let mut o = make_observation();
            match drift {
                "second_registration" => {
                    o.registrations.insert(Uuid::new_v4());
                }
                "second_load" => {
                    o.loaded.insert(Uuid::new_v4());
                }
                "unregistered" => o.registrations.clear(),
                "admin" => o.unknown_administration = true,
                "lease" => o.lease_original_handle_exact = false,
                _ => o.binding.infobase = Uuid::new_v4(),
            }
            assert!(
                require_shutdown_inventory(&o, &binding, &retained).is_err(),
                "{drift}"
            );
        }
        assert!(
            require_shutdown_inventory(
                &make_observation(),
                &binding,
                &BTreeSet::from([binding.infobase, Uuid::new_v4()])
            )
            .is_err()
        );
        for change in [
            "parent",
            "birth",
            "command",
            "foreign_image",
            "unknown_image",
        ] {
            let mut changed = make(12, 10, "rphost.exe");
            match change {
                "parent" => changed.parent = 99,
                "birth" => changed.birth_filetime += 1,
                "command" => changed.command.push('x'),
                "foreign_image" => {
                    changed.executable = std::env::temp_dir().join("foreign/rphost.exe")
                }
                _ => changed.executable = platform.join("powershell.exe"),
            }
            let altered = [&agent, &ras, &changed];
            assert!(
                require_shutdown_graph(&clean, &altered, &platform, &console, &binding).is_err(),
                "{change}"
            );
            if change.ends_with("image") {
                assert!(
                    require_shutdown_graph(&altered, &altered, &platform, &console, &binding)
                        .is_err()
                );
            }
        }
        assert!(
            require_shutdown_graph(&clean, &[&agent, &ras], &platform, &console, &binding).is_err()
        );
        let mut overlap = binding.clone();
        overlap.agent.birth_100ns = 109;
        assert!(require_shutdown_graph(&clean, &clean, &platform, &console, &overlap).is_ok());
        overlap.agent.birth_100ns = 110;
        assert!(require_shutdown_graph(&clean, &clean, &platform, &console, &overlap).is_err());
        overlap = binding;
        overlap.ras.command_sha256 = "0".repeat(64);
        assert!(require_shutdown_graph(&clean, &clean, &platform, &console, &overlap).is_err());
    }

    #[test]
    fn working_image_without_any_listener_still_requires_exact_pinned_executable() {
        let expected = std::env::temp_dir()
            .join("owned-platform")
            .join("rphost.exe");
        let mut row = CensusIdentity {
            pid: 44,
            parent: 40,
            birth_filetime: 101,
            executable: expected.clone(),
            command: "working command".into(),
        };
        assert!(require_working_image(&row, &expected, 100).is_ok());
        row.executable = std::env::temp_dir()
            .join("foreign-platform")
            .join("rphost.exe");
        assert!(require_working_image(&row, &expected, 100).is_err());
        row.executable = expected;
        assert!(require_working_image(&row, &row.executable, 101).is_err());
        assert!(require_working_image(&row, &PathBuf::from("rphost.exe"), 100).is_err());
    }

    #[test]
    fn startup_deadline_never_admits_a_late_success_or_next_command() {
        let deadline = Instant::now();
        assert!(
            remaining_budget(
                Some(deadline),
                deadline - Duration::from_nanos(1),
                Duration::from_secs(120)
            )
            .is_ok()
        );
        assert!(remaining_budget(Some(deadline), deadline, Duration::from_secs(120)).is_err());
        assert!(
            remaining_budget(
                Some(deadline),
                deadline + Duration::from_nanos(1),
                Duration::from_secs(120)
            )
            .is_err()
        );
        assert_eq!(
            remaining_budget(None, deadline, Duration::from_secs(5)).unwrap(),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn every_endpoint_listener_needs_both_complete_owned_censuses() {
        let platform = std::env::temp_dir().join("native-platform");
        let root = std::env::temp_dir().join("managed-fresh-nonce");
        let make = |pid, name: &str| CensusIdentity {
            pid,
            parent: 1,
            birth_filetime: 100,
            executable: platform.join(name),
            command: format!("{name} private"),
        };
        let agent = make(2, "ragent.exe");
        let ras = make(3, "ras.exe");
        let worker = make(4, "rphost.exe");
        let first = vec![&agent, &ras, &worker];
        let listeners = vec![
            Listener { port: 2540, pid: 2 },
            Listener { port: 2545, pid: 3 },
            Listener { port: 2560, pid: 4 },
        ];
        let check = |ls: &[Listener], second: &[&CensusIdentity], ca: &[CensusIdentity]| {
            listener_authority(
                ls,
                &first,
                second,
                &platform,
                &root,
                ca,
                &[],
                (2540, 2),
                (2545, 3),
            )
        };
        assert!(check(&listeners, &first, &[]).unwrap());
        assert!(!check(&[], &first, &[]).unwrap()); // Read-only startup can wait; cannot issue RAC.
        assert!(check(&listeners, &[&agent, &ras], &[]).is_err()); // exited/missing listener
        let mut changed = make(4, "rphost.exe");
        changed.birth_filetime += 1;
        assert!(check(&listeners, &[&agent, &ras, &changed], &[]).is_err());
        for ls in [
            vec![Listener { port: 2540, pid: 4 }],
            vec![Listener {
                port: 2545,
                pid: 99,
            }],
        ] {
            assert!(check(&ls, &first, &[]).is_err());
        }
        let foreign = CensusIdentity {
            pid: 99,
            parent: 1,
            birth_filetime: 200,
            executable: platform.join("ragent.exe"),
            command: root.to_string_lossy().into_owned(),
        };
        assert!(check(&listeners, &first, &[foreign]).is_err());
        let alien = make(4, "powershell.exe");
        assert!(
            listener_authority(
                &listeners,
                &[&agent, &ras, &alien],
                &[&agent, &ras, &alien],
                &platform,
                &root,
                &[],
                &[],
                (2540, 2),
                (2545, 3)
            )
            .is_err()
        );
    }

    #[test]
    fn authentication_opaque_failure_and_forced_os_success_are_not_authority() {
        let measured = DenialFingerprint {
            family: AdminFamily::Agent,
            kind: AdminCredentials::WrongPassword,
            exit: 1,
            stdout: "A".repeat(64),
            stderr: "B".repeat(64),
        };
        assert!(denial_admitted(
            &measured,
            &[DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 1,
                stdout: "A".repeat(64),
                stderr: "B".repeat(64)
            }]
        ));
        for changed in [
            DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 0,
                stdout: "A".repeat(64),
                stderr: "B".repeat(64),
            },
            DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 1,
                stdout: "C".repeat(64),
                stderr: "B".repeat(64),
            },
            DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 1,
                stdout: "A".repeat(64),
                stderr: "C".repeat(64),
            },
        ] {
            assert!(!denial_admitted(&changed, std::slice::from_ref(&measured)));
        }
        assert!(!denial_admitted(&measured, measured_denials()));
        let other_family = DenialFingerprint {
            family: AdminFamily::Cluster,
            ..DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 1,
                stdout: "A".repeat(64),
                stderr: "B".repeat(64),
            }
        };
        let other_kind = DenialFingerprint {
            kind: AdminCredentials::ImplicitOs,
            ..DenialFingerprint {
                family: AdminFamily::Agent,
                kind: AdminCredentials::WrongPassword,
                exit: 1,
                stdout: "A".repeat(64),
                stderr: "B".repeat(64),
            }
        };
        assert!(!denial_admitted(
            &other_family,
            std::slice::from_ref(&measured)
        ));
        assert!(!denial_admitted(
            &other_kind,
            std::slice::from_ref(&measured)
        ));
    }

    #[test]
    fn unknown_and_duplicate_inventory_lines_refuse_instead_of_losing_history() {
        assert!(blocks("infobase: a\ninfobase: b").is_err());
        assert!(blocks("unexpected output").is_err());
        assert!(blocks(&"a: b\n\n".repeat(129)).is_err());
        assert!(blocks("").unwrap().is_empty());
    }
}

fn blocks(text: &str) -> Result<Vec<BTreeMap<String, String>>> {
    let mut result = Vec::new();
    let mut record = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            if !record.is_empty() {
                result.push(std::mem::take(&mut record));
            }
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .context("managed RAC inventory line unparsed")?;
        let key = key.trim();
        if key.is_empty()
            || record
                .insert(key.to_owned(), value.trim().to_owned())
                .is_some()
        {
            bail!("managed RAC duplicate field");
        }
    }
    if !record.is_empty() {
        result.push(record);
    }
    if result.len() > 128 {
        bail!("managed RAC inventory count bound");
    }
    Ok(result)
}
