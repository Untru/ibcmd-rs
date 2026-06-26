use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::trace::TraceAnalysis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageMutationKind {
    ConfigSaveCount,
    ConfigSaveInsert,
    ConfigSaveUpdateBinaryData,
    ConfigSaveUpdateAttributes,
    ConfigSaveMerge,
    ConfigSaveDelete,
    ParamsWrite,
    TransactionBoundary,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageStageRole {
    ConfigHeader,
    ConfigVersion,
    ConfigModuleBody,
    ConfigMetadata,
    ConfigObject,
    ConfigSaveGeneric,
    Params,
    TransactionBoundary,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOperationFamily {
    ConfigHeader,
    ConfigVersion,
    ConfigBundle,
    CommonModuleBody,
    CommonModuleMetadata,
    CommonModuleObject,
    MetadataObject,
    Params,
    TransactionBoundary,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageMappingReport {
    pub files: Vec<PathBuf>,
    pub groups_seen: usize,
    pub mapped_groups: usize,
    pub unmapped_groups: usize,
    pub summaries: Vec<StorageMutationSummary>,
    pub roles: Vec<StorageStageRoleSummary>,
    pub families: Vec<StorageOperationFamilySummary>,
    pub entries: Vec<StorageMappingEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageMutationSummary {
    pub kind: StorageMutationKind,
    pub groups: usize,
    pub total_duration_us: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageStageRoleSummary {
    pub role: StorageStageRole,
    pub groups: usize,
    pub total_duration_us: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageOperationFamilySummary {
    pub family: StorageOperationFamily,
    pub groups: usize,
    pub total_duration_us: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageMappingEntry {
    pub normalized_sql: String,
    pub sample_sql: String,
    pub count: usize,
    pub total_duration_us: u64,
    pub table_names: Vec<String>,
    pub kind: StorageMutationKind,
    pub role: StorageStageRole,
    pub family: StorageOperationFamily,
    pub signals: Vec<String>,
    pub detail: String,
}

pub fn build_storage_mapping(analysis: &TraceAnalysis) -> StorageMappingReport {
    let mut entries = analysis
        .groups
        .iter()
        .map(|group| StorageMappingEntry {
            normalized_sql: group.normalized_sql.clone(),
            sample_sql: group.sample_sql.clone(),
            count: group.count,
            total_duration_us: group.total_duration_us,
            table_names: group.table_names.clone(),
            kind: classify_group(group),
            role: classify_role(group),
            family: classify_family(group),
            signals: classify_signals(group),
            detail: classify_detail(group),
        })
        .collect::<Vec<_>>();

    let mut summaries = summarize_entries(&entries);
    let mut roles = summarize_roles(&entries);
    let mut families = summarize_families(&entries);

    entries.sort_by(|left, right| {
        right
            .total_duration_us
            .cmp(&left.total_duration_us)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.normalized_sql.cmp(&right.normalized_sql))
    });

    let mapped_groups = entries
        .iter()
        .filter(|entry| entry.kind != StorageMutationKind::Other)
        .count();

    StorageMappingReport {
        files: analysis.files.clone(),
        groups_seen: analysis.groups.len(),
        mapped_groups,
        unmapped_groups: entries.len().saturating_sub(mapped_groups),
        summaries: {
            summaries.sort_by(|left, right| {
                right
                    .total_duration_us
                    .cmp(&left.total_duration_us)
                    .then_with(|| left.kind.cmp(&right.kind))
            });
            summaries
        },
        roles: {
            roles.sort_by(|left, right| {
                right
                    .total_duration_us
                    .cmp(&left.total_duration_us)
                    .then_with(|| left.role.cmp(&right.role))
            });
            roles
        },
        families: {
            families.sort_by(|left, right| {
                right
                    .total_duration_us
                    .cmp(&left.total_duration_us)
                    .then_with(|| left.family.cmp(&right.family))
            });
            families
        },
        entries,
    }
}

fn summarize_entries(entries: &[StorageMappingEntry]) -> Vec<StorageMutationSummary> {
    let mut totals = std::collections::BTreeMap::<StorageMutationKind, (usize, u64)>::new();
    for entry in entries {
        let value = totals.entry(entry.kind).or_insert((0, 0));
        value.0 += 1;
        value.1 += entry.total_duration_us;
    }
    totals
        .into_iter()
        .map(
            |(kind, (groups, total_duration_us))| StorageMutationSummary {
                kind,
                groups,
                total_duration_us,
            },
        )
        .collect()
}

fn summarize_roles(entries: &[StorageMappingEntry]) -> Vec<StorageStageRoleSummary> {
    let mut totals = std::collections::BTreeMap::<StorageStageRole, (usize, u64)>::new();
    for entry in entries {
        let value = totals.entry(entry.role).or_insert((0, 0));
        value.0 += 1;
        value.1 += entry.total_duration_us;
    }
    totals
        .into_iter()
        .map(
            |(role, (groups, total_duration_us))| StorageStageRoleSummary {
                role,
                groups,
                total_duration_us,
            },
        )
        .collect()
}

fn summarize_families(entries: &[StorageMappingEntry]) -> Vec<StorageOperationFamilySummary> {
    let mut totals = std::collections::BTreeMap::<StorageOperationFamily, (usize, u64)>::new();
    for entry in entries {
        let value = totals.entry(entry.family).or_insert((0, 0));
        value.0 += 1;
        value.1 += entry.total_duration_us;
    }
    totals
        .into_iter()
        .map(
            |(family, (groups, total_duration_us))| StorageOperationFamilySummary {
                family,
                groups,
                total_duration_us,
            },
        )
        .collect()
}

fn classify_group(group: &crate::trace::QueryGroup) -> StorageMutationKind {
    let sql = group.normalized_sql.as_str();
    if sql == "begin transaction" || sql == "commit transaction" || sql == "rollback transaction" {
        return StorageMutationKind::TransactionBoundary;
    }
    if sql.starts_with("select count_big(*) from configsave") {
        return StorageMutationKind::ConfigSaveCount;
    }
    if sql.starts_with("insert into configsave") {
        return StorageMutationKind::ConfigSaveInsert;
    }
    if sql.starts_with("update configsave set binarydata") {
        return StorageMutationKind::ConfigSaveUpdateBinaryData;
    }
    if sql.starts_with("update configsave set attributes") {
        return StorageMutationKind::ConfigSaveUpdateAttributes;
    }
    if sql.starts_with("merge into configsave") {
        return StorageMutationKind::ConfigSaveMerge;
    }
    if sql.starts_with("delete from configsave") {
        return StorageMutationKind::ConfigSaveDelete;
    }
    if sql.starts_with("insert into params")
        || sql.starts_with("update params")
        || sql.starts_with("delete from params")
    {
        return StorageMutationKind::ParamsWrite;
    }
    StorageMutationKind::Other
}

fn classify_detail(group: &crate::trace::QueryGroup) -> String {
    match classify_group(group) {
        StorageMutationKind::ConfigSaveCount => "counts rows in ConfigSave".to_string(),
        StorageMutationKind::ConfigSaveInsert => primary_table_detail(group, "insert into"),
        StorageMutationKind::ConfigSaveUpdateBinaryData => {
            "updates ConfigSave.BinaryData".to_string()
        }
        StorageMutationKind::ConfigSaveUpdateAttributes => {
            "updates ConfigSave.Attributes".to_string()
        }
        StorageMutationKind::ConfigSaveMerge => "MERGE-based ConfigSave staging".to_string(),
        StorageMutationKind::ConfigSaveDelete => "deletes rows from ConfigSave".to_string(),
        StorageMutationKind::ParamsWrite => primary_table_detail(group, "params write"),
        StorageMutationKind::TransactionBoundary => "transaction boundary".to_string(),
        StorageMutationKind::Other => {
            if group.table_names.is_empty() {
                "unclassified SQL pattern".to_string()
            } else {
                format!("unclassified SQL touching {}", group.table_names.join(", "))
            }
        }
    }
}

fn classify_role(group: &crate::trace::QueryGroup) -> StorageStageRole {
    let sql = group.normalized_sql.as_str();
    let sample = group.sample_sql.as_str();
    let sample_lower = sample.to_ascii_lowercase();

    if sql == "begin transaction" || sql == "commit transaction" || sql == "rollback transaction" {
        return StorageStageRole::TransactionBoundary;
    }
    if sql.starts_with("select count_big(*) from configsave") {
        return StorageStageRole::ConfigHeader;
    }
    if sql.starts_with("merge into configsave") {
        return StorageStageRole::ConfigObject;
    }
    if sql.starts_with("insert into configsave") {
        if sample_lower.contains("n'versions'") {
            return StorageStageRole::ConfigVersion;
        }
        if sample_lower.contains("n'root'") || sample_lower.contains("n'version'") {
            return StorageStageRole::ConfigHeader;
        }
        if sample_lower.contains("n'module'") || sample_lower.contains("file_name = n'module'") {
            return StorageStageRole::ConfigModuleBody;
        }
        if sample_lower.contains("n'metadata'") || sample_lower.contains("file_name = n'metadata'")
        {
            return StorageStageRole::ConfigMetadata;
        }
        if sample_lower.contains("n'newobject'") || sample_lower.contains("values (n'") {
            return StorageStageRole::ConfigObject;
        }
        return StorageStageRole::ConfigSaveGeneric;
    }
    if sql.starts_with("update configsave set binarydata") {
        if sample_lower.contains("file_name = n'versions'") {
            return StorageStageRole::ConfigVersion;
        }
        if sample_lower.contains("file_name = n'module'") {
            return StorageStageRole::ConfigModuleBody;
        }
        return StorageStageRole::ConfigSaveGeneric;
    }
    if sql.starts_with("update configsave set attributes") {
        if sample_lower.contains("file_name = n'versions'") {
            return StorageStageRole::ConfigVersion;
        }
        if sample_lower.contains("file_name = n'metadata'") {
            return StorageStageRole::ConfigMetadata;
        }
        return StorageStageRole::ConfigSaveGeneric;
    }
    if sql.starts_with("delete from configsave") {
        return StorageStageRole::ConfigSaveGeneric;
    }
    if sql.starts_with("insert into params")
        || sql.starts_with("update params")
        || sql.starts_with("delete from params")
    {
        return StorageStageRole::Params;
    }
    StorageStageRole::Other
}

fn classify_family(group: &crate::trace::QueryGroup) -> StorageOperationFamily {
    let sql = group.normalized_sql.as_str();
    let sample_lower = group.sample_sql.to_ascii_lowercase();

    if sql == "begin transaction" || sql == "commit transaction" || sql == "rollback transaction" {
        return StorageOperationFamily::TransactionBoundary;
    }
    if sql.starts_with("select count_big(*) from configsave") {
        return StorageOperationFamily::ConfigHeader;
    }
    if sql.starts_with("insert into configsave")
        && sample_lower.contains("n'root'")
        && sample_lower.contains("n'version'")
    {
        return StorageOperationFamily::ConfigHeader;
    }
    if sql.starts_with("insert into configsave")
        || sql.starts_with("update configsave set binarydata")
    {
        if sample_lower.contains("n'versions'") {
            return StorageOperationFamily::ConfigVersion;
        }
        if sample_lower.contains("n'module'") || sample_lower.contains("file_name = n'module'") {
            return StorageOperationFamily::CommonModuleBody;
        }
        if sample_lower.contains("n'metadata'") || sample_lower.contains("file_name = n'metadata'")
        {
            return StorageOperationFamily::CommonModuleMetadata;
        }
        if sample_lower.contains("n'newobject'") || sample_lower.contains("values (n'") {
            return StorageOperationFamily::CommonModuleObject;
        }
        return StorageOperationFamily::ConfigBundle;
    }
    if sql.starts_with("update configsave set attributes") {
        if sample_lower.contains("file_name = n'versions'") {
            return StorageOperationFamily::ConfigVersion;
        }
        if sample_lower.contains("file_name = n'module'") {
            return StorageOperationFamily::CommonModuleBody;
        }
        if sample_lower.contains("file_name = n'metadata'") {
            return StorageOperationFamily::CommonModuleMetadata;
        }
        return StorageOperationFamily::MetadataObject;
    }
    if sql.starts_with("merge into configsave") || sql.starts_with("delete from configsave") {
        return StorageOperationFamily::ConfigBundle;
    }
    if sql.starts_with("insert into params")
        || sql.starts_with("update params")
        || sql.starts_with("delete from params")
    {
        return StorageOperationFamily::Params;
    }
    StorageOperationFamily::Other
}

fn classify_signals(group: &crate::trace::QueryGroup) -> Vec<String> {
    let sql = group.normalized_sql.as_str();
    let sample_lower = group.sample_sql.to_ascii_lowercase();
    let mut signals = Vec::new();

    if sql == "begin transaction" || sql == "commit transaction" || sql == "rollback transaction" {
        signals.push("transaction boundary".to_string());
    }
    if sql.starts_with("select count_big(*) from configsave") {
        signals.push("counts ConfigSave rows".to_string());
    }
    if sample_lower.contains("n'root'") && sample_lower.contains("n'version'") {
        signals.push("copies root/version rows".to_string());
    }
    if sample_lower.contains("n'versions'") {
        signals.push("touches versions row".to_string());
    }
    if sample_lower.contains("n'module'") || sample_lower.contains("file_name = n'module'") {
        signals.push("module body row".to_string());
    }
    if sample_lower.contains("n'metadata'") || sample_lower.contains("file_name = n'metadata'") {
        signals.push("metadata row".to_string());
    }
    if sample_lower.contains("n'newobject'") {
        signals.push("new object row".to_string());
    }
    if sql.starts_with("merge into configsave") {
        signals.push("merge-based staging".to_string());
    }
    if sql.starts_with("insert into params")
        || sql.starts_with("update params")
        || sql.starts_with("delete from params")
    {
        signals.push("Params table mutation".to_string());
    }

    if signals.is_empty() {
        signals.push("unclassified".to_string());
    }

    signals
}

fn primary_table_detail(group: &crate::trace::QueryGroup, fallback: &str) -> String {
    if let Some(table) = group.table_names.first() {
        format!("{} {}", fallback, table)
    } else {
        format!("{} table", fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StorageMutationKind, StorageOperationFamily, StorageStageRole, build_storage_mapping,
    };
    use crate::trace::{QueryGroup, TraceAnalysis};

    fn group(
        normalized_sql: &str,
        table_names: Vec<&str>,
        count: usize,
        duration: u64,
    ) -> QueryGroup {
        QueryGroup {
            normalized_sql: normalized_sql.to_string(),
            sample_sql: normalized_sql.to_string(),
            count,
            total_duration_us: duration,
            max_duration_us: duration,
            average_duration_us: duration / count as u64,
            event_names: vec!["sql_statement_completed".to_string()],
            total_row_count: 0,
            max_row_count: 0,
            session_ids: vec![],
            database_names: vec![],
            client_hostnames: vec![],
            client_app_names: vec![],
            usernames: vec![],
            transaction_ids: vec![],
            attach_activity_ids: vec![],
            attach_activity_id_xfers: vec![],
            object_names: vec![],
            table_names: table_names.into_iter().map(ToString::to_string).collect(),
            begin_transaction_count: 0,
            commit_transaction_count: 0,
            rollback_transaction_count: 0,
        }
    }

    #[test]
    fn classifies_configsave_patterns() {
        let analysis = TraceAnalysis {
            files: vec!["trace.xml".into()],
            events_seen: 12,
            groups: vec![
                group(
                    "select count_big(*) from configsave where file_name = ? and part_no = ?",
                    vec!["ConfigSave"],
                    1,
                    10,
                ),
                group(
                    "insert into configsave (file_name) values (?)",
                    vec!["ConfigSave"],
                    1,
                    11,
                ),
                group(
                    "update configsave set binarydata = ? where file_name = ?",
                    vec!["ConfigSave"],
                    1,
                    12,
                ),
                group(
                    "update configsave set attributes = ? where file_name = ?",
                    vec!["ConfigSave"],
                    1,
                    13,
                ),
                group(
                    "update configsave set binarydata = 0x01 where file_name = N'Module' and part_no = 0",
                    vec!["ConfigSave"],
                    1,
                    13,
                ),
                group(
                    "update configsave set attributes = 0x02 where file_name = N'Metadata' and part_no = 0",
                    vec!["ConfigSave"],
                    1,
                    13,
                ),
                group(
                    "merge into configsave as target using config as source on target.filename = source.filename when matched then update set target.binarydata = source.binarydata when not matched then insert (filename) values (source.filename)",
                    vec!["Config", "ConfigSave"],
                    1,
                    14,
                ),
                group(
                    "insert into configsave (file_name, creation, modified, attributes, data_size, binarydata, part_no) select file_name, sysutcdatetime(), sysutcdatetime(), attributes, data_size, binarydata, part_no from config where file_name in (?, ?) and part_no = ?",
                    vec!["ConfigSave"],
                    1,
                    15,
                ),
                group(
                    "insert into configsave (file_name, creation, modified, attributes, data_size, binarydata, part_no) select file_name, sysutcdatetime(), sysutcdatetime(), attributes, data_size, binarydata, part_no from config where file_name = ? and part_no = ?",
                    vec!["ConfigSave"],
                    1,
                    16,
                ),
                group(
                    "insert into configsave (file_name, creation, modified, attributes, data_size, binarydata, part_no) select n'versions', sysutcdatetime(), sysutcdatetime(), attributes, data_size, binarydata, part_no from config where file_name = n'versions' and part_no = 0",
                    vec!["ConfigSave"],
                    1,
                    16,
                ),
                group(
                    "insert into params (file_name, creation, modified, attributes, data_size, binarydata, part_no) values (?, sysutcdatetime(), sysutcdatetime(), ?, ?, ?, ?)",
                    vec!["Params"],
                    1,
                    17,
                ),
                group("begin transaction", vec![], 1, 18),
            ],
        };

        let report = build_storage_mapping(&analysis);
        assert_eq!(report.groups_seen, 12);
        assert_eq!(report.mapped_groups, 12);
        assert_eq!(report.unmapped_groups, 0);
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.kind == StorageMutationKind::ConfigSaveMerge)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.kind == StorageMutationKind::ConfigSaveCount)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::ConfigModuleBody)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::ConfigMetadata)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::ConfigObject)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::ConfigVersion)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::Params)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.role == StorageStageRole::TransactionBoundary)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.family == StorageOperationFamily::CommonModuleBody)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.family == StorageOperationFamily::CommonModuleMetadata)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.family == StorageOperationFamily::ConfigBundle)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.family == StorageOperationFamily::ConfigHeader)
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.family == StorageOperationFamily::ConfigVersion)
        );
        assert!(report.entries.iter().any(|entry| {
            entry
                .signals
                .iter()
                .any(|signal| signal == "merge-based staging")
        }));
        assert!(report.entries.iter().any(|entry| {
            entry
                .signals
                .iter()
                .any(|signal| signal == "touches versions row")
        }));
        assert!(report.entries.iter().any(|entry| {
            entry
                .signals
                .iter()
                .any(|signal| signal == "Params table mutation")
        }));
        assert!(
            report
                .summaries
                .iter()
                .any(|summary| summary.kind == StorageMutationKind::ConfigSaveMerge)
        );
        assert!(
            report
                .summaries
                .iter()
                .any(|summary| summary.kind == StorageMutationKind::ParamsWrite)
        );
        assert!(
            report
                .roles
                .iter()
                .any(|summary| summary.role == StorageStageRole::ConfigObject)
        );
        assert!(
            report
                .roles
                .iter()
                .any(|summary| summary.role == StorageStageRole::ConfigVersion)
        );
        assert!(
            report
                .roles
                .iter()
                .any(|summary| summary.role == StorageStageRole::TransactionBoundary)
        );
        assert!(
            report
                .families
                .iter()
                .any(|summary| summary.family == StorageOperationFamily::CommonModuleBody)
        );
        assert!(
            report
                .families
                .iter()
                .any(|summary| summary.family == StorageOperationFamily::ConfigHeader)
        );
        assert!(
            report
                .families
                .iter()
                .any(|summary| summary.family == StorageOperationFamily::ConfigVersion)
        );
        assert!(
            report
                .families
                .iter()
                .any(|summary| summary.family == StorageOperationFamily::Params)
        );
        assert!(
            report
                .families
                .iter()
                .any(|summary| summary.family == StorageOperationFamily::ConfigBundle)
        );
    }

    #[test]
    fn leaves_unknown_patterns_unmapped() {
        let analysis = TraceAnalysis {
            files: vec!["trace.xml".into()],
            events_seen: 1,
            groups: vec![group("select 1", vec![], 1, 4)],
        };

        let report = build_storage_mapping(&analysis);
        assert_eq!(report.mapped_groups, 0);
        assert_eq!(report.unmapped_groups, 1);
        assert_eq!(report.entries[0].kind, StorageMutationKind::Other);
        assert_eq!(report.entries[0].role, StorageStageRole::Other);
        assert_eq!(report.entries[0].family, StorageOperationFamily::Other);
        assert_eq!(report.entries[0].signals, vec!["unclassified".to_string()]);
    }
}
