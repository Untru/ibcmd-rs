//! A source tree against the active Config of a database: which descriptors
//! differ, and is each difference a restructuring?
//!
//! The check on rows ([`super::check_staged`]) judges what the ConfigSave
//! holds. It cannot see a change the stage did not carry: the patch-mode
//! import stages the synonyms and module texts of a tree but drops an added
//! attribute, and a ConfigSave without the attribute looks like a
//! configuration that never had it. This mode judges the tree itself, before
//! or instead of any staging: every metadata file is compared with the
//! descriptor the database holds for the same uuid.
//!
//! The database side is exported with the metadata model: the text an export
//! of the database would write. When the tree's file is that text there is
//! nothing to compare; otherwise both sides are parsed and the differences
//! go through the rules of the other modes, so a tree exported from the
//! database compares as unchanged and a hand-edited one reports exactly its
//! edits.
//!
//! Only descriptors are compared. The files that carry stored data (the
//! predefined items, the flowchart of a business process, the content of an
//! exchange plan, aggregates, additional indexes) are counted in
//! `body_files_not_compared`, not judged: compare an export of the database
//! with the tree ([`super::check_trees`]) for them, or check the ConfigSave
//! the import produced.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::metadata_model::export::tree_version;
use crate::metadata_model::xml::{Element, MetadataXml};
use crate::sql::SqlExec;

use super::check::{Inputs, Labels, RowProvider, plan_problems};
use super::descriptor::{self, ObjectRef};
use super::model::{ObjectOp, Reason, ReasonClass, Verdict};
use super::plan::{self, Decoder, Plan};
use super::roles::{Effect, RowName, file_role, parse_row_name};
use super::rule_id::RuleId;
use super::sql::active_of;
use super::trees::full_name;

/// How much of a file the scan reads to find out whether it is a metadata
/// file and which object it holds: the namespaces of `<MetaDataObject>`
/// take about 1.5 KB.
const HEAD_BYTES: usize = 8192;

/// A metadata file of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TreeObject {
    pub rel: String,
    /// The element's name: `Catalog`, `Form`, ...
    pub kind: String,
    pub uuid: String,
}

/// The tree, listed.
#[derive(Debug, Default)]
pub(super) struct TreeScan {
    /// Files of the tree (`ConfigDumpInfo.xml` aside).
    pub files: usize,
    pub objects: Vec<TreeObject>,
    /// Metadata files whose head does not say which object they hold.
    pub unreadable: Vec<(String, String)>,
    /// Files of roles that only copy rows, counted by role.
    pub safe_bodies: BTreeMap<String, usize>,
    /// Files of roles that carry data, or of no known role: not compared.
    pub not_compared: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Head {
    NotMetadata,
    Object { kind: String, uuid: String },
    Unreadable(String),
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// What the head of an `.xml` file says: not a metadata file, the object it
/// holds (`<MetaDataObject ...><Catalog uuid="...">`), or that it cannot
/// tell.
fn read_head(head: &[u8]) -> Head {
    let Some(start) = find(head, b"<MetaDataObject") else {
        return Head::NotMetadata;
    };
    let rest = &head[start..];
    let Some(end) = rest.iter().position(|byte| *byte == b'>') else {
        return Head::Unreadable("the head of the file ends inside <MetaDataObject>".to_string());
    };
    let after = &rest[end + 1..];
    let Some(open) = after.iter().position(|byte| *byte == b'<') else {
        return Head::Unreadable("<MetaDataObject> holds no object".to_string());
    };
    let tag = &after[open + 1..];
    let Some(close) = tag.iter().position(|byte| *byte == b'>') else {
        return Head::Unreadable(
            "the object element's tag is not in the head of the file".to_string(),
        );
    };
    let tag = String::from_utf8_lossy(&tag[..close]).into_owned();
    let mut parts = tag.trim_end_matches('/').split_whitespace();
    let Some(kind) = parts.next() else {
        return Head::Unreadable("the object element has no name".to_string());
    };
    let Some(position) = tag.find("uuid=\"") else {
        return Head::Unreadable("the object element has no uuid".to_string());
    };
    let value = &tag[position + "uuid=\"".len()..];
    let Some(quote) = value.find('"') else {
        return Head::Unreadable("the object's uuid is not closed".to_string());
    };
    Head::Object {
        kind: kind.to_string(),
        uuid: value[..quote].to_ascii_lowercase(),
    }
}

fn read_head_of(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut head = vec![0u8; HEAD_BYTES];
    let mut filled = 0;
    while filled < head.len() {
        let read = file.read(&mut head[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    head.truncate(filled);
    Ok(head)
}

/// Lists a tree: its metadata files with the object each holds, and its
/// other files by role.
pub(super) fn scan(root: &Path, partial: bool) -> Result<TreeScan> {
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    if !partial && !root.join("Configuration.xml").is_file() {
        bail!(
            "{} holds no Configuration.xml: it is not a configuration tree (--partial compares \
             the objects a partial tree holds)",
            root.display()
        );
    }
    let mut rels = Vec::<String>::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        // The dump info is derived from the tree, never a change of its own.
        if rel == "ConfigDumpInfo.xml" {
            continue;
        }
        rels.push(rel);
    }
    let mut out = TreeScan {
        files: rels.len(),
        ..TreeScan::default()
    };
    let heads = crate::parallel::install_io_bound(|| {
        rels.par_iter()
            .filter(|rel| rel.ends_with(".xml"))
            .map(|rel| {
                let head = read_head_of(&root.join(rel))
                    .map(|head| read_head(&head))
                    .unwrap_or_else(|error| Head::Unreadable(error.to_string()));
                (rel.clone(), head)
            })
            .collect::<Vec<_>>()
    })?;
    let mut metadata = HashSet::<String>::new();
    for (rel, head) in heads {
        match head {
            Head::NotMetadata => {}
            Head::Object { kind, uuid } => {
                metadata.insert(rel.clone());
                out.objects.push(TreeObject { rel, kind, uuid });
            }
            Head::Unreadable(why) => {
                metadata.insert(rel.clone());
                out.unreadable.push((rel, why));
            }
        }
    }
    for rel in &rels {
        if metadata.contains(rel) {
            continue;
        }
        match file_role(rel) {
            Some(role) if role.effect == Effect::Safe => {
                *out.safe_bodies.entry(role.name.to_string()).or_default() += 1;
            }
            _ => out.not_compared.push(rel.clone()),
        }
    }
    out.objects.sort_by(|left, right| left.rel.cmp(&right.rel));
    out.unreadable.sort();
    out.not_compared.sort();
    Ok(out)
}

/// The text of the file an export of a stored descriptor would write.
pub(super) trait Export: Sync {
    fn export_row(&self, kind: &str, row: &[u8]) -> Result<String>;
}

impl Export for Decoder {
    fn export_row(&self, kind: &str, row: &[u8]) -> Result<String> {
        self.export(kind, row)
    }
}

/// What comparing one metadata file with the database found.
enum Outcome {
    Same,
    Added,
    Changed {
        kind: String,
        before: Element,
        after: Element,
    },
    KindChanged {
        old_kind: String,
    },
    Failed {
        kind: String,
        error: String,
    },
}

fn parse_object(bytes: &[u8]) -> Result<Element> {
    let doc = MetadataXml::parse(bytes)?;
    Ok(doc.object()?.clone())
}

fn judge(
    object: &TreeObject,
    inputs: &Inputs,
    plan: &Plan,
    export: &dyn Export,
    read: &(dyn Fn(&str) -> Result<Vec<u8>> + Sync),
) -> Outcome {
    let Some(row) = inputs.old_descriptors.get(&object.uuid) else {
        return Outcome::Added;
    };
    let Some(kind) = plan.kinds.get(&object.uuid).copied() else {
        return Outcome::Failed {
            kind: String::new(),
            error: "the database's descriptors do not say what kind of object this is".to_string(),
        };
    };
    let failed = |error: anyhow::Error| Outcome::Failed {
        kind: kind.to_string(),
        error: format!("{error:#}"),
    };
    let expected = match export.export_row(kind, row) {
        Ok(text) => text,
        Err(error) => return failed(error.context("the database's descriptor cannot be exported")),
    };
    let tree = match read(&object.rel) {
        Ok(bytes) => bytes,
        Err(error) => return failed(error.context("the file cannot be read")),
    };
    if tree == expected.as_bytes() {
        return Outcome::Same;
    }
    let before = match parse_object(expected.as_bytes()) {
        Ok(element) => element,
        Err(error) => return failed(error.context("the database's descriptor cannot be parsed")),
    };
    let after = match parse_object(&tree) {
        Ok(element) => element,
        Err(error) => return failed(error.context("the file cannot be parsed")),
    };
    if before.name != after.name {
        return Outcome::KindChanged {
            old_kind: before.name,
        };
    }
    Outcome::Changed {
        kind: after.name.clone(),
        before,
        after,
    }
}

/// Compares a scanned tree with the descriptors of the plan. `partial`: the
/// tree holds some of the objects; the others are not removals.
#[allow(clippy::too_many_arguments)]
pub(super) fn compare_planned(
    inputs: &Inputs,
    plan: &Plan,
    labels: &Labels,
    export: &dyn Export,
    scan: &TreeScan,
    read: &(dyn Fn(&str) -> Result<Vec<u8>> + Sync),
    partial: bool,
    mut verdict: Verdict,
) -> Verdict {
    verdict.stats.old_files = inputs.old_inventory.len();
    verdict.stats.new_files = scan.files;
    verdict.stats.body_rows_by_role = scan.safe_bodies.clone();
    verdict.stats.body_files_not_compared = scan.not_compared.len();
    verdict.incomplete = !scan.not_compared.is_empty();

    for (rel, why) in &scan.unreadable {
        verdict.stats.unreadable += 1;
        let name = full_name(rel);
        verdict.push_reason(Reason::step(
            ReasonClass::Unknown,
            RuleId::TreeFileUnreadable,
            &name,
            rel,
            "",
            why,
        ));
        descriptor::unresolved(
            &ObjectRef {
                kind: "",
                name: &name,
                file_name: rel,
                id: rel,
            },
            ObjectOp::Changed,
            &mut verdict,
        );
    }

    // One file per object.
    let mut seen = HashMap::<&str, &TreeObject>::new();
    let mut objects = Vec::<&TreeObject>::new();
    for object in &scan.objects {
        if let Some(first) = seen.insert(object.uuid.as_str(), object) {
            verdict.push_reason(Reason::step(
                ReasonClass::Unknown,
                RuleId::SameObjectTwice,
                &full_name(&object.rel),
                &object.rel,
                "",
                &format!(
                    "the same object (uuid {}) is also in {}",
                    object.uuid, first.rel
                ),
            ));
            continue;
        }
        objects.push(object);
    }

    let outcomes = crate::parallel::install(|| {
        objects
            .par_iter()
            .map(|object| (*object, judge(object, inputs, plan, export, read)))
            .collect::<Vec<_>>()
    });
    let outcomes = match outcomes {
        Ok(outcomes) => outcomes,
        Err(error) => {
            verdict.push_reason(Reason::step(
                ReasonClass::Unknown,
                RuleId::ComparisonFailed,
                "Configuration",
                "",
                "",
                &format!("the comparison could not run: {error:#}"),
            ));
            return verdict;
        }
    };

    let mut differing = 0usize;
    for (object, outcome) in outcomes {
        let label = labels.of(&object.uuid);
        match outcome {
            Outcome::Same => {}
            Outcome::Added => {
                differing += 1;
                verdict.stats.added_files += 1;
                let name = full_name(&object.rel);
                descriptor::lifecycle(
                    &ObjectRef {
                        kind: &object.kind,
                        name: &name,
                        file_name: &object.rel,
                        id: &object.uuid,
                    },
                    true,
                    &mut verdict,
                );
            }
            Outcome::Changed {
                kind,
                before,
                after,
            } => {
                verdict.stats.descriptors_compared += 1;
                let count = descriptor::compare(
                    &ObjectRef {
                        kind: &kind,
                        name: &label,
                        file_name: &object.rel,
                        id: &object.uuid,
                    },
                    &before,
                    &after,
                    &mut verdict,
                );
                // Bytes differ but the trees do not: the file is written
                // differently (spacing, a byte-order mark), not changed.
                if count > 0 {
                    differing += 1;
                }
            }
            Outcome::KindChanged { old_kind } => {
                differing += 1;
                descriptor::kind_changed(
                    &ObjectRef {
                        kind: &object.kind,
                        name: &label,
                        file_name: &object.rel,
                        id: &object.uuid,
                    },
                    &old_kind,
                    &mut verdict,
                );
            }
            Outcome::Failed { kind, error } => {
                differing += 1;
                verdict.stats.unreadable += 1;
                verdict.push_reason(Reason {
                    kind: kind.clone(),
                    ..Reason::step(
                        ReasonClass::Unknown,
                        RuleId::RowUndecodable,
                        &label,
                        &object.rel,
                        "",
                        &error,
                    )
                });
                descriptor::unresolved(
                    &ObjectRef {
                        kind: &kind,
                        name: &label,
                        file_name: &object.rel,
                        id: &object.uuid,
                    },
                    ObjectOp::Changed,
                    &mut verdict,
                );
            }
        }
    }
    verdict.stats.staged_rows = differing;

    // Descriptors of the database that the tree does not hold.
    if !partial {
        let mut removed = inputs
            .old_inventory
            .iter()
            .filter(|name| matches!(parse_row_name(name), RowName::Descriptor(_)))
            .filter(|name| !seen.contains_key(name.as_str()))
            .collect::<Vec<_>>();
        removed.sort();
        for name in removed {
            verdict.stats.removed_files += 1;
            verdict.stats.staged_rows += 1;
            let label = labels.of(name);
            match plan.kinds.get(name.as_str()).copied() {
                Some(kind) => descriptor::lifecycle(
                    &ObjectRef {
                        kind,
                        name: &label,
                        file_name: name,
                        id: name,
                    },
                    false,
                    &mut verdict,
                ),
                None => {
                    verdict.push_reason(Reason::step(
                        ReasonClass::Unknown,
                        RuleId::RemovedDescriptorUnplaced,
                        &label,
                        name,
                        "",
                        "removed: a descriptor of a kind the check cannot place",
                    ));
                    descriptor::unresolved(
                        &ObjectRef {
                            kind: "",
                            name: &label,
                            file_name: name,
                            id: name,
                        },
                        ObjectOp::Removed,
                        &mut verdict,
                    );
                }
            }
        }
    }
    verdict
}

/// Plans the database's descriptors and compares the scanned tree with them.
pub(super) fn compare(
    inputs: &Inputs,
    rows: &dyn RowProvider,
    scan: &TreeScan,
    read: &(dyn Fn(&str) -> Result<Vec<u8>> + Sync),
    partial: bool,
) -> Verdict {
    let load = |names: &[String]| rows.old_rows(names).unwrap_or_default();
    let mut plan = plan::build(
        &inputs.old_descriptors,
        inputs.old_root.as_deref(),
        inputs.xml_version.as_deref(),
        &load,
    );
    let labels = Labels::new(&plan, &plan);
    let names = std::mem::take(&mut plan.names);
    // A stored row may be in the newer record format of a native apply.
    let decoder = Decoder::for_comparison(
        names,
        &plan.version,
        plan.compat,
        &[crate::metadata_model::objects::parts::Compat(8, 3, 27)],
    );
    let mut verdict = Verdict::new("tree-db");
    plan_problems(&plan, &plan, &mut verdict);
    compare_planned(
        inputs, &plan, &labels, &decoder, scan, read, partial, verdict,
    )
}

/// Reads the active Config of `database` and compares the source tree at
/// `tree` with it: which descriptors differ, and does each difference need
/// the platform's own apply. Read-only. `partial`: the tree holds some of
/// the objects; the others are not removals.
pub fn check_tree_against_db(
    sql: &SqlExec,
    database: &str,
    tree: &Path,
    xml_version: Option<&str>,
    partial: bool,
) -> Result<Verdict> {
    check_tree_against_db_with_source(sql, database, tree, xml_version, partial, None)
}

pub(crate) fn check_tree_against_db_with_source(
    sql: &SqlExec,
    database: &str,
    tree: &Path,
    xml_version: Option<&str>,
    partial: bool,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Verdict> {
    if let Some(source) = source {
        source.require_source_root(tree)?;
    }
    let scanned = match source.filter(|source| source.original_source().is_some()) {
        Some(source) => scan_original(source, partial)?,
        None => scan(tree, partial)?,
    };
    let version = match xml_version {
        Some(version) => Some(version.to_owned()),
        None => match source {
            Some(source) => source.tree_version()?,
            None => tree_version(tree),
        },
    };
    let active = active_of(sql, database, version.as_deref())?;
    let root = tree.to_path_buf();
    let read = move |rel: &str| -> Result<Vec<u8>> {
        let path = root.join(rel);
        match source {
            Some(source) => source
                .read_source(&path)
                .map(|bytes| bytes.into_vec())
                .map_err(Into::into),
            None => fs::read(&path).with_context(|| format!("failed to read {}", path.display())),
        }
    };
    let verdict = compare(&active.inputs, &active.rows, &scanned, &read, partial);
    if let Some(source) = source {
        source.require_original_unchanged()?;
    }
    Ok(verdict)
}

/// Complete original metadata bytes feed the existing XML identity model.
/// No 8 KiB prefix or swallowed I/O error can turn a real member into absence.
pub(super) fn scan_original(
    source: &crate::module_blob::MetadataSourceContext,
    partial: bool,
) -> Result<TreeScan> {
    let original = source
        .original_source()
        .ok_or_else(|| anyhow::anyhow!("original scan has no source owner"))?;
    original.require_unchanged()?;
    if !partial && original.member("Configuration.xml")?.is_none() {
        bail!("original source holds no Configuration.xml");
    }
    let mut out = TreeScan::default();
    for member in original.baseline().files() {
        let rel = member.path();
        if rel == "ConfigDumpInfo.xml" {
            continue;
        }
        out.files += 1;
        let head = if rel.ends_with(".xml") {
            let bytes = original.source_bytes(rel)?;
            read_head(&bytes)
        } else {
            Head::NotMetadata
        };
        match head {
            Head::Object { kind, uuid } => out.objects.push(TreeObject {
                rel: rel.to_owned(),
                kind,
                uuid,
            }),
            Head::Unreadable(why) => out.unreadable.push((rel.to_owned(), why)),
            Head::NotMetadata => match file_role(rel) {
                Some(role) if role.effect == Effect::Safe => {
                    *out.safe_bodies.entry(role.name.to_owned()).or_default() += 1;
                }
                _ => out.not_compared.push(rel.to_owned()),
            },
        }
    }
    out.unreadable.sort();
    out.objects.sort_by(|left, right| left.rel.cmp(&right.rel));
    out.not_compared.sort();
    original.require_unchanged()?;
    Ok(out)
}

#[cfg(test)]
#[path = "dbtree_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dbtree_original_tests.rs"]
mod original_tests;
