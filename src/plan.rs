use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde::{Deserialize, Serialize};

use crate::source::{SourceFile, SourceKind, SourceManifest, scan_sources_with_prefixes};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadPlan {
    pub summary: PlanSummary,
    pub actions: Vec<PlanAction>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub upsert: usize,
    pub delete: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAction {
    pub action: ActionKind,
    pub path: String,
    pub object_hint: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceDiffReport {
    pub left_root: String,
    pub right_root: String,
    pub summary: SourceDiffSummary,
    pub differences: Vec<SourceDiffEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceDiffSummary {
    pub left_only: usize,
    pub right_only: usize,
    pub different: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDiffEntry {
    pub status: SourceDiffStatus,
    pub path: String,
    pub left_sha256: Option<String>,
    pub right_sha256: Option<String>,
    pub left_size_bytes: Option<u64>,
    pub right_size_bytes: Option<u64>,
    pub kind: Option<SourceKind>,
    pub object_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceDiffSignatureReport {
    pub left_root: String,
    pub right_root: String,
    pub summary: SourceDiffSignatureSummary,
    pub sample_limits: Vec<SourceDiffKindLimit>,
    pub signatures: Vec<SourceDiffSignature>,
    pub errors: Vec<SourceDiffSignatureError>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SourceDiffSignatureSummary {
    pub diff_entries: usize,
    pub changed_xml_pairs: usize,
    pub sampled_xml_pairs: usize,
    pub skipped_non_xml_or_one_sided: usize,
    pub skipped_by_sample_limit: usize,
    pub parse_errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceDiffKindLimit {
    pub kind: String,
    pub max_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceDiffSignature {
    pub kind: String,
    pub signature: SourceDiffSignatureKind,
    pub path: String,
    pub count: usize,
    pub files: usize,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SourceDiffSignatureKind {
    LeftOnlyPath,
    RightOnlyPath,
    ValueOrAttrDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceDiffSignatureError {
    pub path: String,
    pub side: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceDiffExplainReport {
    pub left_root: String,
    pub right_root: String,
    pub path: String,
    pub left_file: String,
    pub right_file: String,
    pub summary: SourceDiffExplainSummary,
    pub differences: Vec<SourceDiffLeafDifference>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SourceDiffExplainSummary {
    pub total_differences: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub value_or_attr_diff: usize,
    pub shown_differences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceDiffLeafDifferenceKind {
    LeftOnly,
    RightOnly,
    ValueOrAttrDiff,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceDiffLeafDifference {
    pub kind: SourceDiffLeafDifferenceKind,
    pub path: String,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceDiffSignatureOptions {
    pub max_files_per_kind: Option<usize>,
    pub kind_limits: BTreeMap<String, usize>,
    pub top: Option<usize>,
    pub examples_per_signature: usize,
}

/// Versioned, per-file evidence for a native-versus-candidate source export.
/// Raw SHA-256 status is intentionally retained even when XML analysis succeeds.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ParityRun {
    pub database: String,
    pub run_id: String,
    pub git_sha: String,
    pub full: bool,
    pub left_root: String,
    pub right_root: String,
    pub raw_summary: SourceDiffSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ParityRow {
    pub database: String,
    pub run_id: String,
    pub path: String,
    pub family: String,
    pub artifact_class: String,
    pub raw_status: SourceDiffStatus,
    pub left_sha256: Option<String>,
    pub right_sha256: Option<String>,
    pub left_size_bytes: Option<u64>,
    pub right_size_bytes: Option<u64>,
    pub kind: Option<SourceKind>,
    pub object_hint: Option<String>,
    pub xml_signatures: Vec<SourceDiffSignature>,
    pub xml_signature_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ParityAggregate {
    pub database: String,
    pub run_id: String,
    pub family: String,
    pub artifact_class: String,
    pub raw_status: SourceDiffStatus,
    pub files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ParityMatrix {
    pub schema_version: u32,
    pub runs: Vec<ParityRun>,
    pub rows: Vec<ParityRow>,
    pub aggregates: Vec<ParityAggregate>,
}

pub const PARITY_MATRIX_SCHEMA_VERSION: u32 = 1;

/// Builds one matrix from an unmodified raw source diff. A scoped run is
/// deliberately represented, but can never report an exact readiness percent.
pub fn build_parity_matrix(
    diff: &SourceDiffReport,
    database: String,
    run_id: String,
    git_sha: String,
    full: bool,
) -> Result<ParityMatrix> {
    let run = ParityRun {
        database: database.clone(),
        run_id: run_id.clone(),
        git_sha,
        full,
        left_root: diff.left_root.clone(),
        right_root: diff.right_root.clone(),
        raw_summary: diff.summary.clone(),
    };
    let mut rows = diff
        .differences
        .iter()
        .map(|entry| parity_row_from_diff_entry(diff, entry, &database, &run_id))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.database
            .cmp(&right.database)
            .then_with(|| left.run_id.cmp(&right.run_id))
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut matrix = ParityMatrix {
        schema_version: PARITY_MATRIX_SCHEMA_VERSION,
        runs: vec![run],
        rows,
        aggregates: Vec::new(),
    };
    matrix.aggregates = parity_aggregates(&matrix.rows);
    validate_parity_matrix(&matrix)?;
    Ok(matrix)
}

pub fn merge_parity_matrices(matrices: &[ParityMatrix]) -> Result<ParityMatrix> {
    let mut runs = Vec::new();
    let mut rows = Vec::new();
    let mut run_keys = BTreeSet::new();
    for matrix in matrices {
        if matrix.schema_version != PARITY_MATRIX_SCHEMA_VERSION {
            return Err(anyhow!(
                "unsupported parity matrix schema version {}",
                matrix.schema_version
            ));
        }
        validate_parity_matrix(matrix)?;
        for run in &matrix.runs {
            let key = (run.database.clone(), run.run_id.clone());
            if !run_keys.insert(key) {
                return Err(anyhow!("duplicate parity matrix database/run-id"));
            }
            runs.push(run.clone());
        }
        rows.extend(matrix.rows.iter().cloned());
    }
    runs.sort_by(|left, right| {
        left.database
            .cmp(&right.database)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    rows.sort_by(|left, right| {
        left.database
            .cmp(&right.database)
            .then_with(|| left.run_id.cmp(&right.run_id))
            .then_with(|| left.path.cmp(&right.path))
    });
    let matrix = ParityMatrix {
        schema_version: PARITY_MATRIX_SCHEMA_VERSION,
        aggregates: parity_aggregates(&rows),
        runs,
        rows,
    };
    validate_parity_matrix(&matrix)?;
    Ok(matrix)
}

pub fn parity_exact_percent(run: &ParityRun) -> Option<f64> {
    if !run.full {
        return None;
    }
    let total = source_diff_total(&run.raw_summary);
    (total != 0).then_some(run.raw_summary.unchanged as f64 * 100.0 / total as f64)
}

pub fn validate_parity_matrix(matrix: &ParityMatrix) -> Result<()> {
    if matrix.schema_version != PARITY_MATRIX_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported parity matrix schema version {}",
            matrix.schema_version
        ));
    }
    let mut run_keys = BTreeSet::new();
    for run in &matrix.runs {
        if run.database.trim().is_empty() || run.run_id.trim().is_empty() {
            return Err(anyhow!(
                "parity matrix run database and run_id must not be empty"
            ));
        }
        if !run_keys.insert((run.database.clone(), run.run_id.clone())) {
            return Err(anyhow!("duplicate parity matrix database/run-id"));
        }
    }
    let mut rows_by_run = BTreeMap::<(String, String), SourceDiffSummary>::new();
    let mut row_paths = BTreeSet::new();
    for row in &matrix.rows {
        let key = (row.database.clone(), row.run_id.clone());
        if !run_keys.contains(&key) {
            return Err(anyhow!("parity matrix row references an unknown run"));
        }
        if !row_paths.insert((row.database.clone(), row.run_id.clone(), row.path.clone())) {
            return Err(anyhow!(
                "duplicate parity matrix path for {}/{}: {}",
                row.database,
                row.run_id,
                row.path
            ));
        }
        let summary = rows_by_run.entry(key).or_default();
        increment_source_diff_summary(summary, &row.raw_status);
    }
    for run in &matrix.runs {
        let actual = rows_by_run
            .remove(&(run.database.clone(), run.run_id.clone()))
            .unwrap_or_default();
        if actual != run.raw_summary {
            return Err(anyhow!(
                "parity matrix rows do not match raw source-diff summary for {}/{}",
                run.database,
                run.run_id
            ));
        }
    }
    let expected = parity_aggregates(&matrix.rows);
    if expected != matrix.aggregates {
        return Err(anyhow!("parity matrix aggregates do not match rows"));
    }
    Ok(())
}

pub fn write_parity_matrix(matrix: &ParityMatrix, output: &Path, overwrite: bool) -> Result<()> {
    write_parity_artifacts(matrix, output, None, overwrite)
}

pub fn read_parity_matrix(path: &Path) -> Result<ParityMatrix> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read parity matrix {}", path.display()))?;
    let matrix = serde_json::from_str::<ParityMatrix>(&text)
        .with_context(|| format!("failed to parse parity matrix {}", path.display()))?;
    validate_parity_matrix(&matrix)?;
    Ok(matrix)
}

pub fn render_parity_matrix_markdown(matrix: &ParityMatrix) -> Result<String> {
    validate_parity_matrix(matrix)?;
    let mut markdown = String::from("# Матрица побайтовой совместимости\n\n");
    markdown.push_str(
        "| База | Запуск | Охват | Совпало | Всего | Готовность |\n|---|---|---|---:|---:|---:|\n",
    );
    for run in &matrix.runs {
        let total = source_diff_total(&run.raw_summary);
        let readiness = parity_exact_percent(run)
            .map(|value| format!("{value:.4}%"))
            .unwrap_or_else(|| "н/д (выборка)".to_string());
        let scope = if run.full {
            "полный"
        } else {
            "выборочный"
        };
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            markdown_escape(&run.database),
            markdown_escape(&run.run_id),
            scope,
            run.raw_summary.unchanged,
            total,
            readiness
        ));
    }
    markdown.push_str(
        "\n## Итоги по семействам\n\n| База | Запуск | Семейство | Совпало | Изменено | Только слева | Только справа | Всего |\n|---|---|---|---:|---:|---:|---:|---:|\n",
    );
    let mut family_summaries = BTreeMap::<(String, String, String), SourceDiffSummary>::new();
    for row in &matrix.rows {
        increment_source_diff_summary(
            family_summaries
                .entry((row.database.clone(), row.run_id.clone(), row.family.clone()))
                .or_default(),
            &row.raw_status,
        );
    }
    for ((database, run_id, family), summary) in family_summaries {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            markdown_escape(&database),
            markdown_escape(&run_id),
            markdown_escape(&family),
            summary.unchanged,
            summary.different,
            summary.left_only,
            summary.right_only,
            source_diff_total(&summary)
        ));
    }
    markdown.push_str(
        "\n## Расхождения\n\n| База | Запуск | Семейство | Класс | Статус | Путь |\n|---|---|---|---|---|---|\n",
    );
    for row in matrix
        .rows
        .iter()
        .filter(|row| row.raw_status != SourceDiffStatus::Unchanged)
    {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {:?} | {} |\n",
            markdown_escape(&row.database),
            markdown_escape(&row.run_id),
            markdown_escape(&row.family),
            markdown_escape(&row.artifact_class),
            row.raw_status,
            markdown_escape(&row.path)
        ));
    }
    Ok(markdown)
}

pub fn write_parity_matrix_markdown(
    matrix: &ParityMatrix,
    output: &Path,
    overwrite: bool,
) -> Result<()> {
    validate_parity_matrix(matrix)?;
    let markdown = render_parity_matrix_markdown(matrix)?;
    publish_artifact_set(
        vec![ArtifactBytes {
            target: output,
            bytes: markdown.as_bytes(),
            validate_json: false,
        }],
        overwrite,
    )
}

/// Validates and stages the JSON and optional Markdown before either target is
/// published. On an ordinary I/O error, both old targets are restored.
pub fn write_parity_artifacts(
    matrix: &ParityMatrix,
    json_output: &Path,
    markdown_output: Option<&Path>,
    overwrite: bool,
) -> Result<()> {
    validate_parity_matrix(matrix)?;
    let json = serde_json::to_string_pretty(matrix)?;
    let markdown = markdown_output
        .map(|_| render_parity_matrix_markdown(matrix))
        .transpose()?;
    let mut artifacts = vec![ArtifactBytes {
        target: json_output,
        bytes: json.as_bytes(),
        validate_json: true,
    }];
    if let (Some(target), Some(bytes)) = (markdown_output, markdown.as_deref()) {
        artifacts.push(ArtifactBytes {
            target,
            bytes: bytes.as_bytes(),
            validate_json: false,
        });
    }
    publish_artifact_set(artifacts, overwrite)
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Upsert,
    Delete,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum SourceDiffStatus {
    LeftOnly,
    RightOnly,
    Different,
    Unchanged,
}

pub fn build_load_plan(baseline: Option<&SourceManifest>, current: &SourceManifest) -> LoadPlan {
    let Some(baseline) = baseline else {
        let mut actions = current
            .files
            .iter()
            .map(|file| PlanAction {
                action: ActionKind::Upsert,
                path: file.path.clone(),
                object_hint: file.object_hint.clone(),
                reason: "no baseline manifest".to_string(),
            })
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| left.path.cmp(&right.path));
        return summarize(actions);
    };

    let baseline_by_path = by_path(&baseline.files);
    let current_by_path = by_path(&current.files);
    let all_paths = baseline_by_path
        .keys()
        .chain(current_by_path.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut actions = Vec::new();
    for path in all_paths {
        match (baseline_by_path.get(&path), current_by_path.get(&path)) {
            (None, Some(current_file)) => actions.push(PlanAction {
                action: ActionKind::Upsert,
                path,
                object_hint: current_file.object_hint.clone(),
                reason: "new file".to_string(),
            }),
            (Some(baseline_file), Some(current_file))
                if baseline_file.sha256 != current_file.sha256 =>
            {
                actions.push(PlanAction {
                    action: ActionKind::Upsert,
                    path,
                    object_hint: current_file.object_hint.clone(),
                    reason: "content hash changed".to_string(),
                })
            }
            (Some(_), Some(current_file)) => actions.push(PlanAction {
                action: ActionKind::Unchanged,
                path,
                object_hint: current_file.object_hint.clone(),
                reason: "same content hash".to_string(),
            }),
            (Some(baseline_file), None) => actions.push(PlanAction {
                action: ActionKind::Delete,
                path,
                object_hint: baseline_file.object_hint.clone(),
                reason: "file removed".to_string(),
            }),
            (None, None) => unreachable!("path union cannot produce an empty match"),
        }
    }

    summarize(actions)
}

pub fn write_plan(plan: &LoadPlan, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(plan)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn diff_source_trees(
    left_root: &Path,
    right_root: &Path,
    prefixes: &[String],
) -> Result<SourceDiffReport> {
    let left = scan_sources_with_prefixes(left_root, prefixes)
        .with_context(|| format!("failed to scan left source tree {}", left_root.display()))?;
    let right = scan_sources_with_prefixes(right_root, prefixes)
        .with_context(|| format!("failed to scan right source tree {}", right_root.display()))?;
    Ok(build_source_diff(&left, &right))
}

pub fn build_source_diff(left: &SourceManifest, right: &SourceManifest) -> SourceDiffReport {
    let left_by_path = by_path(&left.files);
    let right_by_path = by_path(&right.files);
    let all_paths = left_by_path
        .keys()
        .chain(right_by_path.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut differences = Vec::new();
    for path in all_paths {
        let entry = match (left_by_path.get(&path), right_by_path.get(&path)) {
            (Some(left_file), Some(right_file)) if left_file.sha256 == right_file.sha256 => {
                SourceDiffEntry {
                    status: SourceDiffStatus::Unchanged,
                    path,
                    left_sha256: Some(left_file.sha256.clone()),
                    right_sha256: Some(right_file.sha256.clone()),
                    left_size_bytes: Some(left_file.size_bytes),
                    right_size_bytes: Some(right_file.size_bytes),
                    kind: Some(left_file.kind.clone()),
                    object_hint: left_file
                        .object_hint
                        .clone()
                        .or_else(|| right_file.object_hint.clone()),
                }
            }
            (Some(left_file), Some(right_file)) => SourceDiffEntry {
                status: SourceDiffStatus::Different,
                path,
                left_sha256: Some(left_file.sha256.clone()),
                right_sha256: Some(right_file.sha256.clone()),
                left_size_bytes: Some(left_file.size_bytes),
                right_size_bytes: Some(right_file.size_bytes),
                kind: Some(left_file.kind.clone()),
                object_hint: left_file
                    .object_hint
                    .clone()
                    .or_else(|| right_file.object_hint.clone()),
            },
            (Some(left_file), None) => SourceDiffEntry {
                status: SourceDiffStatus::LeftOnly,
                path,
                left_sha256: Some(left_file.sha256.clone()),
                right_sha256: None,
                left_size_bytes: Some(left_file.size_bytes),
                right_size_bytes: None,
                kind: Some(left_file.kind.clone()),
                object_hint: left_file.object_hint.clone(),
            },
            (None, Some(right_file)) => SourceDiffEntry {
                status: SourceDiffStatus::RightOnly,
                path,
                left_sha256: None,
                right_sha256: Some(right_file.sha256.clone()),
                left_size_bytes: None,
                right_size_bytes: Some(right_file.size_bytes),
                kind: Some(right_file.kind.clone()),
                object_hint: right_file.object_hint.clone(),
            },
            (None, None) => unreachable!("path union cannot produce an empty match"),
        };
        differences.push(entry);
    }

    let mut summary = SourceDiffSummary::default();
    for entry in &differences {
        match entry.status {
            SourceDiffStatus::LeftOnly => summary.left_only += 1,
            SourceDiffStatus::RightOnly => summary.right_only += 1,
            SourceDiffStatus::Different => summary.different += 1,
            SourceDiffStatus::Unchanged => summary.unchanged += 1,
        }
    }

    SourceDiffReport {
        left_root: left.root.display().to_string(),
        right_root: right.root.display().to_string(),
        summary,
        differences,
    }
}

pub fn write_source_diff(report: &SourceDiffReport, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn read_source_diff(path: &Path) -> Result<SourceDiffReport> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read source diff {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse source diff {}", path.display()))
}

pub fn explain_source_diff_file(
    left_root: &Path,
    right_root: &Path,
    relative_path: &str,
    leaf_path_prefixes: &[String],
    limit: Option<usize>,
) -> Result<SourceDiffExplainReport> {
    let left_path = source_tree_path(left_root, relative_path);
    let right_path = source_tree_path(right_root, relative_path);
    let left_values = read_indexed_xml_values(&left_path)?;
    let right_values = read_indexed_xml_values(&right_path)?;
    let mut differences = diff_indexed_xml_values(&left_values, &right_values, leaf_path_prefixes);

    let mut summary = SourceDiffExplainSummary {
        total_differences: differences.len(),
        ..SourceDiffExplainSummary::default()
    };
    for difference in &differences {
        match difference.kind {
            SourceDiffLeafDifferenceKind::LeftOnly => summary.left_only += 1,
            SourceDiffLeafDifferenceKind::RightOnly => summary.right_only += 1,
            SourceDiffLeafDifferenceKind::ValueOrAttrDiff => summary.value_or_attr_diff += 1,
        }
    }

    if let Some(limit) = limit {
        differences.truncate(limit);
    }
    summary.shown_differences = differences.len();

    Ok(SourceDiffExplainReport {
        left_root: left_root.display().to_string(),
        right_root: right_root.display().to_string(),
        path: relative_path.to_string(),
        left_file: left_path.display().to_string(),
        right_file: right_path.display().to_string(),
        summary,
        differences,
    })
}

pub fn build_source_diff_signature_report(
    diff: &SourceDiffReport,
    options: &SourceDiffSignatureOptions,
) -> SourceDiffSignatureReport {
    let left_root = Path::new(&diff.left_root);
    let right_root = Path::new(&diff.right_root);
    let mut summary = SourceDiffSignatureSummary {
        diff_entries: diff.differences.len(),
        ..SourceDiffSignatureSummary::default()
    };
    let mut sampled_by_kind = BTreeMap::<String, usize>::new();
    let mut accumulators = BTreeMap::<SignatureKey, SignatureAccumulator>::new();
    let mut errors = Vec::new();

    for entry in &diff.differences {
        let Some(kind) = entry.kind.as_ref() else {
            summary.skipped_non_xml_or_one_sided += 1;
            continue;
        };
        if entry.status != SourceDiffStatus::Different || !is_xml_source_kind(kind) {
            summary.skipped_non_xml_or_one_sided += 1;
            continue;
        }

        summary.changed_xml_pairs += 1;
        let kind_name = source_kind_name(kind).to_string();
        if let Some(limit) = sample_limit_for_kind(&kind_name, options) {
            let sampled = sampled_by_kind.get(&kind_name).copied().unwrap_or(0);
            if sampled >= limit {
                summary.skipped_by_sample_limit += 1;
                continue;
            }
        }

        *sampled_by_kind.entry(kind_name.clone()).or_insert(0) += 1;
        summary.sampled_xml_pairs += 1;

        let left_path = source_tree_path(left_root, &entry.path);
        let right_path = source_tree_path(right_root, &entry.path);
        let left_shape = match read_xml_path_shape(&left_path) {
            Ok(shape) => shape,
            Err(error) => {
                summary.parse_errors += 1;
                errors.push(SourceDiffSignatureError {
                    path: entry.path.clone(),
                    side: "left".to_string(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        let right_shape = match read_xml_path_shape(&right_path) {
            Ok(shape) => shape,
            Err(error) => {
                summary.parse_errors += 1;
                errors.push(SourceDiffSignatureError {
                    path: entry.path.clone(),
                    side: "right".to_string(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        accumulate_xml_shape_diff(
            &kind_name,
            &entry.path,
            &left_shape,
            &right_shape,
            options.examples_per_signature,
            &mut accumulators,
        );
    }

    let mut signatures = accumulators
        .into_iter()
        .map(|(key, value)| SourceDiffSignature {
            kind: key.kind,
            signature: key.signature,
            path: key.path,
            count: value.count,
            files: value.files.len(),
            examples: value.examples,
        })
        .collect::<Vec<_>>();
    signatures.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.files.cmp(&left.files))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.signature.cmp(&right.signature))
            .then_with(|| left.path.cmp(&right.path))
    });
    if let Some(top) = options.top {
        signatures.truncate(top);
    }

    SourceDiffSignatureReport {
        left_root: diff.left_root.clone(),
        right_root: diff.right_root.clone(),
        summary,
        sample_limits: sample_limits(options),
        signatures,
        errors,
    }
}

pub fn write_source_diff_signature_report(
    report: &SourceDiffSignatureReport,
    output: &Path,
) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn write_source_diff_explain_report(
    report: &SourceDiffExplainReport,
    output: &Path,
) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| {
        format!(
            "failed to write source diff explain report {}",
            output.display()
        )
    })
}

fn by_path(files: &[SourceFile]) -> BTreeMap<String, &SourceFile> {
    files.iter().map(|file| (file.path.clone(), file)).collect()
}

fn summarize(actions: Vec<PlanAction>) -> LoadPlan {
    let mut summary = PlanSummary::default();
    for action in &actions {
        match action.action {
            ActionKind::Upsert => summary.upsert += 1,
            ActionKind::Delete => summary.delete += 1,
            ActionKind::Unchanged => summary.unchanged += 1,
        }
    }

    LoadPlan { summary, actions }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SignatureKey {
    kind: String,
    signature: SourceDiffSignatureKind,
    path: String,
}

#[derive(Debug, Default)]
struct SignatureAccumulator {
    count: usize,
    files: BTreeSet<String>,
    examples: Vec<String>,
}

#[derive(Debug, Default)]
struct XmlPathShape {
    paths: BTreeMap<String, PathShape>,
}

#[derive(Debug, Default)]
struct PathShape {
    count: usize,
    fingerprints: BTreeMap<String, usize>,
}

fn source_tree_path(root: &Path, relative: &str) -> std::path::PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn is_xml_source_kind(kind: &SourceKind) -> bool {
    matches!(
        kind,
        SourceKind::ConfigurationRoot
            | SourceKind::MetadataXml
            | SourceKind::Form
            | SourceKind::Template
            | SourceKind::OtherXml
    )
}

fn source_kind_name(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::ConfigurationRoot => "configuration_root",
        SourceKind::MetadataXml => "metadata_xml",
        SourceKind::Module => "module",
        SourceKind::Form => "form",
        SourceKind::Template => "template",
        SourceKind::Binary => "binary",
        SourceKind::OtherXml => "other_xml",
        SourceKind::Other => "other",
    }
}

fn parity_row_from_diff_entry(
    diff: &SourceDiffReport,
    entry: &SourceDiffEntry,
    database: &str,
    run_id: &str,
) -> ParityRow {
    let (family, artifact_class) = classify_parity_path(&entry.path, entry.kind.as_ref());
    let (xml_signatures, xml_signature_error) = match xml_signatures_for_diff_entry(diff, entry) {
        Ok(signatures) => (signatures, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };
    ParityRow {
        database: database.to_string(),
        run_id: run_id.to_string(),
        path: entry.path.clone(),
        family,
        artifact_class,
        raw_status: entry.status.clone(),
        left_sha256: entry.left_sha256.clone(),
        right_sha256: entry.right_sha256.clone(),
        left_size_bytes: entry.left_size_bytes,
        right_size_bytes: entry.right_size_bytes,
        kind: entry.kind.clone(),
        object_hint: entry.object_hint.clone(),
        xml_signatures,
        xml_signature_error,
    }
}

/// Stable path classification used only for reporting; it never changes raw diff status.
pub fn classify_parity_path(path: &str, kind: Option<&SourceKind>) -> (String, String) {
    let normalized = path.replace('\\', "/");
    let components = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let ext_index = components
        .iter()
        .position(|part| part.eq_ignore_ascii_case("Ext"));
    let family = match components.first() {
        Some(part) if part.eq_ignore_ascii_case("Ext") => "Configuration".to_string(),
        Some(part) => (*part).to_string(),
        None => "Unknown".to_string(),
    };
    let direct_ext_name = ext_index
        .filter(|index| index + 2 == components.len())
        .and_then(|index| components.get(index + 1));
    let artifact_class =
        if direct_ext_name.is_some_and(|name| name.eq_ignore_ascii_case("Form.xml")) {
            "form_body"
        } else if direct_ext_name.is_some_and(|name| name.eq_ignore_ascii_case("Template.xml")) {
            "template_body"
        } else if ext_index.is_some() {
            "ext_asset"
        } else if components
            .last()
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".xml"))
        {
            match kind {
                Some(SourceKind::ConfigurationRoot) => "configuration_root",
                Some(SourceKind::MetadataXml) | None => "metadata_xml",
                Some(SourceKind::OtherXml) => "other_xml",
                _ => "xml",
            }
        } else {
            match kind {
                Some(SourceKind::Module) => "module",
                Some(SourceKind::Binary) => "binary",
                Some(SourceKind::Other) | None => "other",
                _ => "other",
            }
        };
    (family, artifact_class.to_string())
}

/// Returns per-file XML signatures without changing the existing aggregate
/// `source-diff-signatures` wire format.
pub fn xml_signatures_for_diff_entry(
    diff: &SourceDiffReport,
    entry: &SourceDiffEntry,
) -> Result<Vec<SourceDiffSignature>> {
    let Some(kind) = entry.kind.as_ref() else {
        return Ok(Vec::new());
    };
    if entry.status != SourceDiffStatus::Different || !is_xml_source_kind(kind) {
        return Ok(Vec::new());
    }
    let left_shape =
        read_xml_path_shape(&source_tree_path(Path::new(&diff.left_root), &entry.path))?;
    let right_shape =
        read_xml_path_shape(&source_tree_path(Path::new(&diff.right_root), &entry.path))?;
    let mut accumulators = BTreeMap::<SignatureKey, SignatureAccumulator>::new();
    let kind_name = source_kind_name(kind);
    accumulate_xml_shape_diff(
        kind_name,
        &entry.path,
        &left_shape,
        &right_shape,
        1,
        &mut accumulators,
    );
    let mut signatures = accumulators
        .into_iter()
        .map(|(key, value)| SourceDiffSignature {
            kind: key.kind,
            signature: key.signature,
            path: key.path,
            count: value.count,
            files: value.files.len(),
            examples: value.examples,
        })
        .collect::<Vec<_>>();
    signatures.sort_by(|left, right| {
        left.signature
            .cmp(&right.signature)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(signatures)
}

fn parity_aggregates(rows: &[ParityRow]) -> Vec<ParityAggregate> {
    let mut counts = BTreeMap::<(String, String, String, String, SourceDiffStatus), usize>::new();
    for row in rows {
        *counts
            .entry((
                row.database.clone(),
                row.run_id.clone(),
                row.family.clone(),
                row.artifact_class.clone(),
                row.raw_status.clone(),
            ))
            .or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(
            |((database, run_id, family, artifact_class, raw_status), files)| ParityAggregate {
                database,
                run_id,
                family,
                artifact_class,
                raw_status,
                files,
            },
        )
        .collect()
}

fn increment_source_diff_summary(summary: &mut SourceDiffSummary, status: &SourceDiffStatus) {
    match status {
        SourceDiffStatus::LeftOnly => summary.left_only += 1,
        SourceDiffStatus::RightOnly => summary.right_only += 1,
        SourceDiffStatus::Different => summary.different += 1,
        SourceDiffStatus::Unchanged => summary.unchanged += 1,
    }
}

fn source_diff_total(summary: &SourceDiffSummary) -> usize {
    summary.left_only + summary.right_only + summary.different + summary.unchanged
}

struct ArtifactBytes<'a> {
    target: &'a Path,
    bytes: &'a [u8],
    validate_json: bool,
}

struct StagedArtifact {
    target: std::path::PathBuf,
    temporary: std::path::PathBuf,
}

static ARTIFACT_NONCE: AtomicU64 = AtomicU64::new(0);

fn publish_artifact_set(artifacts: Vec<ArtifactBytes<'_>>, overwrite: bool) -> Result<()> {
    preflight_artifact_set(&artifacts, overwrite)?;
    let mut staged = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        match stage_artifact(artifact) {
            Ok(value) => staged.push(value),
            Err(error) => {
                cleanup_staged_artifacts(&staged);
                return Err(error);
            }
        }
    }
    let result = if overwrite {
        publish_staged_with_backups(&mut staged)
    } else {
        publish_staged_without_overwrite(&mut staged)
    };
    cleanup_staged_artifacts(&staged);
    result
}

fn preflight_artifact_set(artifacts: &[ArtifactBytes<'_>], overwrite: bool) -> Result<()> {
    let mut targets = BTreeSet::new();
    for artifact in artifacts {
        let identity = publication_path_identity(artifact.target)?;
        if !targets.insert(identity) {
            return Err(anyhow!(
                "parity artifact outputs must be different paths: {}",
                artifact.target.display()
            ));
        }
        if artifact.target.is_dir() {
            return Err(anyhow!(
                "parity artifact target is a directory: {}",
                artifact.target.display()
            ));
        }
        if !overwrite && artifact.target.try_exists()? {
            return Err(anyhow!(
                "refusing to overwrite existing parity artifact {}",
                artifact.target.display()
            ));
        }
    }
    Ok(())
}

fn publication_path_identity(path: &Path) -> Result<String> {
    let absolute = std::path::absolute(path)
        .with_context(|| format!("failed to resolve output path {}", path.display()))?;
    let identity = absolute.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let identity = identity.to_ascii_lowercase();
    Ok(identity)
}

fn stage_artifact(artifact: ArtifactBytes<'_>) -> Result<StagedArtifact> {
    let parent = artifact.target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let temporary = create_unique_sibling(artifact.target, "tmp")?;
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        file.write_all(artifact.bytes)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to flush {}", temporary.display()))?;
        drop(file);
        if artifact.validate_json {
            let staged = read_parity_matrix(&temporary)?;
            let expected: ParityMatrix = serde_json::from_slice(artifact.bytes)?;
            if staged != expected {
                return Err(anyhow!("staged parity matrix did not round-trip exactly"));
            }
        } else if fs::read(&temporary)? != artifact.bytes {
            return Err(anyhow!("staged parity artifact did not round-trip exactly"));
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(StagedArtifact {
        target: artifact.target.to_path_buf(),
        temporary,
    })
}

fn create_unique_sibling(target: &Path, suffix: &str) -> Result<std::path::PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("parity-artifact");
    for _ in 0..32 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let nonce = ARTIFACT_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{name}.{}.{}.{}.{}",
            std::process::id(),
            timestamp,
            nonce,
            suffix
        ));
        if !candidate.try_exists()? {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "failed to allocate a unique sibling for {}",
        target.display()
    ))
}

fn publish_staged_without_overwrite(staged: &mut [StagedArtifact]) -> Result<()> {
    let mut published = Vec::new();
    for artifact in staged.iter() {
        if let Err(error) = fs::hard_link(&artifact.temporary, &artifact.target) {
            for target in published.iter().rev() {
                let _ = fs::remove_file(target);
            }
            return Err(error).with_context(|| {
                format!(
                    "failed to publish parity artifact without overwrite {}",
                    artifact.target.display()
                )
            });
        }
        published.push(artifact.target.clone());
    }
    for artifact in staged.iter() {
        fs::remove_file(&artifact.temporary)
            .with_context(|| format!("failed to remove {}", artifact.temporary.display()))?;
    }
    Ok(())
}

fn publish_staged_with_backups(staged: &mut [StagedArtifact]) -> Result<()> {
    let mut backups = Vec::<(std::path::PathBuf, std::path::PathBuf)>::new();
    for artifact in staged.iter() {
        let target_exists = match artifact.target.try_exists() {
            Ok(value) => value,
            Err(error) => {
                restore_backups(&backups)?;
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", artifact.target.display()));
            }
        };
        if target_exists {
            let backup = match create_unique_sibling(&artifact.target, "backup") {
                Ok(value) => value,
                Err(error) => {
                    restore_backups(&backups)?;
                    return Err(error);
                }
            };
            if let Err(error) = fs::rename(&artifact.target, &backup) {
                restore_backups(&backups)?;
                return Err(error)
                    .with_context(|| format!("failed to preserve {}", artifact.target.display()));
            }
            backups.push((artifact.target.clone(), backup));
        }
    }

    let mut published = Vec::new();
    for artifact in staged.iter() {
        if let Err(error) = fs::hard_link(&artifact.temporary, &artifact.target) {
            for target in published.iter().rev() {
                let _ = fs::remove_file(target);
            }
            restore_backups(&backups)?;
            return Err(error)
                .with_context(|| format!("failed to publish {}", artifact.target.display()));
        }
        published.push(artifact.target.clone());
    }
    for artifact in staged.iter() {
        fs::remove_file(&artifact.temporary)
            .with_context(|| format!("failed to remove {}", artifact.temporary.display()))?;
    }
    for (_, backup) in backups {
        fs::remove_file(&backup)
            .with_context(|| format!("failed to remove backup {}", backup.display()))?;
    }
    Ok(())
}

fn restore_backups(backups: &[(std::path::PathBuf, std::path::PathBuf)]) -> Result<()> {
    let mut first_error = None;
    for (target, backup) in backups.iter().rev() {
        if let Err(error) = fs::rename(backup, target)
            && first_error.is_none()
        {
            first_error = Some(anyhow!(error).context(format!(
                "failed to restore preserved artifact {}",
                target.display()
            )));
        }
    }
    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn cleanup_staged_artifacts(staged: &[StagedArtifact]) {
    for artifact in staged {
        let _ = fs::remove_file(&artifact.temporary);
    }
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
        .replace('\r', "")
}

fn sample_limit_for_kind(kind: &str, options: &SourceDiffSignatureOptions) -> Option<usize> {
    options
        .kind_limits
        .get(kind)
        .copied()
        .or(options.max_files_per_kind)
}

fn sample_limits(options: &SourceDiffSignatureOptions) -> Vec<SourceDiffKindLimit> {
    let mut limits = BTreeMap::<String, usize>::new();
    if let Some(limit) = options.max_files_per_kind {
        for kind in [
            "configuration_root",
            "metadata_xml",
            "form",
            "template",
            "other_xml",
        ] {
            limits.insert(kind.to_string(), limit);
        }
    }
    for (kind, limit) in &options.kind_limits {
        limits.insert(kind.clone(), *limit);
    }
    limits
        .into_iter()
        .map(|(kind, max_files)| SourceDiffKindLimit { kind, max_files })
        .collect()
}

fn read_xml_path_shape(path: &Path) -> Result<XmlPathShape> {
    let xml = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_xml_path_shape(&xml).with_context(|| format!("failed to parse {}", path.display()))
}

fn read_indexed_xml_values(path: &Path) -> Result<BTreeMap<String, String>> {
    let xml = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_indexed_xml_values(&xml).with_context(|| format!("failed to parse {}", path.display()))
}

fn parse_xml_path_shape(xml: &[u8]) -> Result<XmlPathShape> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut shape = XmlPathShape::default();
    let mut stack = Vec::<String>::new();
    let mut values = Vec::<ElementValue>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name(&event);
                stack.push(name);
                values.push(ElementValue::new(attributes_fingerprint(&event)?));
            }
            Ok(Event::Empty(event)) => {
                stack.push(local_name(&event));
                record_xml_path(&stack, attributes_fingerprint(&event)?, &mut shape);
                stack.pop();
            }
            Ok(Event::Text(text)) => {
                if let Some(value) = values.last_mut() {
                    value.text.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(value) = values.last_mut() {
                    value.text.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(_)) => {
                let value = values.pop().unwrap_or_default();
                record_xml_path(&stack, value.fingerprint(), &mut shape);
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(anyhow!("invalid XML: {error}")),
            _ => {}
        }
    }
    Ok(shape)
}

fn parse_indexed_xml_values(xml: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut values = BTreeMap::<String, String>::new();
    let mut path = Vec::<String>::new();
    let mut sibling_counts = vec![BTreeMap::<String, usize>::new()];
    let mut text_stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let segment = next_indexed_segment(&mut sibling_counts, local_name(&event));
                path.push(segment);
                let current = path.join("/");
                for attribute in event.attributes() {
                    let attribute = attribute?;
                    let name =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).to_string();
                    let value = attribute.unescape_value()?.into_owned();
                    values.insert(format!("{current}/@{name}"), value);
                }
                sibling_counts.push(BTreeMap::new());
                text_stack.push(String::new());
            }
            Ok(Event::Empty(event)) => {
                let segment = next_indexed_segment(&mut sibling_counts, local_name(&event));
                path.push(segment);
                let current = path.join("/");
                let mut had_attribute = false;
                for attribute in event.attributes() {
                    let attribute = attribute?;
                    let name =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).to_string();
                    let value = attribute.unescape_value()?.into_owned();
                    values.insert(format!("{current}/@{name}"), value);
                    had_attribute = true;
                }
                if !had_attribute {
                    values.insert(current, String::new());
                }
                path.pop();
            }
            Ok(Event::Text(text)) => {
                if let Some(current) = text_stack.last_mut() {
                    current.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(current) = text_stack.last_mut() {
                    current.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(_)) => {
                let text = text_stack.pop().unwrap_or_default();
                if !text.trim().is_empty() && !path.is_empty() {
                    values.insert(path.join("/"), text.trim().to_string());
                }
                sibling_counts.pop();
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(anyhow!("invalid XML: {error}")),
            _ => {}
        }
    }

    Ok(values)
}

#[derive(Debug, Default)]
struct ElementValue {
    attributes: String,
    text: String,
}

impl ElementValue {
    fn new(attributes: String) -> Self {
        Self {
            attributes,
            text: String::new(),
        }
    }

    fn fingerprint(self) -> String {
        let text = self.text.trim();
        if self.attributes.is_empty() {
            text.to_string()
        } else if text.is_empty() {
            format!("@{}", self.attributes)
        } else {
            format!("@{}|{}", self.attributes, text)
        }
    }
}

fn record_xml_path(stack: &[String], fingerprint: String, shape: &mut XmlPathShape) {
    if stack.is_empty() {
        return;
    }
    let path = stack.join("/");
    let entry = shape.paths.entry(path).or_default();
    entry.count += 1;
    *entry.fingerprints.entry(fingerprint).or_insert(0) += 1;
}

fn next_indexed_segment(
    sibling_counts: &mut [BTreeMap<String, usize>],
    local_name: String,
) -> String {
    let count = sibling_counts
        .last_mut()
        .expect("indexed XML path stack must have a current level")
        .entry(local_name.clone())
        .and_modify(|count| *count += 1)
        .or_insert(1);
    format!("{local_name}[{count}]")
}

fn local_name(event: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(event.local_name().as_ref()).to_string()
}

fn attributes_fingerprint(event: &BytesStart<'_>) -> Result<String> {
    let mut attributes = Vec::<String>::new();
    for attribute in event.attributes() {
        let attribute = attribute?;
        let name = String::from_utf8_lossy(attribute.key.local_name().as_ref()).to_string();
        let value = attribute.unescape_value()?.into_owned();
        attributes.push(format!("{name}={value}"));
    }
    attributes.sort();
    Ok(attributes.join("|"))
}

fn diff_indexed_xml_values(
    left: &BTreeMap<String, String>,
    right: &BTreeMap<String, String>,
    leaf_path_prefixes: &[String],
) -> Vec<SourceDiffLeafDifference> {
    let mut paths = left.keys().chain(right.keys()).cloned().collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| {
            leaf_path_prefixes.is_empty()
                || leaf_path_prefixes
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
        })
        .filter_map(|path| match (left.get(&path), right.get(&path)) {
            (Some(left_value), Some(right_value)) if left_value == right_value => None,
            (Some(left_value), Some(right_value)) => Some(SourceDiffLeafDifference {
                kind: SourceDiffLeafDifferenceKind::ValueOrAttrDiff,
                path,
                left: Some(left_value.clone()),
                right: Some(right_value.clone()),
            }),
            (Some(left_value), None) => Some(SourceDiffLeafDifference {
                kind: SourceDiffLeafDifferenceKind::LeftOnly,
                path,
                left: Some(left_value.clone()),
                right: None,
            }),
            (None, Some(right_value)) => Some(SourceDiffLeafDifference {
                kind: SourceDiffLeafDifferenceKind::RightOnly,
                path,
                left: None,
                right: Some(right_value.clone()),
            }),
            (None, None) => None,
        })
        .collect()
}

fn accumulate_xml_shape_diff(
    kind: &str,
    file: &str,
    left: &XmlPathShape,
    right: &XmlPathShape,
    examples_per_signature: usize,
    accumulators: &mut BTreeMap<SignatureKey, SignatureAccumulator>,
) {
    let all_paths = left
        .paths
        .keys()
        .chain(right.paths.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for path in all_paths {
        let left_shape = left.paths.get(&path);
        let right_shape = right.paths.get(&path);
        let left_count = left_shape.map(|shape| shape.count).unwrap_or(0);
        let right_count = right_shape.map(|shape| shape.count).unwrap_or(0);
        if left_count > right_count {
            push_signature(
                kind,
                SourceDiffSignatureKind::LeftOnlyPath,
                &path,
                left_count - right_count,
                file,
                examples_per_signature,
                accumulators,
            );
        } else if right_count > left_count {
            push_signature(
                kind,
                SourceDiffSignatureKind::RightOnlyPath,
                &path,
                right_count - left_count,
                file,
                examples_per_signature,
                accumulators,
            );
        }

        let common = left_count.min(right_count);
        if common == 0 {
            continue;
        }
        let exact_matches = matching_fingerprint_count(
            &left_shape
                .expect("left shape exists for common count")
                .fingerprints,
            &right_shape
                .expect("right shape exists for common count")
                .fingerprints,
        );
        if common > exact_matches {
            push_signature(
                kind,
                SourceDiffSignatureKind::ValueOrAttrDiff,
                &path,
                common - exact_matches,
                file,
                examples_per_signature,
                accumulators,
            );
        }
    }
}

fn matching_fingerprint_count(
    left: &BTreeMap<String, usize>,
    right: &BTreeMap<String, usize>,
) -> usize {
    left.iter()
        .map(|(fingerprint, left_count)| {
            right
                .get(fingerprint)
                .copied()
                .unwrap_or(0)
                .min(*left_count)
        })
        .sum()
}

fn push_signature(
    kind: &str,
    signature: SourceDiffSignatureKind,
    path: &str,
    count: usize,
    file: &str,
    examples_per_signature: usize,
    accumulators: &mut BTreeMap<SignatureKey, SignatureAccumulator>,
) {
    let key = SignatureKey {
        kind: kind.to_string(),
        signature,
        path: path.to_string(),
    };
    let accumulator = accumulators.entry(key).or_default();
    accumulator.count += count;
    accumulator.files.insert(file.to_string());
    if accumulator.examples.len() < examples_per_signature
        && !accumulator.examples.iter().any(|example| example == file)
    {
        accumulator.examples.push(file.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::source::{SourceFile, SourceKind, SourceManifest};

    use super::{
        ActionKind, SourceDiffEntry, SourceDiffLeafDifference, SourceDiffLeafDifferenceKind,
        SourceDiffReport, SourceDiffSignatureKind, SourceDiffSignatureOptions, SourceDiffStatus,
        SourceDiffSummary, build_load_plan, build_source_diff, build_source_diff_signature_report,
        explain_source_diff_file, parse_indexed_xml_values, parse_xml_path_shape,
    };

    #[test]
    fn detects_added_changed_deleted_and_unchanged_files() {
        let baseline = manifest(vec![
            file("delete.xml", "1"),
            file("same.xml", "2"),
            file("change.xml", "3"),
        ]);
        let current = manifest(vec![
            file("same.xml", "2"),
            file("change.xml", "4"),
            file("new.xml", "5"),
        ]);

        let plan = build_load_plan(Some(&baseline), &current);

        assert_eq!(plan.summary.delete, 1);
        assert_eq!(plan.summary.upsert, 2);
        assert_eq!(plan.summary.unchanged, 1);
        assert!(
            plan.actions
                .iter()
                .any(|item| item.path == "delete.xml" && item.action == ActionKind::Delete)
        );
        assert!(
            plan.actions
                .iter()
                .any(|item| item.path == "change.xml" && item.action == ActionKind::Upsert)
        );
        assert!(
            plan.actions
                .iter()
                .any(|item| item.path == "same.xml" && item.action == ActionKind::Unchanged)
        );
    }

    #[test]
    fn diffs_source_manifests_by_path_and_hash() {
        let left = manifest(vec![
            file("left-only.xml", "1"),
            file("same.xml", "2"),
            file("different.xml", "3"),
        ]);
        let right = manifest(vec![
            file("same.xml", "2"),
            file("different.xml", "4"),
            file("right-only.xml", "5"),
        ]);

        let diff = build_source_diff(&left, &right);

        assert_eq!(diff.summary.left_only, 1);
        assert_eq!(diff.summary.right_only, 1);
        assert_eq!(diff.summary.different, 1);
        assert_eq!(diff.summary.unchanged, 1);
        assert!(diff.differences.iter().any(|item| {
            item.path == "left-only.xml" && item.status == SourceDiffStatus::LeftOnly
        }));
        assert!(diff.differences.iter().any(|item| {
            item.path == "right-only.xml" && item.status == SourceDiffStatus::RightOnly
        }));
        assert!(diff.differences.iter().any(|item| {
            item.path == "different.xml"
                && item.status == SourceDiffStatus::Different
                && item.left_sha256.as_deref() == Some("3")
                && item.right_sha256.as_deref() == Some("4")
        }));
    }

    #[test]
    fn summarizes_xml_diff_signatures_with_per_kind_sampling() -> anyhow::Result<()> {
        let root = temp_root("ibcmd-rs-source-diff-signatures");
        let left = root.join("left");
        let right = root.join("right");
        fs::create_dir_all(left.join("CommonForms/One/Ext"))?;
        fs::create_dir_all(right.join("CommonForms/One/Ext"))?;
        fs::create_dir_all(left.join("CommonForms/Two/Ext"))?;
        fs::create_dir_all(right.join("CommonForms/Two/Ext"))?;
        fs::write(
            left.join("CommonForms/One/Ext/Form.xml"),
            r#"<Form><Group>Usual</Group><Item name="Name">A</Item></Form>"#,
        )?;
        fs::write(
            right.join("CommonForms/One/Ext/Form.xml"),
            r#"<Form><WindowOpeningMode>Auto</WindowOpeningMode><Item name="Name">B</Item></Form>"#,
        )?;
        fs::write(
            left.join("CommonForms/Two/Ext/Form.xml"),
            r#"<Form><WouldBeParsed>left</WouldBeParsed></Form>"#,
        )?;
        fs::write(
            right.join("CommonForms/Two/Ext/Form.xml"),
            r#"<Form><WouldBeParsed>right</WouldBeParsed></Form>"#,
        )?;

        let diff = SourceDiffReport {
            left_root: left.display().to_string(),
            right_root: right.display().to_string(),
            summary: SourceDiffSummary {
                different: 3,
                ..SourceDiffSummary::default()
            },
            differences: vec![
                diff_entry("CommonForms/One/Ext/Form.xml", SourceKind::Form),
                diff_entry("CommonForms/Two/Ext/Form.xml", SourceKind::Form),
                diff_entry("CommonModules/Module.bsl", SourceKind::Module),
            ],
        };
        let options = SourceDiffSignatureOptions {
            max_files_per_kind: Some(1),
            kind_limits: BTreeMap::new(),
            top: Some(20),
            examples_per_signature: 1,
        };

        let report = build_source_diff_signature_report(&diff, &options);

        assert_eq!(report.summary.diff_entries, 3);
        assert_eq!(report.summary.changed_xml_pairs, 2);
        assert_eq!(report.summary.sampled_xml_pairs, 1);
        assert_eq!(report.summary.skipped_by_sample_limit, 1);
        assert_eq!(report.summary.skipped_non_xml_or_one_sided, 1);
        assert_eq!(report.summary.parse_errors, 0);
        assert!(report.signatures.contains(&signature(
            "form",
            SourceDiffSignatureKind::LeftOnlyPath,
            "Form/Group",
            1,
            1,
            vec!["CommonForms/One/Ext/Form.xml"],
        )));
        assert!(report.signatures.contains(&signature(
            "form",
            SourceDiffSignatureKind::RightOnlyPath,
            "Form/WindowOpeningMode",
            1,
            1,
            vec!["CommonForms/One/Ext/Form.xml"],
        )));
        assert!(report.signatures.contains(&signature(
            "form",
            SourceDiffSignatureKind::ValueOrAttrDiff,
            "Form/Item",
            1,
            1,
            vec!["CommonForms/One/Ext/Form.xml"],
        )));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn xml_path_shape_counts_missing_paths_and_value_diffs() -> anyhow::Result<()> {
        let left = parse_xml_path_shape(
            br#"<root><a code="x">1</a><a code="x">1</a><a code="x">2</a><b/></root>"#,
        )?;
        let right =
            parse_xml_path_shape(br#"<root><a code="x">1</a><a code="x">3</a><c/></root>"#)?;
        let mut accumulators = BTreeMap::new();

        super::accumulate_xml_shape_diff(
            "other_xml",
            "sample.xml",
            &left,
            &right,
            2,
            &mut accumulators,
        );

        let signatures = accumulators
            .into_iter()
            .map(|(key, value)| {
                (
                    key.signature,
                    key.path,
                    value.count,
                    value.files.len(),
                    value.examples,
                )
            })
            .collect::<Vec<_>>();
        assert!(signatures.contains(&(
            SourceDiffSignatureKind::LeftOnlyPath,
            "root/a".to_string(),
            1,
            1,
            vec!["sample.xml".to_string()],
        )));
        assert!(signatures.contains(&(
            SourceDiffSignatureKind::ValueOrAttrDiff,
            "root/a".to_string(),
            1,
            1,
            vec!["sample.xml".to_string()],
        )));
        assert!(signatures.contains(&(
            SourceDiffSignatureKind::LeftOnlyPath,
            "root/b".to_string(),
            1,
            1,
            vec!["sample.xml".to_string()],
        )));
        assert!(signatures.contains(&(
            SourceDiffSignatureKind::RightOnlyPath,
            "root/c".to_string(),
            1,
            1,
            vec!["sample.xml".to_string()],
        )));
        Ok(())
    }

    #[test]
    fn parses_indexed_xml_values_for_repeated_siblings() -> anyhow::Result<()> {
        let values = parse_indexed_xml_values(
            br#"<Form><Commands><Command name="A"><Representation>Picture</Representation></Command><Command name="B"><Representation>TextPicture</Representation></Command></Commands></Form>"#,
        )?;

        assert_eq!(
            values.get("Form[1]/Commands[1]/Command[1]/@name"),
            Some(&"A".to_string())
        );
        assert_eq!(
            values.get("Form[1]/Commands[1]/Command[1]/Representation[1]"),
            Some(&"Picture".to_string())
        );
        assert_eq!(
            values.get("Form[1]/Commands[1]/Command[2]/Representation[1]"),
            Some(&"TextPicture".to_string())
        );
        Ok(())
    }

    #[test]
    fn explains_exact_indexed_xml_file_differences() -> anyhow::Result<()> {
        let root = temp_root("ibcmd-rs-source-diff-explain");
        let left = root.join("left");
        let right = root.join("right");
        let relative = "DataProcessors/Test/Forms/Форма/Ext/Form.xml";
        let left_path = left.join("DataProcessors/Test/Forms/Форма/Ext");
        let right_path = right.join("DataProcessors/Test/Forms/Форма/Ext");
        fs::create_dir_all(&left_path)?;
        fs::create_dir_all(&right_path)?;
        fs::write(
            left_path.join("Form.xml"),
            r#"<Form><Commands><Command name="A"><Representation>Picture</Representation></Command><Command name="B"><Representation>Text</Representation></Command></Commands></Form>"#,
        )?;
        fs::write(
            right_path.join("Form.xml"),
            r#"<Form><Commands><Command name="A"><Representation>Picture</Representation></Command><Command name="B"><Representation>TextPicture</Representation><ToolTip>Go</ToolTip></Command></Commands></Form>"#,
        )?;

        let report = explain_source_diff_file(
            &left,
            &right,
            relative,
            &["Form[1]/Commands[1]/Command[2]".to_string()],
            None,
        )?;

        assert_eq!(report.summary.total_differences, 2);
        assert_eq!(report.summary.left_only, 0);
        assert_eq!(report.summary.right_only, 1);
        assert_eq!(report.summary.value_or_attr_diff, 1);
        assert_eq!(
            report.differences,
            vec![
                SourceDiffLeafDifference {
                    kind: SourceDiffLeafDifferenceKind::ValueOrAttrDiff,
                    path: "Form[1]/Commands[1]/Command[2]/Representation[1]".to_string(),
                    left: Some("Text".to_string()),
                    right: Some("TextPicture".to_string()),
                },
                SourceDiffLeafDifference {
                    kind: SourceDiffLeafDifferenceKind::RightOnly,
                    path: "Form[1]/Commands[1]/Command[2]/ToolTip[1]".to_string(),
                    left: None,
                    right: Some("Go".to_string()),
                },
            ]
        );

        fs::remove_dir_all(root)?;
        Ok(())
    }

    fn manifest(files: Vec<SourceFile>) -> SourceManifest {
        SourceManifest {
            root: PathBuf::from("root"),
            generated_at_unix: 0,
            files,
        }
    }

    fn file(path: &str, sha: &str) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            size_bytes: 1,
            sha256: sha.to_string(),
            kind: SourceKind::OtherXml,
            xml_root: None,
            object_hint: None,
        }
    }

    fn diff_entry(path: &str, kind: SourceKind) -> SourceDiffEntry {
        SourceDiffEntry {
            status: SourceDiffStatus::Different,
            path: path.to_string(),
            left_sha256: Some("left".to_string()),
            right_sha256: Some("right".to_string()),
            left_size_bytes: Some(1),
            right_size_bytes: Some(1),
            kind: Some(kind),
            object_hint: None,
        }
    }

    fn signature(
        kind: &str,
        signature: SourceDiffSignatureKind,
        path: &str,
        count: usize,
        files: usize,
        examples: Vec<&str>,
    ) -> super::SourceDiffSignature {
        super::SourceDiffSignature {
            kind: kind.to_string(),
            signature,
            path: path.to_string(),
            count,
            files,
            examples: examples.into_iter().map(str::to_string).collect(),
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{suffix}", std::process::id()))
    }
}
