//! Public-command selection and custody of a fresh, privately created lifetime.
//! No serialized report or caller UUID can construct a managed capability.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::{
    Creation, CreatorOptions, Journal, ManagedOutcome, NativeManagedWorker, PublicationState,
};
use crate::cli::{
    ManagedWorkerCliArgs, MssqlActivateStagedMainArgs, MssqlApplySourceChangeArgs,
    MssqlMainActivationModeArg,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;

/// Selection is checked from all actual fields, including programmatic Args.
/// This boolean selects a route; it cannot construct a managed capability.
pub(crate) fn selects_fresh(group: &ManagedWorkerCliArgs) -> Result<bool> {
    let present = [
        group.managed_worker_parent.is_some(),
        group.managed_platform_bin.is_some(),
        group.managed_powershell.is_some(),
        group.managed_agent_port.is_some(),
        group.managed_cluster_port.is_some(),
        group.managed_ras_port.is_some(),
        group.managed_worker_first.is_some(),
        group.managed_worker_last.is_some(),
        group.managed_timeout_seconds.is_some(),
        group.managed_registration_user.is_some(),
        group.managed_registration_pwd_env.is_some(),
        group.managed_infobase_user.is_some(),
        group.managed_infobase_pwd_env.is_some(),
    ];
    if present.iter().all(|value| !value) {
        return Ok(false);
    }
    if !present.iter().all(|value| *value) {
        bail!(
            "managed worker parameters require --managed-worker-parent and all twelve child parameters before any effects"
        );
    }
    Ok(true)
}

fn require_fresh_selection(group: &ManagedWorkerCliArgs) -> Result<()> {
    if !selects_fresh(group)? {
        bail!("fresh managed entry requires the complete managed worker group");
    }
    Ok(())
}
/// These values are resolved exactly once before creation. They are separate
/// domains even when the operator deliberately chooses the same SQL login.
struct ResolvedInputs {
    creator: CreatorOptions,
    sql_user: String,
    sql_password: String,
}

fn required<T: Clone>(value: &Option<T>, label: &str) -> Result<T> {
    value
        .clone()
        .with_context(|| format!("fresh managed worker requires {label}"))
}
fn nonempty(value: String, label: &str) -> Result<String> {
    if value.trim().is_empty() {
        bail!("fresh managed worker requires nonempty {label}");
    }
    Ok(value)
}
fn password_slot(
    slot: &Option<String>,
    label: &str,
    lookup: &mut impl FnMut(&str) -> Result<String>,
) -> Result<String> {
    let name = nonempty(required(slot, label)?, label)?;
    // Neither the slot name nor its secret value enters diagnostics.
    let value =
        lookup(&name).map_err(|_| anyhow::anyhow!("managed credential slot unavailable"))?;
    nonempty(value, "credential value")
}

fn validate_policy(
    profile: MssqlNativePlatformProfile,
    mode: MssqlMainActivationModeArg,
    dry_run: bool,
    acknowledged: bool,
    sqlcmd: Option<&Path>,
    bcp: Option<&Path>,
    checkpoint: bool,
    interrupt: bool,
    old_cluster: Option<uuid::Uuid>,
    old_infobase: Option<uuid::Uuid>,
    rac: &Path,
    endpoint: &str,
    old_user: Option<&str>,
    old_password: Option<&str>,
) -> Result<()> {
    if profile != MssqlNativePlatformProfile::Platform8_3_27_2214
        || mode != MssqlMainActivationModeArg::Worker
    {
        bail!("fresh managed worker requires platform-8.3.27.2214 and --mode worker");
    }
    if dry_run {
        bail!(
            "fresh managed dry-run is refused before creator effects; use an existing verified target for dry-run"
        );
    }
    if !acknowledged {
        bail!("--allow-non-lab acknowledgement is required before managed creation");
    }
    if sqlcmd.is_some() || bcp.is_some() || checkpoint || interrupt {
        bail!(
            "fresh managed worker requires built-in SQL and no live checkpoint/session interruption"
        );
    }
    if old_cluster.is_some()
        || old_infobase.is_some()
        || rac != Path::new("rac")
        || endpoint != "localhost:1545"
        || old_user.is_some()
        || old_password.is_some()
    {
        bail!(
            "fresh managed worker cannot adopt a supplied UUID, endpoint, rac or infobase credential"
        );
    }
    Ok(())
}

fn resolve_inputs(
    group: &ManagedWorkerCliArgs,
    server: &str,
    database: &str,
    sql_user: Option<&str>,
    sql_password: Option<&str>,
    sql_env: &str,
) -> Result<ResolvedInputs> {
    resolve_inputs_with(
        group,
        server,
        database,
        sql_user,
        sql_password,
        sql_env,
        |name| std::env::var(name).map_err(|_| anyhow::anyhow!("credential slot unavailable")),
    )
}
fn resolve_inputs_with(
    group: &ManagedWorkerCliArgs,
    server: &str,
    database: &str,
    sql_user: Option<&str>,
    sql_password: Option<&str>,
    sql_env: &str,
    mut lookup: impl FnMut(&str) -> Result<String>,
) -> Result<ResolvedInputs> {
    let parent = required(&group.managed_worker_parent, "--managed-worker-parent")?;
    let platform_bin = required(&group.managed_platform_bin, "--managed-platform-bin")?;
    let powershell = required(&group.managed_powershell, "--managed-powershell")?;
    if !parent.is_absolute() || !platform_bin.is_absolute() || !powershell.is_absolute() {
        bail!("managed parent and tool paths must be absolute");
    }
    let agent_port = required(&group.managed_agent_port, "--managed-agent-port")?;
    let cluster_port = required(&group.managed_cluster_port, "--managed-cluster-port")?;
    let ras_port = required(&group.managed_ras_port, "--managed-ras-port")?;
    let worker_first = required(&group.managed_worker_first, "--managed-worker-first")?;
    let worker_last = required(&group.managed_worker_last, "--managed-worker-last")?;
    let timeout = required(&group.managed_timeout_seconds, "--managed-timeout-seconds")?;
    if timeout == 0
        || timeout > 120
        || worker_first == 0
        || worker_last < worker_first
        || worker_last - worker_first > 127
    {
        bail!(
            "managed timeout must be 1..120 seconds and worker range must contain 1..128 nonzero ports"
        );
    }
    let ports = [agent_port, cluster_port, ras_port];
    if ports.contains(&0)
        || ports[0] == ports[1]
        || ports[0] == ports[2]
        || ports[1] == ports[2]
        || ports
            .iter()
            .any(|port| (worker_first..=worker_last).contains(port))
    {
        bail!("managed ports must be nonzero, distinct and outside the worker range");
    }
    let database_server = nonempty(server.to_owned(), "SQL server")?;
    let database = nonempty(database.to_owned(), "database")?;
    let sql_user = nonempty(
        sql_user
            .context("fresh managed worker requires explicit --sql-user")?
            .to_owned(),
        "SQL login",
    )?;
    let sql_password = match sql_password {
        Some(value) => value.to_owned(),
        None => lookup(sql_env).map_err(|_| anyhow::anyhow!("SQL credential slot unavailable"))?,
    };
    let sql_password = nonempty(sql_password, "SQL password")?;
    let database_user = nonempty(
        required(
            &group.managed_registration_user,
            "--managed-registration-user",
        )?,
        "registration SQL login",
    )?;
    let database_password = password_slot(
        &group.managed_registration_pwd_env,
        "--managed-registration-pwd-env",
        &mut lookup,
    )?;
    let infobase_user = nonempty(
        required(&group.managed_infobase_user, "--managed-infobase-user")?,
        "infobase user",
    )?;
    let infobase_password = password_slot(
        &group.managed_infobase_pwd_env,
        "--managed-infobase-pwd-env",
        &mut lookup,
    )?;
    Ok(ResolvedInputs {
        creator: CreatorOptions {
            parent,
            platform_bin,
            powershell,
            agent_port,
            cluster_port,
            ras_port,
            worker_first,
            worker_last,
            database_server,
            database,
            database_user,
            database_password,
            infobase_user,
            infobase_password,
            timeout: Duration::from_secs(timeout),
        },
        sql_user,
        sql_password,
    })
}

pub(crate) fn require_completed(outcome: &ManagedOutcome) -> Result<()> {
    if outcome.publication != PublicationState::Committed
        || !outcome.handoff_proved
        || outcome.diagnostic.is_some()
    {
        // Do not print the underlying native/SQL error; it may carry credentials.
        bail!(
            "managed publication or replacement handoff unproved; original owner must remain resident"
        );
    }
    Ok(())
}

// Each failure preserves its original error/panic object alongside the owner.
// These values are deliberately never formatted or serialized as authority.
struct Failure {
    phase: &'static str,
    _error: Option<anyhow::Error>,
    _panic: Option<Box<dyn Any + Send>>,
}
fn guarded<T>(
    phase: &'static str,
    operation: impl FnOnce() -> Result<T>,
) -> std::result::Result<T, Failure> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(Failure {
            phase,
            _error: Some(error),
            _panic: None,
        }),
        Err(payload) => Err(Failure {
            phase,
            _error: None,
            _panic: Some(payload),
        }),
    }
}

enum Fresh<O, R> {
    Ready(O),
    Retained(R),
}
enum Retained<O, R, T> {
    Ready {
        owner: O,
        failure: Failure,
        _completed_report: Option<T>,
    },
    Creation(R),
}
enum Completion<T, O, R> {
    Known(T),
    Unknown(Retained<O, R, T>),
}

/// SAME owner throughout register/load, consumer borrow and cold proof. A
/// failure does not run shutdown, SQL undo, retry or a new creator.
fn drive<O, R, T>(
    fresh: Fresh<O, R>,
    register: impl FnOnce(&mut O) -> Result<()>,
    consume: impl FnOnce(&mut O) -> Result<T>,
    cold: impl FnOnce(&mut O) -> Result<()>,
) -> Completion<T, O, R> {
    let mut owner = match fresh {
        Fresh::Ready(owner) => owner,
        Fresh::Retained(owner) => return Completion::Unknown(Retained::Creation(owner)),
    };
    if let Err(failure) = guarded("register/load", || register(&mut owner)) {
        return Completion::Unknown(Retained::Ready {
            owner,
            failure,
            _completed_report: None,
        });
    }
    let value = match guarded("publication/handoff", || consume(&mut owner)) {
        Ok(value) => value,
        Err(failure) => {
            return Completion::Unknown(Retained::Ready {
                owner,
                failure,
                _completed_report: None,
            });
        }
    };
    if let Err(failure) = guarded("owned cold shutdown", || cold(&mut owner)) {
        return Completion::Unknown(Retained::Ready {
            owner,
            failure,
            _completed_report: Some(value),
        });
    }
    Completion::Known(value)
}

struct CreatorRetained {
    _runtime: super::native::NativeRuntime,
    journal: Journal,
    _diagnostic: String,
}
fn fresh(options: CreatorOptions) -> Result<Fresh<NativeManagedWorker, CreatorRetained>> {
    Ok(match super::create(options)? {
        Creation::Ready(owner) => Fresh::Ready(owner),
        Creation::Retained {
            runtime,
            journal,
            diagnostic,
        } => Fresh::Retained(CreatorRetained {
            _runtime: runtime,
            journal,
            _diagnostic: diagnostic,
        }),
    })
}

fn finish<T>(completion: Completion<T, NativeManagedWorker, CreatorRetained>) -> Result<T> {
    match completion {
        Completion::Known(value) => Ok(value),
        Completion::Unknown(retained) => {
            let _ = catch_unwind(AssertUnwindSafe(|| match &retained {
                Retained::Ready { owner, failure, .. } => eprintln!(
                    "managed UNKNOWN: phase={}; root={}; nonce={}; journal={}; cluster={}; infobase={}; retaining ORIGINAL owner; no retry/undo/cleanup",
                    failure.phase,
                    owner.binding.root.display(),
                    owner.binding.nonce,
                    owner.journal.path.display(),
                    owner.binding.cluster,
                    owner.binding.infobase
                ),
                Retained::Creation(owner) => eprintln!(
                    "managed UNKNOWN: phase=creation; root={}; nonce={}; journal={}; retaining ORIGINAL runtime and pipes; no retry/adoption/cleanup",
                    owner
                        .journal
                        .path
                        .parent()
                        .unwrap_or_else(|| Path::new(""))
                        .display(),
                    owner.journal.nonce,
                    owner.journal.path.display()
                ),
            }));
            // Resident policy: no ordinary Err unwinding drops original process,
            // reader, kernel-handle or exclusive journal ownership. A wake does
            // not grant a retry. External termination is an operator decision.
            loop {
                std::hint::black_box(&retained);
                std::thread::park();
            }
        }
    }
}

/// This private value is minted only from the same Ready owner's live binding.
/// It contains no authority by itself: every consumer still borrows that owner.
struct GeneratedTarget {
    rac: PathBuf,
    endpoint: String,
    server: String,
    database: String,
    cluster: uuid::Uuid,
    infobase: uuid::Uuid,
}
impl GeneratedTarget {
    fn from_owner(owner: &mut NativeManagedWorker, resolved: &ResolvedInputs) -> Result<Self> {
        owner.prepare()?;
        let binding = &owner.binding;
        if binding.cluster.is_nil()
            || binding.infobase.is_nil()
            || binding.database != resolved.creator.database
        {
            bail!("fresh managed target binding incomplete or changed");
        }
        Ok(Self {
            rac: resolved.creator.platform_bin.join("rac.exe"),
            endpoint: format!("localhost:{}", resolved.creator.ras_port),
            server: resolved.creator.database_server.clone(),
            database: binding.database.clone(),
            cluster: binding.cluster,
            infobase: binding.infobase,
        })
    }
}
fn bind_source(
    target: &GeneratedTarget,
    input: &MssqlApplySourceChangeArgs,
    resolved: &ResolvedInputs,
) -> MssqlApplySourceChangeArgs {
    let mut args = input.clone();
    args.managed_worker = Default::default();
    args.rac = target.rac.clone();
    args.ras_endpoint = target.endpoint.clone();
    args.server = target.server.clone();
    args.database = target.database.clone();
    args.cluster_id = Some(target.cluster);
    args.infobase_id = Some(target.infobase);
    args.sql_user = Some(resolved.sql_user.clone());
    args.sql_pwd = Some(resolved.sql_password.clone());
    args.infobase_user = Some(resolved.creator.infobase_user.clone());
    args.infobase_pwd = Some(resolved.creator.infobase_password.clone());
    args
}
fn bind_activation(
    target: &GeneratedTarget,
    input: &MssqlActivateStagedMainArgs,
    resolved: &ResolvedInputs,
) -> MssqlActivateStagedMainArgs {
    let mut args = input.clone();
    args.managed_worker = Default::default();
    args.rac = target.rac.clone();
    args.ras_endpoint = target.endpoint.clone();
    args.server = target.server.clone();
    args.database = target.database.clone();
    args.cluster_id = Some(target.cluster);
    args.infobase_id = Some(target.infobase);
    args.sql_user = Some(resolved.sql_user.clone());
    args.sql_pwd = Some(resolved.sql_password.clone());
    args.infobase_user = Some(resolved.creator.infobase_user.clone());
    args.infobase_pwd = Some(resolved.creator.infobase_password.clone());
    args
}
fn cold(owner: &mut NativeManagedWorker) -> Result<()> {
    owner.shutdown()?.require_current()
}

pub(crate) fn apply(
    args: &MssqlApplySourceChangeArgs,
) -> Result<crate::mssql_apply::MssqlApplySourceChangeReport> {
    require_fresh_selection(&args.managed_worker)?;
    if args.tail_log_output.is_some() {
        bail!(
            "--tail-log-output is only valid for live activation; fresh managed worker refused before creator effects"
        );
    }
    if args.watch {
        bail!(
            "fresh managed --watch is refused before creator effects; one owned lifetime per invocation"
        );
    }
    if args.extension.is_some() {
        bail!("fresh managed source apply supports main configuration only");
    }
    validate_policy(
        args.platform_profile,
        args.mode,
        args.dry_run,
        args.allow_non_lab,
        args.sqlcmd.as_deref(),
        args.bcp_executable.as_deref(),
        args.live_checkpoint,
        args.interrupt_sessions,
        args.cluster_id,
        args.infobase_id,
        &args.rac,
        &args.ras_endpoint,
        args.infobase_user.as_deref(),
        args.infobase_pwd.as_deref(),
    )?;
    crate::mssql_apply::require_supported_main_source_cohort(args)?;
    if !cfg!(windows) {
        bail!("fresh managed worker requires Windows");
    }
    crate::mssql_apply::preflight_managed_source_inputs(args)?;
    let resolved = resolve_inputs(
        &args.managed_worker,
        &args.server,
        &args.database,
        args.sql_user.as_deref(),
        args.sql_pwd.as_deref(),
        &args.sql_pwd_env,
    )?;
    let created = fresh(resolved.creator.clone())?;
    finish(drive(
        created,
        NativeManagedWorker::register_and_load,
        |owner| {
            let generated = GeneratedTarget::from_owner(owner, &resolved)?;
            let target = bind_source(&generated, args, &resolved);
            crate::mssql_apply::apply_source_change_managed(&target, owner)
        },
        cold,
    ))
}

pub(crate) fn activate(
    args: &MssqlActivateStagedMainArgs,
) -> Result<crate::mssql::MssqlActivateStagedMainReport> {
    require_fresh_selection(&args.managed_worker)?;
    if args.tail_log_output.is_some() {
        bail!(
            "--tail-log-output is only valid for live activation; fresh managed worker refused before creator effects"
        );
    }
    if args.live_compact_recovery {
        bail!("fresh managed worker does not support live recovery");
    }
    validate_policy(
        args.platform_profile,
        args.mode,
        args.dry_run,
        args.allow_non_lab,
        args.sqlcmd.as_deref(),
        args.bcp_executable.as_deref(),
        args.live_checkpoint,
        args.interrupt_sessions,
        args.cluster_id,
        args.infobase_id,
        &args.rac,
        &args.ras_endpoint,
        args.infobase_user.as_deref(),
        args.infobase_pwd.as_deref(),
    )?;
    if !cfg!(windows) {
        bail!("fresh managed worker requires Windows");
    }
    let resolved = resolve_inputs(
        &args.managed_worker,
        &args.server,
        &args.database,
        args.sql_user.as_deref(),
        args.sql_pwd.as_deref(),
        &args.sql_pwd_env,
    )?;
    let created = fresh(resolved.creator.clone())?;
    finish(drive(
        created,
        NativeManagedWorker::register_and_load,
        |owner| {
            let generated = GeneratedTarget::from_owner(owner, &resolved)?;
            let target = bind_activation(&generated, args, &resolved);
            crate::mssql::activate_staged_main_managed(&target, owner)
        },
        cold,
    ))
}

#[cfg(test)]
mod tests;
