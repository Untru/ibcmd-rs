//! New rows in a staged configuration: a form or a template an existing
//! object gains, and body rows an existing object gains.
//!
//! Moving such rows into `Config` is the same `INSERT ... SELECT` as for any
//! other row; what the native apply adds around them (measured on 8.3.27.2214,
//! `docs/apply/own-apply.md`):
//!
//! - the owner's descriptor, staged with the new uuid in the list of its
//!   forms or templates, is replaced like any staged row;
//! - every new object is registered in `_ConfigChngR` for each ordinary
//!   exchange-plan node (not for the plan's own node), with no message number
//!   yet, and its body files are listed in `_ConfigChngR_ExtProps`;
//! - a new body row of an existing object is appended to that object's list
//!   in `_ConfigChngR_ExtProps`;
//! - the main search-information row (`Params`, [`super::si`]) gains a record
//!   per new object, and its `siVersions` entry a new version.
//!
//! [`analyze`] finds all of it, or says why it cannot: whatever is not one of
//! these shapes is a blocker, so that the platform's own apply does it.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Result, anyhow};
use serde::Serialize;
use uuid::Uuid;

use crate::metadata_model::brace::{Brace, parse_row};
use crate::metadata_model::export::names::{owned_groups, remove_owned};
use crate::sql::{SqlClient, SqlParam, SqlValue};

use super::gate::GateBlocker;
use super::model::{RowMeta, RowName, classify_name, quote_ident};
use super::si::{self, Insertion, NewRecord};
use super::sqlgen::ParamsRewrite;
use super::versions::{deflate_row, inflate_row, strip_bom};

const DESCRIPTOR_PATTERN: &str = "________-____-____-____-____________";

/// A new form or template.
#[derive(Debug, Clone, Serialize)]
pub struct NewObject {
    pub uuid: String,
    pub kind: &'static str,
    pub owner: String,
    pub name: String,
    /// The names of its body rows, in the order their files are registered.
    pub bodies: Vec<String>,
}

/// A body row an existing object gains.
#[derive(Debug, Clone, Serialize)]
pub struct NewBody {
    pub object: String,
    pub file_name: String,
}

/// An exchange-plan node changes are registered for.
#[derive(Debug, Clone, Serialize)]
pub struct RegistrationNode {
    /// The exchange plan's number: `_NodeTRef` as an integer, `_Node<plan>` is its node table.
    pub plan: i64,
    /// `_NodeTRef`, 4 bytes as hex.
    pub type_ref: String,
    /// `_NodeRRef`, 16 bytes as hex.
    pub reference: String,
}

/// What the staged new rows need beyond being moved.
#[derive(Debug, Default)]
pub struct NewObjects {
    pub objects: Vec<NewObject>,
    pub bodies: Vec<NewBody>,
    /// Owner descriptors that differ from the active ones by the added
    /// references only.
    pub owners: HashSet<String>,
    /// Lower-cased names of every new row that is accepted.
    pub rows: HashSet<String>,
    /// Kind of every new object, by uuid (for the gate's role check).
    pub kinds: HashMap<String, &'static str>,
    pub nodes: Vec<RegistrationNode>,
    /// The `Params` rows the search-information edit rewrites.
    pub search_info: Vec<ParamsRewrite>,
    /// Records the search-information edit adds.
    pub search_info_records: usize,
}

impl NewObjects {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.bodies.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub new: NewObjects,
    pub blockers: Vec<GateBlocker>,
}

pub struct AnalysisInput<'a> {
    pub client: &'a dyn SqlClient,
    pub database: &'a str,
    pub staged: &'a [RowMeta],
    /// The `Config` rows that share a name with a staged row, by lower-cased
    /// (name, part).
    pub active: &'a HashMap<(String, i32), RowMeta>,
    pub has_change_registrations: bool,
}

/// What the header of an object's descriptor says.
#[derive(Debug)]
pub(super) struct Described {
    pub(super) name: String,
    synonyms: Vec<(String, String)>,
    /// A form's first flag after the header (`0` or `1`).
    form_flag: Option<u8>,
}

fn is_header(node: &Brace) -> bool {
    let Some(members) = node.as_list() else {
        return false;
    };
    members.len() == 9
        && members.first().and_then(Brace::as_atom) == Some("3")
        && members
            .get(1)
            .and_then(Brace::as_list)
            .is_some_and(|identity| {
                identity.len() == 3 && identity.get(1).and_then(Brace::as_atom) == Some("0")
            })
        && members.get(2).and_then(Brace::as_str).is_some()
}

/// The object's own header and the list it stands in, with its index there.
fn find_header(node: &Brace) -> Option<(&[Brace], usize)> {
    let members = node.as_list()?;
    if let Some(index) = members.iter().position(is_header) {
        return Some((members, index));
    }
    members.iter().find_map(find_header)
}

pub(super) fn describe(
    kind: &str,
    uuid: &str,
    plain: &[u8],
) -> std::result::Result<Described, String> {
    let tree =
        parse_row(plain).map_err(|error| format!("the descriptor does not parse: {error}"))?;
    let (members, index) =
        find_header(&tree).ok_or_else(|| "the descriptor has no object header".to_owned())?;
    let header = members[index].as_list().expect("a header is a list");
    let identity = header[1].as_list().expect("a header names its id");
    let header_uuid = identity[2]
        .as_atom()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if header_uuid != uuid {
        return Err(format!(
            "the descriptor's own header carries the id {header_uuid}, not the row's name"
        ));
    }
    let name = header[2].as_str().unwrap_or_default().to_owned();
    if name.is_empty() || name.contains('"') {
        return Err(format!("the object's name {name:?} cannot be listed"));
    }
    let block = header[3]
        .as_list()
        .ok_or_else(|| "the header has no synonym list".to_owned())?;
    let count = block
        .first()
        .and_then(Brace::as_atom)
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| "the synonym list has no count".to_owned())?;
    if block.len() != 1 + 2 * count {
        return Err("the synonym list is not (language, text) pairs".to_owned());
    }
    let mut synonyms = Vec::new();
    for pair in block[1..].chunks(2) {
        match (pair[0].as_str(), pair[1].as_str()) {
            (Some(language), Some(text)) => {
                synonyms.push((language.to_owned(), text.replace('"', "\"\"")))
            }
            _ => return Err("the synonym list is not (language, text) pairs".to_owned()),
        }
    }
    let form_flag = if kind == "Form" {
        match members
            .get(index + 1)
            .and_then(Brace::as_atom)
            .and_then(|flag| flag.parse::<u8>().ok())
        {
            Some(flag @ (0 | 1)) => Some(flag),
            _ => return Err("a form's descriptor has no 0/1 flag behind its header".to_owned()),
        }
    } else {
        None
    };
    Ok(Described {
        name,
        synonyms,
        form_flag,
    })
}

/// The owner side of new objects: how its staged descriptor differs from the
/// active one.
struct OwnerDelta {
    staged_groups: Vec<crate::metadata_model::export::names::OwnedGroup>,
    /// `(uuid, class, kind)` of every object the staged descriptor adds.
    added: Vec<(String, String, &'static str)>,
}

fn blocker(blockers: &mut Vec<GateBlocker>, row: &str, reason: impl Into<String>) {
    blockers.push(GateBlocker {
        row: row.to_owned(),
        reason: reason.into(),
    });
}

fn read_blob(
    client: &dyn SqlClient,
    database: &str,
    table: &str,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    let db = quote_ident(database)?;
    let value = client.query_scalar(
        &format!("SELECT BinaryData FROM {db}.dbo.{table} WHERE FileName = @P1 AND PartNo = 0"),
        &[SqlParam::Text(name)],
    )?;
    match value {
        None => Ok(None),
        Some(SqlValue::Binary(bytes)) => Ok(Some(bytes)),
        Some(other) => Err(anyhow!("{table}.{name} is not binary: {other:?}")),
    }
}

pub fn analyze(input: &AnalysisInput<'_>) -> Result<Analysis> {
    let mut analysis = Analysis::default();
    let mut new_descriptors: Vec<&RowMeta> = Vec::new();
    let mut new_bodies: Vec<(&RowMeta, String, String)> = Vec::new();
    for row in input.staged {
        if row.part != 0 || input.active.contains_key(&row.key()) {
            continue;
        }
        match classify_name(&row.name) {
            RowName::Descriptor(_) => new_descriptors.push(row),
            RowName::Body { owner, suffix } => {
                new_bodies.push((row, owner.to_ascii_lowercase(), suffix.to_owned()))
            }
            // Service rows and unknown names are the gate's to refuse.
            RowName::Service(_) | RowName::Other => {}
        }
    }
    if new_descriptors.is_empty() && new_bodies.is_empty() {
        return Ok(analysis);
    }
    let client = input.client;
    let db = quote_ident(input.database)?;
    let mut blockers: Vec<GateBlocker> = Vec::new();

    let new_uuids: HashSet<String> = new_descriptors
        .iter()
        .map(|row| row.name.to_ascii_lowercase())
        .collect();

    // The staged descriptors that name a new uuid are the candidate owners;
    // the new descriptors themselves are read here as well.
    let mut owner_texts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut new_texts: HashMap<String, Vec<u8>> = HashMap::new();
    if !new_uuids.is_empty() {
        client.read_rows(
            &format!(
                "SELECT FileName, BinaryData FROM {db}.dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE N'{DESCRIPTOR_PATTERN}'"
            ),
            &[],
            &mut |mut row| {
                let name = row.take_text(0)?.to_ascii_lowercase();
                let bytes = row.take_binary(1)?;
                let Ok(plain) = inflate_row(&bytes) else {
                    return Ok(());
                };
                let text = String::from_utf8_lossy(strip_bom(&plain)).to_ascii_lowercase();
                if new_uuids.contains(&name) {
                    if new_uuids
                        .iter()
                        .any(|other| *other != name && text.contains(other.as_str()))
                    {
                        new_texts.insert(name.clone(), Vec::new());
                    } else {
                        new_texts.insert(name, plain);
                    }
                } else if new_uuids.iter().any(|uuid| text.contains(uuid.as_str())) {
                    owner_texts.push((name, plain));
                }
                Ok(())
            },
        )?;
    }

    // Owners: the staged descriptor must be the active one plus references.
    let mut owner_deltas: BTreeMap<String, OwnerDelta> = BTreeMap::new();
    let mut claims: HashMap<String, Vec<String>> = HashMap::new();
    for (owner, staged_plain) in &owner_texts {
        let Some(active_plain) = read_blob(client, input.database, "Config", owner)? else {
            blocker(
                &mut blockers,
                owner,
                "a new object is listed by a descriptor that is itself new: only a form or a template of an existing object can be added",
            );
            continue;
        };
        let (Ok(staged_tree), Ok(active_tree)) = (
            parse_row(staged_plain),
            inflate_row(&active_plain).and_then(|plain| parse_row(&plain)),
        ) else {
            blocker(&mut blockers, owner, "a descriptor that does not parse");
            continue;
        };
        let staged_groups = owned_groups(&staged_tree);
        let active_groups = owned_groups(&active_tree);
        let active_members: HashSet<(String, String)> = active_groups
            .iter()
            .flat_map(|group| {
                group
                    .members
                    .iter()
                    .map(|member| (group.class.clone(), member.clone()))
            })
            .collect();
        let staged_members: HashSet<(String, String)> = staged_groups
            .iter()
            .flat_map(|group| {
                group
                    .members
                    .iter()
                    .map(|member| (group.class.clone(), member.clone()))
            })
            .collect();
        if active_members.difference(&staged_members).next().is_some() {
            blocker(
                &mut blockers,
                owner,
                "the staged descriptor no longer lists an object the active one lists",
            );
            continue;
        }
        let mut added = Vec::new();
        let mut ok = true;
        for group in &staged_groups {
            for member in &group.members {
                if active_members.contains(&(group.class.clone(), member.clone())) {
                    continue;
                }
                if !new_uuids.contains(member) {
                    blocker(
                        &mut blockers,
                        owner,
                        format!(
                            "the staged descriptor lists {member}, which is neither in Config nor a new row"
                        ),
                    );
                    ok = false;
                    continue;
                }
                if !matches!(group.kind, "Form" | "Template") {
                    blocker(
                        &mut blockers,
                        member,
                        format!(
                            "a new {}: only a new form or template is supported",
                            group.kind.to_lowercase()
                        ),
                    );
                    ok = false;
                    continue;
                }
                added.push((member.clone(), group.class.clone(), group.kind));
            }
        }
        if !ok {
            continue;
        }
        let mut stripped = staged_tree.clone();
        let added_set: HashSet<String> = added.iter().map(|(uuid, _, _)| uuid.clone()).collect();
        remove_owned(&mut stripped, &added_set);
        if stripped != active_tree {
            blocker(
                &mut blockers,
                owner,
                "the descriptor changes more than the list of its forms and templates: a metadata change, possibly structural",
            );
            continue;
        }
        for (uuid, _, _) in &added {
            claims.entry(uuid.clone()).or_default().push(owner.clone());
        }
        owner_deltas.insert(
            owner.clone(),
            OwnerDelta {
                staged_groups,
                added,
            },
        );
    }

    // The new objects.
    let mut objects: Vec<NewObject> = Vec::new();
    let mut described: HashMap<String, Described> = HashMap::new();
    let mut group_of: HashMap<String, (String, String)> = HashMap::new(); // uuid -> (owner, class)
    let mut sorted_new: Vec<&RowMeta> = new_descriptors.clone();
    sorted_new.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    for row in sorted_new {
        let uuid = row.name.to_ascii_lowercase();
        let owners = claims.get(&uuid).cloned().unwrap_or_default();
        let [owner] = owners.as_slice() else {
            blocker(
                &mut blockers,
                &row.name,
                if owners.is_empty() {
                    "a new object that no staged descriptor of an existing object lists".to_owned()
                } else {
                    format!("a new object listed by {} descriptors", owners.len())
                },
            );
            continue;
        };
        let delta = &owner_deltas[owner];
        let (_, class, kind) = delta
            .added
            .iter()
            .find(|(member, _, _)| *member == uuid)
            .expect("a claimed object is in the delta")
            .clone();
        let Some(plain) = new_texts.get(&uuid).filter(|plain| !plain.is_empty()) else {
            blocker(
                &mut blockers,
                &row.name,
                "the new object's descriptor names another new object",
            );
            continue;
        };
        let info = match describe(kind, &uuid, plain) {
            Ok(info) => info,
            Err(reason) => {
                blocker(&mut blockers, &row.name, reason);
                continue;
            }
        };
        let mut bodies: Vec<(&RowMeta, &str)> = new_bodies
            .iter()
            .filter(|(_, body_owner, _)| *body_owner == uuid)
            .map(|(row, _, suffix)| (*row, suffix.as_str()))
            .collect();
        bodies.sort_by_key(|(_, suffix)| suffix.to_owned());
        let allowed: &[&str] = if kind == "Form" { &["0", "1"] } else { &["0"] };
        let suffixes: Vec<&str> = bodies.iter().map(|(_, suffix)| *suffix).collect();
        if !suffixes.contains(&"0") || suffixes.iter().any(|suffix| !allowed.contains(suffix)) {
            blocker(
                &mut blockers,
                &row.name,
                format!(
                    "a new {} with the body rows {:?}: only {:?} (with .0) are supported",
                    kind.to_lowercase(),
                    suffixes,
                    allowed
                ),
            );
            continue;
        }
        group_of.insert(uuid.clone(), (owner.clone(), class));
        objects.push(NewObject {
            uuid: uuid.clone(),
            kind,
            owner: owner.clone(),
            name: info.name.clone(),
            bodies: bodies.iter().map(|(row, _)| row.name.clone()).collect(),
        });
        described.insert(uuid, info);
    }
    // New body rows of existing objects.
    let mut appended: Vec<NewBody> = Vec::new();
    let leftover_bodies: Vec<&(&RowMeta, String, String)> = new_bodies
        .iter()
        .filter(|(_, owner, _)| !new_uuids.contains(owner))
        .collect();
    if !leftover_bodies.is_empty() {
        let mut existing: HashSet<String> = HashSet::new();
        let owners: Vec<String> = leftover_bodies
            .iter()
            .map(|(_, owner, _)| owner.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for chunk in owners.chunks(400) {
            let placeholders = (1..=chunk.len())
                .map(|index| format!("@P{index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let params: Vec<SqlParam<'_>> = chunk
                .iter()
                .map(|owner| SqlParam::Text(owner.as_str()))
                .collect();
            client.read_rows(
                &format!(
                    "SELECT FileName FROM {db}.dbo.Config WHERE PartNo = 0 AND FileName IN ({placeholders})"
                ),
                &params,
                &mut |row| {
                    existing.insert(row.text(0)?.to_ascii_lowercase());
                    Ok(())
                },
            )?;
        }
        let mut registered: HashSet<String> = HashSet::new();
        if input.has_change_registrations {
            for chunk in owners.chunks(400) {
                // `_MDObjID` is the id in the platform's byte order.
                let literals = chunk
                    .iter()
                    .filter_map(|owner| Uuid::parse_str(owner).ok())
                    .map(|uuid| format!("0x{}", super::model::hex_upper(&uuid.to_bytes_le())))
                    .collect::<Vec<_>>();
                if literals.is_empty() {
                    continue;
                }
                client.read_rows(
                    &format!(
                        "SELECT DISTINCT CONVERT(varchar(32), _MDObjID, 2) FROM {db}.dbo._ConfigChngR WHERE _MDObjID IN ({})",
                        literals.join(", ")
                    ),
                    &[],
                    &mut |row| {
                        registered.insert(row.text(0)?.to_ascii_uppercase());
                        Ok(())
                    },
                )?;
            }
        }
        let mut per_owner: HashMap<&str, usize> = HashMap::new();
        for (row, owner, _) in &leftover_bodies {
            *per_owner.entry(owner.as_str()).or_default() += 1;
            if !existing.contains(owner) {
                blocker(
                    &mut blockers,
                    &row.name,
                    "a new body row of an object without a descriptor row of its own (a nested object or the configuration): its change registration belongs to another object",
                );
                continue;
            }
            if !input.has_change_registrations {
                blocker(
                    &mut blockers,
                    &row.name,
                    "a new body row, and the database keeps no change registrations to extend",
                );
                continue;
            }
            let id = Uuid::parse_str(owner)
                .map(|uuid| super::model::hex_upper(&uuid.to_bytes_le()))
                .unwrap_or_default();
            if !registered.contains(&id) {
                blocker(
                    &mut blockers,
                    &row.name,
                    "a new body row of an object that no exchange-plan node has registered",
                );
                continue;
            }
            appended.push(NewBody {
                object: owner.clone(),
                file_name: row.name.clone(),
            });
        }
        for body in &appended {
            if per_owner[body.object.as_str()] > 1 {
                blocker(
                    &mut blockers,
                    &body.file_name,
                    "several new body rows of one object: the order of their registration is not known",
                );
            }
        }
    }

    // Change registrations need the nodes.
    let mut nodes = Vec::new();
    if input.has_change_registrations && (!objects.is_empty() || !appended.is_empty()) {
        match registration_nodes(client, &db) {
            Ok(found) => nodes = found,
            Err(reason) => {
                for object in &objects {
                    blocker(&mut blockers, &object.uuid, reason.clone());
                }
                objects.clear();
            }
        }
    }

    // The search information.
    let mut search_info = Vec::new();
    let mut search_info_records = 0;
    if !objects.is_empty() {
        match plan_search_info(client, &db, &objects, &described, &group_of, &owner_deltas) {
            Ok((rewrites, records)) => {
                search_info = rewrites;
                search_info_records = records;
            }
            Err(reason) => {
                for object in &objects {
                    blocker(&mut blockers, &object.uuid, reason.to_string());
                }
                objects.clear();
            }
        }
    }

    let mut rows: HashSet<String> = HashSet::new();
    for object in &objects {
        rows.insert(object.uuid.clone());
        for body in &object.bodies {
            rows.insert(body.to_ascii_lowercase());
        }
    }
    for body in &appended {
        rows.insert(body.file_name.to_ascii_lowercase());
    }
    let owners: HashSet<String> = objects.iter().map(|object| object.owner.clone()).collect();
    let kinds = objects
        .iter()
        .map(|object| (object.uuid.clone(), object.kind))
        .collect();
    analysis.new = NewObjects {
        objects,
        bodies: appended,
        owners,
        rows,
        kinds,
        nodes,
        search_info,
        search_info_records,
    };
    analysis.blockers = blockers;
    Ok(analysis)
}

/// The nodes changes are registered for: every node of an exchange plan that already registers
/// changes and is not the plan's own node -- **including the nodes that have no row in the
/// register** (a node with an initial image has none; the native apply inserts the rows).
pub(super) fn registration_nodes(
    client: &dyn SqlClient,
    db: &str,
) -> std::result::Result<Vec<RegistrationNode>, String> {
    let failed =
        |error: anyhow::Error| format!("the change-registration nodes cannot be read: {error}");
    let mut seen: Vec<(i64, String, String)> = Vec::new();
    client
        .read_rows(
            &format!(
                "SELECT DISTINCT CONVERT(bigint, CONVERT(int, _NodeTRef)), CONVERT(varchar(8), _NodeTRef, 2), CONVERT(varchar(32), _NodeRRef, 2) FROM {db}.dbo._ConfigChngR ORDER BY 1, 3"
            ),
            &[],
            &mut |row| {
                seen.push((
                    row.i64(0)?,
                    row.text(1)?.to_ascii_uppercase(),
                    row.text(2)?.to_ascii_uppercase(),
                ));
                Ok(())
            },
        )
        .map_err(failed)?;
    if seen.is_empty() {
        return Err("no node registers changes yet".to_owned());
    }
    let mut nodes = Vec::new();
    let mut tables: HashMap<i64, HashMap<String, (bool, bool)>> = HashMap::new();
    for (plan, _, _) in &seen {
        if tables.contains_key(plan) {
            continue;
        }
        let table = format!("_Node{plan}");
        let present = client
            .query_scalar(
                &format!("SELECT CASE WHEN OBJECT_ID(N'{db}.dbo.{table}', N'U') IS NULL THEN 0 ELSE 1 END"),
                &[],
            )
            .map_err(failed)?;
        if !matches!(present, Some(SqlValue::Int(1))) {
            return Err(format!(
                "the exchange plan {plan} has no node table {table}"
            ));
        }
        let mut map = HashMap::new();
        client
            .read_rows(
                &format!(
                    "SELECT CONVERT(varchar(32), _IDRRef, 2), CASE WHEN _PredefinedID = 0x00000000000000000000000000000000 THEN 0 ELSE 1 END, CONVERT(int, _Marked) FROM {db}.dbo.{table}"
                ),
                &[],
                &mut |row| {
                    map.insert(
                        row.text(0)?.to_ascii_uppercase(),
                        (row.i64(1)? == 1, row.i64(2)? != 0),
                    );
                    Ok(())
                },
            )
            .map_err(failed)?;
        tables.insert(*plan, map);
    }
    // every registered node must be in its plan's node table
    for (plan, _, reference) in &seen {
        if !tables[plan].contains_key(reference) {
            return Err(format!(
                "a registered node {reference} of the exchange plan {plan} has no row in its node table"
            ));
        }
    }
    // the nodes of each plan, the registered ones and the ones without rows
    let mut plan_types: Vec<(i64, String)> = Vec::new();
    for (plan, type_ref, _) in &seen {
        if !plan_types.iter().any(|(known, _)| known == plan) {
            plan_types.push((*plan, type_ref.clone()));
        }
    }
    for (plan, type_ref) in plan_types {
        let mut references: Vec<(&String, &(bool, bool))> = tables[&plan].iter().collect();
        references.sort();
        for (reference, (is_own_node, marked)) in references {
            if *is_own_node {
                continue;
            }
            if *marked {
                return Err(format!(
                    "the node {reference} is marked for deletion: whether it registers changes is unknown"
                ));
            }
            nodes.push(RegistrationNode {
                plan,
                type_ref: type_ref.clone(),
                reference: reference.clone(),
            });
        }
    }
    Ok(nodes)
}

/// The rewrites of the main search-information row and of `siVersions`.
fn plan_search_info(
    client: &dyn SqlClient,
    db: &str,
    objects: &[NewObject],
    described: &HashMap<String, Described>,
    group_of: &HashMap<String, (String, String)>,
    owner_deltas: &BTreeMap<String, OwnerDelta>,
) -> Result<(Vec<ParamsRewrite>, usize)> {
    let owners: HashSet<&str> = objects.iter().map(|object| object.owner.as_str()).collect();
    struct SiRow {
        name: String,
        data_size: i64,
        sha256: String,
        plain: Vec<u8>,
    }
    let mut candidates: Vec<(SiRow, si::SiMain)> = Vec::new();
    client.read_rows(
        &format!(
            "SELECT FileName, CONVERT(bigint, DataSize), CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2), BinaryData FROM {db}.dbo.Params WHERE PartNo = 0 AND FileName LIKE N'%.si'"
        ),
        &[],
        &mut |mut row| {
            let name = row.take_text(0)?;
            let data_size = row.i64(1)?;
            let sha256 = row.text(2)?.to_ascii_uppercase();
            let bytes = row.take_binary(3)?;
            let Ok(plain) = inflate_row(&bytes) else {
                return Ok(());
            };
            let Ok(main) = si::parse(&plain) else {
                return Ok(());
            };
            if owners.iter().all(|owner| main.index_of(owner).is_some()) {
                candidates.push((
                    SiRow {
                        name,
                        data_size,
                        sha256,
                        plain,
                    },
                    main,
                ));
            }
            Ok(())
        },
    )?;
    let [(row, main)] = candidates.as_slice() else {
        anyhow::bail!(
            "{} search-information rows list the owners of the new objects (exactly one is expected)",
            candidates.len()
        );
    };
    let extra_parts = client.query_scalar(
        &format!("SELECT COUNT_BIG(*) FROM {db}.dbo.Params WHERE FileName = @P1 AND PartNo <> 0"),
        &[SqlParam::Text(&row.name)],
    )?;
    if !matches!(extra_parts, Some(SqlValue::Int(0))) {
        anyhow::bail!("the search-information row {} has several parts", row.name);
    }

    // Where each new record goes.
    let mut pending: BTreeMap<usize, Vec<(usize, NewRecord)>> = BTreeMap::new();
    for (owner, delta) in owner_deltas {
        for group in &delta.staged_groups {
            let new_in_group: Vec<&str> = group
                .members
                .iter()
                .filter(|member| {
                    objects
                        .iter()
                        .any(|object| object.uuid == **member && object.owner == *owner)
                })
                .map(String::as_str)
                .collect();
            if new_in_group.is_empty() {
                continue;
            }
            let kind = main.kind_of_class(&group.class).ok_or_else(|| {
                anyhow!(
                    "the search information has no class {} for the {} the owner gains",
                    group.class,
                    group.kind.to_lowercase()
                )
            })?;
            // The listed members the row already holds, in list order, must be
            // in the same order there.
            let mut known: Vec<(usize, &str)> = Vec::new();
            for (position, member) in group.members.iter().enumerate() {
                if new_in_group.contains(&member.as_str()) {
                    continue;
                }
                if main.index_of(member).is_some() {
                    known.push((position, member.as_str()));
                }
            }
            for pair in known.windows(2) {
                let (a, b) = (
                    main.index_of(pair[0].1).expect("known"),
                    main.index_of(pair[1].1).expect("known"),
                );
                anyhow::ensure!(
                    a < b,
                    "the search information lists {} and {} in another order than the descriptor",
                    pair[0].1,
                    pair[1].1
                );
            }
            for member in &new_in_group {
                let position = group
                    .members
                    .iter()
                    .position(|listed| listed == member)
                    .expect("a new member is listed");
                let previous = known
                    .iter()
                    .rev()
                    .find(|(known_position, _)| *known_position < position)
                    .map(|(_, uuid)| *uuid);
                let next = known
                    .iter()
                    .find(|(known_position, _)| *known_position > position)
                    .map(|(_, uuid)| *uuid);
                let at = main.place(owner, kind, previous, next)?;
                let info = &described[*member];
                let object = objects
                    .iter()
                    .find(|object| object.uuid == *member)
                    .expect("a new member is an object");
                let flags = if object.kind == "Form" {
                    (info.form_flag.unwrap_or(0), 0)
                } else {
                    (0, 0)
                };
                debug_assert_eq!(group_of[*member].0, *owner);
                pending.entry(at).or_default().push((
                    position,
                    NewRecord {
                        uuid: (*member).to_owned(),
                        parent: owner.clone(),
                        kind,
                        name: info.name.clone(),
                        synonyms: info.synonyms.clone(),
                        flags,
                    },
                ));
            }
        }
    }
    let total: usize = pending.values().map(Vec::len).sum();
    anyhow::ensure!(
        total == objects.len(),
        "{} of {} new objects could be placed",
        total,
        objects.len()
    );
    // Records that go to the same place keep the order of their groups (the
    // order the other owners of the kind list them), then their list order.
    let mut insertions: Vec<Insertion> = Vec::new();
    for (at, mut records) in pending {
        if let Some(first) = records.first() {
            let owner_kind = main.records[main
                .index_of(&first.1.parent)
                .expect("a new record's owner is in the row")]
            .kind;
            let order = main.child_order(owner_kind)?;
            let mut failure = None;
            records.sort_by(|(left_position, left), (right_position, right)| {
                if left.kind == right.kind {
                    return left_position.cmp(right_position);
                }
                if order.contains(&(left.kind, right.kind)) {
                    std::cmp::Ordering::Less
                } else if order.contains(&(right.kind, left.kind)) {
                    std::cmp::Ordering::Greater
                } else {
                    failure = Some((left.kind, right.kind));
                    std::cmp::Ordering::Equal
                }
            });
            if let Some((left, right)) = failure {
                anyhow::bail!(
                    "the order of new children of kind {left} and {right} of one owner is unknown"
                );
            }
        }
        insertions.push(Insertion {
            at,
            records: records.into_iter().map(|(_, record)| record).collect(),
        });
    }
    let new_plain = si::insert_records(&row.plain, main, &insertions)?;
    let verify = si::parse(&new_plain)?;
    anyhow::ensure!(
        verify.records.len() == main.records.len() + total,
        "the edited search information does not hold the expected number of records"
    );

    // siVersions: the edited row gets a new version.
    let versions = client.query_rows(
        &format!(
            "SELECT CONVERT(bigint, DataSize), CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2), BinaryData FROM {db}.dbo.Params WHERE FileName = N'siVersions' AND PartNo = 0"
        ),
        &[],
    )?;
    let [versions] = versions.as_slice() else {
        anyhow::bail!("Params holds no single siVersions row");
    };
    let versions_bytes = versions.binary(2)?.to_vec();
    let new_versions = si::set_si_version(&versions_bytes, &row.name, Uuid::new_v4())?;
    Ok((
        vec![
            ParamsRewrite {
                file_name: row.name.clone(),
                old_data_size: row.data_size,
                old_sha256_hex: row.sha256.clone(),
                new_bytes: deflate_row(&new_plain)?,
                set_creation: true,
            },
            ParamsRewrite {
                file_name: "siVersions".to_owned(),
                old_data_size: versions.i64(0)?,
                old_sha256_hex: versions.text(1)?.to_ascii_uppercase(),
                new_bytes: new_versions,
                set_creation: false,
            },
        ],
        total,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORM: &str = "\u{feff}{1,\r\n{1,\r\n{0,\r\n{13,\r\n{3,\r\n{1,0,ade5fa33-4a02-5feb-b400-a41f22e208be},\"ЯНоваяФормаApply\",\r\n{1,\"ru\",\"Форма \"\"А\"\"\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},1,1,\r\n{2,\r\n{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1},\r\n{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}\r\n}\r\n},\r\n{0}\r\n}\r\n},0}";
    const TEMPLATE: &str = "\u{feff}{1,\r\n{2,0,\r\n{3,\r\n{1,0,0808b882-fde2-508b-9da9-4c3765442bbf},\"ЯНовыйМакетApply\",\r\n{1,\"ru\",\"Макет\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},0}";

    #[test]
    fn a_form_descriptor_gives_its_name_synonyms_and_flag() {
        let info = describe(
            "Form",
            "ade5fa33-4a02-5feb-b400-a41f22e208be",
            FORM.as_bytes(),
        )
        .unwrap();
        assert_eq!(info.name, "ЯНоваяФормаApply");
        // quotes stay doubled: that is how the search information writes them
        assert_eq!(
            info.synonyms,
            vec![("ru".to_owned(), "Форма \"\"А\"\"".to_owned())]
        );
        assert_eq!(info.form_flag, Some(1));
    }

    #[test]
    fn a_template_descriptor_has_no_flag() {
        let info = describe(
            "Template",
            "0808b882-fde2-508b-9da9-4c3765442bbf",
            TEMPLATE.as_bytes(),
        )
        .unwrap();
        assert_eq!(info.name, "ЯНовыйМакетApply");
        assert_eq!(info.form_flag, None);
    }

    #[test]
    fn a_descriptor_of_another_id_is_refused() {
        let error = describe(
            "Form",
            "00000000-0000-0000-0000-000000000001",
            FORM.as_bytes(),
        );
        assert!(error.unwrap_err().contains("not the row's name"));
    }

    #[test]
    fn the_owner_delta_is_only_the_added_references() {
        let active = "\u{feff}{1,{3daea016-69b7-4ed4-9453-127911372fe6,2,4f549963-efb2-4d60-9681-322a4740fb01,ab66a386-b7a6-4f20-8e56-ffecbbfeb86b},{d5b0e5ed-256d-401c-9c36-f630cafd8a62,1,8d86192b-f421-4328-983d-40179c7aa2c9},{7,x}}";
        let staged = "\u{feff}{1,{3daea016-69b7-4ed4-9453-127911372fe6,3,4f549963-efb2-4d60-9681-322a4740fb01,0808b882-fde2-508b-9da9-4c3765442bbf,ab66a386-b7a6-4f20-8e56-ffecbbfeb86b},{d5b0e5ed-256d-401c-9c36-f630cafd8a62,2,8d86192b-f421-4328-983d-40179c7aa2c9,ade5fa33-4a02-5feb-b400-a41f22e208be},{7,x}}";
        let active_tree = parse_row(active.as_bytes()).unwrap();
        let mut staged_tree = parse_row(staged.as_bytes()).unwrap();
        let added: HashSet<String> = [
            "0808b882-fde2-508b-9da9-4c3765442bbf",
            "ade5fa33-4a02-5feb-b400-a41f22e208be",
        ]
        .iter()
        .map(|uuid| (*uuid).to_owned())
        .collect();
        assert_eq!(remove_owned(&mut staged_tree, &added), 2);
        assert_eq!(staged_tree, active_tree);
        // another change in the same descriptor is not "only references"
        let mut changed = parse_row(staged.replace("{7,x}", "{7,y}").as_bytes()).unwrap();
        remove_owned(&mut changed, &added);
        assert_ne!(changed, active_tree);
    }
}

#[cfg(test)]
#[path = "objects_analysis_tests.rs"]
mod analysis_tests;
