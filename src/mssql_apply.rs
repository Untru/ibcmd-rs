//! High-level, fail-closed orchestration for one non-structural source change.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
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

#[derive(Debug, Serialize)]
pub struct MssqlApplySourceChangeReport {
    pub schema_version: u32,
    pub database: String,
    pub target: String,
    pub selected_path: String,
    pub mode: String,
    pub dry_run: bool,
    pub executed: bool,
    pub no_op: bool,
    pub changed_paths: Vec<String>,
    pub active_generation_or_root: Option<String>,
    pub proposed_generation_or_root: Option<String>,
    pub tables_changed: Vec<String>,
    pub recovery_token: Option<String>,
    pub staging: Option<Value>,
    pub activation: Option<Value>,
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
    let total_started = Instant::now();
    if !args.dry_run && !args.allow_non_lab {
        bail!("--allow-non-lab acknowledgement is required for source activation");
    }
    if args.extension.is_none() && !args.sqlcmd_trust_cert {
        bail!(
            "main source apply requires explicit --sqlcmd-trust-cert because the legacy main SQL runner trusts the server certificate"
        );
    }

    let source_root = fs::canonicalize(&args.source_root)
        .with_context(|| format!("failed to canonicalize {}", args.source_root.display()))?;
    if !source_root.is_dir() {
        bail!("source root is not a directory: {}", source_root.display());
    }
    let selected_path = normalize_relative_path(&args.source_path)?;
    let work = TemporaryApplyRoot::new()?;
    let active_root = work.path.join("active");
    let proposed_root = work.path.join("proposed");

    let export_started = Instant::now();
    let mut active_generation_or_root = None;
    if let Some(extension) = args.extension.as_deref() {
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
            source_version: args.source_version,
        })?;
        active_generation_or_root = report
            .extensions
            .first()
            .map(|entry| entry.active_cas_root.clone());
    } else {
        crate::mssql_dump::dump_config(&MssqlDumpConfigArgs {
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
            file_names: Vec::new(),
            file_name_lists: Vec::new(),
            inflate: false,
            extract_module_text: true,
            extract_metadata_xml: true,
            require_complete_root_metadata: true,
            require_complete_source_assets: true,
            collect_all_source_asset_diagnostics: false,
            source_version: args.source_version,
            no_binary_rows: true,
            write_binary_rows: false,
            write_manifest: false,
        })?;
    }
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
        MssqlMainActivationModeArg::Exclusive => ActivationMode::Exclusive,
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
    }
    .to_owned();

    if no_op {
        return Ok(MssqlApplySourceChangeReport {
            schema_version: 1,
            database: args.database.clone(),
            target: target_name,
            selected_path,
            mode: mode_name,
            dry_run: args.dry_run,
            executed: false,
            no_op: true,
            changed_paths,
            active_generation_or_root: active_generation_or_root.clone(),
            proposed_generation_or_root: active_generation_or_root,
            tables_changed: Vec::new(),
            recovery_token: None,
            staging: None,
            activation: None,
            timings: MssqlApplySourceChangeTimings {
                active_export_ms,
                classification_ms,
                total_ms: total_started.elapsed().as_millis(),
                ..Default::default()
            },
        });
    }

    let path_prefix = owner_prefix(&selected_path)?;
    let staging_started = Instant::now();
    let staging = if let Some(extension) = args.extension.as_deref() {
        serde_json::to_value(crate::mssql_extension_load::load_extensions(
            &MssqlLoadExtensionArgs {
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
                source_version: args.source_version,
            },
        )?)?
    } else if args.dry_run {
        if args.sql_user.is_some() {
            bail!(
                "main dry-run with SQL authentication is not yet supported by the parity auditor"
            );
        }
        serde_json::to_value(crate::mssql::audit_source_parity(
            &MssqlAuditSourceParityArgs {
                server: args.server.clone(),
                database: args.database.clone(),
                source_root: proposed_root.clone(),
                sqlcmd: args.sqlcmd.clone(),
                batch_size: Some(1),
                source_version: Some(args.source_version),
                path_prefix: vec![path_prefix.clone()],
                output: None,
            },
        )?)?
    } else {
        serde_json::to_value(crate::mssql::stage_source_objects(
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
                source_version: Some(args.source_version),
                path_prefix: vec![path_prefix.clone()],
                script_output: None,
            },
        )?)?
    };
    let staging_ms = staging_started.elapsed().as_millis();

    let activation_started = Instant::now();
    let activation = if args.dry_run {
        None
    } else if let Some(extension) = args.extension.as_deref() {
        Some(serde_json::to_value(
            crate::mssql_extension_load::activate_staged_extension(
                &MssqlActivateStagedExtensionArgs {
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
            )?,
        )?)
    } else {
        Some(serde_json::to_value(crate::mssql::activate_staged_main(
            &MssqlActivateStagedMainArgs {
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
            },
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
    } else if matches!(args.mode, MssqlMainActivationModeArg::Online) {
        vec![
            "ConfigSave".to_owned(),
            "Config".to_owned(),
            "Params".to_owned(),
        ]
    } else {
        vec!["ConfigSave".to_owned(), "Config".to_owned()]
    };

    Ok(MssqlApplySourceChangeReport {
        schema_version: 1,
        database: args.database.clone(),
        target: target_name,
        selected_path,
        mode: mode_name,
        dry_run: args.dry_run,
        executed: !args.dry_run,
        no_op: false,
        changed_paths,
        active_generation_or_root: old_from_activation.or(active_generation_or_root),
        proposed_generation_or_root,
        tables_changed,
        recovery_token,
        staging: Some(staging),
        activation,
        timings: MssqlApplySourceChangeTimings {
            active_export_ms,
            classification_ms,
            staging_ms,
            activation_ms,
            total_ms: total_started.elapsed().as_millis(),
        },
    })
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
