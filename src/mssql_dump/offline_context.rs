use super::*;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
};

pub(crate) const OFFLINE_FORM_CONTEXT_SCHEMA_VERSION: u32 = 1;

/// Reuses the production metadata-text builders for a saved `Config_inflated`
/// corpus. It intentionally accepts only already-inflated UTF-8 text rows.
pub struct OfflineFormContextFactory;

#[allow(dead_code)]
#[derive(Debug)]
pub struct OfflineFormContext {
    pub(crate) type_index: BTreeMap<String, String>,
    pub(crate) dcs_type_index: DcsTypeIndex,
    pub(crate) object_refs: BTreeMap<String, String>,
    pub(crate) information_register_field_refs: InformationRegisterFieldReferenceIndex,
    pub(crate) form_owner_references: BTreeMap<String, String>,
    pub summary: OfflineFormContextSummary,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct OfflineFormContextSummary {
    pub(crate) schema_version: u32,
    pub(crate) source_commit: Option<String>,
    pub(crate) input_rows: usize,
    pub(crate) metadata_rows: usize,
    pub(crate) type_index_entries: usize,
    pub(crate) dcs_type_index_entries: usize,
    pub(crate) object_ref_entries: usize,
    pub(crate) information_register_field_ref_entries: usize,
    pub(crate) form_owner_entries: usize,
    pub(crate) type_collisions: usize,
    pub(crate) warnings: Vec<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) sha256: String,
}

impl OfflineFormContextFactory {
    pub fn from_run_root(
        run_root: &Path,
        source_commit: Option<String>,
    ) -> Result<OfflineFormContext> {
        let manifest_path = run_root.join("candidate_dump/manifest.json");
        let manifest: Value = serde_json::from_slice(
            &fs::read(&manifest_path)
                .with_context(|| format!("read {}", manifest_path.display()))?,
        )?;
        let mut rows = Vec::new();
        for table in manifest
            .get("tables")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("manifest.tables missing"))?
        {
            for row in table
                .get("rows")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("manifest rows missing"))?
            {
                let (Some(file_name), Some(inflated)) = (
                    row.get("file_name").and_then(Value::as_str),
                    row.get("inflated_path").and_then(Value::as_str),
                ) else {
                    continue;
                };
                let path = Path::new(inflated);
                if path.is_absolute()
                    || path.components().any(|c| {
                        matches!(
                            c,
                            Component::ParentDir | Component::RootDir | Component::Prefix(_)
                        )
                    })
                {
                    bail!("unsafe inflated path: {inflated}");
                }
                if !inflated.starts_with("Config_inflated/")
                    && !inflated.starts_with("Config_inflated\\")
                {
                    bail!("unexpected inflated path: {inflated}");
                }
                // Production metadata indexers intentionally ignore dotted body/module rows.
                // Do this before UTF-8 decoding: those rows may be binary blobs.
                if file_name.contains('.') {
                    continue;
                }
                rows.push((
                    file_name.to_string(),
                    fs::read_to_string(run_root.join("candidate_dump").join(path))
                        .with_context(|| format!("read inflated {inflated}"))?,
                ));
            }
        }
        Self::from_plain_rows(rows, source_commit)
    }
    pub(crate) fn from_plain_rows(
        rows: impl IntoIterator<Item = (String, String)>,
        source_commit: Option<String>,
    ) -> Result<OfflineFormContext> {
        let mut rows = rows
            .into_iter()
            .filter(|(file_name, _)| !file_name.contains('.'))
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            bail!("offline context has duplicate Config row file names");
        }
        let mut metadata = Vec::new();
        let errors = Vec::new();
        let mut warnings = Vec::new();
        for (file_name, text) in &rows {
            match metadata_text_row_audit_from_text(
                file_name,
                text.trim_start_matches('\u{feff}').to_string(),
            ) {
                MetadataTextRowAudit::Extracted(mut row) => {
                    normalize_direct_form_metadata(&mut row);
                    metadata.push(row);
                }
                MetadataTextRowAudit::ExtractedWithWarning(mut row, miss) => {
                    warnings.push(format!("{}:{}", miss.file_name, miss.reason.as_str()));
                    normalize_direct_form_metadata(&mut row);
                    metadata.push(row);
                }
                MetadataTextRowAudit::Miss(miss) => {
                    bail!(
                        "offline context incomplete at {}:{}",
                        miss.file_name,
                        miss.reason.as_str()
                    );
                }
            }
        }
        let indexes = build_metadata_type_indexes_from_texts(&metadata);
        if !indexes.reference_collisions.is_empty() {
            bail!(
                "offline context has {} type-index collisions",
                indexes.reference_collisions.len()
            );
        }
        let type_index = indexes.references;
        let object_refs = build_metadata_object_reference_index_from_texts(&metadata);
        let defined =
            build_defined_type_value_owner_reference_index_from_texts(&metadata, &type_index);
        let information_register_field_refs =
            build_information_register_field_reference_index_from_texts(
                &metadata,
                &type_index,
                &defined,
            );
        let form_owner_references = build_complete_form_source_reference_index(&metadata)
            .into_iter()
            .filter_map(|(id, reference)| {
                form_owner_reference_name(&reference).map(|owner| (id, owner))
            })
            .collect::<BTreeMap<_, _>>();
        let dcs_type_index = indexes.dcs;
        let mut canonical = Vec::new();
        for (key, value) in &type_index {
            canonical.push(format!("type\0{key}\0{value}"));
        }
        for (key, value) in &object_refs {
            canonical.push(format!("object\0{key}\0{value}"));
        }
        for (key, value) in &form_owner_references {
            canonical.push(format!("owner\0{key}\0{value}"));
        }
        for (key, value) in &dcs_type_index {
            let value = match value {
                DcsTypeResolution::KeepId => "keep".to_string(),
                DcsTypeResolution::Type { qname } => format!("type:{qname}"),
                DcsTypeResolution::TypeSet { qname } => format!("set:{qname}"),
            };
            canonical.push(format!("dcs\0{key}\0{value}"));
        }
        for (register, fields) in &information_register_field_refs {
            let mut fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}:{}",
                        field.field_reference,
                        field
                            .value_owner_references
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>();
            fields.sort();
            for field in fields {
                canonical.push(format!("ir\0{register}\0{field}"));
            }
        }
        for warning in &warnings {
            canonical.push(format!("warning\0{warning}"));
        }
        for error in &errors {
            canonical.push(format!("error\0{error}"));
        }
        canonical.sort();
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "offline-form-context/v{}\0",
            OFFLINE_FORM_CONTEXT_SCHEMA_VERSION
        ));
        for line in canonical {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        let summary = OfflineFormContextSummary {
            schema_version: OFFLINE_FORM_CONTEXT_SCHEMA_VERSION,
            source_commit,
            input_rows: rows.len(),
            metadata_rows: metadata.len(),
            type_index_entries: type_index.len(),
            dcs_type_index_entries: dcs_type_index.len(),
            object_ref_entries: object_refs.len(),
            information_register_field_ref_entries: information_register_field_refs.len(),
            form_owner_entries: form_owner_references.len(),
            type_collisions: 0,
            warnings,
            errors,
            sha256: format!("{:x}", hasher.finalize()),
        };
        Ok(OfflineFormContext {
            type_index,
            dcs_type_index,
            object_refs,
            information_register_field_refs,
            form_owner_references,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_context_matches_empty_production_builders_and_is_deterministic() {
        let first = OfflineFormContextFactory::from_plain_rows(
            Vec::<(String, String)>::new(),
            Some("abc".to_string()),
        )
        .unwrap();
        let second = OfflineFormContextFactory::from_plain_rows(
            Vec::<(String, String)>::new(),
            Some("abc".to_string()),
        )
        .unwrap();
        let direct = build_metadata_type_indexes_from_texts(&[]);
        assert_eq!(first.type_index, direct.references);
        assert_eq!(first.dcs_type_index, direct.dcs);
        assert_eq!(
            first.object_refs,
            build_metadata_object_reference_index_from_texts(&[])
        );
        assert_eq!(first.summary, second.summary);
    }

    #[test]
    fn offline_context_rejects_duplicate_file_name_fail_closed() {
        let error = OfflineFormContextFactory::from_plain_rows(
            vec![
                ("same".to_string(), "x".to_string()),
                ("same".to_string(), "y".to_string()),
            ],
            None,
        )
        .expect_err("duplicate rows must not choose an arbitrary context");
        assert!(error.to_string().contains("duplicate"));
    }

    #[test]
    fn offline_context_records_structural_warnings_and_skips_dotted_body() {
        let warned = OfflineFormContextFactory::from_plain_rows(
            vec![("eligible".to_string(), "broken".to_string())],
            None,
        )
        .unwrap();
        assert_eq!(warned.summary.input_rows, 1);
        assert_eq!(warned.summary.metadata_rows, 1);
        assert_eq!(
            warned.summary.warnings,
            vec!["eligible:object_code".to_string()]
        );

        let skipped = OfflineFormContextFactory::from_plain_rows(
            vec![("eligible.0".to_string(), "broken".to_string())],
            None,
        )
        .unwrap();
        assert_eq!(skipped.summary.input_rows, 0);
        assert_eq!(skipped.summary.metadata_rows, 0);
        assert!(skipped.summary.warnings.is_empty());
    }

    #[test]
    fn offline_context_matches_non_empty_production_form_owner_builders() {
        let owner_uuid = "11111111-1111-4111-8111-111111111111";
        let form_uuid = "44444444-4444-4444-8444-444444444441";
        let form_list_marker = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let owner_text = format!(
            "{{1,{{33,{{3,{{1,0,{owner_uuid}}},\"ExecutorTask\",{{1,\"en\",\"Executor task\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,0,0,0,0}},{{{form_list_marker},1,{form_uuid}}}}}"
        );
        let form_text = format!(
            "{{1,{{14,{{3,{{1,0,{form_uuid}}},\"ChoiceForm\",{{1,\"en\",\"Choice form\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,1,{{2,{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}},{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}},0}},0}}"
        );
        let input = vec![
            (form_uuid.to_string(), form_text.clone()),
            (owner_uuid.to_string(), owner_text.clone()),
        ];
        let context =
            OfflineFormContextFactory::from_plain_rows(input.clone(), Some("fixture".into()))
                .unwrap();
        let reversed = OfflineFormContextFactory::from_plain_rows(
            input.into_iter().rev(),
            Some("fixture".into()),
        )
        .unwrap();

        let mut direct_rows = vec![
            metadata_text_row_from_text(owner_uuid, owner_text).unwrap(),
            metadata_text_row_from_text(form_uuid, form_text).unwrap(),
        ];
        direct_rows.iter_mut().for_each(|row| {
            normalize_direct_form_metadata(row);
        });
        let direct_types = build_metadata_type_indexes_from_texts(&direct_rows);
        assert_eq!(context.type_index, direct_types.references);
        assert_eq!(context.dcs_type_index, direct_types.dcs);
        assert_eq!(
            context.object_refs,
            build_metadata_object_reference_index_from_texts(&direct_rows)
        );
        assert_eq!(
            context
                .form_owner_references
                .get(form_uuid)
                .map(String::as_str),
            Some("Task.ExecutorTask")
        );
        assert_eq!(context.summary.sha256, reversed.summary.sha256);
    }

    #[test]
    fn saved_run_skips_dotted_binary_before_utf8_decode() {
        let root =
            std::env::temp_dir().join(format!("ibcmd-offline-context-{}", uuid::Uuid::new_v4()));
        let dump = root.join("candidate_dump");
        let inflated = dump.join("Config_inflated");
        fs::create_dir_all(&inflated).unwrap();
        fs::write(inflated.join("metadata.txt"), "broken").unwrap();
        fs::write(inflated.join("body.txt"), [0xff, 0xfe, 0xfd]).unwrap();
        let manifest = serde_json::json!({
            "tables": [{
                "rows": [
                    {
                        "file_name": "eligible",
                        "inflated_path": "Config_inflated/metadata.txt"
                    },
                    {
                        "file_name": "eligible.1",
                        "inflated_path": "Config_inflated/body.txt"
                    }
                ]
            }]
        });
        fs::write(
            dump.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let context = OfflineFormContextFactory::from_run_root(&root, None).unwrap();
        assert_eq!(context.summary.input_rows, 1);
        assert_eq!(context.summary.metadata_rows, 1);
        assert_eq!(context.summary.warnings.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
