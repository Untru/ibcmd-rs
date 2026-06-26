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
            "DefinedTypes",
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
            "ChartOfCharacteristicTypes",
            "ChartOfCalculationTypes",
            "ChartOfCalculationRegisters",
            "DocumentJournal",
            "Report",
            "DataProcessor",
            "Enum",
            "BusinessProcess",
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
        assert!(report.supported_source_families.contains(&"DefinedTypes"));
        assert!(
            report
                .supported_metadata_families
                .contains(&"CommonCommand")
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
                .contains(&"BusinessProcess")
        );
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
