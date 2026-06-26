use anyhow::Result;
use clap::Parser;
use ibcmd_rs::cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
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
        Commands::Compatibility(args) => {
            let report = ibcmd_rs::compatibility::current_compatibility_report();
            if let Some(output) = args.output {
                ibcmd_rs::compatibility::write_compatibility_report(&report, &output)?;
            } else {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        Commands::ProfileRun(args) => {
            let report = ibcmd_rs::profile::run_profiled(args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
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
    }

    Ok(())
}
