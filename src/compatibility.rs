use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CompatibilityReport {
    pub supported_operations: Vec<OperationSupport>,
    pub supported_source_families: Vec<&'static str>,
    pub supported_metadata_families: Vec<&'static str>,
    pub supported_write_patterns: Vec<&'static str>,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct OperationSupport {
    pub name: &'static str,
    pub status: SupportStatus,
    pub detail: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    Implemented,
    Partial,
    Planned,
}

pub fn current_compatibility_report() -> CompatibilityReport {
    CompatibilityReport {
        supported_operations: vec![
            OperationSupport {
                name: "scan",
                status: SupportStatus::Implemented,
                detail: "Deterministic source manifest scan with nested object subtree support.",
            },
            OperationSupport {
                name: "plan",
                status: SupportStatus::Implemented,
                detail: "Manifest diff to load-plan JSON.",
            },
            OperationSupport {
                name: "probe",
                status: SupportStatus::Implemented,
                detail: "Locates local 1C toolchain and emits environment details.",
            },
            OperationSupport {
                name: "profile-run",
                status: SupportStatus::Implemented,
                detail: "Runs external commands and captures timing/output.",
            },
            OperationSupport {
                name: "trace-template",
                status: SupportStatus::Implemented,
                detail: "Generates SQL Server and tech-log templates.",
            },
            OperationSupport {
                name: "trace-analyze",
                status: SupportStatus::Implemented,
                detail: "Groups Extended Events SQL by normalized text with duration and boundary enrichment.",
            },
            OperationSupport {
                name: "storage-map",
                status: SupportStatus::Implemented,
                detail: "Classifies trace groups into storage mutation patterns, stage roles and operation families for ConfigSave and Params.",
            },
            OperationSupport {
                name: "mssql-compare",
                status: SupportStatus::Implemented,
                detail: "Compares table shape, row counts and row checksums for SQL Server infobases.",
            },
            OperationSupport {
                name: "mssql-clone",
                status: SupportStatus::Implemented,
                detail: "Backup/restore clone flow for disposable SQL Server databases.",
            },
            OperationSupport {
                name: "mssql-storage-export/import",
                status: SupportStatus::Implemented,
                detail: "Exports and imports ConfigSave/Params storage bundles via BCP with checksum-backed manifests.",
            },
            OperationSupport {
                name: "mssql-delta-export/import",
                status: SupportStatus::Implemented,
                detail: "Exports and imports staged ConfigSave delta bundles with row digests and manifest validation.",
            },
            OperationSupport {
                name: "module-blob-pack",
                status: SupportStatus::Implemented,
                detail: "Builds common-module body blobs from BSL text.",
            },
            OperationSupport {
                name: "versions-blob-patch",
                status: SupportStatus::Implemented,
                detail: "Patches versions blob entries for staged changes.",
            },
            OperationSupport {
                name: "mssql-stage-*",
                status: SupportStatus::Partial,
                detail: "Common-module, generic metadata, explicit family wrappers, and source-root auto-stage helpers, including combined source-tree staging, are implemented for the supported surface; deeper blob-model work and parity experiments remain under research.",
            },
            OperationSupport {
                name: "cf-bootstrap",
                status: SupportStatus::Partial,
                detail: "Builds a new Format15 or Format16 CF without a base file or installed 1C/EDT runtime for the exact profile/family/asset routes accepted by the fail-closed compiler; unsupported source files prevent publication.",
            },
            OperationSupport {
                name: "ibcmd comparison matrix",
                status: SupportStatus::Planned,
                detail: "Run the four disposable-database experiments and compare against ibcmd timings and SQL traces.",
            },
        ],
        supported_source_families: vec![
            "Catalogs",
            "Documents",
            "InformationRegisters",
            "AccumulationRegisters",
            "AccountingRegisters",
            "CalculationRegisters",
            "ChartsOfCharacteristicTypes",
            "ChartsOfAccounts",
            "ChartsOfCalculationTypes",
            "ChartsOfCalculationRegisters",
            "CommonModules",
            "CommonForms",
            "CommonPictures",
            "CommonTemplates",
            "CommonAttributes",
            "CommandGroups",
            "DocumentJournals",
            "Reports",
            "DataProcessors",
            "Enums",
            "ExchangePlans",
            "EventSubscriptions",
            "FilterCriteria",
            "FunctionalOptions",
            "FunctionalOptionsParameters",
            "HTTPServices",
            "Languages",
            "ScheduledJobs",
            "SessionParameters",
            "SettingsStorages",
            "StyleItems",
            "Styles",
            "Subsystems",
            "Roles",
            "CommonCommands",
            "BusinessProcesses",
            "Bots",
            "DefinedTypes",
            "Tasks",
            "Constants",
            "DocumentNumerators",
            "IntegrationServices",
            "Sequences",
            "WebServices",
            "WSReferences",
            "XDTOPackages",
        ],
        supported_metadata_families: vec![
            "Constant",
            "SessionParameter",
            "SettingsStorage",
            "DefinedType",
            "CommonCommand",
            "CommandGroup",
            "CommonModule",
            "AccumulationRegister",
            "AccountingRegister",
            "CalculationRegister",
            "ChartOfAccounts",
            "ChartOfCalculationTypes",
            "ChartOfCalculationRegisters",
            "ChartOfCharacteristicTypes",
            "Language",
            "CommonPicture",
            "CommonForm",
            "CommonTemplate",
            "CommonAttribute",
            "DocumentJournal",
            "Report",
            "DataProcessor",
            "Enum",
            "Bot",
            "BusinessProcess",
            "ExchangePlan",
            "EventSubscription",
            "FilterCriterion",
            "FunctionalOption",
            "FunctionalOptionsParameter",
            "HTTPService",
            "ScheduledJob",
            "StyleItem",
            "Style",
            "Subsystem",
            "Role",
            "DocumentNumerator",
            "WebService",
            "WSReference",
            "IntegrationService",
            "Sequence",
            "XDTOPackage",
            "Task",
        ],
        supported_write_patterns: vec![
            "no-op load",
            "module-body-only change",
            "metadata-attribute change",
            "new object insert",
            "merge-based ConfigSave staging",
        ],
        notes: vec![
            "Platform build, DBMS version and compatibility mode are captured per run by probe and trace artifacts.",
            "Storage and delta bundle manifests include row checksums where available so imports can verify content integrity.",
            "The current matrix is source-of-truth for implemented coverage; ibcmd parity is still being measured experimentally.",
            "Trace storage mapping is derived from normalized SQL groups and now exposes mutation kinds, stage roles and operation families for ConfigSave and Params.",
            "Non-lab destructive writes remain gated behind explicit confirmation flags.",
        ],
    }
}

pub fn write_compatibility_report(report: &CompatibilityReport, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

#[cfg(test)]
mod tests {
    use super::{SupportStatus, current_compatibility_report};

    #[test]
    fn exposes_current_supported_surface() {
        let report = current_compatibility_report();

        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| item.name == "scan")
        );
        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| item.name == "trace-analyze")
        );
        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| item.name == "storage-map")
        );
        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| item.detail.contains("stage roles"))
        );
        assert!(report.supported_operations.iter().any(|item| {
            item.name == "cf-bootstrap" && matches!(item.status, SupportStatus::Partial)
        }));
        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| item.name == "ibcmd comparison matrix")
        );
        assert!(
            report
                .supported_operations
                .iter()
                .any(|item| matches!(item.status, SupportStatus::Planned))
        );

        assert!(report.supported_source_families.contains(&"Tasks"));
        assert!(report.supported_source_families.contains(&"CommonCommands"));
        assert!(
            report
                .supported_source_families
                .contains(&"CommonAttributes")
        );
        assert!(
            report
                .supported_source_families
                .contains(&"BusinessProcesses")
        );
        assert!(report.supported_source_families.contains(&"Styles"));
        assert!(report.supported_source_families.contains(&"Bots"));
        assert!(report.supported_source_families.contains(&"DefinedTypes"));
        assert!(
            report
                .supported_source_families
                .contains(&"DocumentNumerators")
        );
        assert!(
            report
                .supported_source_families
                .contains(&"IntegrationServices")
        );
        assert!(report.supported_source_families.contains(&"Sequences"));
        assert!(report.supported_source_families.contains(&"WSReferences"));
        assert!(
            report
                .supported_metadata_families
                .contains(&"CommonCommand")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"CommonAttribute")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"ChartOfCharacteristicTypes")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"ChartOfCalculationTypes")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"ChartOfCalculationRegisters")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"AccumulationRegister")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"AccountingRegister")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"CalculationRegister")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"ChartOfAccounts")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"BusinessProcess")
        );
        assert!(report.supported_metadata_families.contains(&"Bot"));
        assert!(
            report
                .supported_metadata_families
                .contains(&"SettingsStorage")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"DocumentNumerator")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"IntegrationService")
        );
        assert!(report.supported_metadata_families.contains(&"Sequence"));
        assert!(report.supported_metadata_families.contains(&"Style"));
        assert!(report.supported_metadata_families.contains(&"WSReference"));
        assert_eq!(
            report
                .supported_metadata_families
                .iter()
                .filter(|value| **value == "CommandGroup")
                .count(),
            1
        );
        assert!(report.supported_metadata_families.contains(&"Task"));
        assert!(
            report
                .supported_write_patterns
                .contains(&"merge-based ConfigSave staging")
        );
    }
}
