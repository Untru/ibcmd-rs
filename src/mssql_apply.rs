//! High-level, fail-closed orchestration for one non-structural source change.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::cli::{
    MssqlActivateStagedExtensionArgs, MssqlActivateStagedMainArgs, MssqlApplySourceChangeArgs,
    MssqlAuditSourceParityArgs, MssqlDumpConfigArgs, MssqlDumpExtensionArgs,
    MssqlLoadExtensionArgs, MssqlMainActivationModeArg, MssqlStageSourceObjectsArgs,
};
use crate::mssql_source_change::{
    ActivationMode, ActivationTarget, SourceFileDigest, SourceInventory, classify_source_change,
};
use crate::sql::{SqlExec, SqlOptions};

#[derive(Debug, Serialize)]
pub struct MssqlApplySourceChangeReport {
    pub schema_version: u32,
    pub database: String,
    pub claimed_platform_profile: String,
    pub verified_platform_profile: String,
    pub storage_schema_sha256: String,
    pub target: String,
    pub selected_path: String,
    pub mode: String,
    pub dry_run: bool,
    pub executed: bool,
    pub no_op: bool,
    pub changed_paths: Vec<String>,
    pub selected_storage_file_names: Vec<String>,
    pub active_generation_or_root: Option<String>,
    pub proposed_generation_or_root: Option<String>,
    pub tables_changed: Vec<String>,
    pub recovery_token: Option<String>,
    pub staging: Option<Value>,
    pub activation: Option<Value>,
    /// What the live gate found before the stage (#409 F-9, F-10); only for the `live` mode on the built-in SQL client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_gate: Option<crate::mssql_live_gate::LiveGateReport>,
    pub timings: MssqlApplySourceChangeTimings,
}

#[derive(Debug, Default, Serialize)]
pub struct MssqlApplySourceChangeTimings {
    pub active_export_ms: u128,
    pub classification_ms: u128,
    pub staging_ms: u128,
    pub activation_ms: u128,
    pub total_ms: u128,
}

pub fn apply_source_change(
    args: &MssqlApplySourceChangeArgs,
) -> Result<MssqlApplySourceChangeReport> {
    if crate::mssql_managed_worker::cli::selects_fresh(&args.managed_worker)? {
        return crate::mssql_managed_worker::cli::apply(args);
    }
    apply_source_change_inner(args, None)
}

pub(crate) fn apply_source_change_managed(
    args: &MssqlApplySourceChangeArgs,
    session: &mut crate::mssql_managed_worker::NativeManagedWorker,
) -> Result<MssqlApplySourceChangeReport> {
    apply_source_change_inner(args, Some(session))
}

fn apply_source_change_inner(
    args: &MssqlApplySourceChangeArgs,
    mut managed: Option<&mut crate::mssql_managed_worker::NativeManagedWorker>,
) -> Result<MssqlApplySourceChangeReport> {
    if args.live_checkpoint {
        bail!(
            "--live-checkpoint is only supported by mssql-activate-staged-main; concurrent source staging is not established, so source apply refused before staging"
        );
    }

    let total_started = Instant::now();
    // Declared policy first: an unsupported build must not reach rac, sqlcmd or
    // the source tree.
    if args.extension.is_some() {
        args.platform_profile.require_extension_write_supported()?;
    } else {
        args.platform_profile.require_main_write_supported()?;
    }
    if args.extension.is_none() {
        require_supported_main_source_cohort(args)?;
        preflight_tail_log(args)?;
        crate::mssql_main_activation::preflight_interrupt_sessions(
            activation_mode(args.mode),
            args.interrupt_sessions,
        )
        .map_err(anyhow::Error::new)?;
    }
    if args.live_checkpoint
        && args.platform_profile
            != crate::mssql_platform_profile::MssqlNativePlatformProfile::Platform8_3_27_2214
    {
        bail!("live continuation checkpoint is measured on platform-8.3.27.2214 only");
    }
    // The argument checks come before the verification, which costs two rac
    // calls and a SQL probe (#409 F-6).
    if !args.dry_run && !args.allow_non_lab {
        bail!("--allow-non-lab acknowledgement is required for source activation");
    }
    if args.extension.is_some()
        && matches!(
            args.mode,
            MssqlMainActivationModeArg::Live | MssqlMainActivationModeArg::Worker
        )
    {
        bail!("live/worker activation is not supported for extensions; use online or exclusive");
    }
    let verification_options =
        crate::mssql_platform_profile::MssqlNativeProfileVerificationOptions {
            sqlcmd: args.sqlcmd.as_deref(),
            rac: &args.rac,
            ras_endpoint: &args.ras_endpoint,
            server: &args.server,
            database: &args.database,
            cluster_id: args.cluster_id,
            infobase_id: args.infobase_id,
            infobase_user: args.infobase_user.as_deref(),
            infobase_pwd: args.infobase_pwd.as_deref(),
            sql_user: args.sql_user.as_deref(),
            sql_pwd: args.sql_pwd.as_deref(),
            sql_pwd_env: &args.sql_pwd_env,
            sqlcmd_trust_cert: args.sqlcmd_trust_cert,
        };
    let profile_verification = if let Some(session) = managed.as_deref_mut() {
        session.verify_target_profile(args.platform_profile, verification_options)?
    } else {
        crate::mssql_platform_profile::verify_mssql_native_profile(
            args.platform_profile,
            verification_options,
        )?
    };

    let source_root = fs::canonicalize(&args.source_root)
        .with_context(|| format!("failed to canonicalize {}", args.source_root.display()))?;
    if !source_root.is_dir() {
        bail!("source root is not a directory: {}", source_root.display());
    }
    let selected_path = normalize_relative_path(&args.source_path)?;
    let work = TemporaryApplyRoot::new()?;
    let active_root = work.path.join("active");
    let proposed_root = work.path.join("proposed");

    let bounded_source_paths = selected_source_closure_paths(&source_root, &selected_path)?;
    let selected_storage_file_names = if args.extension.is_none() {
        selected_storage_file_names_for_source_paths(&source_root, &bounded_source_paths)?
    } else {
        Vec::new()
    };

    let export_started = Instant::now();
    let active_generation_or_root = if let Some(extension) = args.extension.as_deref() {
        let report = crate::mssql_extension_export::dump_extensions(&MssqlDumpExtensionArgs {
            sqlcmd: args.sqlcmd.clone(),
            bcp_executable: args.bcp_executable.clone(),
            server: args.server.clone(),
            sql_user: args.sql_user.clone(),
            sql_pwd: args.sql_pwd.clone(),
            sql_pwd_env: args.sql_pwd_env.clone(),
            sqlcmd_trust_cert: args.sqlcmd_trust_cert,
            database: args.database.clone(),
            extension: Some(extension.to_owned()),
            all_extensions: false,
            output_dir: active_root.clone(),
            overwrite: false,
            // The generation a change applies to is the registry's.
            image: crate::cli::MssqlExtensionImage::Active,
            platform: None,
            source_version: args.source_version,
        })?;
        report
            .extensions
            .first()
            .map(|entry| entry.active_cas_root.clone())
    } else if let Some(generation) = export_active_managed_form_fast(
        args,
        &selected_path,
        &selected_storage_file_names,
        &active_root,
    )? {
        generation
    } else {
        let report = crate::mssql_dump::dump_config_with_sql(
            &MssqlDumpConfigArgs {
                rows_dir: None,
                model_export: false,
                legacy_export: false,
                sqlcmd: args.sqlcmd.clone(),
                bcp_executable: args.bcp_executable.clone(),
                runtime_journal: None,
                server: args.server.clone(),
                sql_user: args.sql_user.clone(),
                sql_pwd: args.sql_pwd.clone(),
                sql_pwd_env: args.sql_pwd_env.clone(),
                database: args.database.clone(),
                output_dir: active_root.clone(),
                overwrite: false,
                include_config_save: false,
                main_configuration: false,
                file_names: selected_storage_file_names.clone(),
                file_name_lists: Vec::new(),
                inflate: false,
                extract_module_text: true,
                extract_metadata_xml: true,
                require_complete_root_metadata: false,
                require_complete_source_assets: false,
                collect_all_source_asset_diagnostics: false,
                platform: None,
                source_version: args.source_version,
                objects: Vec::new(),
                no_binary_rows: true,
                write_binary_rows: false,
                write_manifest: false,
                base: None,
                sync: false,
            },
            Some(main_read_sql(args)?),
        )?;
        ensure_bounded_export_complete(
            &active_root,
            &bounded_source_paths,
            &selected_storage_file_names,
            &report,
        )?;
        active_dynamic_generation(args)?
    };
    let active_export_ms = export_started.elapsed().as_millis();

    copy_source_tree(&active_root, &proposed_root)?;
    overlay_selected_bodies(&source_root, &active_root, &proposed_root, &selected_path)?;

    let classify_started = Instant::now();
    let active_inventory = inventory_from_root(&active_root)?;
    let proposed_inventory = inventory_from_root(&proposed_root)?;
    let target = match args.extension.as_deref() {
        Some(name) => ActivationTarget::extension(name.to_owned())?,
        None => ActivationTarget::Main,
    };
    let activation_mode = match args.mode {
        MssqlMainActivationModeArg::Online => ActivationMode::Online,
        MssqlMainActivationModeArg::Exclusive
        | MssqlMainActivationModeArg::Live
        | MssqlMainActivationModeArg::Worker => ActivationMode::Exclusive,
    };
    let classified = classify_source_change(
        &active_inventory,
        &proposed_inventory,
        &selected_path,
        target,
        activation_mode,
    )?;
    let classification_ms = classify_started.elapsed().as_millis();
    let changed_paths = classified.changed_paths().to_vec();
    let no_op = classified.is_no_op();
    let target_name = args
        .extension
        .as_ref()
        .map(|name| format!("extension:{name}"))
        .unwrap_or_else(|| "main".to_owned());
    let mode_name = match args.mode {
        MssqlMainActivationModeArg::Online => "online",
        MssqlMainActivationModeArg::Exclusive => "exclusive",
        MssqlMainActivationModeArg::Live => "live",
        MssqlMainActivationModeArg::Worker => "worker",
    }
    .to_owned();

    if no_op {
        return Ok(MssqlApplySourceChangeReport {
            schema_version: 1,
            database: args.database.clone(),
            claimed_platform_profile: profile_verification.claimed_platform_profile.clone(),
            verified_platform_profile: profile_verification.verified_platform_profile.clone(),
            storage_schema_sha256: profile_verification.storage_schema_sha256.clone(),
            target: target_name,
            selected_path,
            mode: mode_name,
            dry_run: args.dry_run,
            executed: false,
            no_op: true,
            changed_paths,
            active_generation_or_root: active_generation_or_root.clone(),
            proposed_generation_or_root: active_generation_or_root,
            selected_storage_file_names,
            tables_changed: Vec::new(),
            recovery_token: None,
            staging: None,
            activation: None,
            live_gate: None,
            timings: MssqlApplySourceChangeTimings {
                active_export_ms,
                classification_ms,
                total_ms: total_started.elapsed().as_millis(),
                ..Default::default()
            },
        });
    }
    if let Some(session) = managed.as_deref_mut() {
        if !args.dry_run {
            session.require_source_target(args)?;
        }
    } else {
        preflight_classified_worker_source(args.mode, args.dry_run, &classified)?;
    }
    if args.extension.is_none() {
        preflight_main_publication(args)?;
    }
    // An exclusive activation is refused on the cluster's word when the infobase has
    // clients (#409 F-3), and on the database's when a session other than the tool's
    // own RAS verification is on it; asking before the stage keeps `ConfigSave`
    // untouched. The gate inside the activation's transaction stays the last word.
    if matches!(args.mode, MssqlMainActivationModeArg::Exclusive) {
        let own = crate::mssql_platform_profile::own_ras_processes_for_exclusive(
            &args.rac,
            &args.ras_endpoint,
            profile_verification.verified_cluster_id,
            profile_verification.verified_infobase_id,
        )?;
        crate::mssql_platform_profile::refuse_foreign_sessions(
            &main_read_sql(args)?,
            &args.database,
            &own,
        )?;
    }
    // The live switch is refused, before the stage, when the database has no log backup chain, the tail directory is not there or
    // not writable, or sessions have open work that the switch would roll back (#409 F-9, F-10). The gate at the head of the
    // activation script stays the last word.
    let live_gate = match (
        args.extension.is_none() && matches!(args.mode, MssqlMainActivationModeArg::Live),
        args.tail_log_output.as_deref().and_then(Path::to_str),
    ) {
        (true, Some(tail)) => crate::mssql_live_gate::preflight_live(
            &main_read_sql(args)?,
            &args.database,
            tail,
            args.interrupt_sessions,
            !args.dry_run,
        )?,
        _ => None,
    };
    prepare_compile_tree_for_selected_change(&proposed_root, &selected_path)?;

    let path_prefix = owner_prefix(&selected_path)?;
    let staging_started = Instant::now();
    let build_stage = || -> Result<Value> {
        let staging = if let Some(extension) = args.extension.as_deref() {
            let load_args = MssqlLoadExtensionArgs {
                platform_profile: args.platform_profile,
                rac: args.rac.clone(),
                ras_endpoint: args.ras_endpoint.clone(),
                cluster_id: args.cluster_id,
                infobase_id: args.infobase_id,
                infobase_user: args.infobase_user.clone(),
                infobase_pwd: args.infobase_pwd.clone(),
                sqlcmd: args.sqlcmd.clone(),
                bcp_executable: args.bcp_executable.clone(),
                server: args.server.clone(),
                sql_user: args.sql_user.clone(),
                sql_pwd: args.sql_pwd.clone(),
                sql_pwd_env: args.sql_pwd_env.clone(),
                database: args.database.clone(),
                extension: Some(extension.to_owned()),
                all_extensions: false,
                input_dir: proposed_root.clone(),
                path_prefix: vec![path_prefix.clone()],
                replace_staging: true,
                dry_run: args.dry_run,
                allow_non_lab: args.allow_non_lab,
                sqlcmd_trust_cert: args.sqlcmd_trust_cert,
                platform: None,
                source_version: args.source_version,
            };
            // This command stages and publishes together, so the publisher's
            // registry requirement is verified before the first staged row.
            crate::mssql_extension_load::require_publishable_extension_registry(&load_args)?;
            serde_json::to_value(crate::mssql_extension_load::load_extensions(&load_args)?)?
        } else if args.dry_run {
            if args.sql_user.is_some() {
                bail!(
                    "main dry-run with SQL authentication is not yet supported by the parity auditor"
                );
            }
            serde_json::to_value(crate::mssql::audit_source_parity_with_sql(
                &MssqlAuditSourceParityArgs {
                    server: args.server.clone(),
                    database: args.database.clone(),
                    source_root: proposed_root.clone(),
                    sqlcmd: args.sqlcmd.clone(),
                    batch_size: Some(1),
                    platform: None,
                    source_version: Some(args.source_version),
                    path_prefix: vec![path_prefix.clone()],
                    output: None,
                },
                Some(main_read_sql(args)?),
            )?)?
        } else {
            serde_json::to_value(crate::mssql::stage_source_objects_with_sql(
                &MssqlStageSourceObjectsArgs {
                    server: args.server.clone(),
                    sql_user: args.sql_user.clone(),
                    sql_pwd: args.sql_pwd.clone(),
                    sql_pwd_env: args.sql_pwd_env.clone(),
                    database: args.database.clone(),
                    source_root: proposed_root.clone(),
                    sqlcmd: args.sqlcmd.clone(),
                    replace_config_save: true,
                    allow_non_lab: args.allow_non_lab,
                    batch_size: Some(1),
                    platform: None,
                    source_version: Some(args.source_version),
                    path_prefix: vec![path_prefix.clone()],
                    files: Vec::new(),
                    script_output: None,
                    script_only: false,
                    bulk: false,
                    // One object: a bulk read of every Config row would cost more
                    // than the handful of per-object queries it replaces.
                    per_row: true,
                    bcp_executable: None,
                    base_free: false,
                    verify: false,
                },
                Some(main_read_sql(args)?),
            )?)?
        };
        Ok(staging)
    };
    let staging = if let Some(session) = managed.as_deref_mut().filter(|_| !args.dry_run) {
        session.stage_source(build_stage)?
    } else {
        build_stage()?
    };
    if args.dry_run && args.extension.is_none() {
        ensure_main_dry_run_stageable(&staging)?;
    }
    let staging_ms = staging_started.elapsed().as_millis();

    let activation_started = Instant::now();
    let mut activate_main = |activation_args: &MssqlActivateStagedMainArgs, profile| {
        if let Some(session) = managed.as_deref_mut() {
            crate::mssql::activate_staged_main_managed_verified(activation_args, profile, session)
        } else {
            crate::mssql::activate_staged_main_verified(activation_args, profile)
        }
    };
    let activation = if args.dry_run {
        None
    } else if let Some(extension) = args.extension.as_deref() {
        Some(serde_json::to_value(
            crate::mssql_extension_load::activate_staged_extension_verified(
                &MssqlActivateStagedExtensionArgs {
                    platform_profile: args.platform_profile,
                    rac: args.rac.clone(),
                    ras_endpoint: args.ras_endpoint.clone(),
                    cluster_id: args.cluster_id,
                    infobase_id: args.infobase_id,
                    infobase_user: args.infobase_user.clone(),
                    infobase_pwd: args.infobase_pwd.clone(),
                    sqlcmd: args.sqlcmd.clone(),
                    bcp_executable: args.bcp_executable.clone(),
                    server: args.server.clone(),
                    sql_user: args.sql_user.clone(),
                    sql_pwd: args.sql_pwd.clone(),
                    sql_pwd_env: args.sql_pwd_env.clone(),
                    database: args.database.clone(),
                    extension: extension.to_owned(),
                    mode: args.mode,
                    dry_run: false,
                    allow_non_lab: args.allow_non_lab,
                    sqlcmd_trust_cert: args.sqlcmd_trust_cert,
                    script_output: args.script_output.clone(),
                    recovery_output: args.recovery_output.clone(),
                },
                profile_verification.clone(),
            )?,
        )?)
    } else {
        Some(serde_json::to_value(activate_main(
            &MssqlActivateStagedMainArgs {
                managed_worker: Default::default(),
                live_checkpoint: false,
                live_compact_recovery: false,
                platform_profile: args.platform_profile,
                sqlcmd_trust_cert: args.sqlcmd_trust_cert,
                sqlcmd: args.sqlcmd.clone(),
                bcp_executable: args.bcp_executable.clone(),
                server: args.server.clone(),
                sql_user: args.sql_user.clone(),
                sql_pwd: args.sql_pwd.clone(),
                sql_pwd_env: args.sql_pwd_env.clone(),
                database: args.database.clone(),
                mode: args.mode,
                dry_run: false,
                allow_non_lab: args.allow_non_lab,
                script_output: args.script_output.clone(),
                recovery_output: args.recovery_output.clone(),
                tail_log_output: args.tail_log_output.clone(),
                interrupt_sessions: args.interrupt_sessions,
                rac: args.rac.clone(),
                ras_endpoint: args.ras_endpoint.clone(),
                cluster_id: args.cluster_id,
                infobase_id: args.infobase_id,
                infobase_user: args.infobase_user.clone(),
                infobase_pwd: args.infobase_pwd.clone(),
            },
            profile_verification.clone(),
        )?)?)
    };
    let activation_ms = activation_started.elapsed().as_millis();

    let proposed_generation_or_root = activation
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/activation/new_generation")
                .or_else(|| value.pointer("/activation/published_root_sha1"))
        })
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            staging
                .pointer("/extensions/0/proposed_cas_root")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let old_from_activation = activation
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/activation/old_generation")
                .or_else(|| value.pointer("/activation/active_root_sha1"))
        })
        .and_then(Value::as_str)
        .map(str::to_owned);
    let recovery_token = activation
        .as_ref()
        .and_then(|value| value.pointer("/activation/recovery_token"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let tables_changed = if args.dry_run {
        Vec::new()
    } else if args.extension.is_some() {
        vec![
            "ConfigCASSave".to_owned(),
            "ConfigCAS".to_owned(),
            "_ExtensionsInfo".to_owned(),
        ]
    } else if let Some(touched) = tables_touched_by_config_apply(activation.as_ref()) {
        // the exclusive mode is carried out by the own apply (#408 step 2), which also writes the change
        // registrations and `Files`; the tables are the ones it names
        touched
    } else {
        vec![
            "ConfigSave".to_owned(),
            "Config".to_owned(),
            "Params".to_owned(),
        ]
    };

    Ok(MssqlApplySourceChangeReport {
        schema_version: 1,
        database: args.database.clone(),
        claimed_platform_profile: profile_verification.claimed_platform_profile,
        verified_platform_profile: profile_verification.verified_platform_profile,
        storage_schema_sha256: profile_verification.storage_schema_sha256,
        target: target_name,
        selected_path,
        mode: mode_name,
        dry_run: args.dry_run,
        executed: !args.dry_run,
        no_op: false,
        changed_paths,
        active_generation_or_root: old_from_activation.or(active_generation_or_root),
        proposed_generation_or_root,
        selected_storage_file_names,
        tables_changed,
        recovery_token,
        staging: Some(staging),
        activation,
        live_gate,
        timings: MssqlApplySourceChangeTimings {
            active_export_ms,
            classification_ms,
            staging_ms,
            activation_ms,
            total_ms: total_started.elapsed().as_millis(),
        },
    })
}

pub fn watch_source_changes(args: &MssqlApplySourceChangeArgs) -> Result<()> {
    if crate::mssql_managed_worker::cli::selects_fresh(&args.managed_worker)? {
        bail!("fresh managed --watch is refused before creator effects");
    }
    if args.live_checkpoint {
        bail!(
            "--live-checkpoint is only supported by mssql-activate-staged-main; source watch refused before staging"
        );
    }

    args.platform_profile.require_main_write_supported()?;
    crate::mssql_worker_switch::preflight_worker_execution(
        activation_mode(args.mode),
        args.dry_run,
        false,
    )?;
    require_supported_main_source_cohort(args)?;
    crate::mssql_platform_profile::verify_mssql_native_profile(
        args.platform_profile,
        crate::mssql_platform_profile::MssqlNativeProfileVerificationOptions {
            sqlcmd: args.sqlcmd.as_deref(),
            rac: &args.rac,
            ras_endpoint: &args.ras_endpoint,
            server: &args.server,
            database: &args.database,
            cluster_id: args.cluster_id,
            infobase_id: args.infobase_id,
            infobase_user: args.infobase_user.as_deref(),
            infobase_pwd: args.infobase_pwd.as_deref(),
            sql_user: args.sql_user.as_deref(),
            sql_pwd: args.sql_pwd.as_deref(),
            sql_pwd_env: &args.sql_pwd_env,
            sqlcmd_trust_cert: args.sqlcmd_trust_cert,
        },
    )?;
    if !args.watch {
        bail!("watch_source_changes requires --watch");
    }
    if !matches!(args.mode, MssqlMainActivationModeArg::Worker) {
        bail!("--watch currently requires --mode worker");
    }
    if args.dry_run || args.extension.is_some() {
        bail!("--watch requires a writable main-configuration target");
    }
    if args.script_output.is_some()
        || args.recovery_output.is_some()
        || args.tail_log_output.is_some()
    {
        bail!(
            "--watch allocates unique artifacts automatically; explicit activation artifacts are not accepted"
        );
    }
    let source_root = fs::canonicalize(&args.source_root)
        .with_context(|| format!("failed to canonicalize {}", args.source_root.display()))?;
    let selected_path = normalize_relative_path(&args.source_path)?;
    let paths = selected_source_closure_paths(&source_root, &selected_path)?;
    let mut observed = source_closure_fingerprint(&source_root, &paths)?;
    let mut fingerprint_cache = SourceWatchFingerprint {
        metadata: source_closure_metadata(&source_root, &paths)?,
        digest: observed,
        last_full_read: Instant::now(),
        needs_full_read: false,
    };
    let debounce = Duration::from_millis(args.watch_debounce_ms.clamp(50, 10_000));
    let mut changed_at = None;
    eprintln!(
        "watching {} source file(s); debounce={}ms",
        paths.len(),
        debounce.as_millis()
    );

    loop {
        thread::sleep(Duration::from_millis(100));
        let current = match fingerprint_cache.sample(&source_root, &paths, Instant::now()) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("watch fingerprint error: {error:#}");
                continue;
            }
        };
        if current != observed {
            observed = current;
            changed_at = Some(Instant::now());
            continue;
        }
        if !changed_at.is_some_and(|started| started.elapsed() >= debounce) {
            continue;
        }
        changed_at = None;
        let mut invocation = args.clone();
        invocation.watch = false;
        invocation.script_output = None;
        invocation.recovery_output = None;
        invocation.tail_log_output = None;
        match apply_source_change(&invocation) {
            Ok(report) => println!("{}", serde_json::to_string(&report)?),
            Err(error) => eprintln!(
                "{}",
                serde_json::json!({"event":"activation_error","error":format!("{error:#}")})
            ),
        }
    }
}

/// Read-only input checks before creating a fresh managed lifetime. The normal
/// consumer repeats these checks; this is not a cache or publication authority.
pub(crate) fn preflight_managed_source_inputs(args: &MssqlApplySourceChangeArgs) -> Result<()> {
    let source_root = fs::canonicalize(&args.source_root)
        .with_context(|| format!("failed to canonicalize {}", args.source_root.display()))?;
    if !source_root.is_dir() {
        bail!("source root is not a directory: {}", source_root.display());
    }
    let selected = normalize_relative_path(&args.source_path)?;
    for relative in selected_source_closure_paths(&source_root, &selected)? {
        let path = source_root.join(path_from_slashes(&relative));
        if !path.is_file() {
            bail!("selected source body is absent: {}", path.display());
        }
        reject_reparse_file(&path)?;
    }
    Ok(())
}
pub(crate) fn require_supported_main_source_cohort(
    args: &MssqlApplySourceChangeArgs,
) -> Result<()> {
    let selected = normalize_relative_path(&args.source_path)?;
    let common_module =
        selected.starts_with("CommonModules/") && selected.ends_with("/Ext/Module.bsl");
    let common_form_body = selected.starts_with("CommonForms/")
        && (selected.ends_with("/Ext/Form.xml") || selected.ends_with("/Ext/Form/Module.bsl"));
    if !common_module && !common_form_body {
        // A body owned by a top-level object needs that object's own metadata
        // XML in the active tree, and reconstructing it requires the reference
        // closure of every child and type it names. The bounded export reads
        // only the selected rows, so it cannot produce that descriptor yet;
        // refusing here names the gap instead of failing later inside the
        // bounded completeness check.
        bail!(
            "main writes currently support common-module bodies and common-form bodies; `{selected}` is owned by a top-level metadata object whose reference closure the bounded active export cannot resolve yet"
        );
    }
    if matches!(
        args.platform_profile,
        crate::mssql_platform_profile::MssqlNativePlatformProfile::Platform8_5_1_1150
    ) && !common_module
        && !selected.ends_with("/Ext/Form/Module.bsl")
    {
        bail!(
            "platform-8.5.1.1150 main writes are currently limited to common-module and managed-form-module source bodies"
        );
    }
    Ok(())
}

fn source_closure_fingerprint(source_root: &Path, paths: &[String]) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    for relative in paths {
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative.as_bytes());
        let path = source_root.join(path_from_slashes(relative));
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read watched source {}", path.display()))?;
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(&bytes);
    }
    Ok(digest.finalize().into())
}

// Metadata is an inexpensive change hint, never proof that content is unchanged.
// A periodic full read catches edits with preserved size and modification time.
const WATCH_FULL_READ_INTERVAL: Duration = Duration::from_secs(1);

#[derive(PartialEq, Eq)]
struct SourceWatchMetadata {
    length: u64,
    modified: SystemTime,
}

fn source_closure_metadata(root: &Path, paths: &[String]) -> Result<Vec<SourceWatchMetadata>> {
    paths
        .iter()
        .map(|relative| {
            let path = root.join(path_from_slashes(relative));
            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to stat watched source {}", path.display()))?;
            anyhow::ensure!(metadata.is_file(), "watched source is not a regular file");
            Ok(SourceWatchMetadata {
                length: metadata.len(),
                modified: metadata.modified()?,
            })
        })
        .collect()
}

struct SourceWatchFingerprint {
    metadata: Vec<SourceWatchMetadata>,
    digest: [u8; 32],
    last_full_read: Instant,
    needs_full_read: bool,
}

impl SourceWatchFingerprint {
    fn sample(&mut self, root: &Path, paths: &[String], now: Instant) -> Result<[u8; 32]> {
        let metadata = match source_closure_metadata(root, paths) {
            Ok(value) => value,
            Err(error) => {
                self.needs_full_read = true;
                return Err(error);
            }
        };
        if self.needs_full_read
            || metadata != self.metadata
            || now.saturating_duration_since(self.last_full_read) >= WATCH_FULL_READ_INTERVAL
        {
            self.needs_full_read = true;
            let digest = source_closure_fingerprint(root, paths)?;
            // Commit cache state only after the complete read succeeds.
            self.metadata = metadata;
            self.digest = digest;
            self.last_full_read = now;
            self.needs_full_read = false;
        }
        Ok(self.digest)
    }
}

fn export_active_managed_form_fast(
    args: &MssqlApplySourceChangeArgs,
    selected_path: &str,
    selected_storage_file_names: &[String],
    active_root: &Path,
) -> Result<Option<Option<String>>> {
    let Some((form_root, metadata_id, body_id)) =
        managed_form_fast_target(selected_path, selected_storage_file_names)
    else {
        return Ok(None);
    };

    let sql = main_read_sql(args)?;
    let fetch = |table: &str, names: &std::collections::BTreeSet<String>| {
        crate::mssql_dump::fetch_main_activation_rows(&sql, &args.database, table, names)
    };

    let marker_name = std::collections::BTreeSet::from(["DynamicallyUpdated".to_owned()]);
    let config_marker = fetch("Config", &marker_name)?;
    let params_marker = fetch("Params", &marker_name)?;
    let generations = match (config_marker.as_slice(), params_marker.as_slice()) {
        ([], []) => Vec::new(),
        ([config], [params]) => dynamic_generations(&config.binary_data, &params.binary_data)?,
        _ => bail!("Config/Params dynamic markers must both contain exactly one row"),
    };

    let mut config_names = std::collections::BTreeSet::from([metadata_id.clone(), body_id.clone()]);
    config_names.extend(
        generations
            .iter()
            .map(|generation| dynamic_alias(&body_id, generation)),
    );
    let rows = fetch("Config", &config_names)?;
    let Some(metadata_row) = rows
        .iter()
        .find(|row| row.file_name == *metadata_id && row.part_no == 0)
    else {
        return Ok(None);
    };
    let body_row = generations
        .iter()
        .rev()
        .find_map(|generation| {
            let alias = dynamic_alias(&body_id, generation);
            rows.iter()
                .find(|row| row.file_name == alias && row.part_no == 0)
        })
        .or_else(|| {
            rows.iter()
                .find(|row| row.file_name == body_id && row.part_no == 0)
        });
    let Some(body_row) = body_row else {
        return Ok(None);
    };

    let expected_descriptor = path_from_slashes(&format!("{form_root}.xml"));
    let Some((descriptor_path, descriptor_xml)) =
        crate::mssql_dump::extract_standalone_metadata_source_xml(
            &metadata_row.binary_data,
            &metadata_id,
            &expected_descriptor,
            args.source_version,
        )
    else {
        return Ok(None);
    };
    if descriptor_path != expected_descriptor {
        return Ok(None);
    }
    let Some(form_xml) = crate::mssql_dump::extract_form_body_xml(
        &body_row.binary_data,
        &std::collections::BTreeMap::new(),
    ) else {
        return Ok(None);
    };
    let parsed = crate::module_blob::parse_form_body_blob(&body_row.binary_data)?;
    let packed = crate::module_blob::pack_form_body_blob_from_form_xml(
        &body_row.binary_data,
        form_xml.as_bytes(),
        Some(parsed.module_text.as_bytes()),
    )?;
    if packed.blob != body_row.binary_data {
        return Ok(None);
    }

    let descriptor_path = active_root.join(descriptor_path);
    if let Some(parent) = descriptor_path.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::mssql_dump::write_source_xml_file(
        &descriptor_path,
        descriptor_xml,
        args.source_version,
    )?;
    let form_path = active_root.join(path_from_slashes(&format!("{form_root}/Ext/Form.xml")));
    if let Some(parent) = form_path.parent() {
        fs::create_dir_all(parent)?;
    }
    crate::mssql_dump::write_source_xml_file(&form_path, form_xml, args.source_version)?;
    if !parsed.module_text.is_empty() {
        let module_path = active_root.join(path_from_slashes(&format!(
            "{form_root}/Ext/Form/Module.bsl"
        )));
        if let Some(parent) = module_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(module_path, form_module_source_bytes(&parsed.module_text))?;
    }

    Ok(Some(generations.last().cloned()))
}

fn managed_form_fast_target(
    selected_path: &str,
    selected_storage_file_names: &[String],
) -> Option<(String, String, String)> {
    let form_root = selected_path
        .strip_suffix("/Ext/Form.xml")
        .or_else(|| selected_path.strip_suffix("/Ext/Form/Module.bsl"))?;
    let root_parts = form_root.split('/').collect::<Vec<_>>();
    let is_common = matches!(root_parts.as_slice(), ["CommonForms", _]);
    let is_owned = matches!(root_parts.as_slice(), [_, _, "Forms", _]);
    if !is_common && !is_owned {
        return None;
    }
    let metadata_id = selected_storage_file_names
        .iter()
        .find(|name| name.len() == 36)?
        .clone();
    let body_id = format!("{metadata_id}.0");
    (selected_storage_file_names.len() == 2
        && selected_storage_file_names
            .iter()
            .any(|name| name == &body_id))
    .then(|| (form_root.to_owned(), metadata_id, body_id))
}

fn normalize_relative_path(path: &Path) -> Result<String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        bail!("--path must be a non-empty source-relative path");
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| anyhow!("--path is not valid Unicode"))?;
                if value.is_empty() || value.chars().any(char::is_control) {
                    bail!("--path contains an invalid component");
                }
                parts.push(value);
            }
            _ => bail!("--path cannot contain root, drive, dot, or parent components"),
        }
    }
    Ok(parts.join("/"))
}

fn owner_prefix(selected: &str) -> Result<String> {
    if let Some((owner, _)) = selected.split_once("/Forms/") {
        if owner.is_empty() {
            bail!("managed form has no owner");
        }
        return Ok(owner.to_owned());
    }
    let (owner, _) = selected
        .split_once("/Ext/")
        .ok_or_else(|| anyhow!("selected path has no Ext owner boundary"))?;
    if owner.is_empty() {
        Ok("Configuration".to_owned())
    } else {
        Ok(owner.to_owned())
    }
}

fn overlay_selected_bodies(
    source_root: &Path,
    active_root: &Path,
    proposed_root: &Path,
    selected: &str,
) -> Result<()> {
    let mut paths = vec![selected.to_owned()];
    if let Some(form_root) = selected
        .strip_suffix("/Ext/Form.xml")
        .or_else(|| selected.strip_suffix("/Ext/Form/Module.bsl"))
    {
        for sibling in [
            format!("{form_root}/Ext/Form.xml"),
            format!("{form_root}/Ext/Form/Module.bsl"),
        ] {
            if sibling != selected
                && active_root.join(path_from_slashes(&sibling)).is_file()
                && source_root.join(path_from_slashes(&sibling)).is_file()
            {
                paths.push(sibling);
            }
        }
    }
    paths.sort();
    paths.dedup();
    for path in paths {
        let relative = path_from_slashes(&path);
        let active = active_root.join(&relative);
        let source = source_root.join(&relative);
        if !active.is_file() {
            bail!("selected existing body is absent from active storage export: {path}");
        }
        if !source.is_file() {
            bail!("selected source body is absent: {}", source.display());
        }
        reject_reparse_file(&source)?;
        fs::copy(&source, proposed_root.join(&relative))
            .with_context(|| format!("failed to overlay {path}"))?;
    }
    Ok(())
}

fn prepare_compile_tree_for_selected_change(root: &Path, selected: &str) -> Result<()> {
    let Some(form_root) = selected.strip_suffix("/Ext/Form/Module.bsl") else {
        return Ok(());
    };
    let form_xml = root.join(path_from_slashes(&format!("{form_root}/Ext/Form.xml")));
    if form_xml.is_file() {
        fs::remove_file(&form_xml).with_context(|| {
            format!(
                "failed to isolate module-only form compile {}",
                form_xml.display()
            )
        })?;
    }
    Ok(())
}

fn ensure_main_dry_run_stageable(staging: &Value) -> Result<()> {
    let failures = staging
        .get("prepare_failures")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("main dry-run report has no prepare_failures array"))?;
    if !failures.is_empty() {
        let first = failures[0]
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown prepare failure");
        bail!(
            "selected source change is not stageable: {} prepare failure(s); first: {first}",
            failures.len()
        );
    }
    let prepared = staging
        .get("prepared_total_config_rows")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("main dry-run report has no prepared row count"))?;
    let batches = staging
        .get("batches")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("main dry-run report has no batch plan"))?;
    if prepared == 0 || batches.is_empty() {
        bail!("selected source change produced no stageable Config rows");
    }
    if let Some(error) = staging.get("version_patch_error").and_then(Value::as_str) {
        bail!("selected source change cannot patch versions: {error}");
    }
    Ok(())
}

fn selected_source_closure_paths(source_root: &Path, selected: &str) -> Result<Vec<String>> {
    let mut paths = vec![selected.to_owned()];
    if let Some(form_root) = selected
        .strip_suffix("/Ext/Form.xml")
        .or_else(|| selected.strip_suffix("/Ext/Form/Module.bsl"))
    {
        for sibling in [
            format!("{form_root}/Ext/Form.xml"),
            format!("{form_root}/Ext/Form/Module.bsl"),
        ] {
            if source_root.join(path_from_slashes(&sibling)).is_file() {
                paths.push(sibling);
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn selected_storage_file_names_for_source_paths(
    source_root: &Path,
    source_paths: &[String],
) -> Result<Vec<String>> {
    let registry = crate::compiler::families::assets::SourceAssetRegistry;
    let mut selected = std::collections::BTreeSet::new();
    for source_path in source_paths {
        let relative = path_from_slashes(source_path);
        let components = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let ext_index = components
            .iter()
            .position(|part| part.eq_ignore_ascii_case("Ext"))
            .ok_or_else(|| anyhow!("selected source body has no Ext boundary: {source_path}"))?;
        if ext_index == 0 {
            bail!("selected source body has no metadata owner: {source_path}");
        }
        let owner_name = &components[ext_index - 1];
        let mut owner_parts = components[..ext_index].to_vec();
        *owner_parts
            .last_mut()
            .expect("Ext owner has a preceding component") = format!("{owner_name}.xml");
        let owner_relative = owner_parts.iter().collect::<PathBuf>();
        let owner_path = source_root.join(&owner_relative);
        let owner_xml = fs::read(&owner_path)
            .with_context(|| format!("failed to read selected owner {}", owner_path.display()))?;
        let owner_relative_slashes = owner_relative.to_string_lossy().replace('\\', "/");
        let (family, owner_uuid) = if owner_relative_slashes.starts_with("CommonModules/") {
            let properties = crate::module_blob::parse_common_module_xml_properties(&owner_xml)?;
            ("CommonModule".to_owned(), properties.uuid)
        } else {
            let properties = crate::module_blob::parse_simple_metadata_xml_properties(&owner_xml)?;
            (properties.kind, properties.uuid)
        };
        if family == "Configuration" {
            bail!("bounded apply of configuration-level modules is not yet supported");
        }
        let owner_relative_asset = components[ext_index..].join("/");
        let suffix = if matches!(
            owner_relative_asset.as_str(),
            "Ext/Form.xml" | "Ext/Form/Module.bsl"
        ) && matches!(family.as_str(), "Form" | "CommonForm")
        {
            ".0"
        } else {
            registry
                .route_by_relative_path(&family, &owner_relative_asset)
                .ok_or_else(|| {
                    anyhow!("no bounded storage route for {family}/{owner_relative_asset}")
                })?
                .suffix()
        };
        selected.insert(owner_uuid.clone());
        selected.insert(format!("{owner_uuid}{suffix}"));
        // A nested asset (`Kind/Name/Forms/Form/Ext/...`) is placed by its
        // top-level object, and the classifier requires that descriptor in the
        // active tree, so its own storage row belongs to the closure too.
        if ext_index > 2 {
            let top_level_relative: PathBuf =
                [components[0].clone(), format!("{}.xml", components[1])]
                    .iter()
                    .collect();
            let top_level_path = source_root.join(&top_level_relative);
            let top_level_xml = fs::read(&top_level_path).with_context(|| {
                format!(
                    "failed to read top-level owner {}",
                    top_level_path.display()
                )
            })?;
            let properties =
                crate::module_blob::parse_simple_metadata_xml_properties(&top_level_xml)?;
            selected.insert(properties.uuid);
        }
    }
    if selected.is_empty() {
        bail!("selected source closure resolved to no storage rows");
    }
    Ok(selected.into_iter().collect())
}
fn ensure_bounded_export_complete(
    active_root: &Path,
    required_paths: &[String],
    selected_storage_file_names: &[String],
    report: &crate::mssql_dump::MssqlDumpConfigReport,
) -> Result<()> {
    for path in required_paths {
        if !active_root.join(path_from_slashes(path)).is_file() {
            bail!("selected storage closure did not emit required source body: {path}");
        }
    }
    let table = report
        .tables
        .iter()
        .find(|table| table.table == "Config")
        .ok_or_else(|| anyhow!("bounded export has no Config table report"))?;
    // The dump reads exactly the identities it was given plus the documented
    // module-body expansion of each one, so an object that stores more bodies
    // than the change selects is still bounded. Anything beyond that set would
    // mean the export read rows outside the selected closure.
    let bounded_row_limit =
        crate::mssql_dump::expanded_selected_file_names(selected_storage_file_names).len();
    if table.rows > bounded_row_limit {
        bail!(
            "bounded export returned {} rows for {} reachable storage identities",
            table.rows,
            bounded_row_limit
        );
    }
    if table.rows < selected_storage_file_names.len() {
        bail!(
            "bounded export returned {} rows for {} selected storage identities",
            table.rows,
            selected_storage_file_names.len()
        );
    }
    let root = &table.metadata_root_inventory;
    if !root.candidate_set_complete || root.missing != 0 || root.expected != root.emitted {
        bail!(
            "selected metadata owner identity is incomplete: candidates_complete={}, expected={}, emitted={}, missing={}",
            root.candidate_set_complete,
            root.expected,
            root.emitted,
            root.missing
        );
    }
    let is_form = required_paths
        .iter()
        .any(|path| path.ends_with("/Ext/Form.xml"));
    if !is_form {
        let completeness = &report.source_assets;
        if !completeness.candidate_set_complete
            || completeness.opaque != 0
            || completeness.missing != 0
        {
            bail!(
                "selected storage closure is incomplete: status={:?}, candidates_complete={}, opaque={}, missing={}",
                completeness.status,
                completeness.candidate_set_complete,
                completeness.opaque,
                completeness.missing
            );
        }
        return Ok(());
    }
    if table.source_asset_rows != 1 || table.module_text_rows != 1 || table.metadata_xml_rows != 1 {
        bail!("selected form closure did not emit exactly one owner XML, Form.xml, and Module.bsl");
    }
    let selected = selected_storage_file_names
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for diagnostic in report
        .source_assets
        .affected_assets
        .iter()
        .filter(|entry| selected.contains(entry.source_row_id.as_str()))
    {
        let accepted_identity_note = diagnostic.code
            == "source_asset.form_owner_resolution.uncertain"
            && diagnostic.family == "form"
            && required_paths.contains(&diagnostic.asset_path)
            && active_root
                .join(path_from_slashes(&diagnostic.asset_path))
                .is_file();
        if !accepted_identity_note {
            bail!(
                "selected form storage diagnostic {} for {}",
                diagnostic.code,
                diagnostic.source_row_id
            );
        }
    }
    Ok(())
}

/// The connection the reads of the apply's own preflights use.
fn main_read_sql(args: &MssqlApplySourceChangeArgs) -> Result<SqlExec> {
    let password = if args.sql_user.is_some() {
        args.sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok())
    } else {
        None
    };
    // Export, parity preparation, staging and activation share this policy.
    SqlExec::from_options(SqlOptions {
        sqlcmd: args.sqlcmd.as_deref(),
        bcp: args.bcp_executable.as_deref(),
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: args.sqlcmd_trust_cert,
    })
}

fn preflight_classified_worker_source(
    mode: MssqlMainActivationModeArg,
    dry_run: bool,
    classified: &crate::mssql_source_change::SourceActivationInput,
) -> Result<()> {
    crate::mssql_worker_switch::preflight_worker_execution(
        activation_mode(mode),
        dry_run,
        classified.is_no_op(),
    )
}

fn activation_mode(
    mode: MssqlMainActivationModeArg,
) -> crate::mssql_main_activation::MainActivationMode {
    use crate::mssql_main_activation::MainActivationMode;
    match mode {
        MssqlMainActivationModeArg::Exclusive => MainActivationMode::Exclusive,
        MssqlMainActivationModeArg::Online => MainActivationMode::Online,
        MssqlMainActivationModeArg::Live => MainActivationMode::Live,
        MssqlMainActivationModeArg::Worker => MainActivationMode::Worker,
    }
}

/// Refuses, before the export reads anything, a tail-log argument the
/// activation would refuse after the stage (#409 F-2): `live` needs an
/// absolute SQL Server-local path, no other mode takes one.
fn preflight_tail_log(args: &MssqlApplySourceChangeArgs) -> Result<()> {
    if matches!(args.mode, MssqlMainActivationModeArg::Live)
        && args
            .tail_log_output
            .as_ref()
            .is_some_and(|path| !path.is_absolute())
    {
        bail!("--tail-log-output must be an absolute SQL Server-local path");
    }
    let tail = args
        .tail_log_output
        .as_ref()
        .map(|path| {
            path.to_str()
                .ok_or_else(|| anyhow!("--tail-log-output is not valid Unicode"))
        })
        .transpose()?;
    crate::mssql_main_activation::preflight_tail_log(
        activation_mode(args.mode),
        tail,
        !args.dry_run,
    )
    .map_err(anyhow::Error::new)
}

/// Refuses, before anything is staged, a change the publication state of the
/// database rules out: an ordinary mode on a database that holds online
/// generations (#408), markers that disagree (#409 F-2: both used to be found
/// by the activation, after the stage, with `ConfigSave` left filled).
///
/// The rows are read as stored -- the state the activation checks and writes
/// -- and the plan of the activation makes the same checks
/// ([`crate::mssql_main_activation::preflight_publication`]).
fn preflight_main_publication(args: &MssqlApplySourceChangeArgs) -> Result<()> {
    let sql = main_read_sql(args)?;
    let fetch = |table: &str, names: &std::collections::BTreeSet<String>| {
        crate::mssql_dump::fetch_main_activation_rows(&sql, &args.database, table, names)
    };
    let versions = fetch(
        "Config",
        &std::collections::BTreeSet::from(["versions".to_owned()]),
    )?
    .into_iter()
    .find(|row| row.part_no == 0)
    .ok_or_else(|| anyhow!("Config is missing versions part 0"))?;
    let marker_name = std::collections::BTreeSet::from(["DynamicallyUpdated".to_owned()]);
    let config_marker =
        crate::mssql::exactly_one_optional_marker("Config", fetch("Config", &marker_name)?)?;
    let params_marker =
        crate::mssql::exactly_one_optional_marker("Params", fetch("Params", &marker_name)?)?;
    // The exclusive mode is carried out by the config apply where the built-in SQL client is there, and that
    // apply folds the rows of online generations (#408 step 2); the script of every other route refuses them.
    let executor = crate::mssql_main_activation::MainActivationExecutor::for_mode(
        activation_mode(args.mode),
        matches!(sql.backend(), crate::sql::SqlBackend::Client(_)),
    );
    crate::mssql_main_activation::preflight_publication(
        executor,
        activation_mode(args.mode),
        &versions,
        config_marker.as_ref(),
        params_marker.as_ref(),
    )
    .map_err(anyhow::Error::new)
}

/// The tables the own apply wrote, when it carried out the activation (`config_apply` of the report of
/// `mssql-activate-staged-main`, #408 step 2).
fn tables_touched_by_config_apply(activation: Option<&Value>) -> Option<Vec<String>> {
    let touched = activation?
        .pointer("/config_apply/tables_touched")?
        .as_array()?;
    Some(
        touched
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    )
}

/// The generation an online update left active in the database, if it holds
/// one: the last of the history the two markers record.
///
/// The bounded export already publishes the bodies of that generation (its
/// view resolves the aliases, #409 F-1); this only names the generation the
/// change applies to. It used to read the alias of the selected body as well
/// and write it over the exported file -- a second mechanism for the same job
/// that the export's view had made unreachable (F-16) and that would run again
/// now that the export's view no longer outlives the export.
fn active_dynamic_generation(args: &MssqlApplySourceChangeArgs) -> Result<Option<String>> {
    let marker_name = std::collections::BTreeSet::from(["DynamicallyUpdated".to_owned()]);
    let sql = main_read_sql(args)?;
    let fetch = |table: &str, names: &std::collections::BTreeSet<String>| {
        crate::mssql_dump::fetch_main_activation_rows(&sql, &args.database, table, names)
    };
    let config_marker = fetch("Config", &marker_name)?;
    let params_marker = fetch("Params", &marker_name)?;
    if config_marker.is_empty() && params_marker.is_empty() {
        return Ok(None);
    }
    if config_marker.len() != 1 || params_marker.len() != 1 {
        bail!("Config/Params dynamic markers must both contain exactly one row");
    }
    let generations =
        dynamic_generations(&config_marker[0].binary_data, &params_marker[0].binary_data)?;
    Ok(generations.last().cloned())
}

fn form_module_source_bytes(module_text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(3 + module_text.len());
    bytes.extend_from_slice(b"\xef\xbb\xbf");
    bytes.extend_from_slice(module_text.as_bytes());
    bytes
}

fn dynamic_generations(config: &[u8], params: &[u8]) -> Result<Vec<String>> {
    let parse = |table: &str, payload: &[u8], expected_tag: &str| -> Result<Vec<String>> {
        let text =
            std::str::from_utf8(payload.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(payload))
                .with_context(|| format!("{table}.DynamicallyUpdated is not UTF-8"))?;
        let fields = text
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
            .ok_or_else(|| anyhow!("{table}.DynamicallyUpdated is not braced"))?
            .split(',')
            .map(|value| value.trim().to_owned())
            .collect::<Vec<_>>();
        if fields.first().map(String::as_str) != Some(expected_tag) {
            bail!("{table}.DynamicallyUpdated has an unsupported tag");
        }
        let count = fields
            .get(1)
            .ok_or_else(|| anyhow!("{table}.DynamicallyUpdated has no count"))?
            .parse::<usize>()?;
        if fields.len() != count + 2 {
            bail!("{table}.DynamicallyUpdated count does not match its payload");
        }
        Ok(fields)
    };
    let config = parse("Config", config, "1")?;
    let params = parse("Params", params, "0")?;
    let history = config[2..].to_vec();
    if history.is_empty() {
        bail!("Config.DynamicallyUpdated has no dynamic generation");
    }
    if params.len() < 3 || params[3..] != history {
        bail!("Config/Params dynamic generation histories disagree");
    }
    for generation in &history {
        Uuid::parse_str(generation)
            .with_context(|| format!("dynamic generation is not a UUID: {generation}"))?;
    }
    Ok(history)
}

fn dynamic_alias(name: &str, generation: &str) -> String {
    match name.split_once('.') {
        Some((base, suffix)) => format!("{base}_dynupdate_{generation}.{suffix}"),
        None => format!("{name}_dynupdate_{generation}"),
    }
}
fn path_from_slashes(value: &str) -> PathBuf {
    value.split('/').collect()
}

fn copy_source_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination)?;
    for item in WalkDir::new(source).follow_links(false) {
        let item = item?;
        let relative = item.path().strip_prefix(source)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if item.file_type().is_symlink() {
            bail!("source export contains a link: {}", item.path().display());
        }
        let target = destination.join(relative);
        if item.file_type().is_dir() {
            fs::create_dir(&target)?;
        } else if item.file_type().is_file() {
            fs::copy(item.path(), &target)?;
        }
    }
    Ok(())
}

fn inventory_from_root(root: &Path) -> Result<SourceInventory> {
    let mut files = Vec::new();
    for item in WalkDir::new(root).follow_links(false) {
        let item = item?;
        if item.file_type().is_symlink() {
            bail!(
                "source inventory contains a link: {}",
                item.path().display()
            );
        }
        if !item.file_type().is_file() {
            continue;
        }
        let relative = item
            .path()
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        files.push(SourceFileDigest::for_bytes(
            relative,
            &fs::read(item.path())?,
        )?);
    }
    Ok(SourceInventory::from_files(files)?)
}

fn reject_reparse_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!("source body cannot be a symbolic link: {}", path.display());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x0400 != 0 {
            bail!("source body cannot be a reparse point: {}", path.display());
        }
    }
    Ok(())
}

struct TemporaryApplyRoot {
    path: PathBuf,
}

impl TemporaryApplyRoot {
    fn new() -> Result<Self> {
        let parent = std::env::temp_dir().join("ibcmd-rs");
        fs::create_dir_all(&parent)?;
        let path = parent.join(format!("apply-source-{}", Uuid::new_v4().simple()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryApplyRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod certificate_policy_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_platform_profile::MssqlNativePlatformProfile;

    #[test]
    fn the_tables_of_an_exclusive_promotion_are_the_ones_the_own_apply_wrote() {
        let carried_out_by_the_apply = serde_json::json!({
            "activation": {"executor": "config_apply"},
            "config_apply": {"tables_touched": ["Config", "ConfigSave", "Params", "_ConfigChngR", "Files"]}
        });
        assert_eq!(
            tables_touched_by_config_apply(Some(&carried_out_by_the_apply)),
            Some(
                ["Config", "ConfigSave", "Params", "_ConfigChngR", "Files"]
                    .map(str::to_owned)
                    .to_vec()
            )
        );
        // the script of mssql_main_activation writes no report of the apply
        let script = serde_json::json!({"activation": {"executor": "script"}});
        assert_eq!(tables_touched_by_config_apply(Some(&script)), None);
        assert_eq!(tables_touched_by_config_apply(None), None);
    }

    #[test]
    fn runtime_profile_verification_fails_before_active_export() {
        let args = MssqlApplySourceChangeArgs {
            managed_worker: Default::default(),
            live_checkpoint: false,
            platform_profile: MssqlNativePlatformProfile::Platform8_5_1_1150,
            sqlcmd: Some(PathBuf::from("must-not-run-sqlcmd")),
            bcp_executable: Some(PathBuf::from("must-not-run-bcp")),
            server: "must-not-connect".to_owned(),
            sql_user: None,
            sql_pwd: None,
            sql_pwd_env: "MUST_NOT_READ".to_owned(),
            sqlcmd_trust_cert: true,
            database: "must_not_connect".to_owned(),
            source_root: PathBuf::from("missing-source-must-not-be-read"),
            source_path: PathBuf::from("CommonModules/Test/Ext/Module.bsl"),
            extension: None,
            mode: MssqlMainActivationModeArg::Exclusive,
            dry_run: false,
            allow_non_lab: true,
            platform: None,
            source_version: crate::cli::InfobaseConfigSourceVersion::V2_20,
            script_output: None,
            recovery_output: None,
            tail_log_output: None,
            interrupt_sessions: false,
            rac: PathBuf::from("must-not-run-rac"),
            ras_endpoint: "must-not-connect".to_owned(),
            cluster_id: None,
            infobase_id: None,
            infobase_user: None,
            infobase_pwd: None,
            watch: false,
            watch_debounce_ms: 300,
        };
        let error = apply_source_change(&args).expect_err("unverified write must fail closed");
        // The declared policy stops an unsupported build before rac, sqlcmd or
        // the source tree are touched.
        assert!(error.to_string().contains("explicitly unsupported"));

        let checkpoint = MssqlApplySourceChangeArgs {
            live_checkpoint: true,
            mode: MssqlMainActivationModeArg::Live,
            ..args.clone()
        };
        assert!(
            apply_source_change(&checkpoint)
                .unwrap_err()
                .to_string()
                .contains("refused before staging")
        );
        assert!(
            watch_source_changes(&checkpoint)
                .unwrap_err()
                .to_string()
                .contains("refused before staging")
        );

        let evidenced = MssqlApplySourceChangeArgs {
            live_checkpoint: false,
            platform_profile: MssqlNativePlatformProfile::Platform8_3_27_2214,
            ..args
        };
        let runtime_error =
            apply_source_change(&evidenced).expect_err("runtime verification must still run");
        assert!(runtime_error.to_string().contains("failed to launch rac"));

        // The acknowledgement is an argument: it is refused before rac runs
        // (#409 F-6).
        let unacknowledged = MssqlApplySourceChangeArgs {
            allow_non_lab: false,
            ..evidenced
        };
        let refusal = apply_source_change(&unacknowledged).expect_err("must refuse");
        assert!(refusal.to_string().contains("--allow-non-lab"), "{refusal}");
    }

    #[test]
    fn classified_worker_source_preserves_real_no_op_and_refuses_changed_body() {
        let inventory = |body: &[u8]| {
            SourceInventory::from_files(vec![
                SourceFileDigest::for_bytes("CommonModules/Work.xml", b"metadata").unwrap(),
                SourceFileDigest::for_bytes("CommonModules/Work/Ext/Module.bsl", body).unwrap(),
            ])
            .unwrap()
        };
        let active = inventory(b"Function Value() Export\nReturn 1;\nEndFunction");
        let same = inventory(b"Function Value() Export\nReturn 1;\nEndFunction");
        let changed = inventory(b"Function Value() Export\nReturn 2;\nEndFunction");
        for (proposed, expected_no_op) in [(&same, true), (&changed, false)] {
            let plan = classify_source_change(
                &active,
                proposed,
                "CommonModules/Work/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Exclusive,
            )
            .unwrap();
            assert_eq!(plan.is_no_op(), expected_no_op);
            assert_eq!(plan.changed_paths().is_empty(), expected_no_op);
            let result = preflight_classified_worker_source(
                MssqlMainActivationModeArg::Worker,
                false,
                &plan,
            );
            if expected_no_op {
                result.unwrap();
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("complete loaded-infobase")
                );
            }
            preflight_classified_worker_source(MssqlMainActivationModeArg::Worker, true, &plan)
                .unwrap();
        }
    }

    #[test]
    fn runtime_profile_verification_fails_before_watch_reads_missing_source() {
        let args = MssqlApplySourceChangeArgs {
            managed_worker: Default::default(),
            live_checkpoint: false,
            platform_profile: MssqlNativePlatformProfile::Platform8_5_1_1150,
            sqlcmd: Some(PathBuf::from("must-not-run-sqlcmd")),
            bcp_executable: Some(PathBuf::from("must-not-run-bcp")),
            server: "must-not-connect".to_owned(),
            sql_user: None,
            sql_pwd: None,
            sql_pwd_env: "MUST_NOT_READ".to_owned(),
            sqlcmd_trust_cert: true,
            database: "must_not_connect".to_owned(),
            source_root: PathBuf::from("missing-source-must-not-be-read"),
            source_path: PathBuf::from("CommonModules/Test/Ext/Module.bsl"),
            extension: None,
            mode: MssqlMainActivationModeArg::Worker,
            dry_run: false,
            allow_non_lab: true,
            platform: None,
            source_version: crate::cli::InfobaseConfigSourceVersion::V2_21,
            script_output: None,
            recovery_output: None,
            tail_log_output: None,
            interrupt_sessions: false,
            rac: PathBuf::from("must-not-run-rac"),
            ras_endpoint: "must-not-connect".to_owned(),
            cluster_id: None,
            infobase_id: None,
            infobase_user: None,
            infobase_pwd: None,
            watch: true,
            watch_debounce_ms: 300,
        };
        let error = watch_source_changes(&args).expect_err("unverified watch must fail closed");
        assert!(error.to_string().contains("explicitly unsupported"));

        let evidenced = MssqlApplySourceChangeArgs {
            live_checkpoint: false,
            platform_profile: MssqlNativePlatformProfile::Platform8_3_27_2214,
            ..args
        };
        let runtime_error = watch_source_changes(&evidenced)
            .expect_err("unproved worker ownership must refuse before any external process");
        assert!(
            runtime_error
                .to_string()
                .contains("complete loaded-infobase")
        );
    }

    const OLD: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
    const NEW: &str = "968a0bc0-969b-4bf2-b9ef-19d0e8bf8ce4";
    const OWNER: &str = "ab132638-5188-470d-9432-de85f2b2c7d8";

    #[test]
    fn fast_target_accepts_only_one_exact_managed_form_pair() {
        let metadata = "a627e390-8fad-4a95-afe6-674f54813188".to_owned();
        let body = format!("{metadata}.0");
        let selected = vec![metadata.clone(), body.clone()];
        assert_eq!(
            managed_form_fast_target("CommonForms/Demo/Ext/Form.xml", &selected,),
            Some(("CommonForms/Demo".to_owned(), metadata, body))
        );
        assert_eq!(
            managed_form_fast_target("Catalogs/Goods/Forms/Card/Ext/Form.xml", &selected,),
            Some((
                "Catalogs/Goods/Forms/Card".to_owned(),
                selected[0].clone(),
                selected[1].clone(),
            ))
        );
        let mut extra = selected;
        extra.push("versions".to_owned());
        assert!(
            managed_form_fast_target("CommonForms/Demo/Ext/Form/Module.bsl", &extra,).is_none()
        );
    }

    fn marker(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    #[test]
    fn parses_and_cross_checks_dynamic_generation_history() {
        let config = marker(&format!("{{1,2,{OLD},{NEW}}}"));
        let params = marker(&format!(
            "{{0,3,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{OLD},{NEW}}}"
        ));
        assert_eq!(dynamic_generations(&config, &params).unwrap(), [OLD, NEW]);

        let drifted = marker(&format!(
            "{{0,3,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{NEW},{OLD}}}"
        ));
        assert!(dynamic_generations(&config, &drifted).is_err());
    }

    #[test]
    fn builds_dynamic_alias_without_moving_the_storage_suffix() {
        assert_eq!(
            dynamic_alias(&format!("{OWNER}.0"), NEW),
            format!("{OWNER}_dynupdate_{NEW}.0")
        );
    }

    #[test]
    fn restores_source_bom_for_dynamic_form_module() {
        assert_eq!(
            form_module_source_bytes("Процедура Тест()\nКонецПроцедуры"),
            b"\xef\xbb\xbf\xd0\x9f\xd1\x80\xd0\xbe\xd1\x86\xd0\xb5\xd0\xb4\xd1\x83\xd1\x80\xd0\xb0 \xd0\xa2\xd0\xb5\xd1\x81\xd1\x82()\n\xd0\x9a\xd0\xbe\xd0\xbd\xd0\xb5\xd1\x86\xd0\x9f\xd1\x80\xd0\xbe\xd1\x86\xd0\xb5\xd0\xb4\xd1\x83\xd1\x80\xd1\x8b"
        );
    }

    #[test]
    fn resolves_one_common_module_to_owner_and_body_rows() {
        let root = std::env::temp_dir().join(format!("ibcmd-rs-selection-{}", Uuid::new_v4()));
        let module_dir = root.join("CommonModules").join("Tools").join("Ext");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(
            root.join("CommonModules").join("Tools.xml"),
            format!(
                r#"<MetaDataObject><CommonModule uuid="{OWNER}"><Properties><Name>Tools</Name><Synonym/><Comment/><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties></CommonModule></MetaDataObject>"#
            ),
        )
        .unwrap();
        fs::write(
            module_dir.join("Module.bsl"),
            "Procedure Test()\nEndProcedure\n",
        )
        .unwrap();

        let selected = selected_storage_file_names_for_source_paths(
            &root,
            &["CommonModules/Tools/Ext/Module.bsl".to_owned()],
        )
        .unwrap();
        assert_eq!(selected, [OWNER.to_owned(), format!("{OWNER}.0")]);

        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn module_only_form_compile_removes_layout_from_the_temporary_tree() {
        let root = std::env::temp_dir().join(format!("ibcmd-rs-form-compile-{}", Uuid::new_v4()));
        let ext = root.join("CommonForms").join("Demo").join("Ext");
        fs::create_dir_all(ext.join("Form")).unwrap();
        fs::write(ext.join("Form.xml"), "<Form/>").unwrap();
        fs::write(
            ext.join("Form").join("Module.bsl"),
            "Procedure Test()\nEndProcedure\n",
        )
        .unwrap();

        prepare_compile_tree_for_selected_change(&root, "CommonForms/Demo/Ext/Form/Module.bsl")
            .unwrap();

        assert!(!ext.join("Form.xml").exists());
        assert!(ext.join("Form").join("Module.bsl").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn main_dry_run_refuses_prepare_failures_and_empty_plans() {
        let failed = serde_json::json!({
            "prepare_failures": [{"message": "unsupported form reference"}],
            "prepared_total_config_rows": 0,
            "batches": [],
            "version_patch_error": null
        });
        assert!(ensure_main_dry_run_stageable(&failed).is_err());

        let empty = serde_json::json!({
            "prepare_failures": [],
            "prepared_total_config_rows": 0,
            "batches": [],
            "version_patch_error": null
        });
        assert!(ensure_main_dry_run_stageable(&empty).is_err());

        let stageable = serde_json::json!({
            "prepare_failures": [],
            "prepared_total_config_rows": 2,
            "batches": [{"index": 0}],
            "version_patch_error": null
        });
        ensure_main_dry_run_stageable(&stageable).unwrap();
    }

    #[test]
    fn watched_source_fingerprint_changes_only_with_content() {
        let root = std::env::temp_dir().join(format!("ibcmd-rs-watch-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Module.bsl"), "v1").unwrap();
        let paths = vec!["Module.bsl".to_owned()];
        let first = source_closure_fingerprint(&root, &paths).unwrap();
        fs::write(root.join("Module.bsl"), "v1").unwrap();
        assert_eq!(source_closure_fingerprint(&root, &paths).unwrap(), first);
        fs::write(root.join("Module.bsl"), "v2").unwrap();
        assert_ne!(source_closure_fingerprint(&root, &paths).unwrap(), first);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn watch_cache_periodically_catches_same_size_same_timestamp_edit() {
        let root = std::env::temp_dir().join(format!("ibcmd-rs-watch-cache-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let file = root.join("Module.bsl");
        fs::write(&file, "v1").unwrap();
        let paths = vec!["Module.bsl".to_owned()];
        let now = Instant::now();
        let first = source_closure_fingerprint(&root, &paths).unwrap();
        let mut cache = SourceWatchFingerprint {
            metadata: source_closure_metadata(&root, &paths).unwrap(),
            digest: first,
            last_full_read: now,
            needs_full_read: false,
        };
        for tick in 1..10 {
            assert_eq!(
                cache
                    .sample(&root, &paths, now + Duration::from_millis(tick * 100))
                    .unwrap(),
                first
            );
            assert_eq!(cache.last_full_read, now, "idle polling reread contents");
        }
        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        fs::write(&file, "v2").unwrap();
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert!(source_closure_metadata(&root, &paths).unwrap() == cache.metadata);
        assert_ne!(
            cache
                .sample(&root, &paths, now + WATCH_FULL_READ_INTERVAL)
                .unwrap(),
            first
        );
        let canonical = fs::canonicalize(&root).unwrap();
        assert_eq!(
            canonical.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            canonical
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ibcmd-rs-watch-cache-")
        );
        fs::remove_dir_all(canonical).unwrap();
    }

    #[test]
    fn watch_cache_read_failure_requires_a_fresh_successful_digest() {
        let root = std::env::temp_dir().join(format!("ibcmd-rs-watch-cache-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let file = root.join("Module.bsl");
        fs::write(&file, "v1").unwrap();
        let paths = vec!["Module.bsl".to_owned()];
        let now = Instant::now();
        let first = source_closure_fingerprint(&root, &paths).unwrap();
        let mut cache = SourceWatchFingerprint {
            metadata: source_closure_metadata(&root, &paths).unwrap(),
            digest: first,
            last_full_read: now,
            needs_full_read: false,
        };
        fs::remove_file(&file).unwrap();
        assert!(
            cache
                .sample(&root, &paths, now + Duration::from_millis(100))
                .is_err()
        );
        assert!(cache.needs_full_read);
        assert_eq!(cache.digest, first);
        fs::write(&file, "v2").unwrap();
        assert_ne!(
            cache
                .sample(&root, &paths, now + Duration::from_millis(200))
                .unwrap(),
            first
        );
        assert!(!cache.needs_full_read);
        let canonical = fs::canonicalize(&root).unwrap();
        assert_eq!(
            canonical.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            canonical
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ibcmd-rs-watch-cache-")
        );
        fs::remove_dir_all(canonical).unwrap();
    }
}
