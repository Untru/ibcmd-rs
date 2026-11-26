//! Removed forms and templates: the stage's `deleted` list executed (#393).
//!
//! The native apply never deletes a `Config` row that a stage merely leaves out; a removal travels in the
//! stage's `deleted` row, the list of the row names to take out (`<count>,"<name>",<flag>,...`, flag 0 for a
//! `Config` row, 1 for an element that has no row of its own, an attribute). Measured on twins of the
//! БСП 8.3.27 clone that lose a form and a template (`docs/apply/own-apply.md`, "Removals"):
//!
//! - the rows the list names leave `Config` (the descriptor and every `<uuid>.<n>` of each object);
//! - the owner's descriptor, staged without the reference, is replaced like any staged row;
//! - the change register treats a removed object like a changed one: the message numbers of its rows are reset
//!   and a node that has no row of it gets one, with the object's files;
//! - the main search information (`1a621f0f-....si`) loses the record of each object (its count follows),
//!   `c4629235-....si` loses the object's entry when it has one (a form with a help page has), and
//!   `siVersions` gives both rows a new version;
//! - `DBSchema`, `DBNames` and the other cache rows do not change: neither has a table or a column.
//!
//! [`analyze`] finds all of it, or says why it cannot: a list this analysis does not account for name by
//! name is refused as a whole. [`plan_search_info`] edits the two cache rows once the gate has spoken, on
//! top of whatever the new objects and a restructuring rewrite in the same stage.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow};
use serde::Serialize;
use uuid::Uuid;

use crate::metadata_model::brace::parse_row;
use crate::metadata_model::export::names::{owned_groups, remove_owned};
use crate::restructure::extensions::{
    Adoption, ChangedObject, ExtensionInputs, check as check_adoption,
};
use crate::sql::{SqlClient, SqlExec, SqlValue};

use super::AdoptionReader;
use super::gate::GateBlocker;
use super::model::{RowMeta, RowName, classify_name, hex_upper, quote_ident};
use super::objects::describe;
use super::si;
use super::sqlgen::ParamsRewrite;
use super::versions::{deflate_row, inflate_row, parse_versions, strip_bom};

const DESCRIPTOR_PATTERN: &str = "________-____-____-____-____________";
/// The row of the properties the platform looks up by uuid (help references and the like).
pub const PROPERTIES_ROW: &str = "c4629235-4823-4320-b8b5-1d08f4c6d612.si";
pub(super) const SI_VERSIONS: &str = "siVersions";

/// A form or a template the stage removes.
#[derive(Debug, Clone, Serialize)]
pub struct RemovedObject {
    pub uuid: String,
    pub kind: &'static str,
    pub owner: String,
    pub name: String,
    /// The class id the owner lists it under (the record's kind in the search information is the index
    /// of this class).
    pub class: String,
    /// Its `Config` rows as stored, the descriptor first.
    pub rows: Vec<String>,
    /// `_MDObjID`, 32 hex digits.
    #[serde(skip)]
    pub object_hex: String,
}

/// What a `deleted` list removes and this apply accounts for.
#[derive(Debug, Default)]
pub struct Removals {
    pub objects: Vec<RemovedObject>,
    /// The `Config` rows the script deletes, as stored.
    pub rows: Vec<String>,
    /// The lower-cased names of the list that the removals answer for.
    pub accounted: HashSet<String>,
    /// The owners' descriptors (lower-cased) that differ from the active ones by the removed references
    /// and nothing else.
    pub owners: HashSet<String>,
}

impl Removals {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct RemovalAnalysis {
    pub removals: Removals,
    pub blockers: Vec<GateBlocker>,
}

pub struct RemovalInput<'a> {
    pub client: &'a dyn SqlClient,
    pub database: &'a str,
    /// The names of the list that are `Config` rows (flag 0) and no rows of a dynamic update, as listed.
    pub names: &'a [String],
    /// The rows of a dynamic update `Config` carries (lower-cased names).
    pub overlay_rows: &'a HashSet<String>,
    /// Every `ConfigSave` row.
    pub staged: &'a [RowMeta],
}

fn blocker(blockers: &mut Vec<GateBlocker>, row: &str, reason: impl Into<String>) {
    blockers.push(GateBlocker {
        row: row.to_owned(),
        reason: reason.into(),
    });
}

fn plain_lower(plain: &[u8]) -> String {
    String::from_utf8_lossy(strip_bom(plain)).to_ascii_lowercase()
}

/// Whether the named things of a list are what this analysis can remove, object by object.
pub fn analyze(input: &RemovalInput<'_>) -> Result<RemovalAnalysis> {
    let mut analysis = RemovalAnalysis::default();
    let mut blockers: Vec<GateBlocker> = Vec::new();
    let client = input.client;
    let db = quote_ident(input.database)?;

    // The list, by shape: descriptors name the objects, bodies must belong to a listed one.
    let mut objects: BTreeSet<String> = BTreeSet::new();
    let mut listed: HashSet<String> = HashSet::new();
    for name in input.names {
        listed.insert(name.to_ascii_lowercase());
        match classify_name(name) {
            RowName::Descriptor(uuid) => {
                objects.insert(uuid.to_ascii_lowercase());
            }
            RowName::Body { .. } => {}
            RowName::Service(_) | RowName::Other => blocker(
                &mut blockers,
                name,
                "a name that is neither an object's descriptor nor a body row of one: not a removable form or template",
            ),
        }
    }
    for name in input.names {
        if let RowName::Body { owner, .. } = classify_name(name)
            && !objects.contains(&owner.to_ascii_lowercase())
        {
            blocker(
                &mut blockers,
                name,
                "a body row of an object that stays (a module, picture or help page of it): the removal of one file of an object is not measured",
            );
        }
    }
    if objects.is_empty() || !blockers.is_empty() {
        analysis.blockers = blockers;
        return Ok(analysis);
    }
    let uuids: Vec<String> = objects.iter().cloned().collect();

    // The rows `Config` holds for each object, and the rows `ConfigSave` holds for it.
    let mut config_rows: BTreeMap<String, Vec<(String, i32)>> = BTreeMap::new();
    for chunk in uuids.chunks(400) {
        let list = chunk
            .iter()
            .map(|uuid| format!("N'{uuid}'"))
            .collect::<Vec<_>>()
            .join(", ");
        client.read_rows(
            &format!(
                "SELECT FileName, PartNo FROM {db}.dbo.Config WHERE LEFT(FileName, 36) IN ({list}) ORDER BY FileName, PartNo"
            ),
            &[],
            &mut |row| {
                let name = row.text(0)?.to_owned();
                let part = i32::try_from(row.i64(1)?)?;
                // the filter is on the first 36 characters, so they are a uuid's
                if let Some(head) = name.get(..36).map(str::to_ascii_lowercase) {
                    config_rows.entry(head).or_default().push((name, part));
                }
                Ok(())
            },
        )?;
    }
    let mut kept: Vec<RemovedObjectRows> = Vec::new();
    for uuid in &uuids {
        let rows = config_rows.remove(uuid).unwrap_or_default();
        if !rows.iter().any(|(name, _)| name.eq_ignore_ascii_case(uuid)) {
            blocker(
                &mut blockers,
                uuid,
                "the list names an object whose descriptor Config does not hold",
            );
            continue;
        }
        let mut ok = true;
        let mut names: Vec<String> = Vec::new();
        for (name, part) in &rows {
            if *part != 0 && name.eq_ignore_ascii_case(uuid) {
                blocker(
                    &mut blockers,
                    name,
                    "a descriptor of several parts: not a form or a template",
                );
                ok = false;
                continue;
            }
            let lower = name.to_ascii_lowercase();
            match classify_name(name) {
                RowName::Descriptor(_) | RowName::Body { .. } => {
                    if !listed.contains(&lower) {
                        blocker(
                            &mut blockers,
                            name,
                            "a row of a removed object that the list does not name: the list must name the object's rows exactly",
                        );
                        ok = false;
                    } else if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
                // an alias of a dynamic update, or a name of no known shape
                RowName::Service(_) | RowName::Other => {
                    if !input.overlay_rows.contains(&lower) {
                        blocker(
                            &mut blockers,
                            name,
                            "a row of the removed object that is no descriptor, body or row of a dynamic update",
                        );
                        ok = false;
                    }
                }
            }
        }
        // the names the list gives for the object must all be rows of it
        for name in input.names {
            let lower = name.to_ascii_lowercase();
            let belongs = lower.starts_with(uuid.as_str())
                && matches!(
                    classify_name(name),
                    RowName::Descriptor(_) | RowName::Body { .. }
                );
            if belongs
                && !rows
                    .iter()
                    .any(|(stored, _)| stored.eq_ignore_ascii_case(name))
            {
                blocker(
                    &mut blockers,
                    name,
                    "the list names a row that Config does not hold",
                );
                ok = false;
            }
        }
        // the rows of a dynamic update of the object must be named too (the list is judged whole)
        for overlay in input.overlay_rows {
            if overlay.starts_with(uuid.as_str()) && !listed.contains(overlay) {
                blocker(
                    &mut blockers,
                    overlay,
                    "a row of a dynamic update of the removed object that the list does not name",
                );
                ok = false;
            }
        }
        if input.staged.iter().any(|row| {
            row.name
                .get(..36)
                .is_some_and(|head| head.eq_ignore_ascii_case(uuid))
        }) {
            blocker(
                &mut blockers,
                uuid,
                "the stage removes the object and stages rows of it too",
            );
            ok = false;
        }
        if ok {
            // the descriptor first
            names.sort_by_key(|name| (!name.eq_ignore_ascii_case(uuid), name.to_ascii_lowercase()));
            kept.push(RemovedObjectRows {
                uuid: uuid.clone(),
                rows: names,
            });
        }
    }
    if kept.is_empty() {
        analysis.blockers = blockers;
        return Ok(analysis);
    }
    let removed_uuids: HashSet<String> = kept.iter().map(|object| object.uuid.clone()).collect();

    // Who mentions the objects: the descriptors of the active configuration.
    let mut unreadable = 0usize;
    let mut active_mentions: HashMap<String, Vec<String>> = HashMap::new();
    let mut active_texts: HashMap<String, Vec<u8>> = HashMap::new();
    let mut own_texts: HashMap<String, Vec<u8>> = HashMap::new();
    client.read_rows(
        &format!(
            "SELECT FileName, BinaryData FROM {db}.dbo.Config WHERE PartNo = 0 AND FileName LIKE N'{DESCRIPTOR_PATTERN}'"
        ),
        &[],
        &mut |mut row| {
            let name = row.take_text(0)?.to_ascii_lowercase();
            let bytes = row.take_binary(1)?;
            let Ok(plain) = inflate_row(&bytes) else {
                unreadable += 1;
                return Ok(());
            };
            if removed_uuids.contains(&name) {
                own_texts.insert(name, plain);
                return Ok(());
            }
            let text = plain_lower(&plain);
            let mut mentions = false;
            for uuid in &removed_uuids {
                if text.contains(uuid.as_str()) {
                    active_mentions
                        .entry(uuid.clone())
                        .or_default()
                        .push(name.clone());
                    mentions = true;
                }
            }
            if mentions {
                active_texts.insert(name, plain);
            }
            Ok(())
        },
    )?;
    if unreadable > 0 {
        blocker(
            &mut blockers,
            "",
            format!(
                "{unreadable} descriptor row(s) of Config do not inflate, so that references to the removed objects cannot be excluded"
            ),
        );
    }
    // ... and of the stage.
    let mut staged_unreadable = 0usize;
    let mut staged_mentions: HashMap<String, Vec<String>> = HashMap::new();
    let mut staged_texts: HashMap<String, Vec<u8>> = HashMap::new();
    let candidate_owners: HashSet<String> = active_mentions
        .values()
        .flat_map(|names| names.iter().cloned())
        .collect();
    client.read_rows(
        &format!(
            "SELECT FileName, BinaryData FROM {db}.dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE N'{DESCRIPTOR_PATTERN}'"
        ),
        &[],
        &mut |mut row| {
            let name = row.take_text(0)?.to_ascii_lowercase();
            let bytes = row.take_binary(1)?;
            let Ok(plain) = inflate_row(&bytes) else {
                staged_unreadable += 1;
                return Ok(());
            };
            let text = plain_lower(&plain);
            for uuid in &removed_uuids {
                if text.contains(uuid.as_str()) {
                    staged_mentions
                        .entry(uuid.clone())
                        .or_default()
                        .push(name.clone());
                }
            }
            if candidate_owners.contains(&name) {
                staged_texts.insert(name, plain);
            }
            Ok(())
        },
    )?;
    if staged_unreadable > 0 {
        blocker(
            &mut blockers,
            "",
            format!(
                "{staged_unreadable} staged descriptor row(s) do not inflate, so that references to the removed objects cannot be excluded"
            ),
        );
    }

    // The owner of each object, and what kind of object it is.
    struct Found {
        uuid: String,
        owner: String,
        kind: &'static str,
        class: String,
        name: String,
        rows: Vec<String>,
    }
    let mut found: Vec<Found> = Vec::new();
    let mut by_owner: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for object in &kept {
        let uuid = &object.uuid;
        let owners = active_mentions.get(uuid).cloned().unwrap_or_default();
        let [owner] = owners.as_slice() else {
            blocker(
                &mut blockers,
                uuid,
                if owners.is_empty() {
                    "no descriptor of Config lists the object: an orphan".to_owned()
                } else {
                    format!(
                        "{} descriptors of Config mention the object ({}): only the list of its owner may",
                        owners.len(),
                        owners.join(", ")
                    )
                },
            );
            continue;
        };
        if let Some(names) = staged_mentions.get(uuid) {
            blocker(
                &mut blockers,
                uuid,
                format!(
                    "the staged descriptor {} still mentions the removed object",
                    names.join(", ")
                ),
            );
            continue;
        }
        let Some(active_plain) = active_texts.get(owner) else {
            continue;
        };
        let Ok(active_tree) = parse_row(active_plain) else {
            blocker(&mut blockers, owner, "a descriptor that does not parse");
            continue;
        };
        let groups: Vec<_> = owned_groups(&active_tree)
            .into_iter()
            .filter(|group| group.members.iter().any(|member| member == uuid))
            .collect();
        let [group] = groups.as_slice() else {
            blocker(
                &mut blockers,
                uuid,
                "the owner lists the object in no group of forms or templates (or in several): only a removed form or template is supported",
            );
            continue;
        };
        if !matches!(group.kind, "Form" | "Template") {
            blocker(
                &mut blockers,
                uuid,
                format!(
                    "a removed {}: only a removed form or template is supported",
                    group.kind.to_lowercase()
                ),
            );
            continue;
        }
        let Some(own) = own_texts.get(uuid) else {
            blocker(
                &mut blockers,
                uuid,
                "the descriptor of the object cannot be read",
            );
            continue;
        };
        let name = match describe(group.kind, uuid, own) {
            Ok(described) => described.name,
            Err(reason) => {
                blocker(&mut blockers, uuid, reason);
                continue;
            }
        };
        by_owner.entry(owner.clone()).or_default().push(found.len());
        found.push(Found {
            uuid: uuid.clone(),
            owner: owner.clone(),
            kind: group.kind,
            class: group.class.clone(),
            name,
            rows: object.rows.clone(),
        });
    }

    // The owner's descriptor: the staged one is the active one without the references.
    let mut accepted_owners: HashSet<String> = HashSet::new();
    let mut good_owner: HashSet<String> = HashSet::new();
    for (owner, indexes) in &by_owner {
        let Some(staged_plain) = staged_texts.get(owner) else {
            blocker(
                &mut blockers,
                owner,
                "the descriptor of the owner is not staged: removing the object would leave a reference to it",
            );
            continue;
        };
        let (Ok(staged_tree), Ok(active_tree)) = (
            parse_row(staged_plain),
            parse_row(active_texts.get(owner).map_or(&[][..], Vec::as_slice)),
        ) else {
            blocker(&mut blockers, owner, "a descriptor that does not parse");
            continue;
        };
        let doomed: HashSet<String> = indexes.iter().map(|at| found[*at].uuid.clone()).collect();
        let mut stripped = active_tree.clone();
        let taken = remove_owned(&mut stripped, &doomed);
        if taken != doomed.len() {
            blocker(
                &mut blockers,
                owner,
                "the owner's groups do not list every removed object exactly once",
            );
            continue;
        }
        if stripped != staged_tree {
            blocker(
                &mut blockers,
                owner,
                "the staged descriptor differs from the active one by more than the removed references: a metadata change, possibly structural",
            );
            continue;
        }
        accepted_owners.insert(owner.clone());
        good_owner.insert(owner.clone());
    }
    found.retain(|object| good_owner.contains(&object.owner));

    // No table, no column: the names of the database do not know the object.
    if !found.is_empty() {
        let mut mentioned: Vec<(String, String)> = Vec::new();
        client.read_rows(
            &format!(
                "SELECT FileName, BinaryData FROM {db}.dbo.Params WHERE PartNo = 0 AND (FileName = N'DBNames' OR FileName LIKE N'DBNames-Ext-%')"
            ),
            &[],
            &mut |mut row| {
                let name = row.take_text(0)?;
                let bytes = row.take_binary(1)?;
                match inflate_row(&bytes) {
                    Ok(plain) => {
                        let text = plain_lower(&plain);
                        for object in &found {
                            if text.contains(object.uuid.as_str()) {
                                mentioned.push((object.uuid.clone(), name.clone()));
                            }
                        }
                    }
                    Err(_) => mentioned.push((String::new(), name)),
                }
                Ok(())
            },
        )?;
        for (uuid, row) in &mentioned {
            blocker(
                &mut blockers,
                if uuid.is_empty() { row } else { uuid },
                if uuid.is_empty() {
                    format!(
                        "Params.{row} does not inflate: whether the object has a table cannot be told"
                    )
                } else {
                    format!(
                        "Params.{row} names the object: it has a table or a column of its own, and the platform's apply removes those"
                    )
                },
            );
        }
        if !mentioned.is_empty() {
            let bad: HashSet<&String> = mentioned.iter().map(|(uuid, _)| uuid).collect();
            found.retain(|object| !bad.contains(&object.uuid));
            if mentioned.iter().any(|(uuid, _)| uuid.is_empty()) {
                found.clear();
            }
        }
    }

    // The staged `versions` row must not list a removed name.
    if !found.is_empty() {
        let staged_versions = client.query_scalar(
            &format!("SELECT BinaryData FROM {db}.dbo.ConfigSave WHERE FileName = N'versions' AND PartNo = 0"),
            &[],
        )?;
        if let Some(SqlValue::Binary(bytes)) = staged_versions {
            match parse_versions(&bytes) {
                Ok(row) => {
                    let names: HashSet<String> = row
                        .entries
                        .iter()
                        .map(|(name, _)| name.to_ascii_lowercase())
                        .collect();
                    found.retain(|object| {
                        let listed_here: Vec<&String> = object
                            .rows
                            .iter()
                            .filter(|row| names.contains(&row.to_ascii_lowercase()))
                            .collect();
                        if listed_here.is_empty() {
                            true
                        } else {
                            blocker(
                                &mut blockers,
                                &object.uuid,
                                format!(
                                    "the staged versions row still lists {}",
                                    listed_here
                                        .iter()
                                        .map(|row| row.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                            );
                            false
                        }
                    });
                }
                Err(error) => {
                    blocker(
                        &mut blockers,
                        "versions",
                        format!("the staged versions row does not read: {error}"),
                    );
                    found.clear();
                }
            }
        }
    }

    // The change register needs nothing from this analysis: the native apply treats a removed object like an
    // object that owns a staged row -- it sets the message numbers of its rows to NULL and inserts the row
    // (with the object's files) at a node that has none (twins `fdq`/`fdq2`, docs/apply/own-apply.md,
    // "Removals") -- so the plan hands the removed rows' names to `registrations::plan` like those of a
    // dynamic update that a `deleted` list names.

    // Owners whose objects were all refused are not accepted.
    let alive_owners: HashSet<String> = found.iter().map(|object| object.owner.clone()).collect();
    accepted_owners.retain(|owner| alive_owners.contains(owner));

    let mut removals = Removals {
        owners: accepted_owners,
        ..Removals::default()
    };
    for object in found {
        for row in &object.rows {
            removals.accounted.insert(row.to_ascii_lowercase());
            removals.rows.push(row.clone());
        }
        let object_hex = Uuid::parse_str(&object.uuid)
            .map(|uuid| hex_upper(&uuid.to_bytes_le()))
            .unwrap_or_default();
        removals.objects.push(RemovedObject {
            uuid: object.uuid,
            kind: object.kind,
            owner: object.owner,
            name: object.name,
            class: object.class,
            rows: object.rows,
            object_hex,
        });
    }
    analysis.removals = removals;
    analysis.blockers = blockers;
    Ok(analysis)
}

struct RemovedObjectRows {
    uuid: String,
    rows: Vec<String>,
}

/// The objects of the removals an extension adopts, as blockers: what the platform's apply does to an
/// extension whose adopted object goes is not measured.
pub fn extension_blockers(
    client: &dyn SqlClient,
    sql: &SqlExec,
    database: &str,
    removals: &Removals,
    read_adoptions: AdoptionReader,
) -> Result<Vec<GateBlocker>> {
    let db = quote_ident(database)?;
    let registered = match client.query_scalar(
        &format!(
            "SELECT CASE WHEN OBJECT_ID(N'{db}.dbo._ExtensionsInfo', N'U') IS NULL THEN 0 ELSE (SELECT COUNT_BIG(*) FROM {db}.dbo._ExtensionsInfo) END"
        ),
        &[],
    )? {
        Some(SqlValue::Int(count)) => usize::try_from(count).unwrap_or(0),
        other => return Err(anyhow!("the number of extensions is not a number: {other:?}")),
    };
    if registered == 0 || removals.is_empty() {
        return Ok(Vec::new());
    }
    let adoptions: Vec<Adoption> = read_adoptions(sql, database)?;
    let inputs = ExtensionInputs {
        registered,
        adoptions_read: true,
        adoptions,
        ..ExtensionInputs::default()
    };
    let mut blockers = Vec::new();
    for object in &removals.objects {
        let changed = ChangedObject {
            kind: object.kind,
            name: &object.name,
            uuid: &object.uuid,
            table: "",
        };
        if let Err(refusal) = check_adoption(&inputs, &[changed]) {
            blockers.push(GateBlocker {
                row: object.uuid.clone(),
                reason: format!("{refusal}"),
            });
        }
    }
    Ok(blockers)
}

/// What the search-information edit did, for the report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchInfoRemoval {
    pub records_removed: usize,
    pub property_entries_removed: usize,
}

/// A `Params` row as stored, with the digest the plan saw.
pub(super) struct StoredRow {
    pub(super) name: String,
    pub(super) data_size: i64,
    pub(super) sha256: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn si_rows(client: &dyn SqlClient, db: &str) -> Result<Vec<StoredRow>> {
    let mut rows = Vec::new();
    client.read_rows(
        &format!(
            "SELECT FileName, CONVERT(bigint, DataSize), CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2), BinaryData FROM {db}.dbo.Params WHERE PartNo = 0 AND (FileName LIKE N'%.si' OR FileName = N'siVersions')"
        ),
        &[],
        &mut |mut row| {
            let name = row.take_text(0)?;
            let size = row.i64(1)?;
            let sha = row.text(2)?.to_ascii_uppercase();
            let bytes = row.take_binary(3)?;
            rows.push(StoredRow {
                name,
                data_size: size,
                sha256: sha,
                bytes,
            });
            Ok(())
        },
    )?;
    Ok(rows)
}

/// The bytes and the digest a `Params` row starts from: the text `rewrites` produced for it, when one of them
/// rewrites it (with the digest of the stored row the plan saw), else the stored row.
/// (`siVersions` is stored as plain text, the `.si` rows deflated.)
pub(super) fn base_of(
    stored: &[StoredRow],
    name: &str,
    rewrites: &[ParamsRewrite],
) -> Result<(i64, String, Vec<u8>, bool)> {
    let raw = name == SI_VERSIONS;
    if let Some(rewrite) = rewrites.iter().find(|rewrite| rewrite.file_name == name) {
        let bytes = if raw {
            rewrite.new_bytes.clone()
        } else {
            inflate_row(&rewrite.new_bytes)?
        };
        return Ok((
            rewrite.old_data_size,
            rewrite.old_sha256_hex.clone(),
            bytes,
            rewrite.set_creation,
        ));
    }
    let row = stored
        .iter()
        .find(|row| row.name == name)
        .ok_or_else(|| anyhow!("Params holds no row {name}"))?;
    let bytes = if raw {
        row.bytes.clone()
    } else {
        inflate_row(&row.bytes)?
    };
    Ok((row.data_size, row.sha256.clone(), bytes, true))
}

/// Puts a rewrite of `name` (from the plain text) in the list, in place of one that is there.
pub(super) fn put(
    rewrites: &mut Vec<ParamsRewrite>,
    name: &str,
    size: i64,
    sha: String,
    plain: &[u8],
    set_creation: bool,
) -> Result<()> {
    let rewrite = ParamsRewrite {
        file_name: name.to_owned(),
        old_data_size: size,
        old_sha256_hex: sha,
        new_bytes: if name == SI_VERSIONS {
            plain.to_vec()
        } else {
            deflate_row(plain)?
        },
        set_creation,
    };
    match rewrites.iter_mut().find(|known| known.file_name == name) {
        Some(known) => *known = rewrite,
        None => rewrites.push(rewrite),
    }
    Ok(())
}

/// The rewrites of the search information for the removals, on top of `existing` (what the new objects
/// and a restructuring of the same stage rewrite already): the record of each object leaves the main
/// row, its entry leaves the properties row, and each edited row gets a new version in `siVersions`.
/// A row `existing` rewrites keeps the digest the plan saw of the stored row; the edit is made on the
/// text `existing` produced.
pub fn plan_search_info(
    client: &dyn SqlClient,
    database: &str,
    removals: &Removals,
    existing: Vec<ParamsRewrite>,
) -> Result<(Vec<ParamsRewrite>, SearchInfoRemoval)> {
    let db = quote_ident(database)?;
    let stored = si_rows(client, &db)?;
    let mut rewrites = existing;

    let base_of = |name: &str, rewrites: &[ParamsRewrite]| base_of(&stored, name, rewrites);
    let mut summary = SearchInfoRemoval::default();
    let mut edited: Vec<String> = Vec::new();

    // The main row: the one that lists the objects.
    let mut candidates: Vec<String> = Vec::new();
    for row in &stored {
        let name = &row.name;
        if name == SI_VERSIONS {
            continue;
        }
        let Ok(plain) = inflate_row(&row.bytes) else {
            continue;
        };
        let Ok(main) = si::parse(&plain) else {
            continue;
        };
        if removals
            .objects
            .iter()
            .all(|object| main.index_of(&object.uuid).is_some())
        {
            candidates.push(name.clone());
        }
    }
    let [main_name] = candidates.as_slice() else {
        anyhow::bail!(
            "{} search-information rows list the removed objects (exactly one is expected)",
            candidates.len()
        );
    };
    let (size, sha, plain, set_creation) = base_of(main_name, &rewrites)?;
    let main = si::parse(&plain)?;
    let mut indexes = Vec::new();
    for object in &removals.objects {
        let index = main
            .index_of(&object.uuid)
            .ok_or_else(|| anyhow!("the search information does not list {}", object.uuid))?;
        let record = &main.records[index];
        anyhow::ensure!(
            record.parent == object.owner,
            "the search information gives {} the parent {}, not its owner {}",
            object.uuid,
            record.parent,
            object.owner
        );
        anyhow::ensure!(
            record.name == object.name,
            "the search information names {} {:?}, the descriptor {:?}",
            object.uuid,
            record.name,
            object.name
        );
        anyhow::ensure!(
            main.classes.get(record.kind).map(String::as_str) == Some(object.class.as_str()),
            "the search information files {} under another class than its owner does",
            object.uuid
        );
        anyhow::ensure!(
            main.subtree_end(index) == index + 1,
            "the search information lists children under {}",
            object.uuid
        );
        indexes.push(index);
    }
    let new_plain = si::edit_records(&plain, &main, &[], &indexes)?;
    let verify = si::parse(&new_plain)?;
    anyhow::ensure!(
        verify.records.len() + indexes.len() == main.records.len()
            && removals
                .objects
                .iter()
                .all(|object| verify.index_of(&object.uuid).is_none()),
        "the edited search information does not hold the expected records"
    );
    put(
        &mut rewrites,
        main_name,
        size,
        sha,
        &new_plain,
        set_creation,
    )?;
    edited.push(main_name.clone());
    summary.records_removed = indexes.len();

    // The properties row: the entries of the objects, when it has them.
    if stored.iter().any(|row| row.name == PROPERTIES_ROW) {
        let (size, sha, plain, set_creation) = base_of(PROPERTIES_ROW, &rewrites)?;
        let keys: BTreeSet<String> = removals
            .objects
            .iter()
            .map(|object| object.uuid.clone())
            .collect();
        let (new_plain, removed) = si::remove_property_entries(&plain, &keys)?;
        if removed > 0 {
            put(
                &mut rewrites,
                PROPERTIES_ROW,
                size,
                sha,
                &new_plain,
                set_creation,
            )?;
            edited.push(PROPERTIES_ROW.to_owned());
            summary.property_entries_removed = removed;
        }
    }

    // siVersions: every edited row gets a new version.
    let (size, sha, mut plain, _) = base_of(SI_VERSIONS, &rewrites)?;
    for name in &edited {
        plain = si::set_si_version(&plain, name, Uuid::new_v4())?;
    }
    put(&mut rewrites, SI_VERSIONS, size, sha, &plain, false)?;
    Ok((rewrites, summary))
}

#[cfg(test)]
#[path = "removals_tests.rs"]
mod tests;
