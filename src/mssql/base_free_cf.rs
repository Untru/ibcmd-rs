//! XML → CF with no base (Untru/ibcmd-rs#351): the base-free stage an empty
//! infobase is loaded from (`empty_stage::prepare_empty_stage`) gives every
//! Config row of the tree; a row's stored bytes (raw deflate of its text) are
//! what a `.cf` element holds, so the rows become the container's entries as
//! they are.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, bail};
use ibcmd_core::storage::{
    MultipartIdentity, StorageKey, StoragePatch, StoragePatchEntry, StoragePatchOutcome,
    StoragePatchTarget, StorageProvenance,
};

use crate::legacy_version::InfobaseConfigSourceVersion;

/// The container entries of the tree at `root`: name → stored bytes. Any row
/// the stage could not compile refuses the whole tree, naming each.
pub fn base_free_entries(
    root: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let stage = super::empty_stage::prepare_empty_stage(root, Some(source_version.as_str()))?;
    let failures = stage.failures().collect::<Vec<_>>();
    if !failures.is_empty() {
        let shown = failures
            .iter()
            .map(|failure| {
                format!(
                    "{} {} {}: {}",
                    failure.source,
                    failure.family,
                    failure.file_name.as_deref().unwrap_or(""),
                    failure.error
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        bail!(
            "{} rows of the tree cannot be compiled without a base:\n{shown}",
            failures.len()
        );
    }
    let mut entries = BTreeMap::new();
    for row in stage.rows() {
        match entries.get(&row.file_name) {
            Some(existing) if existing != &row.blob => {
                bail!(
                    "row {} is compiled twice with different bytes",
                    row.file_name
                )
            }
            Some(_) => {}
            None => {
                entries.insert(row.file_name.clone(), row.blob.clone());
            }
        }
    }
    Ok(entries)
}

/// [`base_free_entries`] as the patch `cf bootstrap` publishes, and the
/// entries' total stored bytes.
pub fn base_free_patch(
    root: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<(StoragePatch, usize)> {
    #[cfg(not(feature = "platform-oracle"))]
    let entries = base_free_entries(root, source_version)?;
    #[cfg(feature = "platform-oracle")]
    let entries = research_entries(root, source_version)?;
    let total: usize = entries.values().map(Vec::len).sum();
    let mut patch = Vec::with_capacity(entries.len());
    for (name, bytes) in entries {
        patch.push(StoragePatchEntry::new(
            StoragePatchTarget::new(
                StorageKey::new(&name)?,
                MultipartIdentity::single(),
                StorageProvenance::new(&format!("bootstrap:base-free:{name}"))?,
            ),
            StoragePatchOutcome::compiled(bytes)?,
        ));
    }
    // The patch retains every compiled payload plus keys and provenance, so
    // its budget follows the tree's own size (an ERP-sized tree retains more
    // than the 512 MiB floor) rather than the floor alone.
    let budget = total.saturating_mul(2);
    Ok((
        StoragePatch::with_retained_byte_limit(patch, budget)?,
        total,
    ))
}

/// Entry substitution is an oracle diagnostic, excluded from portable builds.
#[cfg(feature = "platform-oracle")]
fn research_entries(
    root: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut entries = if std::env::var_os("IBCMD_RS_BASE_FREE_ENTRIES_FROM").is_some()
        && std::env::var_os("IBCMD_RS_BASE_FREE_KEEP_OURS").is_none()
    {
        BTreeMap::new()
    } else {
        base_free_entries(root, source_version)?
    };
    // Diagnostics: IBCMD_RS_BASE_FREE_ENTRIES_FROM=<file.cf> takes every
    // entry from that file instead, except those whose name contains one of
    // the comma-separated IBCMD_RS_BASE_FREE_KEEP_OURS substrings; bisects
    // what a platform refuses.
    if let Some(from) = std::env::var_os("IBCMD_RS_BASE_FREE_ENTRIES_FROM") {
        let keep = std::env::var("IBCMD_RS_BASE_FREE_KEEP_OURS").unwrap_or_default();
        let keep = keep
            .split(',')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let source = std::fs::File::open(&from)?;
        let limits = ibcmd_core::limits::ResourceLimits::for_input_bytes(source.metadata()?.len());
        let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli")?;
        let archive = ibcmd_cf::archive::decode_packed_archive(source, limits, profile)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        // IBCMD_RS_BASE_FREE_ENTRIES_ONLY=<file>: only the entries it names,
        // one per line.
        let only = std::env::var_os("IBCMD_RS_BASE_FREE_ENTRIES_ONLY")
            .map(std::fs::read_to_string)
            .transpose()?
            .map(|text| {
                text.lines()
                    .map(str::trim)
                    .map(str::to_owned)
                    .collect::<std::collections::BTreeSet<_>>()
            });
        let mut theirs = BTreeMap::new();
        for (name, payload) in crate::external::export::entries_of(&archive) {
            if only.as_ref().is_some_and(|only| !only.contains(&name)) {
                continue;
            }
            if keep.iter().any(|part| name.contains(part)) {
                if let Some(ours) = entries.get(&name) {
                    theirs.insert(name, ours.clone());
                }
            } else {
                theirs.insert(name, payload);
            }
        }
        entries = theirs;
    }
    Ok(entries)
}
