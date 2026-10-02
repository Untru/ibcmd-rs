//! The objects of a stored configuration, one at a time.
//!
//! An editor works with a few objects at a time: the tree of the
//! configuration level by level, the files of an object someone opened, the
//! export and the comparison of the objects someone selected. This module
//! reads the Config table (a database, or a `--rows-dir` folder of stored
//! rows) only as far as that needs:
//!
//! - every row name and the `versions` row, whose digest tells a changed
//!   configuration from the one read before (every name and file cached for
//!   the old one is dropped then);
//! - `root` and the configuration's own row: the top-level objects by kind,
//!   in the row's order, and the configuration's contained objects -- the
//!   configuration's `Ext` bodies are stored under their ids
//!   (`00000000-...-0002.8` is `Ext/HomePageWorkArea.xml` in
//!   `tests/fixtures/external/home_page`), not under its own uuid;
//! - the descriptor rows of one kind when its group is opened (each row's
//!   name, and the forms, templates, recalculations and nested subsystems it
//!   owns), and the rows of an object's owned objects when it is opened.
//!
//! An object is stored in its descriptor row `<uuid>`, its body rows
//! `<uuid>.<n>` and the rows of the objects it owns; the configuration in its
//! row and the rows of its contained objects. The export of those rows alone
//! -- the `--file-name` selection of `mssql-dump-config` -- writes the
//! object's files exactly as a full export does (`tests/editor_server.rs`
//! compares them with `cf export` object by object). Here that export runs
//! through `mssql_dump::dump_config_with` into memory: the same function and the
//! same arguments as the command line, only the files go to a sink.
//!
//! The export keeps process-wide state (the active offline rows, the storage
//! views, the platform it noted, `crate::cancel`), so the operations of a
//! [`StoredConfiguration`] take one lock and run one at a time.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::MssqlDumpConfigArgs;
use crate::metadata_model::brace::{Brace, parse_row};
use crate::metadata_model::export::names::{own_header, owned_groups, root_members};
use crate::metadata_model::index::{NESTED_COLLECTIONS, ROOT_COLLECTIONS};
use crate::module_blob::inflate_raw;
use crate::mssql_dump::{
    ConfigTable, DumpHooks, FileSink, dump_config_with, stored_config_versions,
    with_database_table, with_rows_dir_table,
};
use crate::platform::PlatformSpec;
use crate::sql::SqlExec;

/// The root the in-memory export names its files under; nothing is written
/// there.
const MEMORY_ROOT: &str = "ibcmd-rs-memory";

/// Exported files kept per configuration, at most (bytes); beyond it the
/// cache starts over.
const CACHE_BYTES: usize = 256 << 20;

static ENGINE: Mutex<()> = Mutex::new(());

fn engine() -> MutexGuard<'static, ()> {
    ENGINE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Where the Config table of one infobase is read from.
#[derive(Clone)]
pub enum ConfigSource {
    /// A folder of `<FileName>__part<N>.bin` files: `mssql-dump-config
    /// --rows-dir`. Read-only, and it holds no ConfigSave.
    RowsDir(PathBuf),
    /// A SQL Server database, through a handle whose connections are kept
    /// between operations.
    Database { sql: SqlExec, database: String },
}

impl ConfigSource {
    /// The database name the export's reads are keyed by (`""` for a folder,
    /// as `mssql-dump-config --rows-dir` without `--database`).
    fn database(&self) -> &str {
        match self {
            Self::RowsDir(_) => "",
            Self::Database { database, .. } => database,
        }
    }
}

/// One object of the configuration, as the tree and the export name it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredObject {
    /// `Configuration`, `Catalog`, `Form`, ...
    pub kind: String,
    pub name: String,
    /// `Configuration`, `Catalog.Банки`, `Catalog.Банки.Form.ФормаЭлемента`.
    pub full_name: String,
    pub uuid: String,
    /// The descriptor's file: `Catalogs/Банки.xml`, `Configuration.xml`.
    pub path: String,
    /// The folder of the object's other files: `Catalogs/Банки`, `Ext` for
    /// the configuration.
    pub folder: String,
}

impl StoredObject {
    /// Whether the export's `path` (`/`-separated, relative) is this
    /// object's: its descriptor or anything under its folder.
    pub fn owns_path(&self, path: &str) -> bool {
        path == self.path
            || path
                .strip_prefix(self.folder.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    }
}

/// An object name the configuration does not hold.
#[derive(Debug)]
pub struct UnknownObject(pub String);

impl std::fmt::Display for UnknownObject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the configuration has no object {}", self.0)
    }
}

impl std::error::Error for UnknownObject {}

/// One node of the configuration tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    /// What `children` takes to open the node: `Configuration`, a group
    /// (`Configuration/Catalogs`, `Catalog.Банки/Forms`) or an object's full
    /// name.
    pub id: String,
    pub label: String,
    /// `configuration`, `group` or `object`.
    pub node_type: &'static str,
    /// The kind of the object, or of the objects of a group.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// The descriptor's file in an export.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The objects of a group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    pub has_children: bool,
}

/// What the export writes for one object, in memory.
#[derive(Debug)]
pub struct ObjectFiles {
    pub object: StoredObject,
    /// Digest of the `versions` entries of the object's rows: changes when
    /// the object changes in the database.
    pub version: String,
    /// Relative `/`-separated path -> bytes, as a full export writes them.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// How the database and a source folder compare for one object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusKind {
    /// Every file of the folder is what the database exports.
    Same,
    /// Both hold the object and some file differs, is missing or is extra.
    Modified,
    /// The database holds the object, the folder has none of its files.
    NotExported,
    /// The folder holds the object, the database does not.
    NotInBase,
}

/// One object of a comparison of the database with a source folder.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectStatus {
    /// Full name (`Catalog.Банки`).
    pub object: String,
    pub kind: String,
    pub status: StatusKind,
    /// The object's version in the database (absent: not in the base).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Files both hold, with different bytes.
    pub changed: Vec<String>,
    /// Files the database exports and the folder lacks.
    pub base_only: Vec<String>,
    /// Files of the object the folder holds and the database does not export.
    pub local_only: Vec<String>,
}

/// What an export of one object into a source folder changed there.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectWrite {
    pub object: String,
    pub version: String,
    /// Files written (new or with other bytes).
    pub written: Vec<String>,
    /// Files of the object the export no longer writes, removed.
    pub removed: Vec<String>,
    /// Files already as the export writes them.
    pub unchanged: usize,
}

/// One step of a long operation, for its caller's progress report.
pub type Progress<'a> = &'a dyn Fn(usize, usize, &str);

// ---------------------------------------------------------------------------
// The inventory: what has been read of the stored configuration.

struct Inventory {
    /// SHA-256 of the stored `versions` row.
    digest: String,
    /// Every stored row by published name.
    names: BTreeSet<String>,
    /// Row -> configVersion, as `versions` records them.
    versions: BTreeMap<String, String>,
    configuration: StoredObject,
    /// The configuration's contained objects (its `Ext` bodies are theirs).
    contained: Vec<String>,
    /// Top-level objects with a stored row, in the configuration row's order.
    top: Vec<(&'static str, String)>,
    /// Objects named so far, by uuid.
    objects: HashMap<String, StoredObject>,
    /// Owner uuid -> its owned objects with a stored row, in row order.
    owned: HashMap<String, Vec<(&'static str, String)>>,
    /// Kinds whose top-level objects are all named.
    named_kinds: HashSet<&'static str>,
    /// Owners whose owned objects are all named.
    named_owners: HashSet<String>,
}

/// A stored row parsed: raw deflate as the table keeps most rows, else the
/// bytes as they are.
fn parse_stored(name: &str, bytes: &[u8]) -> Result<Brace> {
    let text = inflate_raw(bytes).unwrap_or_else(|_| bytes.to_vec());
    parse_row(&text).with_context(|| format!("failed to read the stored row {name}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The collection folder of a kind: `Catalog` -> `Catalogs`, `Form` ->
/// `Forms`, `Interface` -> `Interfaces`.
fn collection_of(kind: &str) -> Option<&'static str> {
    ROOT_COLLECTIONS
        .iter()
        .chain(NESTED_COLLECTIONS)
        .find_map(|(collection, of)| (*of == kind).then_some(*collection))
}

/// The kinds whose objects have rows of their own inside another object.
const OWNED_KINDS: &[&str] = &["Form", "Template", "Recalculation", "Subsystem"];

/// The configuration's contained objects: in each section of its row,
/// `{<class id>,<body>}`, the first `{1,0,<id>}` of the body (the section's
/// own identity, see `metadata_model::root`).
fn contained_ids(configuration: &Brace) -> Vec<String> {
    fn first_identity(node: &Brace) -> Option<String> {
        let members = node.as_list()?;
        if let [one, zero, id] = members
            && one.as_atom() == Some("1")
            && zero.as_atom() == Some("0")
            && let Some(id) = id.as_atom()
            && crate::apply_check::roles::is_uuid(id)
        {
            return Some(id.to_ascii_lowercase());
        }
        members.iter().find_map(first_identity)
    }
    let Some(members) = configuration.as_list() else {
        return Vec::new();
    };
    members
        .iter()
        .skip(3)
        .filter_map(|section| match section.as_list()? {
            [class, body]
                if class
                    .as_atom()
                    .is_some_and(crate::apply_check::roles::is_uuid) =>
            {
                first_identity(body)
            }
            _ => None,
        })
        .collect()
}

/// The uuid `root` names: `{2,<uuid>,...}` (or `{2,{<uuid>},...}`).
fn root_configuration(root: &Brace) -> Option<String> {
    let member = root.as_list()?.get(1)?;
    let uuid = match member {
        Brace::List(items) => items.first()?.as_atom()?,
        other => other.as_atom()?,
    };
    Some(uuid.to_ascii_lowercase())
}

impl Inventory {
    /// The first level: names, `root`, `versions` and the configuration row.
    fn read(table: &ConfigTable<'_>, names: BTreeSet<String>) -> Result<Self> {
        let service = table.rows(
            &["root", "versions"]
                .iter()
                .map(|name| name.to_string())
                .collect(),
        )?;
        let versions_bytes = service
            .get("versions")
            .ok_or_else(|| anyhow!("the Config table has no versions row"))?;
        let digest = sha256_hex(versions_bytes);
        let versions = {
            let entries = names
                .iter()
                .map(|name| {
                    let payload = if name == "versions" {
                        versions_bytes.clone()
                    } else {
                        Vec::new()
                    };
                    (name.clone(), payload)
                })
                .collect::<Vec<_>>();
            stored_config_versions(&entries).unwrap_or_default()
        };
        let root = service
            .get("root")
            .ok_or_else(|| anyhow!("the Config table has no root row"))?;
        let uuid = root_configuration(&parse_stored("root", root)?)
            .ok_or_else(|| anyhow!("the root row names no configuration"))?;
        let rows = table.rows(&BTreeSet::from([uuid.clone()]))?;
        let row = rows
            .get(&uuid)
            .ok_or_else(|| anyhow!("the root row names {uuid}, which has no row"))?;
        let configuration_row = parse_stored(&uuid, row)?;
        let name = own_header(&configuration_row)
            .map(|(_, name)| name)
            .unwrap_or_else(|| uuid.clone());
        let top = root_members(&configuration_row)
            .into_iter()
            .filter(|(kind, uuid)| collection_of(kind).is_some() && names.contains(uuid))
            .collect();
        Ok(Self {
            digest,
            versions,
            configuration: StoredObject {
                kind: "Configuration".to_string(),
                name,
                full_name: "Configuration".to_string(),
                uuid,
                path: "Configuration.xml".to_string(),
                folder: "Ext".to_string(),
            },
            contained: contained_ids(&configuration_row),
            top,
            names,
            objects: HashMap::new(),
            owned: HashMap::new(),
            named_kinds: HashSet::new(),
            named_owners: HashSet::new(),
        })
    }

    /// Names the given objects from their rows; `owner` is the object whose
    /// folder they live in (`None`: top level). Each row's owned objects are
    /// listed on the way.
    fn name(
        &mut self,
        table: &ConfigTable<'_>,
        members: &[(&'static str, String)],
        owner: Option<&StoredObject>,
    ) -> Result<()> {
        let wanted = members
            .iter()
            .filter(|(_, uuid)| !self.objects.contains_key(uuid))
            .map(|(_, uuid)| uuid.clone())
            .collect::<BTreeSet<_>>();
        let rows = table.rows(&wanted)?;
        for (kind, uuid) in members {
            if self.objects.contains_key(uuid) {
                continue;
            }
            let Some(bytes) = rows.get(uuid) else {
                continue;
            };
            let row = parse_stored(uuid, bytes)?;
            let name = own_header(&row)
                .map(|(_, name)| name)
                .unwrap_or_else(|| uuid.clone());
            let collection = collection_of(kind)
                .ok_or_else(|| anyhow!("no folder is known for {kind} objects"))?;
            let (full_name, base) = match owner {
                None => (format!("{kind}.{name}"), collection.to_string()),
                Some(owner) => (
                    format!("{}.{kind}.{name}", owner.full_name),
                    format!("{}/{collection}", owner.folder),
                ),
            };
            let owned = owned_groups(&row)
                .into_iter()
                .filter(|group| OWNED_KINDS.contains(&group.kind))
                .flat_map(|group| {
                    let kind = group.kind;
                    group.members.into_iter().map(move |member| (kind, member))
                })
                .filter(|(_, member)| self.names.contains(member))
                .collect::<Vec<_>>();
            self.owned.insert(uuid.clone(), owned);
            self.objects.insert(
                uuid.clone(),
                StoredObject {
                    kind: kind.to_string(),
                    name: name.clone(),
                    full_name,
                    uuid: uuid.clone(),
                    path: format!("{base}/{name}.xml"),
                    folder: format!("{base}/{name}"),
                },
            );
        }
        Ok(())
    }

    /// Names every top-level object of `kind`.
    fn name_kind(&mut self, table: &ConfigTable<'_>, kind: &'static str) -> Result<()> {
        if self.named_kinds.contains(kind) {
            return Ok(());
        }
        let members = self
            .top
            .iter()
            .filter(|(of, _)| *of == kind)
            .cloned()
            .collect::<Vec<_>>();
        self.name(table, &members, None)?;
        self.named_kinds.insert(kind);
        Ok(())
    }

    /// Names every object `owner` owns (the owner itself is named).
    fn name_owned(&mut self, table: &ConfigTable<'_>, owner: &str) -> Result<()> {
        if self.named_owners.contains(owner) {
            return Ok(());
        }
        let owner_object = self
            .objects
            .get(owner)
            .cloned()
            .ok_or_else(|| anyhow!("object {owner} is not named yet"))?;
        let members = self.owned.get(owner).cloned().unwrap_or_default();
        self.name(table, &members, Some(&owner_object))?;
        self.named_owners.insert(owner.to_string());
        Ok(())
    }

    /// The kinds of the top level, in the configurator's order, with their
    /// number of objects.
    fn kinds(&self) -> Vec<(&'static str, usize)> {
        ROOT_COLLECTIONS
            .iter()
            .chain(NESTED_COLLECTIONS)
            .filter_map(|(_, kind)| {
                let count = self.top.iter().filter(|(of, _)| of == kind).count();
                (count > 0).then_some((*kind, count))
            })
            .collect()
    }

    fn kind_of_collection(collection: &str) -> Option<&'static str> {
        ROOT_COLLECTIONS
            .iter()
            .chain(NESTED_COLLECTIONS)
            .find_map(|(of, kind)| (*of == collection).then_some(*kind))
    }

    /// The object of a full name, naming what it must on the way.
    fn resolve(&mut self, table: &ConfigTable<'_>, full_name: &str) -> Result<StoredObject> {
        let unknown = || anyhow::Error::new(UnknownObject(full_name.to_string()));
        let parts = full_name.trim().split('.').collect::<Vec<_>>();
        if parts
            .first()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("Configuration"))
        {
            return match parts.len() {
                1 => Ok(self.configuration.clone()),
                2 if parts[1].eq_ignore_ascii_case(&self.configuration.name) => {
                    Ok(self.configuration.clone())
                }
                _ => Err(unknown()),
            };
        }
        if parts.len() < 2 || parts.len() % 2 != 0 {
            return Err(unknown());
        }
        let kind = ROOT_COLLECTIONS
            .iter()
            .chain(NESTED_COLLECTIONS)
            .find_map(|(_, kind)| kind.eq_ignore_ascii_case(parts[0]).then_some(*kind))
            .ok_or_else(unknown)?;
        self.name_kind(table, kind)?;
        let top = self
            .top
            .iter()
            .filter(|(of, _)| *of == kind)
            .map(|(_, uuid)| uuid.clone())
            .collect::<Vec<_>>();
        let mut current = self.find(&top, kind, parts[1]).ok_or_else(unknown)?;
        for pair in parts[2..].chunks(2) {
            self.name_owned(table, &current.uuid)?;
            let owned = self
                .owned
                .get(&current.uuid)
                .map(|owned| {
                    owned
                        .iter()
                        .map(|(_, uuid)| uuid.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            current = self.find(&owned, pair[0], pair[1]).ok_or_else(unknown)?;
        }
        Ok(current)
    }

    /// Among `uuids`, the named object of `kind` called `name` (exactly,
    /// else ignoring case, as the platform compares names).
    fn find(&self, uuids: &[String], kind: &str, name: &str) -> Option<StoredObject> {
        let candidates = uuids
            .iter()
            .filter_map(|uuid| self.objects.get(uuid))
            .filter(|object| object.kind.eq_ignore_ascii_case(kind))
            .collect::<Vec<_>>();
        candidates
            .iter()
            .find(|object| object.name == name)
            .or_else(|| {
                candidates
                    .iter()
                    .find(|object| object.name.to_lowercase() == name.to_lowercase())
            })
            .map(|object| (*object).clone())
    }

    /// The stored rows named `id` or `id.<suffix>`.
    fn rows_under(&self, id: &str, out: &mut BTreeSet<String>) {
        let prefix = format!("{id}.");
        if self.names.contains(id) {
            out.insert(id.to_string());
        }
        out.extend(
            self.names
                .range(prefix.clone()..)
                .take_while(|name| name.starts_with(&prefix))
                .cloned(),
        );
    }

    /// Every stored row of an object: its own and its owned objects'.
    fn rows_of(
        &mut self,
        table: &ConfigTable<'_>,
        object: &StoredObject,
    ) -> Result<BTreeSet<String>> {
        let mut rows = BTreeSet::new();
        if object.kind == "Configuration" {
            self.rows_under(&object.uuid, &mut rows);
            for id in self.contained.clone() {
                self.rows_under(&id, &mut rows);
            }
            return Ok(rows);
        }
        let mut pending = vec![object.uuid.clone()];
        while let Some(uuid) = pending.pop() {
            self.rows_under(&uuid, &mut rows);
            self.name_owned(table, &uuid)?;
            pending.extend(
                self.owned
                    .get(&uuid)
                    .into_iter()
                    .flatten()
                    .map(|(_, owned)| owned.clone()),
            );
        }
        Ok(rows)
    }

    /// The version of an object: a digest of its rows' versions.
    fn version_of(&self, rows: &BTreeSet<String>) -> String {
        let mut text = String::new();
        for row in rows {
            text.push_str(row);
            text.push('=');
            text.push_str(self.versions.get(row).map(String::as_str).unwrap_or("-"));
            text.push('\n');
        }
        sha256_hex(text.as_bytes())[..16].to_string()
    }

    /// The children of a tree node (`None`: the root).
    fn children(&mut self, table: &ConfigTable<'_>, node: Option<&str>) -> Result<Vec<TreeNode>> {
        let object_node = |object: &StoredObject, has_children: bool| TreeNode {
            id: object.full_name.clone(),
            label: object.name.clone(),
            node_type: "object",
            kind: object.kind.clone(),
            full_name: Some(object.full_name.clone()),
            uuid: Some(object.uuid.clone()),
            path: Some(object.path.clone()),
            count: None,
            has_children,
        };
        let group_node = |owner: &str, kind: &str, count: usize| {
            let collection = collection_of(kind).unwrap_or(kind);
            TreeNode {
                id: format!("{owner}/{collection}"),
                label: collection.to_string(),
                node_type: "group",
                kind: kind.to_string(),
                full_name: None,
                uuid: None,
                path: None,
                count: Some(count),
                has_children: count > 0,
            }
        };
        let Some(node) = node.map(str::trim).filter(|node| !node.is_empty()) else {
            let configuration = &self.configuration;
            return Ok(vec![TreeNode {
                node_type: "configuration",
                ..object_node(configuration, !self.top.is_empty())
            }]);
        };
        if node.eq_ignore_ascii_case("Configuration") {
            return Ok(self
                .kinds()
                .into_iter()
                .map(|(kind, count)| group_node("Configuration", kind, count))
                .collect());
        }
        if let Some((owner, collection)) = node.rsplit_once('/') {
            let kind = Self::kind_of_collection(collection)
                .ok_or_else(|| anyhow::Error::new(UnknownObject(node.to_string())))?;
            let members = if owner.eq_ignore_ascii_case("Configuration") {
                self.name_kind(table, kind)?;
                self.top
                    .iter()
                    .filter(|(of, _)| *of == kind)
                    .map(|(_, uuid)| uuid.clone())
                    .collect::<Vec<_>>()
            } else {
                let owner = self.resolve(table, owner)?;
                self.name_owned(table, &owner.uuid)?;
                self.owned
                    .get(&owner.uuid)
                    .into_iter()
                    .flatten()
                    .filter(|(of, _)| *of == kind)
                    .map(|(_, uuid)| uuid.clone())
                    .collect()
            };
            return Ok(members
                .iter()
                .filter_map(|uuid| self.objects.get(uuid))
                .map(|object| {
                    let owns = self
                        .owned
                        .get(&object.uuid)
                        .is_some_and(|owned| !owned.is_empty());
                    object_node(object, owns)
                })
                .collect());
        }
        let object = self.resolve(table, node)?;
        let owned = self.owned.get(&object.uuid).cloned().unwrap_or_default();
        let mut groups = Vec::<(&'static str, usize)>::new();
        for (kind, _) in &owned {
            match groups.iter_mut().find(|(of, _)| of == kind) {
                Some((_, count)) => *count += 1,
                None => groups.push((kind, 1)),
            }
        }
        Ok(groups
            .into_iter()
            .map(|(kind, count)| group_node(&object.full_name, kind, count))
            .collect())
    }
}

/// The rows of the named objects, read through the view the caller (the
/// export, for `mssql-dump-config --object`) has begun.
pub(crate) fn stored_rows_of(
    table: &ConfigTable<'_>,
    names: &[String],
) -> Result<BTreeSet<String>> {
    let mut inventory = Inventory::read(table, table.names()?)?;
    let mut rows = BTreeSet::new();
    for name in names {
        let object = inventory.resolve(table, name)?;
        rows.extend(inventory.rows_of(table, &object)?);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// The files of an export, in memory.

struct MemorySink {
    root: PathBuf,
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl FileSink for MemorySink {
    fn accept(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        let key = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, bytes.to_vec());
        Ok(())
    }
}

/// `/`-separated relative path of a file under `root`.
fn relative_slash(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// The files of `object` in a source folder, by relative path (dot folders
/// such as `.git` and `.ibcmd` are not the object's).
fn local_files(dir: &Path, object: &StoredObject) -> Result<BTreeMap<String, PathBuf>> {
    let mut files = BTreeMap::new();
    let descriptor = dir.join(&object.path);
    if descriptor.is_file() {
        files.insert(object.path.clone(), descriptor);
    }
    let folder = dir.join(&object.folder);
    if folder.is_dir() {
        for entry in walkdir::WalkDir::new(&folder)
            .into_iter()
            .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        {
            let entry = entry.with_context(|| format!("failed to walk {}", folder.display()))?;
            if entry.file_type().is_file()
                && let Some(relative) = relative_slash(dir, entry.path())
            {
                files.insert(relative, entry.into_path());
            }
        }
    }
    Ok(files)
}

/// The object a file of an export belongs to: `Configuration.xml` and `Ext/`
/// the configuration's, `<Collection>/<Name>.xml` and `<Collection>/<Name>/`
/// the top-level object's (an owned form's files are its owner's too).
pub fn object_of_path(path: &str) -> Option<String> {
    let path = normalize_path(path)?;
    if path == "Configuration.xml" || path.starts_with("Ext/") {
        return Some("Configuration".to_string());
    }
    let mut parts = path.splitn(3, '/');
    let collection = parts.next()?;
    let second = parts.next()?;
    let kind = ROOT_COLLECTIONS
        .iter()
        .chain(NESTED_COLLECTIONS)
        .find_map(|(of, kind)| (*of == collection).then_some(*kind))?;
    let name = match parts.next() {
        Some(_) => second,
        None => second.strip_suffix(".xml")?,
    };
    (!name.is_empty()).then(|| format!("{kind}.{name}"))
}

/// A relative path as the export names files: `/`-separated, without `.`,
/// `..` or a root.
pub fn normalize_path(path: &str) -> Option<String> {
    let parts = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| *part == ".." || part.contains(':')) {
        return None;
    }
    Some(parts.join("/"))
}

// ---------------------------------------------------------------------------
// One infobase's stored configuration, for an editor.

/// The stored configuration of one infobase as an editor works with it: the
/// tree, the objects' files and their comparison with a source folder. What
/// it read is kept until the database's `versions` row changes.
pub struct StoredConfiguration {
    source: ConfigSource,
    platform: PlatformSpec,
    inventory: Option<Inventory>,
    /// By full name, for the inventory's digest.
    exports: HashMap<String, Arc<ObjectFiles>>,
    cached_bytes: usize,
}

impl StoredConfiguration {
    pub fn new(source: ConfigSource, platform: PlatformSpec) -> Self {
        Self {
            source,
            platform,
            inventory: None,
            exports: HashMap::new(),
            cached_bytes: 0,
        }
    }

    pub fn source(&self) -> &ConfigSource {
        &self.source
    }

    pub fn platform(&self) -> PlatformSpec {
        self.platform
    }

    /// Runs `body` with the table readable and the inventory current: read
    /// anew, and every cached export dropped, when `versions` changed.
    fn with_inventory<T>(
        &mut self,
        body: impl FnOnce(&mut Inventory, &ConfigTable<'_>) -> Result<T>,
    ) -> Result<T> {
        let Self {
            source,
            inventory,
            exports,
            cached_bytes,
            ..
        } = self;
        let run = |table: &ConfigTable<'_>| -> Result<T> {
            let names = table.names()?;
            let current = table
                .rows(&BTreeSet::from(["versions".to_string()]))?
                .get("versions")
                .map(|bytes| sha256_hex(bytes));
            let stale = match (inventory.as_ref(), current) {
                (Some(known), Some(current)) => known.digest != current,
                _ => true,
            };
            if stale {
                *inventory = Some(Inventory::read(table, names)?);
                exports.clear();
                *cached_bytes = 0;
            }
            body(inventory.as_mut().expect("read above"), table)
        };
        match source {
            ConfigSource::RowsDir(dir) => with_rows_dir_table(dir, run),
            ConfigSource::Database { sql, database } => with_database_table(sql, database, run),
        }
    }

    /// The children of a node of the tree (`None`: the configuration).
    pub fn children(&mut self, node: Option<&str>) -> Result<Vec<TreeNode>> {
        let _engine = engine();
        self.with_inventory(|inventory, table| inventory.children(table, node))
    }

    /// The objects of the given full names.
    pub fn resolve(&mut self, names: &[String]) -> Result<Vec<StoredObject>> {
        let _engine = engine();
        self.with_inventory(|inventory, table| {
            names
                .iter()
                .map(|name| inventory.resolve(table, name))
                .collect()
        })
    }

    /// Every object a whole-configuration operation covers: the
    /// configuration and every top-level object.
    fn all_objects(&mut self) -> Result<Vec<StoredObject>> {
        self.with_inventory(|inventory, table| {
            let mut objects = vec![inventory.configuration.clone()];
            for (kind, _) in inventory.kinds() {
                inventory.name_kind(table, kind)?;
            }
            objects.extend(
                inventory
                    .top
                    .iter()
                    .filter_map(|(_, uuid)| inventory.objects.get(uuid).cloned()),
            );
            Ok(objects)
        })
    }

    /// What the export writes for the objects, from the cache or by one
    /// export of all their rows at once (the indexes an export builds are
    /// built once for them all).
    fn export_unlocked(
        &mut self,
        names: &[String],
        progress: Progress<'_>,
    ) -> Result<Vec<Arc<ObjectFiles>>> {
        // The cache is looked at once the inventory is current: a changed
        // `versions` row empties it.
        let resolved = self.with_inventory(|inventory, table| {
            let mut resolved = Vec::new();
            for name in names {
                let object = inventory.resolve(table, name)?;
                let rows = inventory.rows_of(table, &object)?;
                let version = inventory.version_of(&rows);
                resolved.push((object, rows, version));
            }
            Ok(resolved)
        })?;
        let planned = resolved
            .into_iter()
            .map(|(object, rows, version)| {
                let cached = self.exports.get(&object.full_name).cloned();
                (object, cached, rows, version)
            })
            .collect::<Vec<_>>();
        let rows = planned
            .iter()
            .filter(|(_, cached, _, _)| cached.is_none())
            .flat_map(|(_, _, rows, _)| rows.iter().cloned())
            .collect::<BTreeSet<_>>();
        let missing = planned
            .iter()
            .filter(|(_, cached, _, _)| cached.is_none())
            .count();
        let mut files = BTreeMap::new();
        if missing > 0 {
            let what = format!("export of {missing} objects ({} rows)", rows.len());
            progress(0, missing, &what);
            files = self.export_rows(rows)?;
            progress(missing, missing, &what);
        }
        let mut out = Vec::with_capacity(planned.len());
        for (object, cached, _, version) in planned {
            if let Some(cached) = cached {
                out.push(cached);
                continue;
            }
            let own = files
                .iter()
                .filter(|(path, _)| object.owns_path(path))
                .map(|(path, bytes)| (path.clone(), bytes.clone()))
                .collect::<BTreeMap<_, _>>();
            let size = own.values().map(Vec::len).sum::<usize>();
            let exported = Arc::new(ObjectFiles {
                object: object.clone(),
                version,
                files: own,
            });
            if self.cached_bytes + size > CACHE_BYTES {
                self.exports.clear();
                self.cached_bytes = 0;
            }
            self.cached_bytes += size;
            self.exports
                .insert(object.full_name.clone(), exported.clone());
            out.push(exported);
        }
        Ok(out)
    }

    /// The export of these rows into memory: `mssql-dump-config --file-name
    /// ... --extract-metadata-xml --extract-module-text --no-binary-rows
    /// --platform <platform>` (with `--rows-dir` for a folder), files to a
    /// sink.
    fn export_rows(&self, rows: BTreeSet<String>) -> Result<BTreeMap<String, Vec<u8>>> {
        if rows.is_empty() {
            return Ok(BTreeMap::new());
        }
        let args = export_args(&self.source, self.platform, rows.into_iter().collect());
        let sink = Arc::new(MemorySink {
            root: PathBuf::from(MEMORY_ROOT),
            files: Mutex::new(BTreeMap::new()),
        });
        let sql = match &self.source {
            ConfigSource::RowsDir(_) => None,
            ConfigSource::Database { sql, .. } => Some(sql.clone()),
        };
        crate::platform::note_export_platform(self.platform);
        dump_config_with(
            &args,
            DumpHooks {
                sql,
                sink: Some(sink.clone()),
            },
        )?;
        let files = std::mem::take(&mut *sink.files.lock().unwrap_or_else(PoisonError::into_inner));
        Ok(files)
    }

    /// What the export writes for the objects, in memory.
    pub fn export(
        &mut self,
        names: &[String],
        progress: Progress<'_>,
    ) -> Result<Vec<Arc<ObjectFiles>>> {
        let _engine = engine();
        self.export_unlocked(names, progress)
    }

    /// The export of the object a file belongs to, and the file's path as
    /// the export names it: `files.get(path)` is the file as the export
    /// writes it (`None`: it writes no such file). Nothing is written to
    /// disk.
    pub fn read_file(&mut self, path: &str) -> Result<(Arc<ObjectFiles>, String)> {
        let path =
            normalize_path(path).ok_or_else(|| anyhow!("{path:?} is not a relative path"))?;
        let name = object_of_path(&path)
            .ok_or_else(|| anyhow!("{path} is not a file of a configuration object"))?;
        let _engine = engine();
        let exported = self.export_unlocked(&[name], &|_, _, _| {})?;
        let exported = exported.into_iter().next().expect("one object asked for");
        Ok((exported, path))
    }

    /// Compares the objects (`None`: the whole configuration) with a source
    /// folder, file by file.
    pub fn status(
        &mut self,
        names: Option<&[String]>,
        dir: &Path,
        progress: Progress<'_>,
    ) -> Result<Vec<ObjectStatus>> {
        let _engine = engine();
        let (objects, local_only_names) = match names {
            Some(names) => self.with_inventory(|inventory, table| {
                let mut known = Vec::new();
                let mut absent = Vec::new();
                for name in names {
                    match inventory.resolve(table, name) {
                        Ok(object) => known.push(object.full_name),
                        Err(error) if error.is::<UnknownObject>() => absent.push(name.clone()),
                        Err(error) => return Err(error),
                    }
                }
                Ok((known, absent))
            })?,
            None => {
                let objects = self.all_objects()?;
                let known = objects
                    .iter()
                    .map(|object| object.full_name.to_lowercase())
                    .collect::<HashSet<_>>();
                let absent = folder_objects(dir)?
                    .into_iter()
                    .filter(|name| !known.contains(&name.to_lowercase()))
                    .collect();
                (
                    objects.into_iter().map(|object| object.full_name).collect(),
                    absent,
                )
            }
        };
        let exported = self.export_unlocked(&objects, progress)?;
        let mut statuses = Vec::new();
        for (index, files) in exported.iter().enumerate() {
            progress(index + 1, exported.len(), &files.object.full_name);
            crate::cancel::check()?;
            let local = local_files(dir, &files.object)?;
            let mut status = ObjectStatus {
                object: files.object.full_name.clone(),
                kind: files.object.kind.clone(),
                status: StatusKind::Same,
                version: Some(files.version.clone()),
                changed: Vec::new(),
                base_only: Vec::new(),
                local_only: Vec::new(),
            };
            for (path, bytes) in &files.files {
                match local.get(path) {
                    None => status.base_only.push(path.clone()),
                    Some(file) => {
                        let on_disk = fs::read(file)
                            .with_context(|| format!("failed to read {}", file.display()))?;
                        if &on_disk != bytes {
                            status.changed.push(path.clone());
                        }
                    }
                }
            }
            status.local_only = local
                .keys()
                .filter(|path| !files.files.contains_key(*path))
                .cloned()
                .collect();
            status.status = if local.is_empty() {
                StatusKind::NotExported
            } else if status.changed.is_empty()
                && status.base_only.is_empty()
                && status.local_only.is_empty()
            {
                StatusKind::Same
            } else {
                StatusKind::Modified
            };
            statuses.push(status);
        }
        for name in local_only_names {
            let object = folder_object(&name).ok_or_else(|| UnknownObject(name.clone()))?;
            let local = local_files(dir, &object)?;
            statuses.push(ObjectStatus {
                object: name,
                kind: object.kind,
                status: StatusKind::NotInBase,
                version: None,
                changed: Vec::new(),
                base_only: Vec::new(),
                local_only: local.into_keys().collect(),
            });
        }
        Ok(statuses)
    }

    /// Exports the objects into a source folder: each object's files as the
    /// export writes them, and the files of the object the export no longer
    /// writes removed. Nothing else in the folder changes.
    pub fn export_into(
        &mut self,
        names: &[String],
        dir: &Path,
        progress: Progress<'_>,
    ) -> Result<Vec<ObjectWrite>> {
        let _engine = engine();
        let exported = self.export_unlocked(names, progress)?;
        fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
        let mut writes = Vec::new();
        for (index, files) in exported.iter().enumerate() {
            progress(index + 1, exported.len(), &files.object.full_name);
            crate::cancel::check()?;
            let local = local_files(dir, &files.object)?;
            let mut write = ObjectWrite {
                object: files.object.full_name.clone(),
                version: files.version.clone(),
                written: Vec::new(),
                removed: Vec::new(),
                unchanged: 0,
            };
            for (path, file) in &local {
                if !files.files.contains_key(path) {
                    fs::remove_file(file)
                        .with_context(|| format!("failed to remove {}", file.display()))?;
                    write.removed.push(path.clone());
                }
            }
            for (path, bytes) in &files.files {
                let target = dir.join(path);
                if local.contains_key(path) && fs::read(&target).is_ok_and(|old| &old == bytes) {
                    write.unchanged += 1;
                    continue;
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&target, bytes)
                    .with_context(|| format!("failed to write {}", target.display()))?;
                write.written.push(path.clone());
            }
            prune_empty(&dir.join(&files.object.folder));
            writes.push(write);
        }
        Ok(writes)
    }
}

/// The arguments of the export of `file_names`, as the command line spells
/// them: `mssql-dump-config [--rows-dir <dir> | --database <db>] --file-name
/// ... --extract-metadata-xml --extract-module-text --no-binary-rows
/// --platform <platform>`.
fn export_args(
    source: &ConfigSource,
    platform: PlatformSpec,
    file_names: Vec<String>,
) -> MssqlDumpConfigArgs {
    MssqlDumpConfigArgs {
        sqlcmd: None,
        bcp_executable: None,
        runtime_journal: None,
        server: "localhost".to_string(),
        sql_user: None,
        sql_pwd: None,
        sql_pwd_env: crate::settings::ENV_DB_PASSWORD.to_string(),
        database: source.database().to_string(),
        rows_dir: match source {
            ConfigSource::RowsDir(dir) => Some(dir.clone()),
            ConfigSource::Database { .. } => None,
        },
        model_export: false,
        legacy_export: false,
        output_dir: PathBuf::from(MEMORY_ROOT),
        overwrite: false,
        include_config_save: false,
        main_configuration: false,
        file_names,
        file_name_lists: Vec::new(),
        objects: Vec::new(),
        // The editor server exports into memory or one object at a time;
        // the incremental update of a whole folder is the command line's.
        base: None,
        sync: false,
        inflate: false,
        extract_module_text: true,
        extract_metadata_xml: true,
        require_complete_root_metadata: false,
        require_complete_source_assets: false,
        collect_all_source_asset_diagnostics: false,
        platform: Some(platform),
        source_version: platform.xml_version(),
        no_binary_rows: true,
        write_binary_rows: true,
        write_manifest: true,
    }
}

/// The top-level objects a source folder holds: `<Collection>/<Name>.xml`.
fn folder_objects(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for (collection, kind) in ROOT_COLLECTIONS {
        let folder = dir.join(collection);
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries {
            let entry = entry.with_context(|| format!("failed to read {}", folder.display()))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_file()
                && let Some(name) = file_name.strip_suffix(".xml")
            {
                names.push(format!("{kind}.{name}"));
            }
        }
    }
    names.sort();
    Ok(names)
}

/// The paths an object the database does not hold has in a source folder,
/// by its full name (`Catalog.Новый`, `Catalog.Банки.Form.Новая`); `None`
/// for the configuration, which every database holds, and for what is no
/// object name.
fn folder_object(full_name: &str) -> Option<StoredObject> {
    let prefix = source_prefixes(full_name)?.into_iter().next()?;
    if prefix == "Configuration" {
        return None;
    }
    let parts = full_name.trim().split('.').collect::<Vec<_>>();
    let [.., kind, name] = parts.as_slice() else {
        return None;
    };
    Some(StoredObject {
        kind: kind.to_string(),
        name: name.to_string(),
        full_name: full_name.trim().to_string(),
        uuid: String::new(),
        path: format!("{prefix}.xml"),
        folder: prefix,
    })
}

/// Removes the empty folders under `folder` and the folder itself when it is
/// left empty.
fn prune_empty(folder: &Path) {
    if !folder.is_dir() {
        return;
    }
    let dirs = walkdir::WalkDir::new(folder)
        .contents_first(true)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    for dir in dirs {
        // Fails on a folder that still holds something: that one stays.
        let _ = fs::remove_dir(dir);
    }
}

/// Runs `operation` while no operation of this module runs: what else reads
/// or writes through the export's process-wide state (a stage, whose guard
/// exports in memory; a check of ConfigSave) takes its turn with them.
pub fn exclusive<T>(operation: impl FnOnce() -> T) -> T {
    let _engine = engine();
    operation()
}

/// The source paths of an object by its full name, as `--path-prefix` of a
/// stage takes them: `Catalogs/Банки` (its `.xml` and its folder),
/// `Catalogs/Банки/Forms/Ф`; the configuration's `Configuration` and `Ext`.
/// Needs no database: an object the folder adds has no row yet.
pub fn source_prefixes(full_name: &str) -> Option<Vec<String>> {
    let parts = full_name.trim().split('.').collect::<Vec<_>>();
    if parts.first()?.eq_ignore_ascii_case("Configuration") && parts.len() <= 2 {
        return Some(vec!["Configuration".to_string(), "Ext".to_string()]);
    }
    if parts.len() < 2 || parts.len() % 2 != 0 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut segments = Vec::new();
    for (index, pair) in parts.chunks(2).enumerate() {
        let collections = if index == 0 {
            ROOT_COLLECTIONS
        } else {
            NESTED_COLLECTIONS
        };
        let collection = collections
            .iter()
            .chain(if index == 0 {
                &[][..]
            } else {
                ROOT_COLLECTIONS
            })
            .find_map(|(collection, kind)| {
                kind.eq_ignore_ascii_case(pair[0]).then_some(*collection)
            })?;
        segments.push(format!("{collection}/{}", pair[1]));
    }
    Some(vec![segments.join("/")])
}

/// Refuses an operation a rows folder cannot answer.
pub fn require_database(source: &ConfigSource, what: &str) -> Result<()> {
    if let ConfigSource::RowsDir(dir) = source {
        bail!(
            "{what} needs SQL Server: {} is a folder of stored Config rows (--rows-dir), which holds no ConfigSave and is never written",
            dir.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_its_top_level_object() {
        assert_eq!(
            object_of_path("Configuration.xml").as_deref(),
            Some("Configuration")
        );
        assert_eq!(
            object_of_path("Ext/HomePageWorkArea.xml").as_deref(),
            Some("Configuration")
        );
        assert_eq!(
            object_of_path("Catalogs/Банки.xml").as_deref(),
            Some("Catalog.Банки")
        );
        assert_eq!(
            object_of_path("Catalogs\\Банки\\Forms\\Ф\\Ext\\Form.xml").as_deref(),
            Some("Catalog.Банки")
        );
        assert_eq!(
            object_of_path("./CommonModules/М/Ext/Module.bsl").as_deref(),
            Some("CommonModule.М")
        );
        assert_eq!(object_of_path("ConfigDumpInfo.xml"), None);
        assert_eq!(object_of_path("Catalogs"), None);
        assert_eq!(object_of_path("../Catalogs/Банки.xml"), None);
        assert_eq!(object_of_path("Unknown/X.xml"), None);
    }

    #[test]
    fn a_full_name_gives_the_prefixes_a_stage_takes() {
        assert_eq!(
            source_prefixes("Catalog.Банки"),
            Some(vec!["Catalogs/Банки".to_string()])
        );
        assert_eq!(
            source_prefixes("Catalog.Банки.Form.Ф"),
            Some(vec!["Catalogs/Банки/Forms/Ф".to_string()])
        );
        assert_eq!(
            source_prefixes("Subsystem.А.Subsystem.Б"),
            Some(vec!["Subsystems/А/Subsystems/Б".to_string()])
        );
        assert_eq!(
            source_prefixes("Configuration"),
            Some(vec!["Configuration".to_string(), "Ext".to_string()])
        );
        assert_eq!(source_prefixes("Catalog"), None);
        assert_eq!(source_prefixes("Unknown.X"), None);
    }

    #[test]
    fn an_object_only_in_a_folder_has_the_paths_of_its_name() {
        let form = folder_object("Catalog.Банки.Form.Новая").unwrap();
        assert_eq!(form.kind, "Form");
        assert_eq!(form.path, "Catalogs/Банки/Forms/Новая.xml");
        assert_eq!(form.folder, "Catalogs/Банки/Forms/Новая");
        assert!(folder_object("Configuration").is_none());
        assert!(folder_object("Нечто").is_none());
    }

    #[test]
    fn an_object_owns_its_descriptor_and_folder_only() {
        let object = folder_object("Catalog.Банки").unwrap();
        assert!(object.owns_path("Catalogs/Банки.xml"));
        assert!(object.owns_path("Catalogs/Банки/Ext/ObjectModule.bsl"));
        assert!(!object.owns_path("Catalogs/БанкиСчета.xml"));
        assert!(!object.owns_path("Catalogs/БанкиСчета/Ext/ObjectModule.bsl"));
    }

    #[test]
    fn the_configuration_row_names_its_contained_objects() {
        let text = concat!(
            "{2,{ba46775a-8ecc-49a2-8dc9-f4173b708a73},2,",
            "{9cd510cd-abfc-11d4-9434-004095e12fc7,{1,{68,{0,{3,{1,0,00000000-0000-0000-0000-000000000002},\"Б\"}}},0}},",
            "{9fcd25a0-4822-11d4-9414-008048da11f9,{6,{1,{{1,0,00000000-0000-0000-0000-000000000003},00000000-0000-0000-0000-000000000000},0}}},",
            "{{0,\"\",\"\"}}}"
        );
        let row = parse_row(text.as_bytes()).unwrap();
        assert_eq!(
            contained_ids(&row),
            vec![
                "00000000-0000-0000-0000-000000000002".to_string(),
                "00000000-0000-0000-0000-000000000003".to_string(),
            ]
        );
        let root = parse_row(b"{2,ba46775a-8ecc-49a2-8dc9-f4173b708a73,}").unwrap();
        assert_eq!(
            root_configuration(&root).as_deref(),
            Some("ba46775a-8ecc-49a2-8dc9-f4173b708a73")
        );
    }
}
