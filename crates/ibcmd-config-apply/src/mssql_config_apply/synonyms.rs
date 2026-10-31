//! A changed synonym in the object registry.
//!
//! The registry (`1a621f0f-....si`, [`super::si`]) keeps the synonym of every metadata object: the object's own
//! and the ones of its attributes, tabular sections, forms and commands. The native apply of a stage that
//! changes a synonym writes the new text into the object's record (measured on the БСП 8.3.27: the stage of two
//! synonyms changes exactly those two records of that row and nothing of the other fifteen `.si` rows; the
//! record is the only thing that changes). This apply used to leave the old text: it wrote the registry for new
//! forms and templates and for a restructuring only.
//!
//! Every staged object and child object of a descriptor row starts with the same md header,
//! `{3,{1,0,<uuid>},"Name",{<n>,"ru","text",...},"Comment",...}`. A synonym changed is a header of the staged
//! row whose registry record has other pairs. Reading all staged headers also covers a reversion of a dynamic
//! alias to the physical base descriptor. The record of that uuid takes the staged pairs; a uuid
//! the registry does not list (not every header has a record) is left out. The edit is made on the text the
//! stage's other edits of the same row produced (new forms and templates, a restructuring, removals), as the
//! removals do, by [`crate::restructure::caches::registry::set_synonyms`].

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use uuid::Uuid;

use crate::metadata_model::brace::{Brace, parse_row};
use crate::restructure::caches::facts::localized_pairs;
use crate::restructure::caches::registry::{REGISTRY_ROW, set_synonyms};
use crate::sql::{SqlClient, SqlParam};

use super::model::quote_ident;
use super::removals::{SI_VERSIONS, base_of, put, si_rows};
use super::si;
use super::sqlgen::ParamsRewrite;
use super::versions::inflate_row;

/// `(language, text)` pairs, sorted by language.
type Pairs = Vec<(String, String)>;

/// An object (or child object) whose synonym the stage changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynonymChange {
    pub uuid: String,
    pub synonyms: Pairs,
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36 && Uuid::parse_str(text).is_ok()
}

/// The synonyms of every md header in a descriptor row, by uuid (lower case).
pub fn header_synonyms(row: &Brace) -> BTreeMap<String, Pairs> {
    let mut out = BTreeMap::new();
    collect(row, &mut out);
    out
}

fn collect(node: &Brace, out: &mut BTreeMap<String, Pairs>) {
    let Some(items) = node.as_list() else { return };
    if items.first().and_then(Brace::as_atom) == Some("3")
        && let Some(identity) = items.get(1).and_then(Brace::as_list)
        && identity.len() == 3
        && identity.first().and_then(Brace::as_atom) == Some("1")
        && let Some(uuid) = identity.get(2).and_then(Brace::as_atom)
        && is_uuid(uuid)
        && items.get(2).and_then(Brace::as_str).is_some()
        && let Some(synonym) = items.get(3)
        && let Ok(mut pairs) = localized_pairs(synonym)
    {
        pairs.sort();
        out.entry(uuid.to_ascii_lowercase()).or_insert(pairs);
    }
    for child in items {
        collect(child, out);
    }
}

/// The synonyms a staged descriptor changes against the active one.
pub fn changes_between(active: &[u8], staged: &[u8]) -> Result<Vec<SynonymChange>> {
    if active == staged {
        return Ok(Vec::new());
    }
    let before =
        header_synonyms(&parse_row(active).context("the active descriptor does not parse")?);
    let after =
        header_synonyms(&parse_row(staged).context("the staged descriptor does not parse")?);
    Ok(after
        .into_iter()
        .filter(|(uuid, pairs)| before.get(uuid).is_some_and(|old| old != pairs))
        .map(|(uuid, synonyms)| SynonymChange { uuid, synonyms })
        .collect())
}

/// The authoritative synonyms of every header in the staged descriptors `names`.
///
/// Compare these with the registry, not with physical Config rows: a dynamic update may publish another
/// descriptor under an alias, and a stage can revert it to bytes equal to the physical base. Joining to that
/// base (or filtering equal digests before this read) would miss the reversion. New headers absent from the
/// registry are ignored by the writer; headers inserted by an earlier cache edit already have the right text.
/// Descriptor parts are joined before inflation, like other configuration row readers.
pub fn read_changes(
    client: &dyn SqlClient,
    database: &str,
    names: &[String],
) -> Result<Vec<SynonymChange>> {
    let db = quote_ident(database)?;
    let mut stored = BTreeMap::<String, Vec<u8>>::new();
    for chunk in names.chunks(400) {
        let placeholders = (1..=chunk.len())
            .map(|index| format!("@P{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT s.FileName, s.PartNo, s.BinaryData FROM {db}.dbo.ConfigSave s WHERE s.FileName IN ({placeholders}) ORDER BY s.FileName, s.PartNo"
        );
        let params = chunk
            .iter()
            .map(|name| SqlParam::Text(name.as_str()))
            .collect::<Vec<_>>();
        client.read_rows(&query, &params, &mut |mut row| {
            let name = row.take_text(0)?;
            let bytes = row.take_binary(2)?;
            stored.entry(name).or_default().extend_from_slice(&bytes);
            Ok(())
        })?;
    }
    let mut headers = BTreeMap::new();
    for (name, bytes) in stored {
        let staged = inflate_row(&bytes)
            .with_context(|| format!("the staged descriptor {name} does not inflate"))?;
        let parsed = parse_row(&staged)
            .with_context(|| format!("the staged descriptor {name} does not parse"))?;
        for (uuid, synonyms) in header_synonyms(&parsed) {
            if let Some(previous) = headers.insert(uuid.clone(), synonyms.clone()) {
                ensure!(
                    previous == synonyms,
                    "staged descriptors disagree on the synonyms of {uuid}"
                );
            }
        }
    }
    Ok(headers
        .into_iter()
        .map(|(uuid, synonyms)| SynonymChange { uuid, synonyms })
        .collect())
}

/// The rewrites of the registry for the synonym changes, on top of `existing` (what the new forms and
/// templates, a restructuring and the removals of the same stage rewrite already): the records of the changed
/// objects take the new texts and the row gets a new version in `siVersions`. A row `existing` rewrites keeps
/// the digest the plan saw of the stored row; the edit is made on the text `existing` produced. Returns the
/// rewrites and the number of records rewritten (0: the registry says what the stage says, nothing is added).
pub fn plan_search_info(
    client: &dyn SqlClient,
    database: &str,
    changes: &[SynonymChange],
    existing: Vec<ParamsRewrite>,
) -> Result<(Vec<ParamsRewrite>, usize)> {
    let db = quote_ident(database)?;
    let stored = si_rows(client, &db)?;
    let mut rewrites = existing;
    ensure!(
        stored.iter().any(|row| row.name == REGISTRY_ROW),
        "Params holds no object registry ({REGISTRY_ROW})"
    );
    let (size, sha, plain, set_creation) = base_of(&stored, REGISTRY_ROW, &rewrites)?;
    let pairs: Vec<(String, Pairs)> = changes
        .iter()
        .map(|change| (change.uuid.clone(), change.synonyms.clone()))
        .collect();
    let (new_plain, count) = set_synonyms(&plain, &pairs)?;
    if count == 0 {
        return Ok((rewrites, 0));
    }
    let before = si::parse(&plain)?;
    let after = si::parse(&new_plain)?;
    ensure!(
        before.records.len() == after.records.len()
            && before.records.iter().zip(&after.records).all(|(old, new)| (
                &old.uuid,
                &old.parent,
                old.kind,
                &old.name
            ) == (
                &new.uuid,
                &new.parent,
                new.kind,
                &new.name
            )),
        "the edited object registry does not list the same objects"
    );
    put(
        &mut rewrites,
        REGISTRY_ROW,
        size,
        sha,
        &new_plain,
        set_creation,
    )?;
    // siVersions: the edited row gets a new version.
    let (size, sha, mut versions, _) = base_of(&stored, SI_VERSIONS, &rewrites)?;
    versions = si::set_si_version(&versions, REGISTRY_ROW, Uuid::new_v4())?;
    put(&mut rewrites, SI_VERSIONS, size, sha, &versions, false)?;
    Ok((rewrites, count))
}

#[cfg(test)]
#[path = "synonyms_tests.rs"]
mod tests;
