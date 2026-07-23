#![cfg(feature = "platform-oracle")]
//! Research-only infobase orchestration with installed-platform oracle calls.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::version::{PlatformBuild, XmlDialect};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::adapters::mssql_legacy::MssqlLegacyAdapter;
use crate::cli::{
    InfobaseConfigExportArgs, InfobaseConfigFormat, InfobaseConfigImportArgs,
    InfobaseConfigRoundtripArgs, InfobaseConfigSourceVersion, InfobaseConfigSweepArgs,
    MssqlCloneArgs, MssqlDumpConfigArgs, MssqlStageSourceObjectsArgs,
};
use crate::legacy_version::LegacyVersionAxes;
use crate::module_blob::{
    parse_common_module_xml_properties, parse_simple_metadata_xml_properties,
};
use crate::source::scan_sources_with_prefixes;

#[derive(Debug, Serialize)]
pub struct InfobaseConfigExportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    pub source_version: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub password_source: Option<String>,
    pub output_dir: PathBuf,
    pub temp_dump_dir: PathBuf,
    pub exported_files: usize,
    pub raw_rows: usize,
    pub metadata_xml_rows: usize,
    pub module_text_rows: usize,
    pub source_asset_rows: usize,
    pub dump_timings: crate::mssql_dump::MssqlDumpTimingReport,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigImportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    pub source_version: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub source_dir: PathBuf,
    pub staged_rows_before: i64,
    pub staged_rows_after: i64,
    pub scripts: Vec<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigRoundtripReport {
    pub operation: &'static str,
    pub backend_export: &'static str,
    pub backend_import: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub source_db: String,
    pub target_db: String,
    pub db_user: Option<String>,
    pub work_dir: PathBuf,
    pub baseline_dir: PathBuf,
    pub after_apply_dir: PathBuf,
    pub path_prefix: Vec<String>,
    pub reused_source_dir: bool,
    pub selected_after_apply_file_names: usize,
    pub clone: crate::mssql::MssqlCloneReport,
    pub baseline_export: Option<InfobaseConfigExportReport>,
    pub import: InfobaseConfigImportReport,
    pub check: IbcmdConfigStepReport,
    pub apply: IbcmdConfigStepReport,
    pub after_apply_export: InfobaseConfigExportReport,
    pub diff_output: PathBuf,
    pub diff: crate::plan::SourceDiffReport,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigSweepReport {
    pub operation: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub source_db: String,
    pub work_dir: PathBuf,
    pub baseline_dir: PathBuf,
    pub generated_prefixes: bool,
    pub candidate_offset: usize,
    pub candidates_per_family: usize,
    pub stopped_early: bool,
    pub prefixes: Vec<String>,
    pub results: Vec<InfobaseConfigSweepEntryReport>,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigSweepEntryReport {
    pub path_prefix: String,
    pub target_db: String,
    pub status: &'static str,
    pub work_dir: PathBuf,
    pub diff_output: Option<PathBuf>,
    pub selected_after_apply_file_names: Option<usize>,
    pub raw_rows: Option<usize>,
    pub exported_files: Option<usize>,
    pub check_duration_ms: Option<u128>,
    pub apply_duration_ms: Option<u128>,
    pub diff_summary: Option<SourceDiffSummary>,
    pub diff_examples: Option<Vec<SourceDiffSample>>,
    pub error: Option<String>,
    pub cleanup_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SourceDiffSummary {
    pub left_only: usize,
    pub right_only: usize,
    pub different: usize,
    pub unchanged: usize,
}

#[derive(Debug, Serialize)]
pub struct SourceDiffSample {
    pub status: crate::plan::SourceDiffStatus,
    pub path: String,
    pub kind: Option<crate::source::SourceKind>,
    pub object_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IbcmdConfigStepReport {
    pub command: String,
    pub data_dir: PathBuf,
    pub duration_ms: u128,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
struct ConnectionConfig {
    dbms: String,
    db_server: String,
    db_name: String,
    db_user: Option<String>,
    db_pwd: Option<String>,
    password_source: Option<String>,
    format: InfobaseConfigFormat,
    legacy_adapter: MssqlLegacyAdapter,
}

impl ConnectionConfig {
    fn legacy_source_version(&self) -> Result<InfobaseConfigSourceVersion> {
        self.legacy_adapter.legacy_selector().ok_or_else(|| {
            anyhow!(
                "legacy MSSQL adapter does not support XML dialect {}",
                self.legacy_adapter.xml_dialect()
            )
        })
    }
}

pub fn export_config(args: &InfobaseConfigExportArgs) -> Result<InfobaseConfigExportReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;
    export_config_report(
        &config,
        &args.sqlcmd,
        &args.db_pwd_env,
        &args.output_dir,
        args.overwrite,
        Vec::new(),
    )
}

fn export_config_report(
    config: &ConnectionConfig,
    sqlcmd: &Path,
    db_pwd_env: &str,
    output_dir_arg: &Path,
    overwrite: bool,
    file_names: Vec<String>,
) -> Result<InfobaseConfigExportReport> {
    let source_version = config.legacy_source_version()?;
    let output_dir = absolute_path(output_dir_arg)?;
    prepare_output_dir(&output_dir, overwrite)?;
    let dump_args = MssqlDumpConfigArgs {
        sqlcmd: sqlcmd.to_path_buf(),
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: db_pwd_env.to_string(),
        database: config.db_name.clone(),
        output_dir: output_dir.clone(),
        overwrite: false,
        include_config_save: false,
        file_names,
        file_name_lists: Vec::new(),
        inflate: false,
        extract_module_text: true,
        extract_metadata_xml: true,
        require_complete_root_metadata: false,
        no_binary_rows: true,
        write_binary_rows: false,
        write_manifest: false,
        source_version,
    };
    let dump = crate::mssql_dump::dump_config(&dump_args)?;

    let exported_files = count_files(&output_dir)?;

    Ok(InfobaseConfigExportReport {
        operation: "infobase config export",
        backend: "mssql-config-direct",
        format: format_name(config.format),
        source_version: source_version.as_str(),
        dbms: config.dbms.clone(),
        db_server: config.db_server.clone(),
        db_name: config.db_name.clone(),
        db_user: config.db_user.clone(),
        password_source: config.password_source.clone(),
        output_dir,
        temp_dump_dir: dump_args.output_dir,
        exported_files,
        raw_rows: dump.total_rows,
        metadata_xml_rows: dump.total_metadata_xml_rows,
        module_text_rows: dump.total_module_text_rows,
        source_asset_rows: dump.total_source_asset_rows,
        dump_timings: dump.timings,
    })
}

pub fn import_config(args: &InfobaseConfigImportArgs) -> Result<InfobaseConfigImportReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;

    let stage_args = build_import_stage_args(&config, args)?;
    let report = crate::mssql::stage_source_objects(&stage_args)?;
    let source_version = config.legacy_source_version()?;

    Ok(InfobaseConfigImportReport {
        operation: "infobase config import",
        backend: "mssql-configsave-stage",
        format: format_name(config.format),
        source_version: source_version.as_str(),
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        source_dir: stage_args.source_root,
        staged_rows_before: report.before.row_count,
        staged_rows_after: report.after.row_count,
        scripts: report.scripts,
    })
}

pub fn roundtrip_config(
    args: &InfobaseConfigRoundtripArgs,
) -> Result<InfobaseConfigRoundtripReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;

    let settings = match args.settings.as_deref() {
        Some(path) => Some(read_settings(path)?),
        None => None,
    };
    let infobase_user = first_value(
        args.user.as_deref(),
        settings_string_at(&settings, &["ibcmd-rs", "ib-user"])
            .or_else(|| settings_string_at(&settings, &["ibcmd-rs", "user"]))
            .or_else(|| settings_string_at(&settings, &["ibcmd-rs", "usr"]))
            .or_else(|| settings_value(&settings, "ib-user"))
            .or_else(|| settings_value(&settings, "user"))
            .or_else(|| settings_value(&settings, "usr")),
    );
    let infobase_password = resolve_optional_password(
        infobase_user.as_deref(),
        args.password.as_deref(),
        &args.password_env,
    )?;

    let ibcmd = crate::dump_sources::resolve_ibcmd(args.ibcmd.as_deref())?;
    let work_root = absolute_path(&args.work_dir)?;
    fs::create_dir_all(&work_root)
        .with_context(|| format!("failed to create {}", work_root.display()))?;
    let target_db = args
        .target_db
        .clone()
        .unwrap_or_else(|| default_roundtrip_target_db(&config.db_name));
    let run_dir = work_root.join(&target_db);
    prepare_output_dir(&run_dir, args.overwrite)?;

    let baseline_dir = match &args.source_dir {
        Some(path) => absolute_path(path)?,
        None => run_dir.join("baseline"),
    };
    let after_apply_dir = run_dir.join("after_apply");
    let diff_output = run_dir.join("source-diff.json");
    let data_dir = match &args.data_dir {
        Some(path) => absolute_path(path)?,
        None => run_dir.join("ibcmd-data"),
    };
    prepare_output_dir(&data_dir, args.overwrite)?;

    let baseline_export = if args.source_dir.is_some() {
        None
    } else {
        Some(export_config(&InfobaseConfigExportArgs {
            settings: args.settings.clone(),
            format: args.format,
            source_version: args.source_version,
            dbms: Some(config.dbms.clone()),
            db_server: Some(config.db_server.clone()),
            db_name: Some(config.db_name.clone()),
            db_user: config.db_user.clone(),
            db_pwd: config.db_pwd.clone(),
            db_pwd_env: args.db_pwd_env.clone(),
            user: infobase_user.clone(),
            password: infobase_password.clone(),
            password_env: args.password_env.clone(),
            sqlcmd: args.sqlcmd.clone(),
            overwrite: args.overwrite,
            output_dir: baseline_dir.clone(),
        })?)
    };

    let clone = crate::mssql::clone_database(&MssqlCloneArgs {
        server: config.db_server.clone(),
        sqlcmd: args.sqlcmd.clone(),
        source: config.db_name.clone(),
        target: target_db.clone(),
        backup: args.backup.clone(),
        overwrite: args.overwrite,
        allow_non_lab: args.allow_non_lab,
    })?;

    let import = import_config(&InfobaseConfigImportArgs {
        settings: args.settings.clone(),
        format: args.format,
        source_version: args.source_version,
        dbms: Some(config.dbms.clone()),
        db_server: Some(config.db_server.clone()),
        db_name: Some(target_db.clone()),
        db_user: config.db_user.clone(),
        db_pwd: config.db_pwd.clone(),
        db_pwd_env: args.db_pwd_env.clone(),
        user: infobase_user.clone(),
        password: infobase_password.clone(),
        password_env: args.password_env.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: true,
        allow_non_lab: args.allow_non_lab,
        batch_size: args.batch_size,
        path_prefix: args.path_prefix.clone(),
        script_output: args.script_output.clone(),
        source_dir: baseline_dir.clone(),
    })?;

    let check = run_ibcmd_config_step(
        &ibcmd,
        "check",
        &config,
        &target_db,
        infobase_user.as_deref(),
        infobase_password.as_deref(),
        &data_dir,
        Duration::from_secs(args.timeout_sec),
    )?;
    let apply = run_ibcmd_config_step(
        &ibcmd,
        "apply",
        &config,
        &target_db,
        infobase_user.as_deref(),
        infobase_password.as_deref(),
        &data_dir,
        Duration::from_secs(args.timeout_sec),
    )?;

    let selected_after_apply_file_names = if !args.path_prefix.is_empty() {
        let file_names =
            build_selected_dump_file_names_from_source(&baseline_dir, &args.path_prefix)
                .with_context(|| {
                    format!(
                        "failed to resolve selected dump file names from {} for path prefixes {:?}",
                        baseline_dir.display(),
                        args.path_prefix
                    )
                })?;
        if file_names.is_empty() {
            bail!(
                "selected dump file name resolution returned no Config rows for {} and path prefixes {:?}",
                baseline_dir.display(),
                args.path_prefix
            );
        }
        file_names
    } else {
        Vec::new()
    };
    let selected_after_apply_file_name_count = selected_after_apply_file_names.len();
    let after_apply_export = export_config_report(
        &ConnectionConfig {
            dbms: config.dbms.clone(),
            db_server: config.db_server.clone(),
            db_name: target_db.clone(),
            db_user: config.db_user.clone(),
            db_pwd: config.db_pwd.clone(),
            password_source: config.password_source.clone(),
            format: config.format,
            legacy_adapter: config.legacy_adapter.clone(),
        },
        &args.sqlcmd,
        &args.db_pwd_env,
        &after_apply_dir,
        args.overwrite,
        selected_after_apply_file_names,
    )?;

    let diff = crate::plan::diff_source_trees(&baseline_dir, &after_apply_dir, &args.path_prefix)?;
    crate::plan::write_source_diff(&diff, &diff_output)?;

    Ok(InfobaseConfigRoundtripReport {
        operation: "infobase config roundtrip",
        backend_export: "mssql-config-direct",
        backend_import: "mssql-configsave-stage",
        dbms: config.dbms,
        db_server: config.db_server,
        source_db: config.db_name,
        target_db,
        db_user: config.db_user,
        work_dir: run_dir,
        baseline_dir,
        after_apply_dir,
        path_prefix: args.path_prefix.clone(),
        reused_source_dir: args.source_dir.is_some(),
        selected_after_apply_file_names: selected_after_apply_file_name_count,
        clone,
        baseline_export,
        import,
        check,
        apply,
        after_apply_export,
        diff_output,
        diff,
    })
}

pub fn sweep_config(args: &InfobaseConfigSweepArgs) -> Result<InfobaseConfigSweepReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;

    let work_dir = absolute_path(&args.work_dir)?;
    fs::create_dir_all(&work_dir)
        .with_context(|| format!("failed to create {}", work_dir.display()))?;
    let baseline_dir = match &args.source_dir {
        Some(path) => absolute_path(path)?,
        None => work_dir.join(format!("{}_sweep_baseline", config.db_name)),
    };

    if args.source_dir.is_none() {
        export_config(&InfobaseConfigExportArgs {
            settings: args.settings.clone(),
            format: args.format,
            source_version: args.source_version,
            dbms: Some(config.dbms.clone()),
            db_server: Some(config.db_server.clone()),
            db_name: Some(config.db_name.clone()),
            db_user: config.db_user.clone(),
            db_pwd: config.db_pwd.clone(),
            db_pwd_env: args.db_pwd_env.clone(),
            user: args.user.clone(),
            password: args.password.clone(),
            password_env: args.password_env.clone(),
            sqlcmd: args.sqlcmd.clone(),
            overwrite: args.overwrite,
            output_dir: baseline_dir.clone(),
        })?;
    }

    let generated_prefixes = args.path_prefix.is_empty();
    let prefixes = if generated_prefixes {
        representative_sweep_prefixes(
            &baseline_dir,
            &args.family,
            args.max_prefixes,
            args.candidate_offset,
            args.candidates_per_family,
        )?
    } else {
        args.path_prefix.clone()
    };
    if prefixes.is_empty() {
        bail!(
            "no representative source prefixes found under {}",
            baseline_dir.display()
        );
    }

    let mut results = Vec::with_capacity(prefixes.len());
    let mut stopped_early = false;
    for (index, prefix) in prefixes.iter().enumerate() {
        let target_db = format!("{}_sweep_{:02}", config.db_name, index + 1);
        let roundtrip_args = InfobaseConfigRoundtripArgs {
            settings: args.settings.clone(),
            format: args.format,
            source_version: args.source_version,
            dbms: Some(config.dbms.clone()),
            db_server: Some(config.db_server.clone()),
            db_name: Some(config.db_name.clone()),
            db_user: config.db_user.clone(),
            db_pwd: config.db_pwd.clone(),
            db_pwd_env: args.db_pwd_env.clone(),
            user: args.user.clone(),
            password: args.password.clone(),
            password_env: args.password_env.clone(),
            sqlcmd: args.sqlcmd.clone(),
            ibcmd: args.ibcmd.clone(),
            target_db: Some(target_db.clone()),
            backup: None,
            work_dir: work_dir.clone(),
            source_dir: Some(baseline_dir.clone()),
            data_dir: None,
            overwrite: args.overwrite,
            allow_non_lab: args.allow_non_lab,
            timeout_sec: args.timeout_sec,
            batch_size: args.batch_size,
            path_prefix: vec![prefix.clone()],
            script_output: args.script_output.clone(),
        };
        let mut entry = match roundtrip_config(&roundtrip_args) {
            Ok(report) => InfobaseConfigSweepEntryReport {
                path_prefix: prefix.clone(),
                target_db: report.target_db.clone(),
                status: if report.diff.summary.different == 0
                    && report.diff.summary.left_only == 0
                    && report.diff.summary.right_only == 0
                {
                    "ok"
                } else {
                    "diff"
                },
                work_dir: report.work_dir.clone(),
                diff_output: Some(report.diff_output.clone()),
                selected_after_apply_file_names: Some(report.selected_after_apply_file_names),
                raw_rows: Some(report.after_apply_export.raw_rows),
                exported_files: Some(report.after_apply_export.exported_files),
                check_duration_ms: Some(report.check.duration_ms),
                apply_duration_ms: Some(report.apply.duration_ms),
                diff_summary: Some(SourceDiffSummary {
                    left_only: report.diff.summary.left_only,
                    right_only: report.diff.summary.right_only,
                    different: report.diff.summary.different,
                    unchanged: report.diff.summary.unchanged,
                }),
                diff_examples: Some(sample_source_diff_entries(&report.diff, 8)),
                error: None,
                cleanup_error: None,
            },
            Err(error) => InfobaseConfigSweepEntryReport {
                path_prefix: prefix.clone(),
                target_db,
                status: "error",
                work_dir: work_dir.join(format!("{}_sweep_{:02}", config.db_name, index + 1)),
                diff_output: None,
                selected_after_apply_file_names: None,
                raw_rows: None,
                exported_files: None,
                check_duration_ms: None,
                apply_duration_ms: None,
                diff_summary: None,
                diff_examples: None,
                error: Some(format!("{error:#}")),
                cleanup_error: None,
            },
        };
        if args.drop_target_db_after_run {
            if let Err(error) = crate::mssql::drop_database(
                &args.sqlcmd,
                &config.db_server,
                &entry.target_db,
                args.allow_non_lab,
            ) {
                entry.cleanup_error = Some(format!("{error:#}"));
            }
        }
        let stop_here = args.stop_on_first_non_ok && entry.status != "ok";
        results.push(entry);
        if stop_here {
            stopped_early = index + 1 < prefixes.len();
            break;
        }
    }

    Ok(InfobaseConfigSweepReport {
        operation: "infobase config sweep",
        dbms: config.dbms,
        db_server: config.db_server,
        source_db: config.db_name,
        work_dir,
        baseline_dir,
        generated_prefixes,
        candidate_offset: args.candidate_offset,
        candidates_per_family: args.candidates_per_family,
        stopped_early,
        prefixes,
        results,
    })
}

fn representative_sweep_prefixes(
    source_root: &Path,
    families: &[String],
    max_prefixes: Option<usize>,
    candidate_offset: usize,
    candidates_per_family: usize,
) -> Result<Vec<String>> {
    if candidates_per_family == 0 {
        bail!("candidates_per_family must be at least 1");
    }
    let manifest = crate::source::scan_sources(source_root)?;
    let requested = if families.is_empty() {
        DEFAULT_SWEEP_FAMILIES
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    } else {
        families.to_vec()
    };
    let requested_set = requested
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    let mut candidates =
        std::collections::BTreeMap::<String, Vec<RepresentativeSweepPrefixStats>>::new();
    for file in &manifest.files {
        let relative = file.path.replace('\\', "/");
        if relative == "Configuration.xml"
            || relative.matches('/').count() != 1
            || !relative.ends_with(".xml")
        {
            continue;
        }
        let mut parts = relative.split('/');
        let Some(family) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        if !requested_set.contains(&family.to_ascii_lowercase()) {
            continue;
        }
        let prefix = format!("{family}/{}", name.trim_end_matches(".xml"));
        let stats = representative_sweep_prefix_stats(&manifest.files, &prefix);
        candidates
            .entry(family.to_string())
            .or_default()
            .push(stats);
    }

    let mut prefixes = Vec::new();
    for family in requested {
        if let Some(candidates) = candidates.get_mut(&family) {
            candidates.sort_by(|left, right| {
                right
                    .kind_weight
                    .cmp(&left.kind_weight)
                    .then_with(|| right.file_count.cmp(&left.file_count))
                    .then_with(|| right.total_bytes.cmp(&left.total_bytes))
                    .then_with(|| left.prefix.cmp(&right.prefix))
            });
            for selected in candidates
                .iter()
                .skip(candidate_offset)
                .take(candidates_per_family)
            {
                prefixes.push(selected.prefix.clone());
            }
        }
    }
    if let Some(limit) = max_prefixes {
        prefixes.truncate(limit);
    }
    Ok(prefixes)
}

fn sample_source_diff_entries(
    diff: &crate::plan::SourceDiffReport,
    limit: usize,
) -> Vec<SourceDiffSample> {
    let mut samples = Vec::new();
    for status in [
        crate::plan::SourceDiffStatus::Different,
        crate::plan::SourceDiffStatus::LeftOnly,
        crate::plan::SourceDiffStatus::RightOnly,
    ] {
        for entry in diff
            .differences
            .iter()
            .filter(|entry| entry.status == status)
        {
            samples.push(SourceDiffSample {
                status: entry.status.clone(),
                path: entry.path.clone(),
                kind: entry.kind.clone(),
                object_hint: entry.object_hint.clone(),
            });
            if samples.len() >= limit {
                return samples;
            }
        }
    }
    samples
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RepresentativeSweepPrefixStats {
    prefix: String,
    kind_weight: usize,
    file_count: usize,
    total_bytes: u64,
}

fn representative_sweep_prefix_stats(
    files: &[crate::source::SourceFile],
    prefix: &str,
) -> RepresentativeSweepPrefixStats {
    let prefix_dir = format!("{prefix}/");
    let prefix_xml = format!("{prefix}.xml");
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    let mut kind_weight = 0usize;
    for file in files {
        let relative = file.path.replace('\\', "/");
        if relative != prefix_xml && !relative.starts_with(&prefix_dir) {
            continue;
        }
        file_count += 1;
        total_bytes += file.size_bytes;
        kind_weight += match file.kind {
            crate::source::SourceKind::Form => 5,
            crate::source::SourceKind::Template => 4,
            crate::source::SourceKind::Module => 3,
            crate::source::SourceKind::MetadataXml => 2,
            crate::source::SourceKind::ConfigurationRoot => 0,
            crate::source::SourceKind::Binary
            | crate::source::SourceKind::OtherXml
            | crate::source::SourceKind::Other => 1,
        };
    }
    RepresentativeSweepPrefixStats {
        prefix: prefix.to_string(),
        kind_weight,
        file_count,
        total_bytes,
    }
}

const DEFAULT_SWEEP_FAMILIES: &[&str] = &[
    "Catalogs",
    "Documents",
    "Reports",
    "DataProcessors",
    "InformationRegisters",
    "AccumulationRegisters",
    "AccountingRegisters",
    "CalculationRegisters",
    "Enums",
    "CommonModules",
    "CommonCommands",
];

fn build_import_stage_args(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<MssqlStageSourceObjectsArgs> {
    Ok(MssqlStageSourceObjectsArgs {
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: args.db_pwd_env.clone(),
        database: config.db_name.clone(),
        source_root: absolute_path(&args.source_dir)?,
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        batch_size: args.batch_size,
        source_version: Some(config.legacy_source_version()?),
        path_prefix: args.path_prefix.clone(),
        script_output: args.script_output.clone(),
    })
}

#[derive(Clone)]
struct SelectedSourceOwner {
    kind: String,
    uuid: String,
    is_common_module: bool,
}

fn build_selected_dump_file_names_from_source(
    source_root: &Path,
    path_prefix: &[String],
) -> Result<Vec<String>> {
    let manifest = scan_sources_with_prefixes(source_root, path_prefix)?;
    let mut cache = std::collections::BTreeMap::<PathBuf, Option<SelectedSourceOwner>>::new();
    let mut selected = std::collections::BTreeSet::<String>::new();
    for file in &manifest.files {
        let relative = PathBuf::from(&file.path);
        for file_name in selected_dump_file_names_for_path(source_root, &relative, &mut cache)? {
            selected.insert(file_name);
        }
    }
    Ok(selected.into_iter().collect())
}

fn selected_dump_file_names_for_path(
    source_root: &Path,
    relative: &Path,
    cache: &mut std::collections::BTreeMap<PathBuf, Option<SelectedSourceOwner>>,
) -> Result<Vec<String>> {
    let path = relative.to_string_lossy().replace('\\', "/");
    let file_name = relative
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if path == "Configuration.xml" {
        return Ok(Vec::new());
    }

    if file_name.eq_ignore_ascii_case("Module.bsl")
        && path.ends_with("/Ext/Form/Module.bsl")
        && let Some(owner) = load_selected_source_owner(source_root, relative, cache)?
    {
        return Ok(vec![owner.uuid.clone(), format!("{}.0", owner.uuid)]);
    }

    if file_name.eq_ignore_ascii_case("Form.xml")
        && path.ends_with("/Ext/Form.xml")
        && let Some(owner) = load_selected_source_owner(source_root, relative, cache)?
    {
        return Ok(vec![owner.uuid.clone(), format!("{}.0", owner.uuid)]);
    }

    if path.contains("/Ext/Help/")
        || (file_name.eq_ignore_ascii_case("Help.xml") && path.ends_with("/Ext/Help.xml"))
    {
        if let Some(owner) = load_selected_source_owner(source_root, relative, cache)? {
            return Ok(vec![
                owner.uuid.clone(),
                infer_help_body_id_for_owner(&owner.kind, &owner.uuid),
            ]);
        }
    }

    if file_name.eq_ignore_ascii_case("Predefined.xml")
        && path.ends_with("/Ext/Predefined.xml")
        && let Some(owner) = load_selected_source_owner(source_root, relative, cache)?
    {
        if let Some(suffix) = predefined_data_body_suffix_for_owner_kind(&owner.kind) {
            return Ok(vec![
                owner.uuid.clone(),
                format!("{}.{}", owner.uuid, suffix),
            ]);
        }
    }

    if file_name.eq_ignore_ascii_case("Template.xml")
        || file_name.eq_ignore_ascii_case("Template.bin")
        || file_name.eq_ignore_ascii_case("Template.txt")
    {
        if path.contains("/Ext/Template.") {
            if let Some(owner) = load_selected_source_owner(source_root, relative, cache)? {
                return Ok(vec![owner.uuid.clone(), format!("{}.0", owner.uuid)]);
            }
        }
    }

    if file_name.eq_ignore_ascii_case("CommandModule.bsl")
        && path.contains("/Commands/")
        && let Some(file_names) = selected_nested_command_file_names(source_root, relative)?
    {
        return Ok(file_names);
    }

    if file_name.ends_with(".bsl") && path.contains("/Ext/") {
        if let Some(owner) = load_selected_source_owner(source_root, relative, cache)? {
            if file_name.eq_ignore_ascii_case("CommandModule.bsl") {
                return Ok(vec![owner.uuid.clone(), format!("{}.2", owner.uuid)]);
            }
            if owner.is_common_module && file_name.eq_ignore_ascii_case("Module.bsl") {
                return Ok(vec![owner.uuid.clone(), format!("{}.0", owner.uuid)]);
            }
            if let Some(suffix) = object_module_body_suffix_for_file(&owner.kind, file_name) {
                return Ok(vec![
                    owner.uuid.clone(),
                    format!("{}.{}", owner.uuid, suffix),
                ]);
            }
        }
        return Err(anyhow!(
            "unsupported source module path for selected dump: {}",
            relative.display()
        ));
    }

    if file_name.ends_with(".xml") && !path.contains("/Ext/") {
        let bytes = fs::read(source_root.join(relative))
            .with_context(|| format!("failed to read {}", source_root.join(relative).display()))?;
        if path.starts_with("CommonModules/") {
            let properties = parse_common_module_xml_properties(&bytes)?;
            return Ok(vec![properties.uuid]);
        }
        let properties = parse_simple_metadata_xml_properties(&bytes)?;
        return Ok(vec![properties.uuid]);
    }

    Ok(Vec::new())
}

fn selected_nested_command_file_names(
    source_root: &Path,
    relative: &Path,
) -> Result<Option<Vec<String>>> {
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let Some(commands_index) = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("Commands"))
    else {
        return Ok(None);
    };
    if commands_index == 0 || commands_index + 3 >= parts.len() {
        return Ok(None);
    }
    let command_name = parts[commands_index + 1].clone();
    let mut owner_parts = parts[..commands_index].to_vec();
    let Some(owner_name) = owner_parts.last_mut() else {
        return Ok(None);
    };
    *owner_name = format!("{owner_name}.xml");
    let owner_relative = PathBuf::from_iter(owner_parts.iter().map(PathBuf::from));
    let owner_path = source_root.join(&owner_relative);
    if !owner_path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&owner_path)
        .with_context(|| format!("failed to read {}", owner_path.display()))?;
    let Some(uuid) = find_nested_command_uuid(&bytes, &command_name)? else {
        return Ok(None);
    };
    Ok(Some(vec![uuid.clone(), format!("{uuid}.2")]))
}

fn find_nested_command_uuid(bytes: &[u8], command_name: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut in_command = false;
    let mut current_uuid = None::<String>;
    let mut current_name = None::<String>;
    let mut reading_name = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let local = event.local_name();
                if local.as_ref() == b"Command" {
                    in_command = true;
                    current_name = None;
                    current_uuid = event.attributes().flatten().find_map(|attr| {
                        (attr.key.local_name().as_ref() == b"uuid")
                            .then(|| String::from_utf8_lossy(attr.value.as_ref()).to_string())
                    });
                } else if in_command && local.as_ref() == b"Name" {
                    reading_name = true;
                }
            }
            Ok(Event::End(event)) => {
                let local = event.local_name();
                if local.as_ref() == b"Name" {
                    reading_name = false;
                } else if local.as_ref() == b"Command" {
                    if current_name.as_deref() == Some(command_name) {
                        return Ok(current_uuid);
                    }
                    in_command = false;
                    current_uuid = None;
                    current_name = None;
                    reading_name = false;
                }
            }
            Ok(Event::Text(text)) => {
                if in_command && reading_name {
                    current_name = Some(String::from_utf8_lossy(text.as_ref()).to_string());
                }
            }
            Ok(Event::Eof) => return Ok(None),
            Err(error) => {
                return Err(anyhow!(
                    "failed to parse nested command metadata XML: {error}"
                ));
            }
            _ => {}
        }
        buf.clear();
    }
}

fn load_selected_source_owner(
    source_root: &Path,
    relative: &Path,
    cache: &mut std::collections::BTreeMap<PathBuf, Option<SelectedSourceOwner>>,
) -> Result<Option<SelectedSourceOwner>> {
    let Some(owner_relative) = owner_metadata_relative_path(relative) else {
        return Ok(None);
    };
    if let Some(cached) = cache.get(&owner_relative) {
        return Ok(cached.clone());
    }
    let owner_path = source_root.join(&owner_relative);
    if !owner_path.is_file() {
        cache.insert(owner_relative, None);
        return Ok(None);
    }
    let bytes = fs::read(&owner_path)
        .with_context(|| format!("failed to read {}", owner_path.display()))?;
    let owner = if owner_relative
        .to_string_lossy()
        .replace('\\', "/")
        .starts_with("CommonModules/")
    {
        let properties = parse_common_module_xml_properties(&bytes)?;
        Some(SelectedSourceOwner {
            kind: "CommonModule".to_string(),
            uuid: properties.uuid,
            is_common_module: true,
        })
    } else {
        let properties = parse_simple_metadata_xml_properties(&bytes)?;
        Some(SelectedSourceOwner {
            kind: properties.kind,
            uuid: properties.uuid,
            is_common_module: false,
        })
    };
    cache.insert(owner_relative, owner.clone());
    Ok(owner)
}

fn owner_metadata_relative_path(relative: &Path) -> Option<PathBuf> {
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let ext_index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("Ext"))?;
    if ext_index == 0 {
        return None;
    }
    let owner_name = parts.get(ext_index - 1)?.clone();
    let mut owner_parts = parts[..ext_index].to_vec();
    *owner_parts.last_mut()? = format!("{owner_name}.xml");
    Some(owner_parts.into_iter().collect())
}

fn infer_help_body_id_for_owner(kind: &str, uuid: &str) -> String {
    let suffix = if matches!(kind, "Form" | "CommonForm") {
        "1"
    } else {
        "5"
    };
    format!("{uuid}.{suffix}")
}

fn object_module_body_suffix_for_file(kind: &str, file_name: &str) -> Option<&'static str> {
    match (kind, file_name) {
        ("Bot", "Module.bsl") => Some("1"),
        ("CommonCommand", "CommandModule.bsl") => Some("2"),
        ("Constant", "ValueManagerModule.bsl") => Some("0"),
        ("Constant", "ManagerModule.bsl") => Some("1"),
        ("FilterCriterion", "ManagerModule.bsl") => Some("0"),
        ("SettingsStorage", "ManagerModule.bsl") => Some("8"),
        ("Sequence", "RecordSetModule.bsl") => Some("0"),
        ("Catalog", "ObjectModule.bsl") => Some("0"),
        ("Catalog", "ManagerModule.bsl") => Some("3"),
        ("Report", "ObjectModule.bsl")
        | ("DataProcessor", "ObjectModule.bsl")
        | ("Document", "ObjectModule.bsl") => Some("0"),
        ("Report", "ManagerModule.bsl")
        | ("DataProcessor", "ManagerModule.bsl")
        | ("Document", "ManagerModule.bsl") => Some("2"),
        ("Enum", "ManagerModule.bsl") => Some("0"),
        ("ExchangePlan", "ObjectModule.bsl") => Some("2"),
        ("ExchangePlan", "ManagerModule.bsl") => Some("3"),
        ("AccumulationRegister", "RecordSetModule.bsl")
        | ("AccountingRegister", "RecordSetModule.bsl")
        | ("CalculationRegister", "RecordSetModule.bsl")
        | ("InformationRegister", "RecordSetModule.bsl") => Some("1"),
        ("AccumulationRegister", "ManagerModule.bsl")
        | ("AccountingRegister", "ManagerModule.bsl")
        | ("CalculationRegister", "ManagerModule.bsl")
        | ("InformationRegister", "ManagerModule.bsl") => Some("2"),
        ("DocumentJournal", "ManagerModule.bsl") => Some("1"),
        ("Task", "ObjectModule.bsl") => Some("6"),
        ("Task", "ManagerModule.bsl") => Some("7"),
        ("BusinessProcess", "ObjectModule.bsl") => Some("6"),
        ("BusinessProcess", "ManagerModule.bsl") => Some("8"),
        ("ChartOfCharacteristicTypes", "ObjectModule.bsl") => Some("15"),
        ("ChartOfCharacteristicTypes", "ManagerModule.bsl") => Some("16"),
        ("HTTPService", "Module.bsl")
        | ("WebService", "Module.bsl")
        | ("IntegrationService", "Module.bsl") => Some("0"),
        _ => None,
    }
}

fn default_roundtrip_target_db(source_db: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    format!("{source_db}_roundtrip_{now}")
}

fn resolve_optional_password(
    user: Option<&str>,
    cli_password: Option<&str>,
    password_env: &str,
) -> Result<Option<String>> {
    if user.is_none() {
        return Ok(None);
    }
    if let Some(value) = cli_password.filter(|value| !value.is_empty()) {
        return Ok(Some(value.to_string()));
    }
    if let Ok(value) = env::var(password_env) {
        return Ok(Some(value));
    }
    Ok(None)
}

fn render_command(program: &Path, command: &Command) -> String {
    let mut parts = vec![program.display().to_string()];
    parts.extend(
        command
            .get_args()
            .map(|value| value.to_string_lossy().to_string()),
    );
    parts.join(" ")
}

fn tool_path_arg(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

fn run_ibcmd_config_step(
    ibcmd: &Path,
    action: &str,
    config: &ConnectionConfig,
    target_db: &str,
    infobase_user: Option<&str>,
    infobase_password: Option<&str>,
    data_dir: &Path,
    timeout: Duration,
) -> Result<IbcmdConfigStepReport> {
    let mut command = Command::new(ibcmd);
    command
        .arg("infobase")
        .arg("config")
        .arg(action)
        .arg(format!("--dbms={}", config.dbms))
        .arg(format!("--db-server={}", config.db_server))
        .arg(format!("--db-name={target_db}"))
        .arg(format!("--data={}", tool_path_arg(data_dir)));
    if let Some(user) = &config.db_user {
        command.arg(format!("--db-user={user}"));
    }
    if let Some(password) = &config.db_pwd {
        command.arg(format!("--db-pwd={password}"));
    }
    if let Some(user) = infobase_user {
        command.arg(format!("--user={user}"));
    }
    if let Some(password) = infobase_password {
        command.arg(format!("--password={password}"));
    }
    if action.eq_ignore_ascii_case("apply") {
        command
            .arg("--dynamic=disable")
            .arg("--session-terminate=force");
    }

    let rendered = render_command(ibcmd, &command);
    let started = SystemTime::now();
    let output = crate::dump_sources::run_with_timeout(command, timeout)
        .with_context(|| format!("failed to run ibcmd config {action}"))?;
    let duration_ms = started.elapsed()?.as_millis();
    if output.timed_out {
        bail!(
            "ibcmd config {action} timed out after {} seconds",
            timeout.as_secs()
        );
    }
    if !output.success {
        bail!(
            "ibcmd config {action} failed with exit code {:?}\nstdout:\n{}\nstderr:\n{}",
            output.exit_code,
            output.stdout,
            output.stderr
        );
    }

    Ok(IbcmdConfigStepReport {
        command: rendered,
        data_dir: data_dir.to_path_buf(),
        duration_ms,
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn resolve_connection(
    settings_path: Option<&Path>,
    cli_format: Option<InfobaseConfigFormat>,
    cli_source_version: Option<InfobaseConfigSourceVersion>,
    cli_dbms: Option<&str>,
    cli_db_server: Option<&str>,
    cli_db_name: Option<&str>,
    cli_db_user: Option<&str>,
    cli_db_pwd: Option<&str>,
    db_pwd_env: &str,
) -> Result<ConnectionConfig> {
    let settings = match settings_path {
        Some(path) => Some(read_settings(path)?),
        None => None,
    };

    let format = cli_format
        .or_else(|| settings_format(&settings))
        .unwrap_or(InfobaseConfigFormat::Xml);
    let legacy_adapter = resolve_legacy_adapter(&settings, cli_source_version)?;
    let dbms = first_value(cli_dbms, settings_value(&settings, "dbms-type"))
        .unwrap_or_else(|| "MSSQLServer".to_string());
    let db_server = first_value(cli_db_server, settings_value(&settings, "dbms-server"))
        .unwrap_or_else(|| "localhost".to_string());
    let db_name = first_value(cli_db_name, settings_value(&settings, "dbms-base"))
        .ok_or_else(|| anyhow!("database name is required: pass --db-name or --settings"))?;
    let db_user = first_value(cli_db_user, settings_value(&settings, "dbms-user"));
    let (db_pwd, password_source) = match db_user {
        Some(_) => resolve_password(cli_db_pwd, &settings, db_pwd_env)?,
        None => (None, None),
    };

    Ok(ConnectionConfig {
        dbms,
        db_server,
        db_name,
        db_user,
        db_pwd,
        password_source,
        format,
        legacy_adapter,
    })
}

fn read_settings(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read settings {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn settings_value(settings: &Option<Value>, name: &str) -> Option<String> {
    settings
        .as_ref()?
        .get("vrunner")?
        .get(name)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn settings_format(settings: &Option<Value>) -> Option<InfobaseConfigFormat> {
    let value = settings_string_at(settings, &["ibcmd-rs", "config-format"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "format"]))
        .or_else(|| settings_string_at(settings, &["format"]))
        .or_else(|| settings_value(settings, "format"))?;
    parse_format(&value)
}

fn resolve_legacy_adapter(
    settings: &Option<Value>,
    cli_source_version: Option<InfobaseConfigSourceVersion>,
) -> Result<MssqlLegacyAdapter> {
    let xml_dialect = match cli_source_version {
        Some(selector) => selector.version_axes().xml_dialect().clone(),
        None => settings_xml_dialect(settings)?.unwrap_or_else(|| {
            XmlDialect::parse(InfobaseConfigSourceVersion::V2_20.as_str())
                .expect("default legacy XML dialect is valid")
        }),
    };
    let version_axes = LegacyVersionAxes::new(
        xml_dialect,
        settings_platform_build(settings)?,
        None,
        None,
        None,
    );
    let legacy_adapter = MssqlLegacyAdapter::new(version_axes)?;
    if legacy_adapter.legacy_selector().is_none() {
        bail!(
            "legacy MSSQL adapter does not support XML dialect {}",
            legacy_adapter.xml_dialect()
        );
    }
    Ok(legacy_adapter)
}

fn settings_xml_dialect(settings: &Option<Value>) -> Result<Option<XmlDialect>> {
    let value = settings_string_at(settings, &["ibcmd-rs", "source-version"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xml-version"]))
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xcf-version"]))
        .or_else(|| settings_string_at(settings, &["source-version"]))
        .or_else(|| settings_string_at(settings, &["xml-version"]))
        .or_else(|| settings_string_at(settings, &["xcf-version"]))
        .or_else(|| settings_value(settings, "source-version"))
        .or_else(|| settings_value(settings, "xml-version"))
        .or_else(|| settings_value(settings, "xcf-version"));
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(selector) = parse_legacy_source_selector(&value) {
        return Ok(Some(selector.version_axes().xml_dialect().clone()));
    }
    XmlDialect::parse(value.trim())
        .map(Some)
        .map_err(|error| anyhow!("invalid XML dialect `{value}` in settings: {error}"))
}

fn settings_platform_build(settings: &Option<Value>) -> Result<Option<PlatformBuild>> {
    let value = settings_string_at(settings, &["ibcmd-rs", "platform-version"])
        .or_else(|| settings_string_at(settings, &["platform-version"]))
        .or_else(|| settings_value(settings, "platform-version"));
    let Some(value) = value else {
        return Ok(None);
    };
    PlatformBuild::parse(value.trim())
        .map(Some)
        .map_err(|error| anyhow!("invalid platform build `{value}` in settings: {error}"))
}

fn settings_string_at(settings: &Option<Value>, path: &[&str]) -> Option<String> {
    let mut current = settings.as_ref()?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn parse_format(value: &str) -> Option<InfobaseConfigFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "xml" | "ibcmd-xml" | "source-tree" => Some(InfobaseConfigFormat::Xml),
        _ => None,
    }
}

fn parse_legacy_source_selector(value: &str) -> Option<InfobaseConfigSourceVersion> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "2.20" | "20" | "8.3" | "8.3.27" => Some(InfobaseConfigSourceVersion::V2_20),
        "2.21" | "21" | "8.5" | "8.5.1" => Some(InfobaseConfigSourceVersion::V2_21),
        _ if normalized.starts_with("8.3.27.") => Some(InfobaseConfigSourceVersion::V2_20),
        _ if normalized.starts_with("8.5.1.") => Some(InfobaseConfigSourceVersion::V2_21),
        _ => None,
    }
}

fn first_value(cli: Option<&str>, settings: Option<String>) -> Option<String> {
    cli.filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or(settings)
}

fn resolve_password(
    cli_db_pwd: Option<&str>,
    settings: &Option<Value>,
    db_pwd_env: &str,
) -> Result<(Option<String>, Option<String>)> {
    if let Some(value) = cli_db_pwd.filter(|value| !value.is_empty()) {
        return Ok((Some(value.to_string()), Some("--db-pwd".to_string())));
    }
    if let Ok(value) = env::var(db_pwd_env) {
        return Ok((Some(value), Some(format!("env:{db_pwd_env}"))));
    }
    if let Some(value) = settings_value(settings, "dbms-pwd") {
        return Ok((Some(value), Some("settings".to_string())));
    }
    bail!("database password is required when --db-user or settings dbms-user is set")
}

fn ensure_mssql(dbms: &str) -> Result<()> {
    if dbms.eq_ignore_ascii_case("MSSQLServer") || dbms.eq_ignore_ascii_case("MSSQL") {
        return Ok(());
    }
    bail!("unsupported dbms for direct infobase config operation: {dbms}")
}

fn format_name(format: InfobaseConfigFormat) -> &'static str {
    match format {
        InfobaseConfigFormat::Xml => "xml",
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        if fs::read_dir(path)?.next().is_some() && !overwrite {
            bail!(
                "output directory is not empty: {}. Pass --overwrite or --force",
                path.display()
            );
        }
        if overwrite {
            clear_directory(path)?;
        }
    } else {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn is_internal_dump_path(relative: &Path) -> bool {
    use std::ffi::OsStr;
    use std::path::Component;

    if relative == Path::new("manifest.json") {
        return true;
    }
    let Some(Component::Normal(first)) = relative.components().next() else {
        return false;
    };
    matches!(
        first,
        name if name == OsStr::new("Config")
            || name == OsStr::new("ConfigSave")
            || name == OsStr::new("Config_inflated")
            || name == OsStr::new("ConfigSave_inflated")
            || name == OsStr::new("Config_module_text")
            || name == OsStr::new("ConfigSave_module_text")
    )
}

fn count_files(root: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn predefined_data_body_suffix_for_owner_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" | "ChartOfCharacteristicTypes" => Some("1c"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::InfobaseConfigImportArgs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn reads_format_from_top_level_settings() {
        let settings: Option<Value> = Some(serde_json::json!({
            "format": "ibcmd-xml",
            "ibcmd-rs": {
                "source-version": "8.5.1"
            },
            "vrunner": {
                "dbms-base": "servicedesk"
            }
        }));

        assert_eq!(settings_format(&settings), Some(InfobaseConfigFormat::Xml));
        assert_eq!(
            settings_xml_dialect(&settings)
                .unwrap()
                .map(|dialect| dialect.to_string()),
            Some("2.21".to_string())
        );
        assert_eq!(settings_platform_build(&settings).unwrap(), None);
        assert_eq!(
            settings_value(&settings, "dbms-base"),
            Some("servicedesk".to_string())
        );
    }

    #[test]
    fn settings_version_axes_are_separate_and_fail_closed() {
        let default = resolve_legacy_adapter(&None, None).unwrap();
        assert_eq!(default.xml_dialect().to_string(), "2.20");
        assert_eq!(default.version_axes().platform_build(), None);

        let platform_only = Some(serde_json::json!({
            "ibcmd-rs": { "platform-version": "8.5.1.1150" }
        }));
        let resolved = resolve_legacy_adapter(&platform_only, None).unwrap();
        assert_eq!(resolved.xml_dialect().to_string(), "2.20");
        assert_eq!(
            resolved
                .version_axes()
                .platform_build()
                .map(ToString::to_string)
                .as_deref(),
            Some("8.5.1.1150")
        );

        for dialect in ["2.17", "2.99"] {
            let settings = Some(serde_json::json!({
                "ibcmd-rs": { "xml-version": dialect }
            }));
            let error = resolve_legacy_adapter(&settings, None).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("legacy MSSQL adapter does not support XML dialect")
            );
        }

        let malformed_xml = Some(serde_json::json!({
            "ibcmd-rs": { "xml-version": "not-a-version" }
        }));
        assert!(
            resolve_legacy_adapter(&malformed_xml, None)
                .unwrap_err()
                .to_string()
                .contains("invalid XML dialect")
        );

        let malformed_platform = Some(serde_json::json!({
            "ibcmd-rs": { "platform-version": "8.5.invalid" }
        }));
        assert!(
            resolve_legacy_adapter(&malformed_platform, None)
                .unwrap_err()
                .to_string()
                .contains("invalid platform build")
        );
    }

    #[test]
    fn skips_only_internal_dump_roots() {
        assert!(is_internal_dump_path(Path::new("Config/versions.bin")));
        assert!(is_internal_dump_path(Path::new("Config_module_text/a.bsl")));
        assert!(is_internal_dump_path(Path::new("manifest.json")));
        assert!(!is_internal_dump_path(Path::new("Configuration.xml")));
        assert!(!is_internal_dump_path(Path::new(
            "Constants/UseFeature.xml"
        )));
        assert!(!is_internal_dump_path(Path::new("ConfigDumpInfo.xml")));
    }

    #[test]
    fn builds_import_stage_args_with_sql_auth() {
        let config = ConnectionConfig {
            dbms: "MSSQLServer".to_string(),
            db_server: "localhost".to_string(),
            db_name: "ut_ibcmd".to_string(),
            db_user: Some("sa_import".to_string()),
            db_pwd: Some("secret".to_string()),
            password_source: Some("--db-pwd".to_string()),
            format: InfobaseConfigFormat::Xml,
            legacy_adapter: MssqlLegacyAdapter::from_legacy_selector(
                InfobaseConfigSourceVersion::V2_21,
            ),
        };
        let args = InfobaseConfigImportArgs {
            settings: None,
            format: Some(InfobaseConfigFormat::Xml),
            source_version: Some(InfobaseConfigSourceVersion::V2_21),
            dbms: Some("MSSQLServer".to_string()),
            db_server: Some("localhost".to_string()),
            db_name: Some("ut_ibcmd".to_string()),
            db_user: Some("sa_import".to_string()),
            db_pwd: Some("secret".to_string()),
            db_pwd_env: "IMPORT_SQL_PWD".to_string(),
            user: None,
            password: None,
            password_env: "IBCMD_USER_PSW".to_string(),
            sqlcmd: PathBuf::from("sqlcmd"),
            replace_config_save: true,
            allow_non_lab: true,
            batch_size: Some(250),
            path_prefix: vec!["Catalogs/Валюты".to_string()],
            script_output: Some(PathBuf::from(r"C:\temp\stage.sql")),
            source_dir: PathBuf::from(r".\fixtures\source"),
        };

        let stage_args = build_import_stage_args(&config, &args).unwrap();

        assert_eq!(stage_args.server, "localhost");
        assert_eq!(stage_args.sql_user.as_deref(), Some("sa_import"));
        assert_eq!(stage_args.sql_pwd.as_deref(), Some("secret"));
        assert_eq!(stage_args.sql_pwd_env, "IMPORT_SQL_PWD");
        assert_eq!(stage_args.database, "ut_ibcmd");
        assert_eq!(stage_args.batch_size, Some(250));
        assert_eq!(
            stage_args.source_version,
            Some(InfobaseConfigSourceVersion::V2_21)
        );
        assert_eq!(stage_args.path_prefix, vec!["Catalogs/Валюты".to_string()]);
        assert_eq!(
            stage_args.script_output,
            Some(PathBuf::from(r"C:\temp\stage.sql"))
        );
    }

    #[test]
    fn default_roundtrip_target_db_keeps_source_prefix() {
        let target = default_roundtrip_target_db("ut_ibcmd");
        assert!(target.starts_with("ut_ibcmd_roundtrip_"));
    }

    #[test]
    fn samples_source_diff_entries_prioritize_different_before_one_sided() {
        let diff = crate::plan::SourceDiffReport {
            left_root: "left".to_string(),
            right_root: "right".to_string(),
            summary: crate::plan::SourceDiffSummary {
                left_only: 1,
                right_only: 1,
                different: 2,
                unchanged: 0,
            },
            differences: vec![
                crate::plan::SourceDiffEntry {
                    status: crate::plan::SourceDiffStatus::LeftOnly,
                    path: "Catalogs/LeftOnly.xml".to_string(),
                    left_sha256: Some("a".to_string()),
                    right_sha256: None,
                    left_size_bytes: Some(1),
                    right_size_bytes: None,
                    kind: Some(crate::source::SourceKind::MetadataXml),
                    object_hint: Some("Catalog.LeftOnly".to_string()),
                },
                crate::plan::SourceDiffEntry {
                    status: crate::plan::SourceDiffStatus::Different,
                    path: "Documents/DiffA.xml".to_string(),
                    left_sha256: Some("b".to_string()),
                    right_sha256: Some("c".to_string()),
                    left_size_bytes: Some(2),
                    right_size_bytes: Some(3),
                    kind: Some(crate::source::SourceKind::Form),
                    object_hint: Some("Document.DiffA".to_string()),
                },
                crate::plan::SourceDiffEntry {
                    status: crate::plan::SourceDiffStatus::RightOnly,
                    path: "Reports/RightOnly.xml".to_string(),
                    left_sha256: None,
                    right_sha256: Some("d".to_string()),
                    left_size_bytes: None,
                    right_size_bytes: Some(4),
                    kind: Some(crate::source::SourceKind::MetadataXml),
                    object_hint: Some("Report.RightOnly".to_string()),
                },
                crate::plan::SourceDiffEntry {
                    status: crate::plan::SourceDiffStatus::Different,
                    path: "Documents/DiffB.xml".to_string(),
                    left_sha256: Some("e".to_string()),
                    right_sha256: Some("f".to_string()),
                    left_size_bytes: Some(5),
                    right_size_bytes: Some(6),
                    kind: Some(crate::source::SourceKind::Template),
                    object_hint: Some("Document.DiffB".to_string()),
                },
            ],
        };

        let sample = sample_source_diff_entries(&diff, 3);
        assert_eq!(sample.len(), 3);
        assert_eq!(sample[0].status, crate::plan::SourceDiffStatus::Different);
        assert_eq!(sample[0].path, "Documents/DiffA.xml");
        assert_eq!(sample[1].status, crate::plan::SourceDiffStatus::Different);
        assert_eq!(sample[1].path, "Documents/DiffB.xml");
        assert_eq!(sample[2].status, crate::plan::SourceDiffStatus::LeftOnly);
        assert_eq!(sample[2].path, "Catalogs/LeftOnly.xml");
    }

    #[test]
    fn strips_extended_windows_prefix_for_external_tools() {
        assert_eq!(
            tool_path_arg(Path::new(r"\\?\E:\ibcmd_lab\roundtrip\ibcmd-data")),
            r"E:\ibcmd_lab\roundtrip\ibcmd-data"
        );
    }

    #[test]
    fn builds_selected_dump_file_names_from_source_prefix() {
        let root = temp_root("ibcmd-rs-selected-dump-names");
        fs::create_dir_all(root.join("Catalogs/Products/Ext")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Ext/Help")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Forms/ItemForm/Ext/Form")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Commands/Open/Ext")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Templates/Print/Ext")).unwrap();
        fs::create_dir_all(root.join("CommonModules/Utils/Ext")).unwrap();

        fs::write(
            root.join("Catalogs/Products.xml"),
            br#"<MetaDataObject><Catalog uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"><Properties><Name>Products</Name></Properties><ChildObjects><Command uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc"><Properties><Name>Open</Name></Properties></Command></ChildObjects></Catalog></MetaDataObject>"#,
        )
        .unwrap();
        fs::write(root.join("Catalogs/Products/Ext/ObjectModule.bsl"), b"").unwrap();
        fs::write(root.join("Catalogs/Products/Ext/Help.xml"), b"<Help/>").unwrap();
        fs::write(root.join("Catalogs/Products/Ext/Help/ru.html"), b"help").unwrap();
        fs::write(
            root.join("Catalogs/Products/Ext/Predefined.xml"),
            b"<PredefinedData/>",
        )
        .unwrap();

        fs::write(
            root.join("Catalogs/Products/Forms/ItemForm.xml"),
            br#"<MetaDataObject><Form uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"><Properties><Name>ItemForm</Name></Properties></Form></MetaDataObject>"#,
        )
        .unwrap();
        fs::write(
            root.join("Catalogs/Products/Forms/ItemForm/Ext/Form.xml"),
            br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20"/>"#,
        )
        .unwrap();
        fs::write(
            root.join("Catalogs/Products/Forms/ItemForm/Ext/Form/Module.bsl"),
            b"",
        )
        .unwrap();

        fs::write(
            root.join("Catalogs/Products/Commands/Open/Ext/CommandModule.bsl"),
            b"",
        )
        .unwrap();

        fs::write(
            root.join("Catalogs/Products/Templates/Print.xml"),
            br#"<MetaDataObject><Template uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd"><Properties><Name>Print</Name></Properties></Template></MetaDataObject>"#,
        )
        .unwrap();
        fs::write(
            root.join("Catalogs/Products/Templates/Print/Ext/Template.xml"),
            b"<Template/>",
        )
        .unwrap();

        fs::write(
            root.join("CommonModules/Utils.xml"),
            br#"<MetaDataObject><CommonModule uuid="eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"><Properties><Name>Utils</Name><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties></CommonModule></MetaDataObject>"#,
        )
        .unwrap();
        fs::write(root.join("CommonModules/Utils/Ext/Module.bsl"), b"").unwrap();

        let selected =
            build_selected_dump_file_names_from_source(&root, &["Catalogs/Products".to_string()])
                .unwrap();

        assert!(selected.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()));
        assert!(selected.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.1c".to_string()));
        assert!(selected.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0".to_string()));
        assert!(selected.contains(&"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.5".to_string()));
        assert!(selected.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string()));
        assert!(selected.contains(&"bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0".to_string()));
        assert!(selected.contains(&"cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string()));
        assert!(selected.contains(&"cccccccc-cccc-4ccc-cccc-cccccccccccc.2".to_string()));
        assert!(selected.contains(&"dddddddd-dddd-4ddd-dddd-dddddddddddd".to_string()));
        assert!(selected.contains(&"dddddddd-dddd-4ddd-dddd-dddddddddddd.0".to_string()));

        let common_module =
            build_selected_dump_file_names_from_source(&root, &["CommonModules/Utils".to_string()])
                .unwrap();
        assert!(common_module.contains(&"eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee".to_string()));
        assert!(common_module.contains(&"eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee.0".to_string()));
    }

    #[test]
    fn selects_representative_sweep_prefixes_by_family() {
        let root = temp_root("ibcmd-rs-sweep-prefixes");
        fs::create_dir_all(root.join("Catalogs")).unwrap();
        fs::create_dir_all(root.join("Documents")).unwrap();
        fs::create_dir_all(root.join("CommonModules")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Rich/Forms/Main/Ext")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Ext")).unwrap();

        fs::write(root.join("Configuration.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Catalogs/Alpha.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Catalogs/Rich.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(
            root.join("Catalogs/Rich/Forms/Main.xml"),
            b"<MetaDataObject/>",
        )
        .unwrap();
        fs::write(
            root.join("Catalogs/Rich/Forms/Main/Ext/Form.xml"),
            b"<Form/>",
        )
        .unwrap();
        fs::write(root.join("Catalogs/Zeta.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Documents/Sales.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("CommonModules/Utils.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Catalogs/Products/Ext/ignored.txt"), b"").unwrap();

        let selected = representative_sweep_prefixes(
            &root,
            &["Catalogs".to_string(), "Documents".to_string()],
            None,
            0,
            1,
        )
        .unwrap();
        assert_eq!(
            selected,
            vec!["Catalogs/Rich".to_string(), "Documents/Sales".to_string()]
        );

        let limited = representative_sweep_prefixes(
            &root,
            &[
                "Catalogs".to_string(),
                "Documents".to_string(),
                "CommonModules".to_string(),
            ],
            Some(2),
            0,
            1,
        )
        .unwrap();
        assert_eq!(
            limited,
            vec!["Catalogs/Rich".to_string(), "Documents/Sales".to_string()]
        );
    }

    #[test]
    fn selects_deeper_representative_sweep_candidates_with_offset() {
        let root = temp_root("ibcmd-rs-sweep-prefixes-offset");
        fs::create_dir_all(root.join("Catalogs/Rich/Forms/Main/Ext")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Products/Ext")).unwrap();
        fs::create_dir_all(root.join("Catalogs/Zeta")).unwrap();

        fs::write(root.join("Configuration.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Catalogs/Rich.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(
            root.join("Catalogs/Rich/Forms/Main.xml"),
            b"<MetaDataObject/>",
        )
        .unwrap();
        fs::write(
            root.join("Catalogs/Rich/Forms/Main/Ext/Form.xml"),
            b"<Form/>",
        )
        .unwrap();
        fs::write(root.join("Catalogs/Products.xml"), b"<MetaDataObject/>").unwrap();
        fs::write(root.join("Catalogs/Products/Ext/ObjectModule.bsl"), b"").unwrap();
        fs::write(root.join("Catalogs/Zeta.xml"), b"<MetaDataObject/>").unwrap();

        let selected =
            representative_sweep_prefixes(&root, &["Catalogs".to_string()], None, 1, 2).unwrap();
        assert_eq!(
            selected,
            vec!["Catalogs/Products".to_string(), "Catalogs/Zeta".to_string()]
        );
    }
}
