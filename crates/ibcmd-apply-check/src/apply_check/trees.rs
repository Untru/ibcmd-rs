//! The check on two XML source trees: the tree of the configuration as it is
//! (an export of the database) against the tree that would replace it.
//!
//! It shares the descriptor rules with the check on rows. A metadata XML is
//! compared with its counterpart of the other tree by object (its `uuid`),
//! not by file name, so a moved object is one object; the other files
//! (modules, forms, templates, help, pictures, ...) are told apart by where
//! they lie under the object's `Ext` folder.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::metadata_model::index::kind_of_collection;
use crate::metadata_model::xml::{Element, MetadataXml, parse_element_tree};

use super::check::items_relation;
use super::descriptor::{self, ObjectRef};
use super::model::{Note, ObjectOp, Reason, ReasonClass, Verdict};
use super::roles::{Effect, file_role};
use super::rule_id::RuleId;

/// A file of a tree.
struct Entry {
    path: PathBuf,
    size: u64,
}

fn list(root: &Path) -> Result<BTreeMap<String, Entry>> {
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let mut out = BTreeMap::new();
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
        let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        out.insert(
            rel,
            Entry {
                path: entry.path().to_path_buf(),
                size,
            },
        );
    }
    Ok(out)
}

/// `Catalogs/X.xml` -> `Catalog.X`; `Catalogs/X/Forms/F.xml` ->
/// `Catalog.X.Form.F`; `Configuration.xml` -> `Configuration`.
pub fn full_name(rel: &str) -> String {
    let stem = rel.strip_suffix(".xml").unwrap_or(rel);
    if stem == "Configuration" {
        return "Configuration".to_string();
    }
    let mut out = Vec::<String>::new();
    for pair in stem.split('/').collect::<Vec<_>>().chunks(2) {
        match pair {
            [folder, name] => {
                out.push(kind_of_collection(folder).unwrap_or(folder).to_string());
                out.push((*name).to_string());
            }
            [single] => out.push((*single).to_string()),
            _ => {}
        }
    }
    out.join(".")
}

fn is_metadata_xml(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    head.windows(b"<MetaDataObject".len())
        .any(|window| window == b"<MetaDataObject")
}

struct Object {
    rel: String,
    uuid: String,
    kind: String,
    element: Element,
}

fn parse_object(rel: &str, bytes: &[u8]) -> Result<(String, Object)> {
    let doc = MetadataXml::parse(bytes).with_context(|| format!("failed to parse {rel}"))?;
    let element = doc.object()?.clone();
    let uuid = element
        .attr("uuid")
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| rel.to_string());
    Ok((
        uuid.clone(),
        Object {
            rel: rel.to_string(),
            uuid,
            kind: element.name.clone(),
            element,
        },
    ))
}

enum Diff {
    Same,
    Changed { old: Vec<u8>, new: Vec<u8> },
    OnlyOld(Vec<u8>),
    OnlyNew(Vec<u8>),
}

fn compare_file(old: Option<&Entry>, new: Option<&Entry>) -> Result<Diff> {
    match (old, new) {
        // The same file on both sides (a tree against itself).
        (Some(old), Some(new)) if old.path == new.path => Ok(Diff::Same),
        (Some(old), Some(new)) => {
            let old_bytes = fs::read(&old.path)
                .with_context(|| format!("failed to read {}", old.path.display()))?;
            if old.size == new.size {
                let new_bytes = fs::read(&new.path)
                    .with_context(|| format!("failed to read {}", new.path.display()))?;
                if old_bytes == new_bytes {
                    return Ok(Diff::Same);
                }
                return Ok(Diff::Changed {
                    old: old_bytes,
                    new: new_bytes,
                });
            }
            let new_bytes = fs::read(&new.path)
                .with_context(|| format!("failed to read {}", new.path.display()))?;
            Ok(Diff::Changed {
                old: old_bytes,
                new: new_bytes,
            })
        }
        (Some(old), None) => {
            Ok(Diff::OnlyOld(fs::read(&old.path).with_context(|| {
                format!("failed to read {}", old.path.display())
            })?))
        }
        (None, Some(new)) => {
            Ok(Diff::OnlyNew(fs::read(&new.path).with_context(|| {
                format!("failed to read {}", new.path.display())
            })?))
        }
        (None, None) => Ok(Diff::Same),
    }
}

/// Compares two trees.
pub fn check_trees(old_root: &Path, new_root: &Path) -> Result<Verdict> {
    let old = list(old_root)?;
    let new = list(new_root)?;
    let mut verdict = Verdict::new("trees");
    verdict.stats.old_files = old.len();
    verdict.stats.new_files = new.len();

    let names = old
        .keys()
        .chain(new.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let names = names.into_iter().collect::<Vec<_>>();
    let diffs = crate::parallel::install(|| {
        names
            .par_iter()
            .map(|rel| (rel.clone(), compare_file(old.get(rel), new.get(rel))))
            .filter(|(_, diff)| !matches!(diff, Ok(Diff::Same)))
            .collect::<Vec<_>>()
    })?;
    verdict.stats.staged_rows = diffs.len();

    let mut old_only = HashMap::<String, Object>::new();
    let mut new_only = HashMap::<String, Object>::new();
    let mut files = Vec::<(String, Diff)>::new();
    for (rel, diff) in diffs {
        let diff = match diff {
            Ok(diff) => diff,
            Err(error) => {
                verdict.stats.unreadable += 1;
                verdict.push_reason(Reason::step(
                    ReasonClass::Unknown,
                    RuleId::TreeFileUnreadable,
                    &full_name(&rel),
                    &rel,
                    "",
                    &format!("{error:#}"),
                ));
                continue;
            }
        };
        files.push((rel, diff));
    }

    // Descriptors first: the pairs by object, then the objects on one side.
    let mut bodies = Vec::<(String, Diff)>::new();
    for (rel, diff) in files {
        let is_descriptor = rel.ends_with(".xml")
            && match &diff {
                Diff::Changed { old, new } => is_metadata_xml(old) && is_metadata_xml(new),
                Diff::OnlyOld(bytes) | Diff::OnlyNew(bytes) => is_metadata_xml(bytes),
                Diff::Same => false,
            };
        if !is_descriptor {
            bodies.push((rel, diff));
            continue;
        }
        match diff {
            Diff::Changed { old, new } => {
                verdict.stats.descriptors_compared += 1;
                match (parse_object(&rel, &old), parse_object(&rel, &new)) {
                    (Ok((_, before)), Ok((_, after))) => {
                        compare_pair(&rel, &before, &after, &mut verdict)
                    }
                    (Err(error), _) | (_, Err(error)) => unreadable(&rel, &error, &mut verdict),
                }
            }
            Diff::OnlyOld(bytes) => match parse_object(&rel, &bytes) {
                Ok((uuid, object)) => {
                    old_only.insert(uuid, object);
                }
                Err(error) => unreadable(&rel, &error, &mut verdict),
            },
            Diff::OnlyNew(bytes) => match parse_object(&rel, &bytes) {
                Ok((uuid, object)) => {
                    new_only.insert(uuid, object);
                }
                Err(error) => unreadable(&rel, &error, &mut verdict),
            },
            Diff::Same => {}
        }
    }
    // A moved or renamed file is the same object.
    let moved = old_only
        .keys()
        .filter(|uuid| new_only.contains_key(*uuid))
        .cloned()
        .collect::<Vec<_>>();
    for uuid in moved {
        let (Some(before), Some(after)) = (old_only.remove(&uuid), new_only.remove(&uuid)) else {
            continue;
        };
        verdict.stats.descriptors_compared += 1;
        compare_pair(&after.rel, &before, &after, &mut verdict);
    }
    for object in old_only.into_values() {
        verdict.stats.removed_files += 1;
        let name = full_name(&object.rel);
        descriptor::lifecycle(
            &ObjectRef {
                kind: &object.kind,
                name: &name,
                file_name: &object.rel,
                id: &object.uuid,
            },
            false,
            &mut verdict,
        );
    }
    for object in new_only.into_values() {
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

    // Everything else.
    for (rel, diff) in bodies {
        let what = match &diff {
            Diff::Changed { .. } => "changed",
            Diff::OnlyOld(_) => {
                verdict.stats.removed_files += 1;
                "removed"
            }
            Diff::OnlyNew(_) => {
                verdict.stats.added_files += 1;
                "added"
            }
            Diff::Same => continue,
        };
        let owner = body_owner(&rel);
        match file_role(&rel) {
            Some(role) if role.effect == Effect::Safe => {
                *verdict
                    .stats
                    .body_rows_by_role
                    .entry(role.name.to_string())
                    .or_default() += 1;
            }
            Some(role)
                if role.name == "Content"
                    && matches!(&diff, Diff::Changed { old, new } if content_relation(old, new).is_some()) =>
            {
                verdict.stats.body_rows_compared += 1;
                let change = match &diff {
                    Diff::Changed { old, new } => content_relation(old, new).unwrap_or_default(),
                    _ => "",
                };
                verdict.push_note(Note {
                    object: owner,
                    file_name: rel,
                    property: role.name.to_string(),
                    change: change.to_string(),
                });
            }
            Some(role) => {
                verdict.stats.body_rows_compared += 1;
                let class = if role.effect == Effect::Data {
                    ReasonClass::Data
                } else {
                    ReasonClass::Structure
                };
                let rule = match &diff {
                    Diff::Changed { .. } => RuleId::BodyContentChanged,
                    Diff::OnlyOld(_) => RuleId::BodyRowRemoved,
                    _ => RuleId::BodyRowAdded,
                };
                verdict.push_reason(Reason::step(class, rule, &owner, &rel, role.name, what));
            }
            None => verdict.push_reason(Reason::step(
                ReasonClass::Unknown,
                RuleId::TreeFileUnknown,
                &owner,
                &rel,
                "",
                &format!("{what}: a file the check does not know"),
            )),
        }
    }
    Ok(verdict)
}

/// The items of an exchange plan's `Content.xml` (`Metadata`, `AutoRecord`),
/// order aside; `None` when the file is not one.
#[allow(
    clippy::type_complexity,
    reason = "the two item lists of the file, each a pair of names"
)]
fn content_items(bytes: &[u8]) -> Option<(Vec<(String, String)>, Vec<(String, String)>)> {
    let root = parse_element_tree(bytes).ok()?;
    if root.name != "ExchangePlanContent" {
        return None;
    }
    let mut items = Vec::new();
    for child in &root.children {
        if child.name != "Item" {
            return None;
        }
        items.push((
            child.child_text("Metadata")?.to_string(),
            child.child_text("AutoRecord")?.to_string(),
        ));
    }
    items.sort();
    Some((root.attrs.clone(), items))
}

/// How two `Content.xml` differ when the platform lets it pass (see
/// `check::items_relation`): the same items in another order, or the same
/// objects with other flags of automatic registration.
fn content_relation(old: &[u8], new: &[u8]) -> Option<&'static str> {
    let (old, new) = (content_items(old)?, content_items(new)?);
    if old.0 != new.0 {
        return None;
    }
    items_relation(&old.1, &new.1)
}

fn unreadable(rel: &str, error: &anyhow::Error, verdict: &mut Verdict) {
    verdict.stats.unreadable += 1;
    let name = full_name(rel);
    verdict.push_reason(Reason::step(
        ReasonClass::Unknown,
        RuleId::TreeFileUnreadable,
        &name,
        rel,
        "",
        &format!("{error:#}"),
    ));
    descriptor::unresolved(
        &ObjectRef {
            kind: "",
            name: &name,
            file_name: rel,
            id: rel,
        },
        ObjectOp::Changed,
        verdict,
    );
}

fn compare_pair(rel: &str, before: &Object, after: &Object, verdict: &mut Verdict) {
    let name = full_name(rel);
    if before.kind != after.kind {
        descriptor::kind_changed(
            &ObjectRef {
                kind: &after.kind,
                name: &name,
                file_name: rel,
                id: &after.uuid,
            },
            &before.kind,
            verdict,
        );
        return;
    }
    descriptor::compare(
        &ObjectRef {
            kind: &after.kind,
            name: &name,
            file_name: rel,
            id: &after.uuid,
        },
        &before.element,
        &after.element,
        verdict,
    );
}

/// The object a body file belongs to: the path up to its `Ext` folder.
fn body_owner(rel: &str) -> String {
    let parts = rel.split('/').collect::<Vec<_>>();
    match parts.iter().rposition(|part| *part == "Ext") {
        Some(0) => "Configuration".to_string(),
        Some(index) => full_name(&format!("{}.xml", parts[..index].join("/"))),
        None => rel.to_string(),
    }
}

#[cfg(test)]
#[path = "trees_tests.rs"]
mod tests;
