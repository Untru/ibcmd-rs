use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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

#[derive(Debug, Clone, Default)]
pub struct SourceDiffSignatureOptions {
    pub max_files_per_kind: Option<usize>,
    pub kind_limits: BTreeMap<String, usize>,
    pub top: Option<usize>,
    pub examples_per_signature: usize,
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

pub fn read_source_diff(path: &Path) -> Result<SourceDiffReport> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read source diff {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse source diff {}", path.display()))
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
        ActionKind, SourceDiffEntry, SourceDiffReport, SourceDiffSignatureKind,
        SourceDiffSignatureOptions, SourceDiffStatus, SourceDiffSummary, build_load_plan,
        build_source_diff, build_source_diff_signature_report, parse_xml_path_shape,
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
