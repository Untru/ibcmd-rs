use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use ibcmd_rs::cli::{Cli, Commands};
use ibcmd_rs::commands::platform::PlatformFlag;
use ibcmd_rs::plan::SourceDiffSignatureOptions;

/// mimalloc instead of the Windows process heap: the export's workers allocate
/// millions of short-lived strings and vectors, and on the process heap they
/// spent their time waiting on each other for it.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// The commands walk deeply nested sources (form item trees, brace bodies) on
/// the calling thread, whose default stack on Windows is 1 MiB; they run on a
/// thread with room for the deepest of them instead.
fn main() -> Result<()> {
    const STACK_BYTES: usize = 256 * 1024 * 1024;
    std::thread::Builder::new()
        .name("ibcmd-rs".to_string())
        .stack_size(STACK_BYTES)
        .spawn(run)
        .map_err(|error| anyhow::anyhow!("failed to start the command thread: {error}"))?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn run() -> Result<()> {
    // Parsed through the matches, which the settings of the database commands
    // read to tell a given flag from its default.
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let subcommand = matches
        .subcommand()
        .map(|(_, matches)| matches.clone())
        .unwrap_or_default();
    let subcommand: &ArgMatches = &subcommand;

    match cli.command {
        Commands::Convert(args) => {
            let result = ibcmd_rs::conversion::convert(&args);
            let report = match &result {
                Ok(report) => report,
                Err(error) => error.report(),
            };
            let rendered = serde_json::to_string_pretty(report)?;
            let report_path_conflict = report
                .errors
                .iter()
                .any(|error| error.code == "conversion.report-path-conflict");
            if let Some(path) = &args.report
                && !report_path_conflict
            {
                ibcmd_rs::conversion::write_report(report, path)?;
            }
            match result {
                Ok(_) => println!("{rendered}"),
                Err(_) => {
                    eprintln!("{rendered}");
                    std::process::exit(2);
                }
            }
        }
        Commands::Cf(args) => match ibcmd_rs::commands::cf::run(args) {
            Ok(report) => println!("{}", serde_json::to_string_pretty(&report)?),
            Err(error) => {
                let rendered = serde_json::to_string_pretty(error.report())
                    .unwrap_or_else(|_| "{\"schema_version\":1,\"command\":\"cf\",\"ok\":false,\"errors\":[{\"code\":\"report_serialization_failed\",\"message\":\"failed to serialize CF error report\"}]}".to_owned());
                eprintln!("{rendered}");
                std::process::exit(2);
            }
        },
        // The platform ibcmd's modes: its syntax, its messages, its exit codes.
        Commands::Infobase(args) => std::process::exit(ibcmd_rs::dropin::run_infobase(&args.args)),
        Commands::Help(args) => std::process::exit(ibcmd_rs::dropin::run_help(&args.args)),
        Commands::Server(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("server")),
        Commands::Serve(_) => std::process::exit(ibcmd_rs::server::run_stdio()),
        Commands::Objects(args) => ibcmd_rs::commands::objects::run(args)?,
        Commands::Eventlog(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("eventlog")),
        Commands::Config(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("config")),
        Commands::Extension(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("extension")),
        Commands::MobileApp(_) => {
            std::process::exit(ibcmd_rs::dropin::run_other_mode("mobile-app"))
        }
        Commands::MobileClient(_) => {
            std::process::exit(ibcmd_rs::dropin::run_other_mode("mobile-client"))
        }
        Commands::Session(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("session")),
        Commands::Lock(_) => std::process::exit(ibcmd_rs::dropin::run_other_mode("lock")),
        Commands::BinaryDataStorage(_) => {
            std::process::exit(ibcmd_rs::dropin::run_other_mode("binary-data-storage"))
        }
        #[cfg(feature = "platform-oracle")]
        Commands::Probe(args) => {
            let report = ibcmd_rs::probe::probe_environment(args);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::Scan(args) => {
            let manifest = ibcmd_rs::source::scan_sources(&args.root)?;
            if let Some(output) = args.output {
                ibcmd_rs::source::write_manifest(&manifest, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            }
        }
        Commands::AuditSpreadsheetTemplates(args) => {
            let report = ibcmd_rs::source_audit::audit_spreadsheet_templates(&args.root)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditSpreadsheetRoundtrip(args) => {
            let report = ibcmd_rs::source_audit::audit_spreadsheet_template_roundtrip(&args.root)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditFormSources(args) => {
            let report = ibcmd_rs::source_audit::audit_form_sources(&args.root)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::FormDiffCandidates(args) => {
            let report = ibcmd_rs::form_matrix::analyze_form_diff_candidates(&args)?;
            if let Some(output) = &args.output {
                ibcmd_rs::form_matrix::write_form_diff_candidate_report(&report, output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::FormProvenanceCorpus(args) => {
            let report = ibcmd_rs::form_provenance::run_form_provenance_corpus(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MxlLineProvenanceCorpus(args) => {
            let report = ibcmd_rs::mxl_line_provenance::run_mxl_line_provenance_corpus(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::FormContextSummary(args) => {
            let context =
                ibcmd_rs::mssql_dump::offline_context::OfflineFormContextFactory::from_run_root(
                    &args.run_root,
                    args.source_commit,
                )?;
            println!("{}", serde_json::to_string_pretty(&context.summary)?);
        }
        Commands::AuditSourceLoadCoverage(args) => {
            let report = ibcmd_rs::source_audit::audit_source_load_coverage(&args.root)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditNativeFormWriter(args) => {
            let report =
                ibcmd_rs::source_audit::audit_native_form_writer(&args.root, &args.bodies)?;
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditRoleRightsWriter(args) => {
            let report =
                ibcmd_rs::source_audit::audit_role_rights_writer(&args.root, &args.inflated)?;
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
                println!(
                    "roles {} (no Rights.xml {}), compiled {}, refused {}, round trip {}/{}, loader accepts {}, stored {}, plain identical {} ({} without dangling refs; {} rows hold dangling refs), entries {}, order {}, tail {}",
                    report.roles,
                    report.without_rights_xml,
                    report.compiled,
                    report.refused.values().sum::<usize>(),
                    report.round_trip_identical,
                    report.compiled,
                    report.loader_accepted,
                    report.stored_rows,
                    report.plain_identical,
                    report.plain_identical_without_dangling,
                    report.stored_with_dangling,
                    report.entries_identical,
                    report.order_identical,
                    report.tail_identical,
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditDcsTemplateWriter(args) => {
            let report = ibcmd_rs::dcs_template_audit::audit_dcs_template_writer(
                &args.root,
                &args.bodies,
                &args.dump,
            )?;
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditHelpWriter(mut args) => {
            args.apply_platform_flag();
            let report = ibcmd_rs::source_audit::audit_help_writer(
                &args.root,
                &args.inflated,
                args.source_version,
            )?;
            for (family, counts) in &report.families {
                println!(
                    "{family}: files {} compiled {} plain {} round-trip {} refused {} (stored round-trip {}, no stored row {})",
                    counts.files,
                    counts.compiled,
                    counts.plain_identical,
                    counts.round_trip_identical,
                    counts.refused.values().sum::<usize>(),
                    counts.stored_round_trip_identical,
                    counts.no_stored_row,
                );
                for (reason, count) in &counts.refused {
                    println!("    refused {count}: {reason}");
                }
            }
            let mut shown = BTreeMap::<(&str, &str), usize>::new();
            for difference in &report.differences {
                let seen = shown
                    .entry((difference.family.as_str(), difference.check))
                    .or_insert(0);
                *seen += 1;
                if *seen <= 3 {
                    println!(
                        "  {} {} {} [{}]: {}",
                        difference.family,
                        difference.check,
                        difference.file,
                        difference.row,
                        difference.detail.chars().take(600).collect::<String>()
                    );
                }
            }
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditInterfaceWriter(mut args) => {
            args.apply_platform_flag();
            let report = ibcmd_rs::source_audit::audit_interface_writer(
                &args.root,
                &args.inflated,
                args.source_version,
            )?;
            for (family, counts) in &report.families {
                println!(
                    "{family}: files {} compiled {} round-trip {} plain {} refused {} (stored round-trip {}, no stored row {})",
                    counts.files,
                    counts.compiled,
                    counts.round_trip_identical,
                    counts.plain_identical,
                    counts.refused.values().sum::<usize>(),
                    counts.stored_round_trip_identical,
                    counts.no_stored_row,
                );
                for (reason, count) in &counts.refused {
                    println!("    refused {count}: {reason}");
                }
            }
            let mut shown = BTreeMap::<(&str, &str), usize>::new();
            for difference in &report.differences {
                let seen = shown
                    .entry((difference.family.as_str(), difference.check))
                    .or_insert(0);
                *seen += 1;
                if *seen <= 3 {
                    println!(
                        "  {} {} {} [{}]: {}",
                        difference.check,
                        difference.family,
                        difference.file,
                        difference.row,
                        difference.detail
                    );
                }
            }
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditEmptyStage(mut args) => {
            args.apply_platform_flag();
            let options = ibcmd_rs::mssql::EmptyStageAuditOptions {
                max_samples: args.max_samples,
                diff_dir: args.diff_dir,
                manifest: args.manifest,
                rows_out: args.rows_out,
            };
            let report = ibcmd_rs::mssql::audit_empty_stage(
                &args.root,
                &args.rows,
                args.source_version.as_deref(),
                &options,
            )?;
            println!("{}", ibcmd_rs::mssql::empty_stage_summary(&report));
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditMetadataExport(mut args) => {
            args.apply_platform_flag();
            use ibcmd_rs::metadata_model::export::{audit, tree_version};
            let options = audit::ExportAuditOptions {
                kinds: args.kinds,
                max_samples: args.max_samples,
                diff_dir: args.diff_dir,
                timing_threads: args.timing_threads,
            };
            let version = args
                .source_version
                .or_else(|| tree_version(&args.root))
                .unwrap_or_else(|| "2.20".to_string());
            let report = audit::audit_export(&args.root, &args.rows, &version, &options)?;
            println!("{}", audit::summary_table(&report));
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditNameIndex(mut args) => {
            args.apply_platform_flag();
            let report = ibcmd_rs::mssql_dump::model_export::audit_name_index(
                &args.root,
                &args.rows,
                args.source_version,
                args.max_samples,
            )?;
            let comparison = &report.comparison;
            for (label, part) in [
                ("names", &comparison.names),
                ("types", &comparison.types),
                ("predefined", &comparison.predefined),
            ] {
                println!(
                    "{label:<11} expected {:>7} built {:>7} equal {:>7} missing {:>6} extra {:>6} different {:>6}",
                    part.expected,
                    part.actual,
                    part.equal,
                    part.missing,
                    part.extra,
                    part.different
                );
                for (kind, counts) in &part.by_kind {
                    println!(
                        "    {kind:<36} missing {:>6} extra {:>6} different {:>6}",
                        counts.missing, counts.extra, counts.different
                    );
                }
            }
            println!(
                "equal: {}; compatibility: rows {:?}, tree {:?}; built from rows in {} ms, from the tree in {} ms",
                report.equal,
                report.rows_compatibility_mode,
                report.tree_compatibility_mode,
                report.build_rows_ms,
                report.build_tree_ms
            );
            println!(
                "of which: rows {} ms, legacy indexes {} ms, model index {} ms",
                report.fetch_ms, report.legacy_ms, report.model_ms
            );
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditMetadataCompiler(mut args) => {
            args.apply_platform_flag();
            use ibcmd_rs::metadata_model::audit;
            let options = audit::DescriptorAuditOptions {
                kinds: args.kinds,
                max_samples: args.max_samples,
                diff_dir: args.diff_dir,
            };
            let report = audit::audit_descriptor_compiler(
                &args.root,
                &args.rows,
                &args.source_version,
                &options,
            )?;
            println!("{}", audit::summary_table(&report));
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            }
        }
        Commands::AuditMxlWriter(args) => {
            let report = ibcmd_rs::source_audit::audit_mxl_writer(&args.root, &args.bodies)?;
            if let Some(output) = args.output {
                std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::AuditFormBodyBlockers(args) => {
            let report = ibcmd_rs::source_audit::audit_form_body_blockers(&args.root)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::Plan(args) => {
            let current = ibcmd_rs::source::read_manifest(&args.current)?;
            let baseline = match args.baseline {
                Some(path) => Some(ibcmd_rs::source::read_manifest(&path)?),
                None => None,
            };
            let plan = ibcmd_rs::plan::build_load_plan(baseline.as_ref(), &current);
            if let Some(output) = args.output {
                ibcmd_rs::plan::write_plan(&plan, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        Commands::SourceDiff(args) => {
            let report =
                ibcmd_rs::plan::diff_source_trees(&args.left, &args.right, &args.path_prefix)?;
            if let Some(output) = args.output {
                ibcmd_rs::plan::write_source_diff(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::SourceDiffExplain(args) => {
            let (left_root, right_root) = if let Some(diff_path) = &args.diff {
                let diff = ibcmd_rs::plan::read_source_diff(diff_path)?;
                (
                    std::path::PathBuf::from(diff.left_root),
                    std::path::PathBuf::from(diff.right_root),
                )
            } else {
                let left_root = args
                    .left_root
                    .clone()
                    .ok_or_else(|| anyhow!("--left-root is required when --diff is omitted"))?;
                let right_root = args
                    .right_root
                    .clone()
                    .ok_or_else(|| anyhow!("--right-root is required when --diff is omitted"))?;
                (left_root, right_root)
            };
            let limit = if args.limit == 0 {
                None
            } else {
                Some(args.limit)
            };
            let report = ibcmd_rs::plan::explain_source_diff_file(
                &left_root,
                &right_root,
                &args.path,
                &args.leaf_path_prefix,
                limit,
            )?;
            if let Some(output) = args.output {
                ibcmd_rs::plan::write_source_diff_explain_report(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::SourceDiffSignatures(args) => {
            let diff = ibcmd_rs::plan::read_source_diff(&args.diff)?;
            let options = SourceDiffSignatureOptions {
                max_files_per_kind: args.max_files_per_kind,
                kind_limits: parse_kind_limits(&args.kind_limit)?,
                top: Some(args.top),
                examples_per_signature: args.examples_per_signature,
            };
            let report = ibcmd_rs::plan::build_source_diff_signature_report(&diff, &options);
            if let Some(output) = args.output {
                ibcmd_rs::plan::write_source_diff_signature_report(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::SourceDiffMatrix(args) => {
            if !args.full && !args.scoped {
                return Err(anyhow!(
                    "pass exactly one of --full or --scoped for source-diff-matrix"
                ));
            }
            let diff = ibcmd_rs::plan::read_source_diff(&args.diff)?;
            let matrix = ibcmd_rs::plan::build_parity_matrix(
                &diff,
                args.database,
                args.run_id,
                args.git_sha,
                args.full,
            )?;
            ibcmd_rs::plan::write_parity_artifacts(
                &matrix,
                &args.output,
                args.markdown.as_deref(),
                args.overwrite,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": matrix.schema_version,
                    "runs": &matrix.runs,
                    "rows": matrix.rows.len(),
                    "json": &args.output,
                    "markdown": &args.markdown,
                }))?
            );
        }
        Commands::SourceDiffMatrixMerge(args) => {
            let matrices = args
                .matrices
                .iter()
                .map(|path| ibcmd_rs::plan::read_parity_matrix(path))
                .collect::<Result<Vec<_>>>()?;
            let matrix = ibcmd_rs::plan::merge_parity_matrices(&matrices)?;
            ibcmd_rs::plan::write_parity_artifacts(
                &matrix,
                &args.output,
                args.markdown.as_deref(),
                args.overwrite,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": matrix.schema_version,
                    "runs": &matrix.runs,
                    "rows": matrix.rows.len(),
                    "json": &args.output,
                    "markdown": &args.markdown,
                }))?
            );
        }
        Commands::SourceThreeWayOracle(args) => {
            let report = ibcmd_rs::source_oracle::run_source_three_way_oracle(&args)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema_version": report.schema_version,
                    "rows": report.rows.len(),
                    "summary": &report.summary,
                    "json": &args.output,
                    "markdown": &args.markdown,
                }))?
            );
        }
        Commands::Settings(args) => match args.command {
            ibcmd_rs::cli::SettingsCommands::Show(args) => {
                let settings = ibcmd_rs::settings::Settings::load(args.config.as_deref())?;
                let report = ibcmd_rs::settings::show::settings_report(
                    &settings,
                    args.platform.as_deref(),
                    args.db_server.as_deref(),
                    args.db_name.as_deref(),
                )?;
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print!("{}", ibcmd_rs::settings::show::render(&report));
                }
            }
        },
        Commands::Compatibility(args) => {
            let report = ibcmd_rs::compatibility::current_compatibility_report()?;
            if let Some(output) = args.output {
                ibcmd_rs::compatibility::write_compatibility_report(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        #[cfg(feature = "platform-oracle")]
        Commands::ProfileRun(args) => {
            let report = ibcmd_rs::profile::run_profiled(args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        #[cfg(feature = "platform-oracle")]
        Commands::DumpSources(args) => {
            let report = ibcmd_rs::dump_sources::dump_sources(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlDumpConfig(mut args) => {
            ibcmd_rs::settings::commands::prepare_dump_config(&mut args, subcommand)?;
            let report = ibcmd_rs::mssql_dump::dump_config(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlSaveConfig(args) => {
            let report = ibcmd_rs::mssql_dump::config_save::save_config_command(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlLoadConfig(args) => {
            let report = ibcmd_rs::mssql::cf_load_stage::load_config_command(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlExtensionList(args) => {
            let report = ibcmd_rs::mssql_extensions::list_extensions(&args)?;
            match args.format {
                ibcmd_rs::cli::MssqlExtensionListFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                ibcmd_rs::cli::MssqlExtensionListFormat::Table => {
                    print!(
                        "{}",
                        ibcmd_rs::mssql_extensions::render_extension_table(&report)
                    );
                }
            }
        }
        Commands::MssqlDumpExtension(mut args) => {
            ibcmd_rs::settings::commands::prepare_dump_extension(&mut args, subcommand)?;
            let report = ibcmd_rs::mssql_extension_export::dump_extensions(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlLoadExtension(mut args) => {
            ibcmd_rs::settings::commands::prepare_load_extension(&mut args, subcommand)?;
            let report = ibcmd_rs::mssql_extension_load::load_extensions(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlActivateStagedExtension(args) => {
            let report = ibcmd_rs::mssql_extension_load::activate_staged_extension(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlDumpTimingSummary(args) => {
            let summaries = ibcmd_rs::mssql_dump::read_dump_timing_summaries(&args.input)?;
            let json = serde_json::to_string_pretty(&summaries)?;
            if let Some(output) = args.output {
                std::fs::write(output, json)?;
            } else {
                println!("{json}");
            }
        }
        Commands::TraceTemplate(args) => {
            ibcmd_rs::templates::write_trace_templates(&args.output_dir, args.overwrite)?;
            println!("Trace templates written to {}", args.output_dir.display());
        }
        Commands::TraceAnalyze(args) => {
            let analysis = ibcmd_rs::trace::analyze_trace_files(&args.input)?;
            if let Some(output) = args.output {
                ibcmd_rs::trace::write_trace_analysis(&analysis, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            }
        }
        Commands::StorageMap(args) => {
            let analysis = ibcmd_rs::trace::analyze_trace_files(&args.input)?;
            let report = ibcmd_rs::storage_map::build_storage_mapping(&analysis);
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::MssqlCompare(args) => {
            let report = ibcmd_rs::mssql::compare_databases(&args)?;
            if let Some(output) = args.output {
                ibcmd_rs::mssql::write_compare_report(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::MssqlActivationSnapshot(args) => {
            let report = ibcmd_rs::mssql::capture_activation_snapshot(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlActivationDiff(args) => {
            let report = ibcmd_rs::mssql::diff_activation_snapshots(&args)?;
            if let Some(output) = args.output {
                ibcmd_rs::mssql::write_activation_diff(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::MssqlActivateStagedMain(args) => {
            let report = ibcmd_rs::mssql::activate_staged_main(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlApplyCheck(args) => {
            std::process::exit(ibcmd_rs::apply_check::cli::run_mssql_apply_check(&args)?)
        }
        Commands::ApplyCheckTrees(args) => {
            std::process::exit(ibcmd_rs::apply_check::cli::run_apply_check_trees(&args)?)
        }
        Commands::MssqlConfigApply(args) => {
            ibcmd_rs::mssql_config_apply::run_command(&args)?;
        }
        Commands::MssqlApplySourceChange(mut args) => {
            ibcmd_rs::settings::commands::prepare_apply_source_change(&mut args, subcommand)?;
            if args.watch {
                ibcmd_rs::mssql_apply::watch_source_changes(&args)?;
            } else {
                let report = ibcmd_rs::mssql_apply::apply_source_change(&args)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::MssqlRestructure(args) => {
            let report = ibcmd_rs::restructure::command::run(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlAuditSourceParity(mut args) => {
            ibcmd_rs::settings::commands::prepare_audit_source_parity(&mut args, subcommand)?;
            let report = ibcmd_rs::mssql::audit_source_parity(&args)?;
            if let Some(output) = args.output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(&output, json)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::MssqlClone(args) => {
            let report = ibcmd_rs::mssql::clone_database(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStorageExport(args) => {
            let report = ibcmd_rs::mssql::export_storage_bundle(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStorageImport(args) => {
            let report = ibcmd_rs::mssql::import_storage_bundle(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlDeltaExport(args) => {
            let report = ibcmd_rs::mssql::export_delta_bundle(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlDeltaImport(args) => {
            let report = ibcmd_rs::mssql::import_delta_bundle(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::ModuleBlobPack(args) => {
            let report = ibcmd_rs::module_blob::pack_module_blob(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::VersionsBlobPatch(args) => {
            let report = ibcmd_rs::module_blob::patch_versions_blob(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonModule(args) => {
            let report = ibcmd_rs::mssql::stage_common_module(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonModules(args) => {
            let report = ibcmd_rs::mssql::stage_common_modules(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonModuleMetadata(args) => {
            let report = ibcmd_rs::mssql::stage_common_module_metadata(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonModuleObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_module_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonModuleObjects(args) => {
            let report = ibcmd_rs::mssql::stage_common_module_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageMetadataObjects(args) => {
            let report = ibcmd_rs::mssql::stage_metadata_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSourceMetadataObjects(args) => {
            let report = ibcmd_rs::mssql::stage_source_metadata_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSourceCommonModuleObjects(args) => {
            let report = ibcmd_rs::mssql::stage_source_common_module_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSourceObjects(mut args) => {
            ibcmd_rs::settings::commands::prepare_stage_source_objects(&mut args, subcommand)?;
            let report = ibcmd_rs::mssql::stage_source_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageExchangePlanObject(args) => {
            let report = ibcmd_rs::mssql::stage_exchange_plan_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageBusinessProcessObject(args) => {
            let report = ibcmd_rs::mssql::stage_business_process_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageDocumentJournalObject(args) => {
            let report = ibcmd_rs::mssql::stage_document_journal_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageReportObject(args) => {
            let report = ibcmd_rs::mssql::stage_report_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageDataProcessorObject(args) => {
            let report = ibcmd_rs::mssql::stage_data_processor_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCatalogObject(args) => {
            let report = ibcmd_rs::mssql::stage_catalog_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageInformationRegisterObject(args) => {
            let report = ibcmd_rs::mssql::stage_information_register_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageScheduledJobObject(args) => {
            let report = ibcmd_rs::mssql::stage_scheduled_job_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageXdtopackageObject(args) => {
            let report = ibcmd_rs::mssql::stage_xdtopackage_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageRoleObject(args) => {
            let report = ibcmd_rs::mssql::stage_role_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageConstantObject(args) => {
            let report = ibcmd_rs::mssql::stage_constant_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageDefinedTypeObject(args) => {
            let report = ibcmd_rs::mssql::stage_defined_type_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSessionParameterObject(args) => {
            let report = ibcmd_rs::mssql::stage_session_parameter_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSettingsStorageObject(args) => {
            let report = ibcmd_rs::mssql::stage_settings_storage_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageFunctionalOptionObject(args) => {
            let report = ibcmd_rs::mssql::stage_functional_option_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageFunctionalOptionsParameterObject(args) => {
            let report = ibcmd_rs::mssql::stage_functional_options_parameter_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageEventSubscriptionObject(args) => {
            let report = ibcmd_rs::mssql::stage_event_subscription_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageHTTPServiceObject(args) => {
            let report = ibcmd_rs::mssql::stage_http_service_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageWebServiceObject(args) => {
            let report = ibcmd_rs::mssql::stage_web_service_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonAttributeObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_attribute_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageLanguageObject(args) => {
            let report = ibcmd_rs::mssql::stage_language_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageStyleItemObject(args) => {
            let report = ibcmd_rs::mssql::stage_style_item_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageStyleObject(args) => {
            let report = ibcmd_rs::mssql::stage_style_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageBotObject(args) => {
            let report = ibcmd_rs::mssql::stage_bot_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageDocumentNumeratorObject(args) => {
            let report = ibcmd_rs::mssql::stage_document_numerator_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageIntegrationServiceObject(args) => {
            let report = ibcmd_rs::mssql::stage_integration_service_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSequenceObject(args) => {
            let report = ibcmd_rs::mssql::stage_sequence_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageWSReferenceObject(args) => {
            let report = ibcmd_rs::mssql::stage_ws_reference_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageTaskObject(args) => {
            let report = ibcmd_rs::mssql::stage_task_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSubsystemObject(args) => {
            let report = ibcmd_rs::mssql::stage_subsystem_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommandGroupObject(args) => {
            let report = ibcmd_rs::mssql::stage_command_group_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageEnumObject(args) => {
            let report = ibcmd_rs::mssql::stage_enum_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageDocumentObject(args) => {
            let report = ibcmd_rs::mssql::stage_document_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageFilterCriteriaObject(args) => {
            let report = ibcmd_rs::mssql::stage_filter_criteria_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageAccountingRegisterObject(args) => {
            let report = ibcmd_rs::mssql::stage_accounting_register_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageAccumulationRegisterObject(args) => {
            let report = ibcmd_rs::mssql::stage_accumulation_register_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCalculationRegisterObject(args) => {
            let report = ibcmd_rs::mssql::stage_calculation_register_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageChartOfCharacteristicTypesObject(args) => {
            let report = ibcmd_rs::mssql::stage_chart_of_characteristic_types_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageChartOfAccountsObject(args) => {
            let report = ibcmd_rs::mssql::stage_chart_of_accounts_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageChartOfCalculationTypesObject(args) => {
            let report = ibcmd_rs::mssql::stage_chart_of_calculation_types_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageChartOfCalculationRegistersObject(args) => {
            let report = ibcmd_rs::mssql::stage_chart_of_calculation_registers_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonCommandObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_command_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonFormObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_form_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonPictureObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_picture_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageCommonTemplateObject(args) => {
            let report = ibcmd_rs::mssql::stage_common_template_object(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn parse_kind_limits(values: &[String]) -> Result<BTreeMap<String, usize>> {
    let mut limits = BTreeMap::new();
    for value in values {
        let Some((kind, limit)) = value.split_once('=') else {
            return Err(anyhow!(
                "invalid --kind-limit {value:?}; expected KIND=COUNT"
            ));
        };
        let kind = kind.trim();
        if kind.is_empty() {
            return Err(anyhow!(
                "invalid --kind-limit {value:?}; kind must not be empty"
            ));
        }
        let limit = limit
            .trim()
            .parse::<usize>()
            .map_err(|error| anyhow!("invalid --kind-limit {value:?}: {error}"))?;
        limits.insert(kind.to_string(), limit);
    }
    Ok(limits)
}
