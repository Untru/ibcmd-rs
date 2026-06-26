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
        Commands::MssqlStageSourceMetadataObjects(args) => {
            let report = ibcmd_rs::mssql::stage_source_metadata_objects(&args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::MssqlStageSourceCommonModuleObjects(args) => {
            let report = ibcmd_rs::mssql::stage_source_common_module_objects(&args)?;
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
