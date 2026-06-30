use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
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

#[derive(Debug, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Upsert,
    Delete,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::source::{SourceFile, SourceKind, SourceManifest};

    use super::{ActionKind, SourceDiffStatus, build_load_plan, build_source_diff};

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
}
