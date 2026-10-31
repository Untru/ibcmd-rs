//! Change registrations of the nodes of a distributed infobase for the objects a stage changes.
//!
//! The change register `_ConfigChngR` holds one row per (node, object) the platform keeps track of, and
//! `_ConfigChngR_ExtProps` the files of the object that changed since the node's last message. A node that
//! got an initial image has **no rows** (the platform deletes them when it creates the image); a node made
//! afterwards has none either. The native apply registers a changed object at such a node by *inserting* the
//! row, `_MessageNo` NULL, with the list of the changed files as it lists them (measured on twins of the
//! БСП 8.3.27 clone with two imaged nodes and one plain new node: `docs/apply/own-apply.md`, "Exchange
//! plans"). An apply that only updates the rows that exist leaves those nodes without the change, and the
//! message for the node carries nothing.
//!
//! What this module decides at plan time, from the staged names and the register:
//!
//! - **the nodes**: every node of an exchange plan that registers changes, except the plan's own node
//!   ([`super::objects::registration_nodes`], read from the plans' node tables and not only from the
//!   register);
//! - **the changed objects**: the objects whose uuid a staged (or dropped) name starts with, and those whose
//!   file list names a staged row, when the register knows them at some node;
//! - **the pairs to insert**: the (node, object) pairs with no row, and the file list of each: the staged and
//!   dropped body files that belong to the object (none when only its descriptor is staged).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Result;
use uuid::Uuid;

use super::errors::NeedsNativeApply;
use super::model::{RowMeta, hex_upper, quote_ident};
use super::objects::{RegistrationNode, registration_nodes};
use super::sqlgen::{AppendedFile, ObjectRegistration};
use crate::sql::SqlClient;

/// What the plan hands to the script and to the recovery artifact.
#[derive(Debug, Default, Clone)]
pub struct RegistrationPlan {
    /// The nodes that register changes (all but the plans' own nodes).
    pub nodes: Vec<RegistrationNode>,
    /// The owners of the rows a `deleted` list names, `_MDObjID` hex: their rows are reset, too.
    pub extra_objects: Vec<String>,
    /// The changed objects that miss a row at some node, with the files their lists get.
    pub additions: Vec<ObjectRegistration>,
    /// The (node index, object hex) pairs that get a row.
    pub missing: Vec<(usize, String)>,
    /// Rows the script inserts into `_ConfigChngR`, and into `_ConfigChngR_ExtProps` for them.
    pub added_rows: i64,
    pub added_file_rows: i64,
    /// The bodies a `deleted` list names, appended to the lists of the existing rows.
    pub dropped_files: Vec<AppendedFile>,
    /// (plan number, how many nodes of it are not the plan's own): asserted again under the locks.
    pub node_counts: Vec<(i64, i64)>,
    /// How many objects the stage changes that the register knows.
    pub changed_objects: usize,
}

impl RegistrationPlan {
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty() && self.extra_objects.is_empty() && self.dropped_files.is_empty()
    }
}

/// `_MDObjID` of the object a row name belongs to: the uuid the name starts with, in the platform's byte
/// order, upper-case hex. `None` for names that are not `<uuid>`, `<uuid>.<n>` or an alias of them.
pub fn object_of(name: &str) -> Option<String> {
    let head = name.get(..36)?;
    let uuid = Uuid::parse_str(head).ok()?;
    match name.as_bytes().get(36) {
        None | Some(b'.') | Some(b'_') => Some(hex_upper(&uuid.to_bytes_le())),
        _ => None,
    }
}

/// Whether the row is a body file (`<uuid>.<n>` or `<uuid>_dynupdate_<generation>.<n>`), which is what
/// an object's file list names; a descriptor and its alias are not files.
pub fn is_body_file(name: &str) -> bool {
    if object_of(name).is_none() || name.len() <= 37 {
        return false;
    }
    match name.rfind('.') {
        Some(dot) if dot >= 36 => {
            let suffix = &name[dot + 1..];
            !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    }
}

/// The body files of each object, from the staged names and the dropped names, and the ones
/// the register's lists name for objects whose names do not start with their own uuid.
pub fn files_by_object(
    staged: &[String],
    dropped: &[String],
    listed: &[(String, String)],
) -> BTreeMap<String, Vec<String>> {
    let mut files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for name in staged.iter().chain(dropped.iter()) {
        if is_body_file(name)
            && let Some(object) = object_of(name)
        {
            files.entry(object).or_default().insert(name.clone());
        }
    }
    for (object, file) in listed {
        files
            .entry(object.clone())
            .or_default()
            .insert(file.clone());
    }
    files
        .into_iter()
        .map(|(object, names)| (object, names.into_iter().collect()))
        .collect()
}

/// Plans the registrations: which objects the stage changes, which nodes miss a row for them.
pub fn plan(
    client: &dyn SqlClient,
    database: &str,
    staged: &[RowMeta],
    dropped: &[String],
) -> Result<RegistrationPlan> {
    let db = quote_ident(database)?;
    let staged_names: Vec<String> = staged.iter().map(|row| row.name.clone()).collect();

    // Candidate objects: by the uuid the names start with.
    let mut candidates: BTreeSet<String> = BTreeSet::new();
    for name in &staged_names {
        if let Some(object) = object_of(name) {
            candidates.insert(object);
        }
    }
    let mut extra_objects: BTreeSet<String> = BTreeSet::new();
    for name in dropped {
        if let Some(object) = object_of(name) {
            extra_objects.insert(object.clone());
            candidates.insert(object);
        }
    }

    // Objects whose file lists name a staged row (a nested object's file under its owner).
    let mut listed: Vec<(String, String)> = Vec::new();
    let mut known: HashSet<String> = HashSet::new();
    client.read_rows(
        &format!(
            "SELECT DISTINCT CONVERT(varchar(32), r._MDObjID, 2), s.FileName FROM {db}.dbo._ConfigChngR_ExtProps e JOIN {db}.dbo.ConfigSave s ON s.FileName = e._FileName JOIN {db}.dbo._ConfigChngR r ON r._IDRRef = e._ConfigChngR_IDRRef WHERE s.PartNo = 0"
        ),
        &[],
        &mut |row| {
            let object = row.text(0)?.to_ascii_uppercase();
            candidates.insert(object.clone());
            listed.push((object, row.text(1)?.to_owned()));
            Ok(())
        },
    )?;

    // Which of them the register knows, and at which nodes.
    let mut present: HashSet<(String, String)> = HashSet::new();
    let list: Vec<String> = candidates.iter().cloned().collect();
    for chunk in list.chunks(400) {
        let literals = chunk
            .iter()
            .map(|object| format!("0x{object}"))
            .collect::<Vec<_>>()
            .join(", ");
        client.read_rows(
            &format!(
                "SELECT CONVERT(varchar(32), _NodeRRef, 2), CONVERT(varchar(32), _MDObjID, 2) FROM {db}.dbo._ConfigChngR WHERE _MDObjID IN ({literals})"
            ),
            &[],
            &mut |row| {
                let node = row.text(0)?.to_ascii_uppercase();
                let object = row.text(1)?.to_ascii_uppercase();
                known.insert(object.clone());
                present.insert((node, object));
                Ok(())
            },
        )?;
    }
    let changed: Vec<String> = candidates
        .into_iter()
        .filter(|object| known.contains(object))
        .collect();

    let mut plan = RegistrationPlan {
        changed_objects: changed.len(),
        extra_objects: extra_objects
            .into_iter()
            .filter(|object| known.contains(object))
            .collect(),
        ..RegistrationPlan::default()
    };
    // The bodies a `deleted` list names are appended to the lists that exist.
    for name in dropped {
        if is_body_file(name)
            && let Some(object) = object_of(name)
            && known.contains(&object)
        {
            plan.dropped_files.push(AppendedFile {
                object_hex: object,
                file_name: name.clone(),
            });
        }
    }
    if changed.is_empty() {
        return Ok(plan);
    }

    // The nodes: only when some changed object exists (a stage that changes nothing the register
    // knows needs no answer about the nodes).
    let nodes = match registration_nodes(client, &db) {
        Ok(nodes) => nodes,
        Err(reason) if reason == "no node registers changes yet" => Vec::new(),
        Err(reason) => return Err(NeedsNativeApply::apply(reason).into()),
    };
    let mut counts: BTreeMap<i64, i64> = BTreeMap::new();
    for node in &nodes {
        *counts.entry(node.plan).or_default() += 1;
    }
    plan.node_counts = counts.into_iter().collect();

    let lists = files_by_object(&staged_names, dropped, &listed);
    for object in &changed {
        let files = lists.get(object).cloned().unwrap_or_default();
        let mut missing_here = 0i64;
        for (index, node) in nodes.iter().enumerate() {
            if !present.contains(&(node.reference.to_ascii_uppercase(), object.clone())) {
                plan.missing.push((index, object.clone()));
                missing_here += 1;
            }
        }
        if missing_here > 0 {
            plan.added_rows += missing_here;
            plan.added_file_rows += missing_here * files.len() as i64;
            plan.additions.push(ObjectRegistration {
                object_hex: object.clone(),
                files,
            });
        }
    }
    plan.nodes = nodes;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJECT: &str = "11c3a251-991a-4890-9629-0b09f36abbdf";
    const OBJECT_HEX: &str = "51A2C3111A99904896290B09F36ABBDF";

    #[test]
    fn the_object_of_a_name_is_the_uuid_it_starts_with_in_the_platforms_byte_order() {
        assert_eq!(object_of(OBJECT).as_deref(), Some(OBJECT_HEX));
        assert_eq!(
            object_of(&format!("{OBJECT}.0")).as_deref(),
            Some(OBJECT_HEX)
        );
        assert_eq!(
            object_of(&format!(
                "{OBJECT}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0"
            ))
            .as_deref(),
            Some(OBJECT_HEX)
        );
        for name in [
            "root",
            "version",
            "versions",
            "deleted",
            "DynamicallyUpdated",
            "versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",
        ] {
            assert_eq!(object_of(name), None, "{name}");
        }
        // 36 characters that are a uuid followed by something else are no object row
        assert_eq!(object_of(&format!("{OBJECT}x")), None);
    }

    #[test]
    fn only_body_files_are_listed_never_a_descriptor_or_its_alias() {
        assert!(is_body_file(&format!("{OBJECT}.0")));
        assert!(is_body_file(&format!("{OBJECT}.12")));
        assert!(is_body_file(&format!(
            "{OBJECT}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0"
        )));
        assert!(!is_body_file(OBJECT));
        assert!(!is_body_file(&format!(
            "{OBJECT}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b"
        )));
        assert!(!is_body_file(&format!("{OBJECT}.")));
        assert!(!is_body_file("root"));
    }

    #[test]
    fn the_files_of_an_object_are_the_changed_bodies_and_the_ones_its_lists_name() {
        let alias = format!("{OBJECT}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0");
        let staged = vec![
            format!("{OBJECT}.0"),
            OBJECT.to_owned(),
            "ba059c99-b392-42dd-8e4e-2e425601fce6".to_owned(),
            "versions".to_owned(),
        ];
        let dropped = vec![
            alias.clone(),
            format!("{OBJECT}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b"),
        ];
        let listed = vec![(
            "AAAA".to_owned(),
            "ba059c99-b392-42dd-8e4e-2e425601fce6.2".to_owned(),
        )];
        let files = files_by_object(&staged, &dropped, &listed);
        assert_eq!(files[OBJECT_HEX], vec![format!("{OBJECT}.0"), alias]);
        assert_eq!(
            files["AAAA"],
            vec!["ba059c99-b392-42dd-8e4e-2e425601fce6.2".to_owned()]
        );
        // a descriptor alone lists nothing
        let only_descriptor = files_by_object(&[OBJECT.to_owned()], &[], &[]);
        assert!(only_descriptor.is_empty());
    }
}
