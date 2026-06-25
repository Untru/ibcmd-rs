use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::source::{SourceFile, SourceManifest};

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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Upsert,
    Delete,
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

    use super::{ActionKind, build_load_plan};

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
