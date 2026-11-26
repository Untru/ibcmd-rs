//! Incremental export (`cf export <file> <tree> --update`): a tree exported
//! with `--index` from an earlier version of a configuration or extension is
//! brought up to the new file by rewriting only the entries whose
//! `configVersion` moved.
//!
//! Only module, form, template and other body entries (`<uuid>.<n>`) are
//! rewritten in place. A changed metadata row (`<uuid>`) may rename what other
//! objects' XML names, an added or removed entry changes the configuration's
//! child lists, and an external object keeps no versions at all: each of
//! those takes a full export. The rewritten entries come out of an export of
//! the new file's metadata rows and the changed bodies only -- the metadata
//! rows carry every name the bodies' XML resolves -- so the tree ends exactly
//! as a full export of the new file writes it (`tests/cf_update.rs`).
//!
//! The tree is someone's working copy, so an update changes it only when
//! nothing can be lost: it needs the tree's index (without one the tree's
//! own edits cannot be told from the export), it refuses a tree with edits
//! not loaded yet, and it writes nothing until every file it will write is
//! exported. A full update exports aside and then replaces only the files
//! the index names, so a `.git` or anything else in a dot directory stays.
//! While files change the tree carries a mark (`index::begin_update`); a
//! tree still marked is refused by the next update and load.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    legacy_version::InfobaseConfigSourceVersion,
    load::{self, index},
    mssql_dump::{StorageImageSourceExportReport, stored_config_versions},
};

/// How the tree was brought up to the new file.
#[derive(Clone, Debug, Default, Serialize)]
pub struct UpdateSummary {
    /// `incremental`, `unchanged` or `full`.
    pub mode: &'static str,
    /// Why a full export was needed, when it was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Entries whose `configVersion` moved.
    pub changed_entries: Vec<String>,
    /// Files written again.
    pub rewritten: Vec<String>,
    /// Files of the changed entries the new file no longer produces.
    pub removed: Vec<String>,
}

/// Whether `tree` may be updated: `Ok(true)` when it holds nothing yet (a
/// plain export into it), `Ok(false)` for an indexed tree without edits;
/// `Err` says why not. `ConfigDumpInfo.xml` is not an edit: the export owns
/// it, and a tree may lack it.
pub fn preflight(tree: &Path) -> std::result::Result<bool, String> {
    if index::update_interrupted(tree) {
        return Err(interrupted(tree));
    }
    if !tree.exists() {
        return Ok(true);
    }
    let mut entries = fs::read_dir(tree)
        .map_err(|source| format!("failed to read {}: {source}", tree.display()))?;
    if entries.next().is_none() {
        return Ok(true);
    }
    let index = index::read_tree_index(tree).ok_or(
        "the tree has no index this version reads: export it with --index (or again with --overwrite)",
    )?;
    let diff = index::diff_against_index(tree, &index).map_err(|source| format!("{source:#}"))?;
    let mut edits = diff
        .changed
        .iter()
        .chain(&diff.added)
        .chain(&diff.removed)
        .filter(|path| path.as_str() != "ConfigDumpInfo.xml")
        .cloned()
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return Ok(false);
    }
    edits.sort();
    let shown = edits
        .iter()
        .take(10)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let more = if edits.len() > 10 {
        format!(" and {} more", edits.len() - 10)
    } else {
        String::new()
    };
    Err(format!(
        "the tree has edits not loaded yet ({shown}{more}): load them with `cf load`, or export \
         again with --overwrite to drop them"
    ))
}

pub fn interrupted(tree: &Path) -> String {
    format!(
        "an earlier update of {} was interrupted: the tree mixes two files; export it again with \
         --overwrite",
        tree.display()
    )
}

/// Replaces the files of `tree` that `index` names by the export in
/// `scratch`; nothing else in the tree is touched. The tree is marked until
/// the caller writes the new index and calls `index::finish_update`.
pub fn replace_tree(tree: &Path, scratch: &Path, index: &index::TreeIndex) -> Result<()> {
    index::begin_update(tree)?;
    for path in index.paths() {
        remove_if_present(&tree.join(path))?;
    }
    for path in load::relative_files(scratch)? {
        let target = tree.join(&path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(scratch.join(&path), &target)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    prune_empty_dirs(tree);
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(source) if source.kind() != std::io::ErrorKind::NotFound => {
            Err(source).with_context(|| format!("failed to remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Removes the directories the old export left empty (never a dot directory).
fn prune_empty_dirs(tree: &Path) {
    let dirs = walkdir::WalkDir::new(tree)
        .min_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    for dir in dirs {
        // Fails on a directory that still has files: that one stays.
        let _ = fs::remove_dir(dir);
    }
}

/// `Some(unchanged)` when the tree's index already names `input` (same
/// digest, same dialect): nothing to decode or export.
pub fn already_current(
    tree: &Path,
    input: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Option<UpdateSummary> {
    let index = index::read_tree_index(tree)?;
    let current =
        index.dialect == source_version.as_str() && index::file_sha256(input).ok()? == index.base;
    current.then(|| UpdateSummary {
        mode: "unchanged",
        ..UpdateSummary::default()
    })
}

/// The update plan: `Ok(changed entries)` when an incremental export will do,
/// `Err(reason)` when only a full one will.
pub fn plan(
    tree: &Path,
    entries: &[(String, Vec<u8>)],
    source_version: InfobaseConfigSourceVersion,
) -> std::result::Result<BTreeSet<String>, String> {
    let index =
        index::read_tree_index(tree).ok_or("the tree has no index (export it with --index)")?;
    if index.dialect != source_version.as_str() {
        return Err(format!("the tree is of dialect {}", index.dialect));
    }
    let old = indexed_versions(tree, &index);
    if old.is_empty() {
        return Err("the index records no versions".to_owned());
    }
    let new = stored_config_versions(entries).ok_or("the file records no versions")?;
    let old_ids = old.keys().collect::<BTreeSet<_>>();
    let new_ids = new.keys().collect::<BTreeSet<_>>();
    if old_ids != new_ids {
        return Err("objects were added or removed".to_owned());
    }
    let changed = new
        .iter()
        .filter(|(id, version)| old.get(*id) != Some(version))
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(metadata) = changed.iter().find(|id| !id.contains('.')) {
        return Err(format!("metadata row {metadata} changed"));
    }
    Ok(changed)
}

/// Brings `tree` up to `input` (its `entries`, of `profile`) by rewriting the
/// `changed` body entries; `scratch` receives the partial export.
pub fn update_incrementally(
    input: &Path,
    tree: &Path,
    profile: &str,
    entries: Vec<(String, Vec<u8>)>,
    changed: &BTreeSet<String>,
    source_version: InfobaseConfigSourceVersion,
    scratch: &Path,
) -> Result<(Option<StorageImageSourceExportReport>, UpdateSummary)> {
    let index = index::read_tree_index(tree).context("the tree index went away")?;
    let old_versions = indexed_versions(tree, &index);
    let new_versions = stored_config_versions(&entries).context("the file records no versions")?;
    let mut summary = UpdateSummary {
        mode: if changed.is_empty() {
            "unchanged"
        } else {
            "incremental"
        },
        changed_entries: changed.iter().cloned().collect(),
        ..UpdateSummary::default()
    };
    if changed.is_empty() {
        // Another file with the same versions (resaved): only the index
        // moves on to it.
        index::update_tree_index(tree, input, &index, &new_versions, &[], &[])?;
        return Ok((None, summary));
    }
    // A CFE manifest binds every packed row. Retain the full extension while
    // decoding into scratch so its graph and digests can be validated; only
    // the changed outputs below are copied to the indexed source tree.
    let is_extension = entries.iter().any(|(name, _)| name == "configinfo")
        && !entries.iter().any(|(name, _)| name == "root");
    let subset = if is_extension {
        entries
    } else {
        entries
            .into_iter()
            .filter(|(name, _)| !name.contains('.') || changed.contains(name))
            .collect::<Vec<_>>()
    };
    let report =
        crate::extension::export_entries_to_source(profile, subset, scratch, true, source_version)?;
    // Nothing is written until every changed entry is exported: a failed
    // one would leave its old files removed and the index moved on. (The
    // partial export's own `versions` fails, lacking most entries; the
    // update rewrites ConfigDumpInfo.xml itself.)
    let failed = report
        .storage
        .entries
        .iter()
        .filter(|entry| {
            changed.contains(&entry.logical_key)
                && entry.disposition == ibcmd_cf::export::StorageExportDisposition::Failed
        })
        .map(|entry| {
            format!(
                "{}{}",
                entry.logical_key,
                entry
                    .message
                    .as_deref()
                    .map(|message| format!(" ({message})"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    if !failed.is_empty() {
        anyhow::bail!(
            "{} entries of the new file failed to export, the tree is unchanged: {}",
            failed.len(),
            failed.join("; ")
        );
    }

    index::begin_update(tree)?;
    let mut rewritten = Vec::new();
    for entry in &report.storage.entries {
        if !changed.contains(&entry.logical_key) {
            continue;
        }
        for output in &entry.outputs {
            let output = output.replace('\\', "/");
            let target = tree.join(&output);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(scratch.join(&output), &target)
                .with_context(|| format!("failed to write {}", target.display()))?;
            rewritten.push((output, entry.logical_key.clone()));
        }
    }
    let written = rewritten
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<BTreeSet<_>>();
    let mut removed = Vec::new();
    for (path, key) in index.keys() {
        if changed.contains(&key) && !written.contains(path.as_str()) {
            remove_if_present(&tree.join(&path))?;
            removed.push(path);
        }
    }
    // A tree whose export skipped entries has no ConfigDumpInfo.xml; the
    // index keeps the versions either way.
    let dump_info = tree.join("ConfigDumpInfo.xml");
    if !changed.is_empty() && dump_info.exists() {
        rewrite_config_versions(&dump_info, changed, &old_versions, &new_versions)?;
        rewritten.push(("ConfigDumpInfo.xml".to_owned(), String::new()));
    }
    index::update_tree_index(tree, input, &index, &new_versions, &rewritten, &removed)?;
    index::finish_update(tree)?;
    summary.rewritten = rewritten.into_iter().map(|(path, _)| path).collect();
    summary.rewritten.sort();
    summary.rewritten.dedup();
    summary.removed = removed;
    Ok((Some(report), summary))
}

/// The versions the tree was exported at: the index's, or, for an index
/// written before it kept them, the tree's ConfigDumpInfo.xml.
fn indexed_versions(tree: &Path, index: &index::TreeIndex) -> BTreeMap<String, String> {
    if index.versions.is_empty() {
        load::config_versions(&tree.join("ConfigDumpInfo.xml"))
    } else {
        index.versions.clone()
    }
}

/// ConfigDumpInfo.xml with the changed entries' configVersion moved on; the
/// entries and names stay, as nothing was added, removed or renamed.
fn rewrite_config_versions(
    path: &Path,
    changed: &BTreeSet<String>,
    old: &BTreeMap<String, String>,
    new: &BTreeMap<String, String>,
) -> Result<()> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let id = line
            .find(" id=\"")
            .map(|at| &line[at + 5..])
            .and_then(|rest| rest.split('"').next());
        match id.filter(|id| changed.contains(*id)) {
            Some(id) => {
                let (Some(before), Some(after)) = (old.get(id), new.get(id)) else {
                    out.push_str(line);
                    continue;
                };
                out.push_str(&line.replace(
                    &format!("configVersion=\"{before}\""),
                    &format!("configVersion=\"{after}\""),
                ));
            }
            None => out.push_str(line),
        }
    }
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}
