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
    pub selected_storage_file_names: Vec<String>,
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
            source_version: args.source_version,
        })?;
        report
            .extensions
            .first()
            .map(|entry| entry.active_cas_root.clone())
    } else {
        let report = crate::mssql_dump::dump_config(&MssqlDumpConfigArgs {
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
            file_names: selected_storage_file_names.clone(),
            file_name_lists: Vec::new(),
            inflate: false,
            extract_module_text: true,
            extract_metadata_xml: true,
            require_complete_root_metadata: false,
            require_complete_source_assets: false,
            collect_all_source_asset_diagnostics: false,
            source_version: args.source_version,
            no_binary_rows: true,
            write_binary_rows: false,
            write_manifest: false,
        })?;
        ensure_bounded_export_complete(
            &active_root,
            &bounded_source_paths,
            &selected_storage_file_names,
            &report,
        )?;
        overlay_active_dynamic_module(
            args,
            &selected_path,
            &selected_storage_file_names,
            &active_root,
        )?
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
            selected_storage_file_names,
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
    prepare_compile_tree_for_selected_change(&proposed_root, &selected_path)?;

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
    if args.dry_run && args.extension.is_none() {
        ensure_main_dry_run_stageable(&staging)?;
    }
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
        selected_storage_file_names,
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
    if table.rows != selected_storage_file_names.len() {
        bail!(
            "bounded export returned {} rows for {} exact storage identities",
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

fn overlay_active_dynamic_module(
    args: &MssqlApplySourceChangeArgs,
    selected_path: &str,
    selected_storage_file_names: &[String],
    active_root: &Path,
) -> Result<Option<String>> {
    let marker_name = std::collections::BTreeSet::from(["DynamicallyUpdated".to_owned()]);
    let password = if args.sql_user.is_some() {
        args.sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok())
    } else {
        None
    };
    let fetch = |table: &str, names: &std::collections::BTreeSet<String>| {
        crate::mssql_dump::fetch_main_activation_rows_bcp(
            &args.sqlcmd,
            &args.bcp_executable,
            &args.server,
            args.sql_user.as_deref(),
            password.as_deref(),
            &args.database,
            table,
            names,
        )
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
    let generation = generations
        .last()
        .expect("a dynamic marker has at least one generation")
        .clone();
    let body_id = selected_storage_file_names
        .iter()
        .find(|name| name.len() > 37 && name.as_bytes().get(36) == Some(&b'.'))
        .ok_or_else(|| anyhow!("selected storage closure has no body row"))?;
    let aliases = generations
        .iter()
        .map(|candidate| dynamic_alias(body_id, candidate))
        .collect::<std::collections::BTreeSet<_>>();
    let rows = fetch("Config", &aliases)?;
    let row = generations.iter().rev().find_map(|candidate| {
        let alias = dynamic_alias(body_id, candidate);
        rows.iter()
            .find(|row| row.file_name == alias && row.part_no == 0)
    });
    let Some(row) = row else {
        return Ok(Some(generation));
    };
    if selected_path.ends_with("/Ext/Form.xml") {
        bail!(
            "bounded apply of a form layout over an existing dynamic version of that form is not yet supported"
        );
    }
    let active_path = active_root.join(path_from_slashes(selected_path));
    let text = if selected_path.ends_with("/Ext/Form/Module.bsl") {
        crate::module_blob::parse_form_body_blob(&row.binary_data)?
            .module_text
            .into_bytes()
    } else {
        crate::module_blob::unpack_module_blob_text(&row.binary_data)?
    };
    fs::write(&active_path, text).with_context(|| {
        format!(
            "failed to overlay active dynamic body {}",
            active_path.display()
        )
    })?;
    Ok(Some(generation))
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
mod tests {
    use super::*;

    const OLD: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
    const NEW: &str = "968a0bc0-969b-4bf2-b9ef-19d0e8bf8ce4";
    const OWNER: &str = "ab132638-5188-470d-9432-de85f2b2c7d8";

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
}
