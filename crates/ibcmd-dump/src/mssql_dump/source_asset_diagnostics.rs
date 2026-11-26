use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::SourceAssetCompletenessEntry;

pub(super) const MAX_SOURCE_ASSET_DIAGNOSTIC_CLUSTERS: usize = 128;
pub(super) const MAX_SOURCE_ASSET_DIAGNOSTIC_SAMPLES: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssetDiagnosticClusterReport {
    pub total_clusters: usize,
    pub omitted_clusters: usize,
    pub omitted_entries: usize,
    pub clusters: Vec<SourceAssetDiagnosticCluster>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssetDiagnosticCluster {
    pub family: String,
    pub code: String,
    pub classification: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error_class: Option<String>,
    pub property: String,
    pub property_profile: String,
    pub affected_entries: usize,
    pub omitted_samples: usize,
    pub samples: Vec<SourceAssetDiagnosticSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceAssetDiagnosticSample {
    pub asset_path: String,
    pub source_row_id: String,
    pub form_item_tag: String,
    pub property_slot: usize,
    pub raw_length: usize,
    pub raw_sha256: String,
}

type ClusterKey = (String, String, String, Option<String>, String, String);

pub(super) fn cluster_source_asset_diagnostics(
    entries: &[SourceAssetCompletenessEntry],
) -> SourceAssetDiagnosticClusterReport {
    let mut grouped = BTreeMap::<ClusterKey, Vec<SourceAssetDiagnosticSample>>::new();
    for entry in entries.iter().filter(|entry| {
        !entry.family.is_empty()
            && entry.family != "unknown"
            && !entry.property.is_empty()
            && !entry.property_profile.is_empty()
    }) {
        let key = (
            entry.family.clone(),
            entry.code.clone(),
            entry.classification.clone(),
            entry.parse_error_class.clone(),
            entry.property.clone(),
            entry.property_profile.clone(),
        );
        grouped
            .entry(key)
            .or_default()
            .push(SourceAssetDiagnosticSample {
                asset_path: entry.asset_path.clone(),
                source_row_id: entry.source_row_id.clone(),
                form_item_tag: entry.form_item_tag.clone(),
                property_slot: entry.property_slot,
                raw_length: entry.raw_length,
                raw_sha256: entry.raw_sha256.clone(),
            });
    }

    let total_clusters = grouped.len();
    let mut omitted_entries = 0usize;
    let mut clusters = Vec::with_capacity(total_clusters.min(MAX_SOURCE_ASSET_DIAGNOSTIC_CLUSTERS));
    for (index, (key, mut samples)) in grouped.into_iter().enumerate() {
        samples.sort();
        let affected_entries = samples.len();
        if index >= MAX_SOURCE_ASSET_DIAGNOSTIC_CLUSTERS {
            omitted_entries += affected_entries;
            continue;
        }
        let omitted_samples = affected_entries.saturating_sub(MAX_SOURCE_ASSET_DIAGNOSTIC_SAMPLES);
        samples.truncate(MAX_SOURCE_ASSET_DIAGNOSTIC_SAMPLES);
        let (family, code, classification, parse_error_class, property, property_profile) = key;
        clusters.push(SourceAssetDiagnosticCluster {
            family,
            code,
            classification,
            parse_error_class,
            property,
            property_profile,
            affected_entries,
            omitted_samples,
            samples,
        });
    }

    SourceAssetDiagnosticClusterReport {
        total_clusters,
        omitted_clusters: total_clusters.saturating_sub(clusters.len()),
        omitted_entries,
        clusters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_dump::{SourceAssetCompletenessReport, SourceAssetCompletenessScope};

    fn entry(
        family: &str,
        code: &str,
        profile: &str,
        path: &str,
        raw_sha256: &str,
    ) -> SourceAssetCompletenessEntry {
        SourceAssetCompletenessEntry {
            family: family.to_owned(),
            code: code.to_owned(),
            classification: "opaque_property_rejected".to_owned(),
            parse_error_class: Some("primary_malformed".to_owned()),
            table: "Config".to_owned(),
            source_row_id: format!("{path}.0"),
            asset_path: path.to_owned(),
            form_owner_reference: None,
            form_item_id: "1".to_owned(),
            form_item_tag: "InputField".to_owned(),
            property: "ChoiceParameterLinks".to_owned(),
            property_profile: profile.to_owned(),
            property_slot: 26,
            raw_length: 42,
            raw_sha256: raw_sha256.to_owned(),
        }
    }

    #[test]
    fn equivalent_entries_form_one_sorted_bounded_cluster() {
        let entries = vec![
            entry("form", "opaque", "5006_5007", "z/Form.xml", "b"),
            entry("form", "opaque", "5006_5007", "a/Form.xml", "a"),
            entry("form", "opaque", "5006_5007", "m/Form.xml", "c"),
            entry("form", "opaque", "5006_5007", "x/Form.xml", "d"),
        ];

        let report = cluster_source_asset_diagnostics(&entries);

        assert_eq!(report.total_clusters, 1);
        assert_eq!(report.omitted_clusters, 0);
        assert_eq!(report.clusters[0].affected_entries, 4);
        assert_eq!(report.clusters[0].omitted_samples, 1);
        assert_eq!(
            report.clusters[0]
                .samples
                .iter()
                .map(|sample| sample.asset_path.as_str())
                .collect::<Vec<_>>(),
            vec!["a/Form.xml", "m/Form.xml", "x/Form.xml"]
        );
    }

    #[test]
    fn grouping_is_deterministic_and_serialization_has_no_raw_payload() {
        let raw_payload = "sentinel-raw-payload";
        let mut entries = vec![
            entry("form", "b", "p2", "b/Form.xml", "hash-b"),
            entry("form", "a", "p1", "a/Form.xml", "hash-a"),
        ];
        let first = serde_json::to_string(&cluster_source_asset_diagnostics(&entries)).unwrap();
        entries.reverse();
        let second = serde_json::to_string(&cluster_source_asset_diagnostics(&entries)).unwrap();

        assert_eq!(first, second);
        assert!(!first.contains(raw_payload));
        assert!(first.contains("hash-a"));
    }

    #[test]
    fn every_cluster_key_dimension_separates_profiles() {
        let mut entries = Vec::new();
        let baseline = entry("form", "code", "profile", "0/Form.xml", "0");
        entries.push(baseline.clone());

        let mut changed = baseline.clone();
        changed.family = "moxel".to_owned();
        entries.push(changed);
        let mut changed = baseline.clone();
        changed.code = "other-code".to_owned();
        entries.push(changed);
        let mut changed = baseline.clone();
        changed.classification = "other-classification".to_owned();
        entries.push(changed);
        let mut changed = baseline.clone();
        changed.parse_error_class = Some("other-error".to_owned());
        entries.push(changed);
        let mut changed = baseline.clone();
        changed.property = "OtherProperty".to_owned();
        entries.push(changed);
        let mut changed = baseline;
        changed.property_profile = "other-profile".to_owned();
        entries.push(changed);

        let report = cluster_source_asset_diagnostics(&entries);

        assert_eq!(report.total_clusters, 7);
        assert_eq!(report.clusters.len(), 7);
    }

    #[test]
    fn cluster_limit_retains_exact_totals_and_reports_overflow() {
        let entries = (0..=MAX_SOURCE_ASSET_DIAGNOSTIC_CLUSTERS)
            .map(|index| {
                entry(
                    "form",
                    &format!("code-{index:03}"),
                    "profile",
                    &format!("{index:03}/Form.xml"),
                    &format!("hash-{index:03}"),
                )
            })
            .collect::<Vec<_>>();

        let report = cluster_source_asset_diagnostics(&entries);

        assert_eq!(report.total_clusters, 129);
        assert_eq!(report.clusters.len(), 128);
        assert_eq!(report.omitted_clusters, 1);
        assert_eq!(report.omitted_entries, 1);
        assert_eq!(report.clusters[0].code, "code-000");
        assert_eq!(report.clusters[127].code, "code-127");
    }

    #[test]
    fn audit_entries_are_not_misclassified_as_raw_profile_clusters() {
        let mut audit = entry("unknown", "source_asset_missing", "", "missing", "");
        audit.property.clear();

        let report = cluster_source_asset_diagnostics(&[audit]);

        assert_eq!(report.total_clusters, 0);
        assert!(report.clusters.is_empty());
    }

    #[test]
    fn report_merge_order_produces_identical_schema_v2_clusters() {
        fn report_with(entry: SourceAssetCompletenessEntry) -> SourceAssetCompletenessReport {
            let mut report = SourceAssetCompletenessReport::default();
            report.record_opaque(vec![entry]);
            report.finish_inventory(SourceAssetCompletenessScope::Full, true, 1);
            report
        }

        let left = report_with(entry("form", "b", "p2", "b/Form.xml", "hash-b"));
        let right = report_with(entry("form", "a", "p1", "a/Form.xml", "hash-a"));
        let mut left_then_right = left.clone();
        left_then_right.merge(&right);
        let mut right_then_left = right;
        right_then_left.merge(&left);

        assert_eq!(left_then_right.schema_version, 2);
        assert_eq!(
            serde_json::to_string(&left_then_right.diagnostic_clusters).unwrap(),
            serde_json::to_string(&right_then_left.diagnostic_clusters).unwrap()
        );
    }

    #[test]
    fn schema_v1_report_without_clusters_deserializes_and_upgrades_on_finalize() {
        let mut legacy_json =
            serde_json::to_value(SourceAssetCompletenessReport::default()).unwrap();
        let object = legacy_json.as_object_mut().unwrap();
        object.insert("schema_version".to_owned(), serde_json::json!(1));
        object.remove("diagnostic_clusters");

        let mut legacy: SourceAssetCompletenessReport =
            serde_json::from_value(legacy_json).unwrap();
        assert_eq!(legacy.schema_version, 1);
        assert_eq!(legacy.diagnostic_clusters.total_clusters, 0);

        legacy.finish_inventory(SourceAssetCompletenessScope::Full, true, 0);
        assert_eq!(legacy.schema_version, 2);
        assert_eq!(legacy.diagnostic_clusters.total_clusters, 0);
    }
}
