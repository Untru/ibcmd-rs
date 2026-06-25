use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CompatibilityReport {
    pub supported_operations: Vec<OperationSupport>,
    pub supported_source_families: Vec<&'static str>,
    pub supported_metadata_families: Vec<&'static str>,
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
                name: "mssql-compare",
                status: SupportStatus::Implemented,
                detail: "Compares table shape and row counts for SQL Server infobases.",
            },
            OperationSupport {
                name: "mssql-clone",
                status: SupportStatus::Implemented,
                detail: "Backup/restore clone flow for disposable SQL Server databases.",
            },
            OperationSupport {
                name: "mssql-storage-export/import",
                status: SupportStatus::Implemented,
                detail: "Exports and imports ConfigSave/Params storage bundles via BCP.",
            },
            OperationSupport {
                name: "mssql-delta-export/import",
                status: SupportStatus::Implemented,
                detail: "Exports and imports staged ConfigSave delta bundles.",
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
                detail: "Common-module and simple metadata staging are implemented; broader family coverage remains under research.",
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
            "Subsystems",
            "Roles",
            "CommonCommands",
            "BusinessProcesses",
            "Tasks",
            "Constants",
            "WebServices",
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
            "Language",
            "CommonPicture",
            "CommonForm",
            "CommonTemplate",
            "CommonAttribute",
            "CommandGroup",
            "DocumentJournal",
            "Report",
            "DataProcessor",
            "Enum",
            "ExchangePlan",
            "EventSubscription",
            "FilterCriterion",
            "FunctionalOption",
            "FunctionalOptionsParameter",
            "HTTPService",
            "ScheduledJob",
            "StyleItem",
            "Subsystem",
            "Role",
            "WebService",
            "XDTOPackage",
            "Task",
        ],
        notes: vec![
            "Platform build, DBMS version and compatibility mode are captured per run by probe and trace artifacts.",
            "The current matrix is source-of-truth for implemented coverage; ibcmd parity is still being measured experimentally.",
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
                .contains(&"BusinessProcesses")
        );
        assert!(
            report
                .supported_metadata_families
                .contains(&"CommonCommand")
        );
        assert!(report.supported_metadata_families.contains(&"Task"));
    }
}
