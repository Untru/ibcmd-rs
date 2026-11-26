//! The plan of the restructure: attributes and tabular sections of catalogs and documents.
//!
//! Input: the stored schema and names, and the staged image (`ConfigSave`) against the stored one
//! (`Config`). Output: the new `DBSchema` text, the new `DBNames`, the derived caches (`Params` `*.si`)
//! and the statements of the rebuild. Nothing here touches a database; `reader` fills the input, `exec`
//! runs the statements.
//!
//! What is checked, fail closed (anything else is refused with the reason):
//! 1. the staged image holds the same files as the stored one (no object added or removed) plus the
//!    `deleted` marker, which names removed attributes and nothing else;
//! 2. across every descriptor row of every kind, the new, the removed, the retyped and the re-indexed
//!    attributes all sit in catalogs or documents (a new attribute also in a tabular section of one); none
//!    changes its use; a retyped one is a variable string whose limit grows; a re-indexed one switches
//!    `DontIndex` <-> `Index` or `DontIndex` <-> `IndexWithAdditionalOrder`; the attributes of a tabular
//!    section only get new ones, and a new section stands after the ones the object has;
//! 3. each such object is otherwise unchanged as far as its table goes (its shape: hierarchy, code and
//!    description lengths, number length, ...; its tabular sections but for the new ones and the new
//!    attributes of the old ones) and it has no predefined data, no data history, no subordination;
//! 4. every stored attribute of it maps to the field the stored `DBSchema` entry has (the generator
//!    reproduces the stored entry before it is trusted to extend it);
//! 5. each new attribute has a supported type.
//!
//! Not checked (needs the decoders of the other kinds, that is the restructuring check's job): a changed
//! property of another object that changes its table (a register's dimension order, ...). Run this only on
//! an image the check has passed.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::metadata_model::brace::{Brace, parse_row};
use crate::restructure::caches::change::{ChangedObject as ChangedForCaches, Staged, rewrite};
use crate::restructure::caches::plan::rows as cache_rows;
use crate::restructure::catalog::{AttributeFacts, type_entries};
use crate::restructure::create;
use crate::restructure::extensions::{ChangedObject, ExtensionInputs};
use crate::restructure::names::{DbNames, deflate, inflate, next_number, version_row};
use crate::restructure::object::{ObjectFacts, ObjectKind};
use crate::restructure::registry::{self, ObjectAdditions};
use crate::restructure::schema::{
    Column, DbSchema, FieldEntry, PhysicalTable, SqlType, TableView, TypeEntry, add_field_indexes,
    add_subtable_field_index, create_index_sql, create_table_sql, field_columns, insert_field,
    physical_tables, push_subtable, remove_field, remove_field_indexes, replace_field,
    subtable_entry, subtable_mut,
};
use crate::restructure::storage::EMPTY_GENERATION;
use crate::restructure::xdto;

/// The staged image against the stored one, reduced to what the plan reads.
#[derive(Clone, Debug, Default)]
pub struct StagedImage {
    /// `FileName` of every `Config` row (any part).
    pub old_files: BTreeSet<String>,
    /// `FileName` of every `ConfigSave` row (any part).
    pub new_files: BTreeSet<String>,
    /// The `Config` rows named by a bare uuid (object descriptors), as stored (raw deflate).
    pub old_descriptors: BTreeMap<String, Vec<u8>>,
    /// The same rows of `ConfigSave`.
    pub new_descriptors: BTreeMap<String, Vec<u8>>,
    /// The `deleted` row of `ConfigSave`, as stored.
    pub deleted: Option<Vec<u8>>,
}

/// Everything the plan reads.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    /// `SchemaStorage.CurrentSchema` of `SchemaID 0`.
    pub schema: Vec<u8>,
    /// `Params.DBNames`, as stored.
    pub main_names: Vec<u8>,
    /// `Params.DBNames-Ext-*`, as stored, by row name.
    pub extension_names: Vec<(String, Vec<u8>)>,
    /// The `RefSInf<n>` tables (predefined-data state) that hold rows.
    pub predefined_tables: BTreeSet<String>,
    /// The `Params` rows `*.si` (derived caches) and `siVersions`, as stored: name and bytes.
    pub cache_rows: Vec<(String, Vec<u8>)>,
    /// The `DataSize` column of those rows (the guard of a rewrite compares it); a row without an
    /// entry has the size of its bytes.
    pub cache_sizes: BTreeMap<String, i64>,
    /// The `root` row of `Config`, as stored: it names the configuration whose descriptor lists the
    /// objects in the order the platform walks them.
    pub root_row: Vec<u8>,
    pub staged: StagedImage,
    /// The stored descriptors (`Config` rows named by a bare uuid) of every catalog, document, common attribute
    /// and defined type, read when the stage creates an object (S1-F): the traversal of the tabular sections,
    /// the common attributes that apply and the types an attribute names need them all. Empty otherwise.
    pub objects: BTreeMap<String, Vec<u8>>,
    /// The extensions of the infobase and the objects they adopt (S1-I). The default is an infobase
    /// without extensions; a reader that fills it must fill `adoptions` too, or the plan refuses.
    pub extensions: ExtensionInputs,
}

/// Choices that keep a plan reproducible.
#[derive(Clone, Debug, Default)]
pub struct PlanOptions {
    /// The guid of the new `DBNamesVersion-DBNames`; random when absent.
    pub names_version: Option<String>,
    pub method: Method,
    /// Leave the XDTO model cache (`Params` `*.si`) as it is, stale.
    pub skip_xdto: bool,
    /// Leave the object registry (`Params` `1a621f0f-....si`) as it is, stale.
    pub skip_registry: bool,
}

/// How the changed table gets its new column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    /// The platform's way: a new-generation copy of the object's tables, then drop and rename.
    #[default]
    Rebuild,
    /// Research (is it acceptable to the platform?): `ALTER TABLE ... ADD`, the column lands at
    /// the end of the physical table instead of in the schema's place.
    AlterAdd,
}

/// A column added in place (`Method::AlterAdd`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlterColumn {
    pub table: String,
    pub column: Column,
    /// The value of the existing rows (a SQL literal or expression).
    pub value: String,
    /// For a nullable column: the `_Folder` value of the rows that get the value (the others stay NULL).
    pub marker: Option<String>,
}

/// One new attribute and the field it becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Addition {
    pub uuid: String,
    pub name: String,
    /// The number handed out from the shared counter.
    pub number: u64,
    /// `Fld<number>`.
    pub field: FieldEntry,
    /// Where the field went in the table's field list (0 = first).
    pub position: usize,
    /// `Use` of a catalog attribute (0 `ForItem`, 1 `ForFolder`, 2 `ForFolderAndItem`).
    pub usage: Option<i64>,
    /// The `Indexing` the attribute is added with: 0 `DontIndex`, 1 `Index`, 2 `IndexWithAdditionalOrder`.
    pub indexing: i64,
    /// The declared indexes the object's entry got for it (`ByFieldFld12`, ...): the same as for a switch.
    pub indexes: Vec<String>,
}

impl Addition {
    /// The `_Folder` value of the rows that get the default when the field is nullable: items for an
    /// attribute `ForItem`, folders for one `ForFolder`; the other rows get NULL.
    pub fn folder_marker(&self) -> Option<&'static str> {
        match self.usage {
            Some(0) => Some("0x01"),
            Some(1) => Some("0x00"),
            _ => None,
        }
    }
}

/// One removed attribute and the field that goes with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    pub uuid: String,
    pub name: String,
    /// The number of its `Fld` entry in `DBNames` (the entry stays there).
    pub number: u64,
    /// The field the stored entry had.
    pub field: FieldEntry,
    /// The declared indexes that went with the field.
    pub indexes: Vec<String>,
}

/// One widened attribute: a variable string that gets a longer limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Widening {
    pub uuid: String,
    pub name: String,
    /// The number of its `Fld` entry in `DBNames`.
    pub number: u64,
    /// The field the stored entry had, and the field it becomes (the same but for the length).
    pub before: FieldEntry,
    pub after: FieldEntry,
    /// The limit before and after, in characters.
    pub from: u64,
    pub to: u64,
}

/// One attribute whose `Indexing` is switched: the declared indexes of the table entry change, the columns
/// stay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSwitch {
    pub uuid: String,
    pub name: String,
    /// The number of its `Fld` entry in `DBNames` (unchanged: a switched flag adds no name).
    pub number: u64,
    pub field: String,
    /// `Indexing` before and after: 0 `DontIndex`, 1 `Index`, 2 `IndexWithAdditionalOrder`.
    pub from: i64,
    pub to: i64,
    /// The declared indexes added (`ByFieldFld12`) or removed (`ByFieldFld12`, `ByDocDate (- Fld12)`).
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// A tabular section of a rebuilt object that is new, or that gets new attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionPlan {
    pub uuid: String,
    pub name: String,
    /// The sub-table, `VT155`.
    pub table: String,
    /// A new section: its table is created (not rebuilt) and its `VT` and `LineNo` entries are new in `DBNames`.
    pub created: Option<CreatedSection>,
    /// The attributes that are new (every attribute of a new section) and the fields they became; `position` is
    /// the place in the sub-table's field list, whose first field is the line number.
    pub additions: Vec<Addition>,
    /// The declared indexes the sub-table got (`ByFieldFld12`).
    pub indexes: Vec<String>,
}

/// The numbers a new tabular section took from the shared counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreatedSection {
    /// `VT<table_number>`.
    pub table_number: u64,
    /// `LineNo<line_no>`.
    pub line_no: u64,
}

pub fn indexing_word(mode: i64) -> &'static str {
    match mode {
        0 => "DontIndex",
        1 => "Index",
        _ => "IndexWithAdditionalOrder",
    }
}

/// One physical table of the rebuilt object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TablePlan {
    /// The columns the table has now.
    pub old_columns: Vec<Column>,
    /// The table it becomes.
    pub table: PhysicalTable,
    /// The column list and the select list of the copy.
    pub insert_columns: Vec<String>,
    pub insert_values: Vec<String>,
    /// The table has no old table: it is created empty in the new generation (a new sub-table, the tables of a
    /// new object), never copied and never dropped. `old_columns` and the copy lists are empty.
    pub create: bool,
}

impl TablePlan {
    /// A table the plan creates from nothing.
    pub fn created(table: PhysicalTable) -> Self {
        Self {
            old_columns: Vec::new(),
            table,
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
            create: true,
        }
    }
}

/// One rebuilt object.
#[derive(Clone, Debug)]
pub struct ObjectPlan {
    pub kind: ObjectKind,
    /// `Reference20`, `Document39`.
    pub object: String,
    pub object_uuid: String,
    pub object_name: String,
    pub additions: Vec<Addition>,
    pub removals: Vec<Removal>,
    pub widenings: Vec<Widening>,
    pub switches: Vec<IndexSwitch>,
    /// The tabular sections that are new or got new attributes, in the order of the descriptor.
    pub sections: Vec<SectionPlan>,
    pub tables: Vec<TablePlan>,
    /// The columns of `Method::AlterAdd`.
    pub alter: Vec<AlterColumn>,
    /// The object is new (S1-F): all its tables are created, nothing is copied or dropped, and the
    /// attribute lists above are empty (its attributes are part of the object, not changes of it).
    pub created: bool,
}

/// A `Params` row of the derived caches, rewritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheUpdate {
    pub row_name: String,
    /// The stored form (raw deflate, or plain text for `siVersions`).
    pub row: Vec<u8>,
    /// What changed in it.
    pub what: String,
    /// The platform writes the `*.si` rows anew (`Creation` moves too); `siVersions` only moves `Modified`.
    pub set_creation: bool,
}

/// The whole plan.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The rebuilt objects, in the order the platform handles them.
    pub objects: Vec<ObjectPlan>,
    /// The new `DBSchema` / `CurrentSchema` text (BOM included).
    pub new_schema: Vec<u8>,
    /// The new `DBNames` text (BOM included) and its stored form.
    pub new_names_text: Vec<u8>,
    pub new_names_row: Vec<u8>,
    /// The new `DBNamesVersion-DBNames` row.
    pub names_version_row: Vec<u8>,
    pub old_schema_sha256: String,
    /// Size and SHA-256 (hex) of the stored `DBNames` row the plan was made from.
    pub old_names_size: usize,
    pub old_names_sha256: String,
    pub method: Method,
    /// The derived caches rewritten with the plan.
    pub caches: Vec<CacheUpdate>,
}

impl ObjectPlan {
    /// `new attributes Fld1 = A, Fld2 = B; removed attributes Fld3 = C`, or `new catalog, tables ...`.
    pub fn changes(&self) -> String {
        if self.created {
            return format!(
                "new {}, tables {}",
                self.kind.label(),
                self.tables
                    .iter()
                    .map(|table| table.table.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let mut parts = Vec::new();
        if !self.additions.is_empty() {
            parts.push(format!(
                "new attributes {}",
                self.additions
                    .iter()
                    .map(|addition| format!("{} = {}", addition.field.name, addition.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.removals.is_empty() {
            parts.push(format!(
                "removed attributes {}",
                self.removals
                    .iter()
                    .map(|removal| format!("{} = {}", removal.field.name, removal.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.widenings.is_empty() {
            parts.push(format!(
                "widened attributes {}",
                self.widenings
                    .iter()
                    .map(|widening| format!(
                        "{} = {} ({} -> {})",
                        widening.after.name, widening.name, widening.from, widening.to
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.switches.is_empty() {
            parts.push(format!(
                "switched indexes {}",
                self.switches
                    .iter()
                    .map(|switch| format!(
                        "{} = {} ({} -> {})",
                        switch.field,
                        switch.name,
                        indexing_word(switch.from),
                        indexing_word(switch.to)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let created: Vec<String> = self
            .sections
            .iter()
            .filter(|section| section.created.is_some())
            .map(|section| {
                format!(
                    "{} = {} ({} attribute(s))",
                    section.table,
                    section.name,
                    section.additions.len()
                )
            })
            .collect();
        if !created.is_empty() {
            parts.push(format!("new tabular sections {}", created.join(", ")));
        }
        let grown: Vec<String> = self
            .sections
            .iter()
            .filter(|section| section.created.is_none())
            .map(|section| {
                format!(
                    "{}: {}",
                    section.name,
                    section
                        .additions
                        .iter()
                        .map(|addition| format!("{} = {}", addition.field.name, addition.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        if !grown.is_empty() {
            parts.push(format!(
                "new attributes of tabular sections {}",
                grown.join("; ")
            ));
        }
        parts.join("; ")
    }

    /// The attributes are added or removed (the derived caches list them), not only changed in place.
    pub fn changes_the_attribute_list(&self) -> bool {
        !self.additions.is_empty() || !self.removals.is_empty() || !self.sections.is_empty()
    }
}

impl Plan {
    /// Every physical table of every rebuilt object.
    pub fn tables(&self) -> impl Iterator<Item = &TablePlan> {
        self.objects.iter().flat_map(|object| object.tables.iter())
    }

    /// Every new attribute of every tabular section of every object.
    pub fn section_additions(&self) -> impl Iterator<Item = &Addition> {
        self.objects
            .iter()
            .flat_map(|object| object.sections.iter())
            .flat_map(|section| section.additions.iter())
    }

    /// Every new attribute of every object.
    pub fn additions(&self) -> impl Iterator<Item = &Addition> {
        self.objects
            .iter()
            .flat_map(|object| object.additions.iter())
    }

    /// Every removed attribute of every object.
    pub fn removals(&self) -> impl Iterator<Item = &Removal> {
        self.objects
            .iter()
            .flat_map(|object| object.removals.iter())
    }

    /// Every widened attribute of every object.
    pub fn widenings(&self) -> impl Iterator<Item = &Widening> {
        self.objects
            .iter()
            .flat_map(|object| object.widenings.iter())
    }

    /// Every attribute of every object whose index is switched.
    pub fn switches(&self) -> impl Iterator<Item = &IndexSwitch> {
        self.objects
            .iter()
            .flat_map(|object| object.switches.iter())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A stored or staged row as text: raw deflate when it is, itself otherwise.
pub fn row_bytes(stored: &[u8]) -> Vec<u8> {
    inflate(stored).unwrap_or_else(|_| stored.to_vec())
}

// ---------------------------------------------------------------------------
// The staged image against the stored one.

/// What the whole image says about attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Inventory {
    /// The descriptor the attribute sits in.
    owner: String,
    name: String,
    pattern: String,
    /// The wrapper's element after the body (indexing for the shapes that have one).
    flag: String,
}

fn attribute_inventory(
    rows: &BTreeMap<String, Vec<u8>>,
    only: &BTreeSet<String>,
) -> Result<HashMap<String, Inventory>> {
    let mut out = HashMap::new();
    for (owner, stored) in rows {
        if !only.contains(owner) {
            continue;
        }
        let tree = parse_row(&row_bytes(stored))
            .with_context(|| format!("descriptor {owner} is not brace text"))?;
        scan(&tree, owner, &mut out);
    }
    Ok(out)
}

fn is_body(node: &Brace) -> bool {
    let Some(items) = node.as_list() else {
        return false;
    };
    items.first().and_then(Brace::as_atom) == Some("27")
        && items.get(1).and_then(Brace::as_list).is_some_and(|typed| {
            typed.first().and_then(Brace::as_atom) == Some("2")
                && typed
                    .get(2)
                    .and_then(Brace::as_list)
                    .and_then(|pattern| pattern.first())
                    .and_then(Brace::as_str)
                    == Some("Pattern")
        })
}

fn register(body: &Brace, flag: &str, owner: &str, out: &mut HashMap<String, Inventory>) {
    let typed = body.at(&[1]).and_then(Brace::as_list).unwrap_or_default();
    let uuid = typed
        .get(1)
        .and_then(|header| header.at(&[1, 2]))
        .and_then(Brace::as_atom)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = typed
        .get(1)
        .and_then(|header| header.at(&[2]))
        .and_then(Brace::as_str)
        .unwrap_or_default()
        .to_owned();
    let pattern = typed
        .get(2)
        .map(crate::metadata_model::brace::serialize)
        .unwrap_or_default();
    out.entry(uuid).or_insert(Inventory {
        owner: owner.to_owned(),
        name,
        pattern,
        flag: flag.to_owned(),
    });
}

fn scan(node: &Brace, owner: &str, out: &mut HashMap<String, Inventory>) {
    let Some(items) = node.as_list() else {
        return;
    };
    // A catalog's attributes: the collection `{<class>,<n>,{{v,<body>,<indexing>,<use>,...},0}...}`.
    // `Use` decides whether the field is nullable, so it is part of what must not change.
    if items.first().and_then(Brace::as_atom) == Some(crate::restructure::catalog::ATTRIBUTES) {
        for item in items.iter().skip(2) {
            let Some(record) = item.at(&[0]).and_then(Brace::as_list) else {
                continue;
            };
            if let Some(body) = record.get(1).filter(|body| is_body(body)) {
                let atom = |index: usize| {
                    record
                        .get(index)
                        .and_then(Brace::as_atom)
                        .unwrap_or_default()
                };
                register(body, &format!("{}|{}", atom(2), atom(3)), owner, out);
            }
        }
    }
    // Any other wrapper record {v,<body>,<indexing>...}: the element after the body is the indexing.
    if let Some(body) = items.get(1).filter(|body| is_body(body)) {
        let indexing = items.get(2).and_then(Brace::as_atom).unwrap_or_default();
        register(body, &format!("{indexing}|"), owner, out);
    }
    if is_body(node) {
        register(node, "", owner, out);
    }
    for child in items {
        scan(child, owner, out);
    }
}

/// The staged image may be the whole configuration (what a native `config import` stages) or only the
/// files that changed (`import files --partial`): a file that is not staged stays as it is. What it must
/// not do is add a file (a new object) or delete one (the `deleted` marker names the deletions).
fn check_files(image: &StagedImage) -> Result<()> {
    let added: Vec<_> = image
        .new_files
        .difference(&image.old_files)
        .filter(|name| name.as_str() != "deleted")
        .collect();
    if !added.is_empty() {
        bail!(
            "the staged image adds {} file(s) (first: {:?}): a new object is not supported",
            added.len(),
            added.first()
        );
    }
    Ok(())
}

/// The list the `deleted` row of a stage holds: `<count>,"<id>",<flag>,...` (a byte order mark first;
/// `0` for a stage that removes nothing). The platform's import writes one whenever the stage removes a
/// metadata element that has an id -- an attribute is one -- and lists the ids with the flag 1.
pub fn parse_deleted(stored: &[u8]) -> Result<Vec<(String, i64)>> {
    let bytes = row_bytes(stored);
    let text = String::from_utf8(bytes).context("the deleted row is not UTF-8")?;
    let text = text.trim_start_matches('\u{feff}').trim();
    let mut tokens = text.split(',').map(str::trim);
    let count: usize = tokens
        .next()
        .and_then(|token| token.parse().ok())
        .context("the deleted row has no count")?;
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        let id = tokens
            .next()
            .and_then(|token| token.strip_prefix('"')?.strip_suffix('"'))
            .context("the deleted row lists fewer names than it counts")?;
        let flag: i64 = tokens
            .next()
            .and_then(|token| token.parse().ok())
            .context("the deleted row has a name without a flag")?;
        list.push((id.to_ascii_lowercase(), flag));
    }
    if tokens.next().is_some() {
        bail!("the deleted row has text after its list");
    }
    Ok(list)
}

/// A row of an online update the target has pending: `DynamicallyUpdated`, `versions_dynupdate_<g>`,
/// `<uuid>_dynupdate_<g>` and `<uuid>_dynupdate_<g>.0` (`id` lower-cased, as [`parse_deleted`] gives it).
/// The platform's import lists them in `deleted` (as Config rows, flag 0) whenever the target carries
/// such an update, and so does this program's; the apply drops them with the update it makes.
fn is_pending_update_row(id: &str) -> bool {
    id == "dynamicallyupdated" || id.contains("_dynupdate_")
}

/// The stage's `deleted` row may name the attributes the stage removes and the rows of a pending online
/// update, and nothing else: a removed file or object is not this restructuring's.
fn check_deleted(image: &StagedImage, removed: &BTreeSet<String>) -> Result<()> {
    let Some(stored) = &image.deleted else {
        return Ok(());
    };
    for (id, flag) in parse_deleted(stored)? {
        if flag == 0 && is_pending_update_row(&id) {
            continue;
        }
        if !removed.contains(&id) {
            bail!(
                "the staged image deletes {id}, which is not an attribute it removes: not supported"
            );
        }
        if flag != 1 {
            bail!("the deleted row gives {id} the flag {flag}, the platform writes 1");
        }
    }
    Ok(())
}

/// The new and the removed attributes of one object.
struct Change {
    /// The descriptor of the object whose attributes changed.
    owner: String,
    /// The new attributes' uuids.
    added: BTreeSet<String>,
    /// The removed attributes' uuids.
    removed: BTreeSet<String>,
    /// The uuids of the attributes whose type text differs (`check_object` and `plan_object` decide
    /// whether it is a widening of a variable string, the one change of a type that is supported).
    retyped: BTreeSet<String>,
    /// The uuids of the attributes whose `Indexing` differs and nothing else of their flags (`Use`):
    /// `plan_object` decides whether the switch is one the platform was traced on.
    reindexed: BTreeSet<String>,
}

/// The flags of an attribute wrapper as the inventory keeps them, `<indexing>|<use>`, without the indexing.
fn flags_but_indexing(flag: &str) -> &str {
    flag.split_once('|').map_or("", |(_, rest)| rest)
}

/// Compares the attribute inventory of the two images: the new and the removed attributes by object.
fn find_changes(image: &StagedImage) -> Result<Vec<Change>> {
    // Only descriptors whose text differs are parsed: the rest cannot have changed a fact.
    let mut differing = BTreeSet::new();
    for (name, new) in &image.new_descriptors {
        match image.old_descriptors.get(name) {
            Some(old) if row_bytes(old) == row_bytes(new) => {}
            _ => {
                differing.insert(name.clone());
            }
        }
    }
    let old = attribute_inventory(&image.old_descriptors, &differing)?;
    let new = attribute_inventory(&image.new_descriptors, &differing)?;
    let mut added: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut removed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut retyped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reindexed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (uuid, entry) in &new {
        match old.get(uuid) {
            None => {
                added
                    .entry(entry.owner.clone())
                    .or_default()
                    .insert(uuid.clone());
            }
            Some(before) => {
                if before.pattern != entry.pattern {
                    retyped
                        .entry(entry.owner.clone())
                        .or_default()
                        .insert(uuid.clone());
                }
                if before.flag != entry.flag {
                    // The indexing alone can be switched; a change of `Use` or of anything else in the
                    // wrapper cannot.
                    if flags_but_indexing(&before.flag) != flags_but_indexing(&entry.flag) {
                        bail!(
                            "attribute {} of {} changes its use or flags ({:?} -> {:?}): not supported",
                            entry.name,
                            entry.owner,
                            before.flag,
                            entry.flag
                        );
                    }
                    reindexed
                        .entry(entry.owner.clone())
                        .or_default()
                        .insert(uuid.clone());
                }
                if before.owner != entry.owner {
                    bail!(
                        "attribute {} moves between objects: not supported",
                        entry.name
                    );
                }
            }
        }
    }
    for (uuid, entry) in &old {
        if !new.contains_key(uuid) {
            removed
                .entry(entry.owner.clone())
                .or_default()
                .insert(uuid.clone());
        }
    }
    let owners: BTreeSet<&String> = added
        .keys()
        .chain(removed.keys())
        .chain(retyped.keys())
        .chain(reindexed.keys())
        .collect();
    Ok(owners
        .into_iter()
        .map(|owner| Change {
            owner: owner.clone(),
            added: added.get(owner).cloned().unwrap_or_default(),
            removed: removed.get(owner).cloned().unwrap_or_default(),
            retyped: retyped.get(owner).cloned().unwrap_or_default(),
            reindexed: reindexed.get(owner).cloned().unwrap_or_default(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// The plan.

/// The field a widened variable string becomes, and the limit before and after. Only a longer limit of a
/// limited variable string is supported: the column stays `nvarchar(n)` and the copy reads it as it is. A
/// fixed string, an unlimited one (`nvarchar(max)`), a shorter limit, another kind of type and any other
/// difference of the type text are refused.
fn widened_field(
    before: &AttributeFacts,
    after: &AttributeFacts,
    stored: &FieldEntry,
    nullable: bool,
) -> Result<(FieldEntry, u64, u64)> {
    const VARIABLE: u64 = 0x8000_0000;
    let refuse = |why: &str| {
        anyhow::anyhow!(
            "attribute {} changes its type ({why}): only a longer limit of a variable string is supported",
            after.name
        )
    };
    let (Ok(was), Ok(now)) = (
        type_entries(&before.pattern_node),
        type_entries(&after.pattern_node),
    ) else {
        return Err(refuse("a type that is not mapped to a field"));
    };
    let ([was], [now]) = (was.as_slice(), now.as_slice()) else {
        return Err(refuse("a composite type"));
    };
    if was.tag != "S" || now.tag != "S" {
        return Err(refuse("not a string"));
    }
    if was.a & VARIABLE == 0 || now.a & VARIABLE == 0 {
        return Err(refuse("a fixed string"));
    }
    let (from, to) = (was.a & !VARIABLE, now.a & !VARIABLE);
    if from == 0 || to == 0 {
        return Err(refuse("an unlimited string"));
    }
    if to < from {
        return Err(refuse("the limit is shorter"));
    }
    if to == from {
        return Err(refuse(
            "the limit is the same, another part of the type differs",
        ));
    }
    let item = |node: &Brace| {
        node.as_list()
            .and_then(|items| items.get(1))
            .and_then(Brace::as_list)
            .map(<[Brace]>::to_vec)
    };
    let same_but_length = match (item(&before.pattern_node), item(&after.pattern_node)) {
        (Some(a), Some(b)) => {
            a.len() == b.len()
                && a.iter().zip(&b).enumerate().all(|(index, (x, y))| {
                    index == 1
                        || crate::metadata_model::brace::serialize(x)
                            == crate::metadata_model::brace::serialize(y)
                })
        }
        _ => false,
    };
    if !same_but_length {
        return Err(refuse("more than the limit differs"));
    }
    if stored.types.as_slice() != std::slice::from_ref(was) || stored.nullable != nullable {
        return Err(refuse("the stored field is not what the stored type makes"));
    }
    Ok((
        FieldEntry::new(&stored.name, nullable, vec![now.clone()]),
        from,
        to,
    ))
}

/// The select-list expression of a column of the new table: the old column of the same name (a widened
/// string is read as it is, the new column is longer), or the default of a column the old table did not
/// have.
fn select_expr(
    column: &Column,
    alias: &str,
    old: &HashMap<&str, &Column>,
    widened: bool,
    folder_marker: Option<&str>,
    entry: Option<&TypeEntry>,
) -> Result<String> {
    if let Some(before) = old.get(column.name.as_str()) {
        let widening = widened
            && before.nullable == column.nullable
            && matches!(
                (&before.sql_type, &column.sql_type),
                (SqlType::NVarChar(from), SqlType::NVarChar(to)) if to > from
            );
        if !widening && (before.sql_type != column.sql_type || before.nullable != column.nullable) {
            bail!(
                "column {} changes from {:?} to {:?}: a type change is converted by the platform, not by this prototype",
                column.name,
                before.sql_type,
                column.sql_type
            );
        }
        return Ok(format!("{alias}.{}", column.name));
    }
    default_expr(column, folder_marker, entry)
}

/// The type in a `CAST(... AS <type>)`.
fn cast_type(sql_type: &SqlType) -> Result<String> {
    Ok(match sql_type {
        SqlType::Binary(n) => format!("BINARY({n})"),
        SqlType::VarBinary(n) => format!("VARBINARY({n})"),
        SqlType::VarBinaryMax => "VARBINARY(MAX)".to_owned(),
        SqlType::Numeric(p, s) => format!("NUMERIC({p}, {s})"),
        SqlType::Int => "INT".to_owned(),
        SqlType::BigInt => "BIGINT".to_owned(),
        SqlType::DateTime2 => "DATETIME2(0)".to_owned(),
        SqlType::NVarChar(n) => format!("NVARCHAR({n})"),
        SqlType::NVarCharMax => "NVARCHAR(MAX)".to_owned(),
        SqlType::NChar(n) => format!("NCHAR({n})"),
        SqlType::Timestamp => bail!("a timestamp column has no default"),
    })
}

/// The value of a column the old table did not have (measured in cases a2, h and the types case).
/// `folder_marker` is the value of `_Folder` of the rows that get the default when the column is
/// nullable (`0x01` items for an attribute `ForItem`, `0x00` folders for one `ForFolder`); the other
/// rows get NULL.
fn default_expr(
    column: &Column,
    folder_marker: Option<&str>,
    entry: Option<&TypeEntry>,
) -> Result<String> {
    if column.nullable && folder_marker.is_none() {
        bail!(
            "no default is verified for the nullable column {}",
            column.name
        );
    }
    let value = match &column.sql_type {
        SqlType::Binary(1) => "0x00".to_owned(),
        SqlType::Binary(n) => format!("0x{}", "00".repeat(*n as usize)),
        SqlType::VarBinaryMax => match entry {
            Some(entry) if entry.tag == "B" && entry.a == 0x8000_0000 => {
                "0x01010800000000000000EFBBBF7B2255227D".to_owned()
            }
            _ => bail!(
                "no default is verified for the binary column {}",
                column.name
            ),
        },
        SqlType::Numeric(p, s) => format!("CAST(0 AS NUMERIC({p}, {s}))"),
        SqlType::Int | SqlType::BigInt => "0".to_owned(),
        SqlType::DateTime2 => "CAST('2001-01-01T00:00:00' AS DATETIME2(0))".to_owned(),
        SqlType::NVarChar(_) | SqlType::NVarCharMax => "N''".to_owned(),
        SqlType::NChar(n) => format!("CAST(N'{}' AS NCHAR({n}))", " ".repeat(*n as usize)),
        other => bail!("no default is known for {other:?} ({})", column.name),
    };
    if let (true, Some(marker)) = (column.nullable, folder_marker) {
        // A field of a hierarchical catalog that only items (or only folders) have: the default for
        // them, NULL for the others (`_Folder = 0x01` marks an item, `0x00` a folder).
        return Ok(format!(
            "CAST(CASE WHEN T1._Folder = {marker} THEN {value} END AS {})",
            cast_type(&column.sql_type)?
        ));
    }
    Ok(value)
}

/// The columns of the copy, in the order the platform lists them: the fields first, then, for a
/// sub-table, the separator columns, the owner key and the line key.
fn insert_order(table: &PhysicalTable) -> Vec<&Column> {
    let prefix = table.implicit_prefix;
    if prefix == 0 {
        return table.columns.iter().collect();
    }
    let (head, fields) = table.columns.split_at(prefix);
    let mut order: Vec<&Column> = fields.iter().collect();
    order.extend(&head[1..prefix - 1]);
    order.push(&head[0]);
    order.push(&head[prefix - 1]);
    order
}

const ROOT_CATALOGS: &str = "cf4abea6-37b2-11d4-940f-008048da11f9";
const ROOT_DOCUMENTS: &str = "061d872a-5787-460e-95ac-ed74ea3a3e84";

/// The uuid of the configuration's own descriptor, which the `root` row names.
pub fn configuration_uuid(root_row: &[u8]) -> Result<String> {
    let root = parse_row(&row_bytes(root_row)).context("the root row")?;
    Ok(root
        .as_list()
        .and_then(|items| items.get(1))
        .and_then(Brace::as_atom)
        .context("the root row names no configuration")?
        .to_ascii_lowercase())
}

/// The position of every catalog and document in the configuration's own lists.
fn configuration_order(inputs: &Inputs) -> Result<HashMap<String, usize>> {
    let configuration = configuration_uuid(&inputs.root_row)?;
    let descriptor = inputs
        .staged
        .old_descriptors
        .get(&configuration)
        .context("the configuration's own descriptor is not among the stored rows")?;
    let tree = parse_row(&row_bytes(descriptor)).context("the configuration's descriptor")?;
    let mut order = HashMap::new();
    collect_order(&tree, &mut order);
    Ok(order)
}

fn collect_order(node: &Brace, out: &mut HashMap<String, usize>) {
    let Some(members) = node.as_list() else {
        return;
    };
    if let (Some(class), Some(count)) = (
        members.first().and_then(Brace::as_atom),
        members.get(1).and_then(Brace::as_atom),
    ) && (class == ROOT_CATALOGS || class == ROOT_DOCUMENTS)
        && let Ok(count) = count.parse::<usize>()
        && members.len() == count + 2
        && members[2..].iter().all(|member| member.as_atom().is_some())
    {
        for (position, member) in members[2..].iter().enumerate() {
            if let Some(uuid) = member.as_atom() {
                out.insert(uuid.to_ascii_lowercase(), position);
            }
        }
        return;
    }
    for member in members {
        collect_order(member, out);
    }
}

/// One changed object, read.
struct Prepared {
    change: Change,
    old: ObjectFacts,
    new: ObjectFacts,
    /// The number of its main table (`Reference20` -> 20).
    number: u64,
}

/// The stored side: schema and names, and the counters the objects share.
struct Stored<'a> {
    schema: &'a DbSchema,
    main_names: &'a DbNames,
    predefined: &'a BTreeSet<String>,
}

/// What the objects share while they are planned one after another.
pub struct Running {
    /// The next number of the counter shared with the extensions.
    pub next: u64,
    pub names_after: DbNames,
}

impl Running {
    /// Hands out the next number of the shared counter to `uuid` as an entry of `kind` (`Fld`, `VT`, `LineNo`,
    /// `Reference`, ...) and records the entry in the names the plan publishes.
    pub fn allocate(&mut self, uuid: &str, kind: &str) -> Result<u64> {
        if self.names_after.number_of(uuid, kind).is_some() {
            bail!("{uuid} has an entry of kind {kind} in DBNames already");
        }
        let number = self.next;
        self.names_after.append(uuid, kind, number)?;
        self.next += 1;
        Ok(number)
    }

    /// Takes the next number of the shared counter and records no entry for it: the header moves to it (the
    /// platform hands one out for every catalog a stage creates, `create::allocate`).
    pub fn reserve(&mut self) {
        self.names_after.max = self.next;
        self.next += 1;
    }
}

/// Builds the plan.
pub fn plan(inputs: &Inputs, options: &PlanOptions) -> Result<Plan> {
    // The catalogs and documents the stage creates (S1-F) are planned apart from the changed objects: the rest
    // of the image is what the scan for changed attributes looks at.
    let created = create::find_created(&inputs.staged)?;
    create::check_count(&created)?;
    let rest = create::without_created(&inputs.staged, &created);
    check_files(&rest)?;
    let changes = find_changes(&rest)?;
    if changes.is_empty() && created.is_empty() {
        bail!(
            "the staged image adds no attribute, removes none, retypes none and switches no index: nothing this prototype restructures"
        );
    }
    let removed_everywhere: BTreeSet<String> = changes
        .iter()
        .flat_map(|change| change.removed.iter().cloned())
        .collect();
    check_deleted(&inputs.staged, &removed_everywhere)?;

    let schema = DbSchema::parse(&inputs.schema).context("the stored DBSchema")?;
    let main_names = DbNames::parse_row(&inputs.main_names).context("the stored DBNames")?;
    let extension_names = inputs
        .extension_names
        .iter()
        .map(|(name, row)| DbNames::parse_row(row).with_context(|| format!("the stored {name}")))
        .collect::<Result<Vec<_>>>()?;

    let mut prepared = Vec::new();
    for change in changes {
        let read = |rows: &BTreeMap<String, Vec<u8>>, which: &str| -> Result<ObjectFacts> {
            let row = rows
                .get(&change.owner)
                .with_context(|| format!("the {which} image has no descriptor {}", change.owner))?;
            ObjectFacts::parse(&parse_row(&row_bytes(row))?).with_context(|| {
                format!(
                    "the {which} descriptor {} is not a catalog or a document this prototype reads",
                    change.owner
                )
            })
        };
        let old = read(&inputs.staged.old_descriptors, "stored")?;
        let new = read(&inputs.staged.new_descriptors, "staged")?;
        check_object(&old, &new, &change)?;
        let number = main_names
            .number_of(new.uuid(), new.kind().table_kind())
            .with_context(|| {
                format!("{} {} has no table number", new.kind().label(), new.name())
            })?;
        prepared.push(Prepared {
            change,
            old,
            new,
            number,
        });
    }
    // An object an extension adopts is the platform's to change (S1-I).
    let tables: Vec<String> = prepared
        .iter()
        .map(|item| format!("{}{}", item.new.kind().table_kind(), item.number))
        .collect();
    let changed: Vec<ChangedObject<'_>> = prepared
        .iter()
        .zip(&tables)
        .map(|(item, table)| ChangedObject {
            kind: item.new.kind().label(),
            name: item.new.name(),
            uuid: item.new.uuid(),
            table,
        })
        .collect();
    crate::restructure::extensions::check(&inputs.extensions, &changed)?;
    // The platform walks the kinds in the configuration's order and the objects of a kind in the order
    // the configuration's descriptor lists them (traced: the types case; not by table number, not by name).
    if prepared.len() > 1 {
        let order = configuration_order(inputs)?;
        let mut keyed = Vec::new();
        for item in prepared {
            let position = *order.get(item.new.uuid()).with_context(|| {
                format!(
                    "the configuration does not list {} {}",
                    item.new.kind().label(),
                    item.new.name()
                )
            })?;
            keyed.push(((item.new.kind(), position), item));
        }
        keyed.sort_by_key(|(key, _)| *key);
        prepared = keyed.into_iter().map(|(_, item)| item).collect();
    }

    // Numbers come from the counter shared with the extensions.
    let mut counter_sources: Vec<&DbNames> = extension_names.iter().collect();
    counter_sources.push(&main_names);
    let mut running = Running {
        next: next_number(counter_sources.iter().copied()),
        names_after: main_names.clone(),
    };
    let stored = Stored {
        schema: &schema,
        main_names: &main_names,
        predefined: &inputs.predefined_tables,
    };

    // The created objects take their numbers first, all of them (`create::allocate`: the main tables, then the
    // attributes, then the sections). Then the platform walks the kinds and the objects of a kind in the
    // configuration's order: a changed object takes the numbers of its new attributes, a created catalog one number
    // no entry records (`create::walk_positions`; traced on N3, N6 and N7, which put a created catalog before and
    // after a changed one, and a created document beside a changed catalog). Every created table is empty.
    let mut created_entries = Vec::new();
    let mut created_in_order = Vec::new();
    let mut create_context = None;
    let mut reserves: Vec<usize> = Vec::new();
    if !created.is_empty() {
        if options.method != Method::Rebuild {
            bail!("a new object is created through the new generation, not by ALTER TABLE");
        }
        let context = create::context(inputs)?;
        created_in_order = create::in_platform_order(created, &context)?;
        create::allocate(&mut running, &created_in_order)?;
        let places = create::walk_positions(&context)?;
        for item in &created_in_order {
            if !matches!(item.kind, crate::restructure::object::ObjectKind::Catalog) {
                continue;
            }
            let key = *places
                .get(&item.uuid.to_ascii_lowercase())
                .with_context(|| {
                    format!(
                        "the configuration does not list the new catalog {}",
                        item.facts.name
                    )
                })?;
            reserves.push(
                prepared
                    .iter()
                    .filter(|other| {
                        places
                            .get(&other.new.uuid().to_ascii_lowercase())
                            .is_some_and(|place| *place < key)
                    })
                    .count(),
            );
        }
        reserves.sort_unstable();
        create_context = Some(context);
    }
    let mut objects = Vec::new();
    let mut entries = Vec::new();
    for (index, item) in prepared.iter().enumerate() {
        while reserves.first() == Some(&index) {
            reserves.remove(0);
            running.reserve();
        }
        let (object, entry) = plan_object(&stored, &mut running, item, options.method)?;
        entries.push((object.object.clone(), entry));
        objects.push(object);
    }
    for _ in reserves {
        running.reserve();
    }
    if let Some(context) = &create_context {
        for item in &created_in_order {
            let (object, entry) = create::plan_created(&running, &schema, inputs, context, item)?;
            created_entries.push(entry);
            objects.push(object);
        }
    }

    // Publish: the schema with the entries moved to the end (rebuilt tables sit at the end of the list in the
    // order they were rebuilt; ahead of ConfigChngR where that is the last table, `DbSchema::insert_rebuilt`), the
    // names with the new numbers.
    // The tables of the created objects come first (the platform makes them before it rebuilds the changed ones:
    // N3, N6, N7).
    let mut new_schema = schema.clone();
    for entry in created_entries {
        new_schema.insert_rebuilt(entry);
    }
    for (object, entry) in entries {
        new_schema.remove(&object)?;
        new_schema.insert_rebuilt(entry);
    }
    let new_names_text = running.names_after.to_text();
    let new_names_row = deflate(&new_names_text)?;
    let version = options
        .names_version
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // The caches list the attributes (the XDTO model its properties, the object registry its records) and
    // say nothing of a string's limit: a stage that only widens strings leaves them as they are.
    let mut caches = Vec::new();
    if create_context.is_some() && objects.iter().any(|object| !object.removals.is_empty()) {
        bail!(
            "a new object together with removed attributes of existing objects is not built: the caches of the new object are chained on the additions only (N3, N6 and N7 trace additions)"
        );
    }
    if create_context.is_some() && objects.iter().any(|object| !object.sections.is_empty()) {
        bail!(
            "a new object together with new tabular sections (or attributes of them) of existing objects is not built: both rewrite the registry, the index of the generated types and the XDTO model"
        );
    }
    if let Some(object) = objects
        .iter()
        .find(|object| !object.removals.is_empty() && !object.sections.is_empty())
    {
        bail!(
            "{} {} removes an attribute and adds a tabular section (or attributes of one) in one stage: not built (the caches of a removal are not composed with the ones of a section of the same object)",
            object.kind.label(),
            object.object_name
        );
    }
    // The objects with tabular sections get their rows from the caches module (it also does their attributes); the
    // others -- attributes added or removed -- from the edits of `xdto_update` and `registry_update`, made on the
    // rows the first left. The two sets edit different objects, so the order does not matter.
    let with_sections: Vec<usize> = (0..prepared.len())
        .filter(|&index| !objects[index].sections.is_empty())
        .collect();
    let with_attributes: Vec<usize> = (0..prepared.len())
        .filter(|&index| {
            objects[index].sections.is_empty() && objects[index].changes_the_attribute_list()
        })
        .collect();
    let mut rows: Vec<RowText> = Vec::new();
    if !(with_sections.is_empty() || options.skip_xdto && options.skip_registry) {
        // A tabular section reaches a third row (the index of the generated types) and the row types of the
        // XDTO model; with both the XDTO model and the registry left alone the index goes with them.
        let subset: Vec<&Prepared> = with_sections
            .iter()
            .map(|&index| &prepared[index])
            .collect();
        for row in section_cache_rows(inputs, &subset)? {
            if (options.skip_registry && row.name == cache_rows::REGISTRY)
                || (options.skip_xdto && row.name == cache_rows::XDTO)
            {
                continue;
            }
            rows.push(row);
        }
    }
    if !with_attributes.is_empty() {
        let subset: Vec<&Prepared> = with_attributes
            .iter()
            .map(|&index| &prepared[index])
            .collect();
        let planned: Vec<&ObjectPlan> = with_attributes
            .iter()
            .map(|&index| &objects[index])
            .collect();
        if !options.skip_xdto {
            let row = xdto_update(inputs, &subset, &planned, &rows)?;
            put_row(&mut rows, row);
        }
        if !options.skip_registry {
            let row = registry_update(inputs, &subset, &planned, &rows)?;
            put_row(&mut rows, row);
        }
    }
    if !with_sections.is_empty() {
        rows.sort_by(|a, b| a.name.cmp(&b.name));
    }
    for row in rows {
        caches.push(CacheUpdate {
            row_name: row.name,
            row: deflate(&row.text)?,
            what: row.what,
            set_creation: true,
        });
    }
    if let Some(context) = &create_context {
        create::extend_caches(
            inputs,
            context,
            &created_in_order,
            &running.names_after,
            options,
            &mut caches,
        )?;
    }
    if !caches.is_empty() {
        let names: Vec<&str> = caches.iter().map(|cache| cache.row_name.as_str()).collect();
        caches.push(versions_update(inputs, &names)?);
    }
    Ok(Plan {
        objects,
        new_schema: new_schema.to_text(),
        new_names_text,
        new_names_row,
        names_version_row: version_row(&version),
        old_schema_sha256: sha256_hex(&inputs.schema),
        old_names_size: inputs.main_names.len(),
        old_names_sha256: sha256_hex(&inputs.main_names),
        method: options.method,
        caches,
    })
}

/// Plans one object: the fields, the tables and the copy.
fn plan_object(
    stored: &Stored<'_>,
    running: &mut Running,
    item: &Prepared,
    method: Method,
) -> Result<(ObjectPlan, Brace)> {
    let new_facts = &item.new;
    let kind = new_facts.kind();
    let object = format!("{}{}", kind.table_kind(), item.number);
    // The tables the object's other names stand for. The change-registration table of an exchange
    // plan (`ReferenceChngR`, `DocumentChngR`) is not rebuilt with the object (cases a2, b); the
    // predefined-data table (`RefSInf`) is empty for a catalog without predefined items and was not
    // rebuilt either, but for the catalog that has them (14 of 75) the platform rebuilt it with the
    // object (case h): that one is refused. Any other table of the object is refused.
    let change_registration = format!("{}ChngR", kind.table_kind());
    for entry in stored.main_names.entries_of(new_facts.uuid()) {
        if entry.kind == kind.table_kind() {
            continue;
        }
        let table = format!("{}{}", entry.kind, entry.number);
        if stored.schema.position(&table).is_none() {
            continue;
        }
        let harmless = if entry.kind == change_registration {
            true
        } else if entry.kind == "RefSInf" {
            !stored.predefined.contains(&table)
        } else {
            false
        };
        if !harmless {
            bail!(
                "{} {} has the companion table {table}: the platform rebuilds it with the object (predefined data?), not supported",
                kind.label(),
                new_facts.name()
            );
        }
    }
    let old_position = stored
        .schema
        .position(&object)
        .with_context(|| format!("the stored DBSchema has no table {object}"))?;
    let old_view = stored.schema.view(old_position)?;
    let old_fields = old_view.fields()?;
    check_stored_fields(&item.old, &old_view, &old_fields, stored.main_names)?;
    check_stored_sections(&item.old, &old_view, stored.main_names)?;
    let has_folder = old_fields.iter().any(|field| field.name == "Folder");

    let mut fields_now = old_fields.clone();
    // The new table entry: the stored one, the removed fields (and their indexes) out first, then the new
    // fields in. The names of the removed stay in DBNames: the platform never takes an entry out.
    let mut entry = stored.schema.tables()[old_position].clone();
    let mut removals = Vec::new();
    for attribute in item.old.attributes() {
        if !item.change.removed.contains(&attribute.uuid) {
            continue;
        }
        let number = stored
            .main_names
            .number_of(&attribute.uuid, "Fld")
            .with_context(|| format!("attribute {} has no number in DBNames", attribute.name))?;
        let field_name = format!("Fld{number}");
        let position = fields_now
            .iter()
            .position(|field| field.name == field_name)
            .with_context(|| format!("the stored table has no field {field_name}"))?;
        let field = fields_now.remove(position);
        let taken = remove_field(&mut entry, &field_name)?;
        if taken != field {
            bail!("the field {field_name} of the entry is not the field of the table view");
        }
        let indexes = remove_field_indexes(&mut entry, &field_name)
            .with_context(|| format!("attribute {}", attribute.name))?;
        removals.push(Removal {
            uuid: attribute.uuid.clone(),
            name: attribute.name.clone(),
            number,
            field,
            indexes,
        });
    }
    if removals.len() != item.change.removed.len() {
        bail!(
            "some removed attributes are not among the own attributes of {} {}",
            kind.label(),
            new_facts.name()
        );
    }
    if method == Method::AlterAdd && !removals.is_empty() {
        bail!("the alter method adds columns only");
    }

    // The retyped attributes: each is a variable string whose limit grows. The field keeps its place and its
    // name; only its type entry changes (declared indexes name the field, so they stay as they are).
    let mut widenings = Vec::new();
    for attribute in new_facts.attributes() {
        if !item.change.retyped.contains(&attribute.uuid) {
            continue;
        }
        let Some(before) = item
            .old
            .attributes()
            .iter()
            .find(|candidate| candidate.uuid == attribute.uuid)
        else {
            continue;
        };
        let number = stored
            .main_names
            .number_of(&attribute.uuid, "Fld")
            .with_context(|| format!("attribute {} has no number in DBNames", attribute.name))?;
        let field_name = format!("Fld{number}");
        let position = fields_now
            .iter()
            .position(|field| field.name == field_name)
            .with_context(|| format!("the stored table has no field {field_name}"))?;
        let nullable = new_facts.nullable(attribute, has_folder);
        let (after, from, to) = widened_field(before, attribute, &fields_now[position], nullable)?;
        replace_field(&mut entry, &after)?;
        widenings.push(Widening {
            uuid: attribute.uuid.clone(),
            name: attribute.name.clone(),
            number,
            before: fields_now[position].clone(),
            after: after.clone(),
            from,
            to,
        });
        fields_now[position] = after;
    }
    if widenings.len() != item.change.retyped.len() {
        bail!(
            "some retyped attributes are not among the own attributes of {} {}",
            kind.label(),
            new_facts.name()
        );
    }
    if method == Method::AlterAdd && !widenings.is_empty() {
        bail!("the alter method adds columns only");
    }

    let mut additions = Vec::new();
    // Metadata order decides both the numbers and the places.
    let attributes = new_facts.attributes();
    for (index, attribute) in attributes.iter().enumerate() {
        if !item.change.added.contains(&attribute.uuid) {
            continue;
        }
        let indexing = match attribute.indexing {
            Some(mode @ 0..=2) => mode,
            other => bail!(
                "attribute {} has the indexing {other:?}: not one of the three modes",
                attribute.name
            ),
        };
        if indexing != 0 && method == Method::AlterAdd {
            bail!("the alter method adds columns without indexes");
        }
        let entries = type_entries(&attribute.pattern_node)
            .with_context(|| format!("attribute {}", attribute.name))?;
        let nullable = new_facts.nullable(attribute, has_folder);
        let number = running.allocate(&attribute.uuid, "Fld")?;
        let field = FieldEntry::new(&format!("Fld{number}"), nullable, entries);
        // After the field of the attribute before it; the first attribute goes after the
        // standard fields.
        let position = match index.checked_sub(1).map(|before| &attributes[before]) {
            Some(previous) => {
                let previous_number = running
                    .names_after
                    .number_of(&previous.uuid, "Fld")
                    .with_context(|| format!("attribute {} has no field number", previous.name))?;
                fields_now
                    .iter()
                    .position(|candidate| candidate.name == format!("Fld{previous_number}"))
                    .with_context(|| {
                        format!("the stored table has no field Fld{previous_number}")
                    })?
                    + 1
            }
            None => fields_now
                .iter()
                .rposition(|candidate| kind.standard_fields().contains(&candidate.name.as_str()))
                .map_or(0, |last| last + 1),
        };
        fields_now.insert(position, field.clone());
        additions.push(Addition {
            uuid: attribute.uuid.clone(),
            name: attribute.name.clone(),
            number,
            field,
            position,
            usage: attribute.usage,
            indexing,
            indexes: Vec::new(),
        });
    }
    for addition in &additions {
        insert_field(&mut entry, addition.position, &addition.field)?;
    }
    // The indexes of the attributes that come indexed (after the fields are in place: the entries sit in the order of
    // the fields; the same entries as for a switched flag).
    for addition in additions
        .iter_mut()
        .filter(|addition| addition.indexing != 0)
    {
        addition.indexes =
            add_field_indexes(&mut entry, &addition.field.name, addition.indexing == 2)
                .with_context(|| format!("the new attribute {}", addition.name))?;
    }

    // The switched indexes (after the fields are in place: the entries sit in the order of the fields).
    let mut switches = Vec::new();
    for attribute in attributes.iter() {
        if !item.change.reindexed.contains(&attribute.uuid) {
            continue;
        }
        let Some(before) = item
            .old
            .attributes()
            .iter()
            .find(|candidate| candidate.uuid == attribute.uuid)
        else {
            continue;
        };
        let (Some(from), Some(to)) = (before.indexing, attribute.indexing) else {
            bail!(
                "attribute {} has no indexing to switch: not supported",
                attribute.name
            );
        };
        // Traced on the platform: DontIndex <-> Index (a flat catalog, a hierarchical one, a document) and
        // DontIndex <-> IndexWithAdditionalOrder (a hierarchical catalog, a document); Index <->
        // IndexWithAdditionalOrder is not.
        if !matches!((from, to), (0, 1 | 2) | (1 | 2, 0)) {
            bail!(
                "attribute {} switches its index from {} to {}: only DontIndex <-> Index and DontIndex <-> IndexWithAdditionalOrder are traced",
                attribute.name,
                indexing_word(from),
                indexing_word(to)
            );
        }
        let number = stored
            .main_names
            .number_of(&attribute.uuid, "Fld")
            .with_context(|| format!("attribute {} has no number in DBNames", attribute.name))?;
        let field = format!("Fld{number}");
        let (mut added, mut removed) = (Vec::new(), Vec::new());
        if to == 0 {
            removed = remove_field_indexes(&mut entry, &field)
                .with_context(|| format!("attribute {}", attribute.name))?;
            if removed.is_empty() {
                bail!(
                    "attribute {} was indexed in the metadata but the stored table has no index for {field}",
                    attribute.name
                );
            }
        } else {
            added = add_field_indexes(&mut entry, &field, to == 2)
                .with_context(|| format!("attribute {}", attribute.name))?;
        }
        switches.push(IndexSwitch {
            uuid: attribute.uuid.clone(),
            name: attribute.name.clone(),
            number,
            field,
            from,
            to,
            added,
            removed,
        });
    }
    if switches.len() != item.change.reindexed.len() {
        bail!(
            "some switched indexes are not among the own attributes of {} {}",
            kind.label(),
            new_facts.name()
        );
    }
    if method == Method::AlterAdd && !switches.is_empty() {
        bail!("the alter method adds columns only");
    }

    // The tabular sections, after the own attributes (the numbers come in that order): the new attributes of
    // the sections that were there, and the tables of the new sections. The numbers of a new section are its
    // `VT` and its `LineNo`, then one for each of its attributes.
    let sections = plan_sections(stored, running, item, &object, &mut entry)?;
    let section_added: usize = sections.iter().map(|section| section.additions.len()).sum();
    if additions.len() + section_added != item.change.added.len() {
        bail!(
            "some new attributes are not among the attributes of {} {} or of its tabular sections",
            kind.label(),
            new_facts.name()
        );
    }
    if method == Method::AlterAdd && !sections.is_empty() {
        bail!("the alter method adds columns to the main table only");
    }

    let new_view = TableView::new(&entry)?;
    let old_tables = physical_tables(&old_view)?;
    let new_tables = physical_tables(&new_view)?;
    let created: Vec<String> = sections
        .iter()
        .filter(|section| section.created.is_some())
        .map(|section| format!("_{object}_{}", section.table))
        .collect();
    if old_tables.len() + created.len() != new_tables.len() {
        bail!("the rebuild changes the number of tables of {object}");
    }
    let every_addition: Vec<&Addition> = additions
        .iter()
        .chain(sections.iter().flat_map(|section| section.additions.iter()))
        .collect();
    let mut tables = Vec::new();
    for (index, after) in new_tables.into_iter().enumerate() {
        let Some(before) = old_tables.get(index) else {
            // A table the change creates: it stands after the old ones, in the order of its section.
            if created.get(index - old_tables.len()) != Some(&after.name) {
                bail!(
                    "the new table {} is not one of the new sections' tables",
                    after.name
                );
            }
            tables.push(TablePlan::created(after));
            continue;
        };
        if before.name != after.name {
            bail!("table {} became {}", before.name, after.name);
        }
        let alias = format!("T{}", index + 1);
        let old_columns: HashMap<&str, &Column> = before
            .columns
            .iter()
            .map(|column| (column.name.as_str(), column))
            .collect();
        let mut insert_columns = Vec::new();
        let mut insert_values = Vec::new();
        for column in insert_order(&after) {
            if column.sql_type == SqlType::Timestamp {
                continue;
            }
            if column.identity {
                bail!(
                    "column {} is an identity column: not supported",
                    column.name
                );
            }
            // The new field this column belongs to (`_Fld1`, `_Fld1_S`, ...): its first type entry
            // and the folder marker of a nullable one.
            let new_field = every_addition.iter().find(|addition| {
                let base = format!("_{}", addition.field.name);
                column.name == base || column.name.starts_with(&format!("{base}_"))
            });
            let widened = widenings
                .iter()
                .any(|widening| column.name == format!("_{}", widening.after.name));
            insert_columns.push(column.name.clone());
            insert_values.push(select_expr(
                column,
                &alias,
                &old_columns,
                widened,
                new_field.and_then(|addition| addition.folder_marker()),
                new_field.and_then(|addition| addition.field.types.first()),
            )?);
        }
        tables.push(TablePlan {
            old_columns: before.columns.clone(),
            table: after,
            insert_columns,
            insert_values,
            create: false,
        });
    }

    let mut alter = Vec::new();
    if method == Method::AlterAdd {
        for addition in &additions {
            let columns = field_columns(&addition.field)?;
            let [column] = &columns[..] else {
                bail!(
                    "the alter method takes fields of one column, {} has {}",
                    addition.name,
                    columns.len()
                );
            };
            let mut plain = column.clone();
            plain.nullable = false;
            alter.push(AlterColumn {
                table: format!("_{object}"),
                value: default_expr(&plain, None, addition.field.types.first())?,
                marker: addition.folder_marker().map(str::to_owned),
                column: column.clone(),
            });
        }
    }
    Ok((
        ObjectPlan {
            kind,
            object,
            object_uuid: new_facts.uuid().to_owned(),
            object_name: new_facts.name().to_owned(),
            additions,
            removals,
            widenings,
            switches,
            sections,
            tables,
            alter,
            created: false,
        },
        entry,
    ))
}

/// The fields of the sub-table `name` of an object's table entry.
fn subtable_fields(entry: &Brace, name: &str) -> Result<Vec<FieldEntry>> {
    let view = TableView::new(entry)?;
    view.subtables()?
        .into_iter()
        .find(|sub| sub.name() == name)
        .with_context(|| format!("the table has no sub-table {name}"))?
        .fields()
}

/// The tabular sections of a changed object: a section the object has got new attributes (their fields go
/// into its sub-table), a section it did not have is a new sub-table. Traced (cases e1, e3, e4) and
/// numbered in the order of the descriptor after the object's own attributes: a section that was there takes
/// one number for each new attribute; a new one takes `VT`, `LineNo` and then one for each attribute.
fn plan_sections(
    stored: &Stored<'_>,
    running: &mut Running,
    item: &Prepared,
    object: &str,
    entry: &mut Brace,
) -> Result<Vec<SectionPlan>> {
    let mut plans = Vec::new();
    let old_sections = item.old.sections();
    for section in item.new.sections() {
        let index_of = |attribute: &AttributeFacts| -> Result<bool> {
            match attribute.indexing {
                Some(0) => Ok(false),
                Some(1) => Ok(true),
                other => bail!(
                    "attribute {} of tabular section {} has the indexing {other:?}: only DontIndex and Index are traced in a tabular section",
                    attribute.name,
                    section.name
                ),
            }
        };
        let is_new = |attribute: &AttributeFacts| item.change.added.contains(&attribute.uuid);
        if old_sections.iter().any(|old| old.uuid == section.uuid) {
            if !section.attributes.iter().any(is_new) {
                continue;
            }
            let number = stored
                .main_names
                .number_of(&section.uuid, "VT")
                .with_context(|| format!("tabular section {} has no table number", section.name))?;
            let table = format!("VT{number}");
            let mut additions = Vec::new();
            let mut indexes = Vec::new();
            for (position, attribute) in section.attributes.iter().enumerate() {
                if !is_new(attribute) {
                    continue;
                }
                let indexed = index_of(attribute)?;
                let entries = type_entries(&attribute.pattern_node).with_context(|| {
                    format!(
                        "attribute {} of tabular section {}",
                        attribute.name, section.name
                    )
                })?;
                let number = running.allocate(&attribute.uuid, "Fld")?;
                let field = FieldEntry::new(&format!("Fld{number}"), false, entries);
                let fields = subtable_fields(entry, &table)?;
                // After the field of the attribute before it; the first goes after the line number.
                let at = match position
                    .checked_sub(1)
                    .map(|before| &section.attributes[before])
                {
                    Some(previous) => {
                        let previous_number = running
                            .names_after
                            .number_of(&previous.uuid, "Fld")
                            .with_context(|| {
                                format!("attribute {} has no field number", previous.name)
                            })?;
                        fields
                            .iter()
                            .position(|candidate| candidate.name == format!("Fld{previous_number}"))
                            .with_context(|| {
                                format!("the sub-table {table} has no field Fld{previous_number}")
                            })?
                            + 1
                    }
                    None => 1,
                };
                let sub = subtable_mut(entry, &table)?;
                insert_field(sub, at, &field)?;
                if indexed {
                    indexes.push(add_subtable_field_index(sub, &field.name)?);
                }
                additions.push(Addition {
                    uuid: attribute.uuid.clone(),
                    name: attribute.name.clone(),
                    number,
                    field,
                    position: at,
                    usage: None,
                    indexing: i64::from(indexed),
                    indexes: Vec::new(),
                });
            }
            plans.push(SectionPlan {
                uuid: section.uuid.clone(),
                name: section.name.clone(),
                table,
                created: None,
                additions,
                indexes,
            });
        } else {
            let table_number = running.allocate(&section.uuid, "VT")?;
            let line_no = running.allocate(&section.uuid, "LineNo")?;
            let mut fields = Vec::new();
            let mut declared = Vec::new();
            let mut additions = Vec::new();
            for attribute in &section.attributes {
                let indexed = index_of(attribute)?;
                let entries = type_entries(&attribute.pattern_node).with_context(|| {
                    format!(
                        "attribute {} of tabular section {}",
                        attribute.name, section.name
                    )
                })?;
                let number = running.allocate(&attribute.uuid, "Fld")?;
                let field = FieldEntry::new(&format!("Fld{number}"), false, entries);
                if indexed {
                    declared.push((
                        format!("ByField{}", field.name),
                        vec![field.name.clone(), "ID".to_owned()],
                    ));
                }
                additions.push(Addition {
                    uuid: attribute.uuid.clone(),
                    name: attribute.name.clone(),
                    number,
                    field: field.clone(),
                    position: fields.len() + 1,
                    usage: None,
                    indexing: i64::from(indexed),
                    indexes: Vec::new(),
                });
                fields.push(field);
            }
            let table = format!("VT{table_number}");
            push_subtable(
                entry,
                subtable_entry(
                    &table,
                    object,
                    &format!("LineNo{line_no}"),
                    &fields,
                    &declared,
                ),
            )?;
            plans.push(SectionPlan {
                uuid: section.uuid.clone(),
                name: section.name.clone(),
                table,
                created: Some(CreatedSection {
                    table_number,
                    line_no,
                }),
                additions,
                indexes: declared.into_iter().map(|(name, _)| name).collect(),
            });
        }
    }
    Ok(plans)
}

/// The parsed descriptor of a uuid: from `first`, else from `second`; each is parsed once.
fn memoized_descriptors<'a>(
    first: &'a BTreeMap<String, Vec<u8>>,
    second: &'a BTreeMap<String, Vec<u8>>,
) -> impl Fn(&str) -> Option<Brace> + 'a {
    let cache: RefCell<HashMap<String, Option<Brace>>> = RefCell::new(HashMap::new());
    move |uuid: &str| {
        let uuid = uuid.to_ascii_lowercase();
        if let Some(hit) = cache.borrow().get(&uuid) {
            return hit.clone();
        }
        let parsed = first
            .get(&uuid)
            .or_else(|| second.get(&uuid))
            .and_then(|stored| parse_row(&row_bytes(stored)).ok());
        cache.borrow_mut().insert(uuid, parsed.clone());
        parsed
    }
}

/// The cache rows of a stage that adds tabular sections or attributes of them: the object registry, the index of
/// the generated types and the XDTO model, from the descriptors before and after (`caches::change::rewrite`,
/// measured against the native result in cases e1, e3, e4, e5 and e6) for the objects `prepared` (the ones with
/// sections; the plan refuses an object that also removes).
fn section_cache_rows(inputs: &Inputs, prepared: &[&Prepared]) -> Result<Vec<RowText>> {
    let configuration = configuration_uuid(&inputs.root_row)?;
    // The descriptor rows parsed once: the traversal of the caches asks for every object of a kind, again and
    // again.
    let empty = BTreeMap::new();
    let before_row = memoized_descriptors(&inputs.staged.old_descriptors, &empty);
    let after_row = memoized_descriptors(
        &inputs.staged.new_descriptors,
        &inputs.staged.old_descriptors,
    );
    let root = after_row(&configuration)
        .context("the configuration's own descriptor is not among the rows")?;
    let changed: Vec<ChangedForCaches> = prepared
        .iter()
        .map(|item| ChangedForCaches {
            kind: match item.new.kind() {
                ObjectKind::Catalog => "Catalog",
                ObjectKind::Document => "Document",
            },
            uuid: item.new.uuid().to_owned(),
            table_number: None,
            has_help: false,
            has_predefined: false,
        })
        .collect();
    let cache = |name: &str| -> Option<Vec<u8>> {
        inputs
            .cache_rows
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, stored)| inflate(stored).unwrap_or_else(|_| stored.clone()))
    };
    let rows = rewrite(&Staged {
        root: &root,
        before: &before_row,
        after: &after_row,
        changed: &changed,
        cache: &cache,
    })
    .context("the cache rows of the tabular sections")?;
    let mut updates = Vec::new();
    for row in rows {
        let what = match row.name {
            name if name == cache_rows::REGISTRY => {
                "records of the new tabular sections and attributes in the object registry"
            }
            name if name == cache_rows::TYPE_INDEX => {
                "entries of the new tabular sections in the index of the generated types"
            }
            name if name == cache_rows::XDTO => {
                "row types, properties and property lines of the tabular sections in the XDTO model"
            }
            _ => "rewritten by the caches module",
        };
        updates.push(RowText {
            name: row.name.to_owned(),
            text: row.text,
            what: what.to_owned(),
        });
    }
    Ok(updates)
}

/// A cache row as text (inflated), what the plan changed in it, before it is deflated into a [`CacheUpdate`].
struct RowText {
    name: String,
    text: Vec<u8>,
    what: String,
}

/// The row `name` as the earlier edits of this plan left it, if they did.
fn edited<'a>(rows: &'a [RowText], name: &str) -> Option<&'a [u8]> {
    rows.iter()
        .find(|row| row.name.eq_ignore_ascii_case(name))
        .map(|row| row.text.as_slice())
}

/// Puts a row among the rows: a row of the same name is replaced (its text already holds the earlier edits).
fn put_row(rows: &mut Vec<RowText>, row: RowText) {
    match rows
        .iter_mut()
        .find(|kept| kept.name.eq_ignore_ascii_case(&row.name))
    {
        Some(kept) => {
            kept.what = format!("{}; {}", kept.what, row.what);
            kept.text = row.text;
        }
        None => rows.push(row),
    }
}

/// The cache row of the XDTO model: a property for each new attribute, after the property of the
/// attribute before it, and no property for a removed one.
fn xdto_update(
    inputs: &Inputs,
    prepared: &[&Prepared],
    objects: &[&ObjectPlan],
    earlier: &[RowText],
) -> Result<RowText> {
    let first = prepared
        .iter()
        .zip(objects)
        .find(|(_, object)| object.changes_the_attribute_list())
        .map(|(item, _)| item)
        .context("no object to update the XDTO model for")?;
    let needle = format!("{}.{}", first.new.kind().xdto_object(), first.new.name());
    for (name, stored) in &inputs.cache_rows {
        let text = match edited(earlier, name) {
            Some(text) => text.to_vec(),
            None => inflate(stored).unwrap_or_default(),
        };
        if !xdto::is_model(&text) {
            continue;
        }
        let mut model = xdto::Model::open(&text)?;
        if !model.has_object_type(&needle) {
            continue;
        }
        let mut count = 0usize;
        for (item, object) in prepared.iter().zip(objects) {
            let kind = item.new.kind();
            let type_name = format!("{}.{}", kind.xdto_object(), item.new.name());
            let attributes = item.new.attributes();
            for removal in &object.removals {
                model.remove_property(&type_name, &removal.name)?;
                count += 1;
            }
            for addition in &object.additions {
                let index = attributes
                    .iter()
                    .position(|attribute| attribute.uuid == addition.uuid)
                    .context("a new attribute is not among the object's attributes")?;
                let entry = addition
                    .field
                    .types
                    .first()
                    .context("a new field has no type")?;
                let line = xdto::property_line(entry, &addition.name, addition.field.nullable)?;
                let after = index
                    .checked_sub(1)
                    .map(|before| attributes[before].name.as_str());
                model.add_property(
                    &type_name,
                    after,
                    kind.xdto_standard(),
                    &addition.name,
                    &line,
                )?;
                count += 1;
            }
        }
        return Ok(RowText {
            name: name.clone(),
            text: model.to_text(),
            what: format!("{count} property line(s) added or removed in the XDTO model"),
        });
    }
    bail!("no XDTO model cache row knows {needle}: pass --skip-xdto to leave the caches alone")
}

/// The cache row of the object registry: a record for each new attribute.
fn registry_update(
    inputs: &Inputs,
    prepared: &[&Prepared],
    objects: &[&ObjectPlan],
    earlier: &[RowText],
) -> Result<RowText> {
    let (name, stored) = inputs
        .cache_rows
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(registry::REGISTRY_ROW))
        .with_context(|| {
            format!(
                "Params has no row {}: pass --skip-registry to leave the caches alone",
                registry::REGISTRY_ROW
            )
        })?;
    let text = match edited(earlier, name) {
        Some(text) => text.to_vec(),
        None => inflate(stored).context("the object registry row is not raw deflate")?,
    };
    let added: Vec<BTreeSet<String>> = objects
        .iter()
        .map(|object| {
            object
                .additions
                .iter()
                .map(|addition| addition.uuid.clone())
                .collect()
        })
        .collect();
    let additions: Vec<ObjectAdditions<'_>> = prepared
        .iter()
        .zip(&added)
        .map(|(item, added)| ObjectAdditions {
            kind: item.new.kind(),
            owner: item.new.uuid(),
            attributes: item.new.attributes(),
            added,
        })
        .collect();
    let removed: BTreeSet<String> = objects
        .iter()
        .flat_map(|object| object.removals.iter().map(|removal| removal.uuid.clone()))
        .collect();
    let mut updated = registry::remove_attributes(&text, &removed)?;
    let count: usize = added.iter().map(BTreeSet::len).sum();
    if count > 0 {
        updated = registry::add_attributes(&updated, &additions)?;
    }
    Ok(RowText {
        name: name.clone(),
        text: updated,
        what: format!(
            "{count} record(s) of attributes added, {} removed in the object registry",
            removed.len()
        ),
    })
}

/// `siVersions` with a new version for each cache row rewritten.
fn versions_update(inputs: &Inputs, rows: &[&str]) -> Result<CacheUpdate> {
    let (name, stored) = inputs
        .cache_rows
        .iter()
        .find(|(name, _)| name == registry::VERSIONS_ROW)
        .with_context(|| format!("Params has no row {}", registry::VERSIONS_ROW))?;
    let text = stored
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(stored.as_slice());
    let updated = registry::bump_versions(text, rows)?;
    let mut row = Vec::new();
    if stored.starts_with(&[0xEF, 0xBB, 0xBF]) {
        row.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    row.extend_from_slice(&updated);
    Ok(CacheUpdate {
        row_name: name.clone(),
        row,
        what: format!("new versions of {} cache row(s)", rows.len()),
        set_creation: false,
    })
}

/// The object is the same as far as the table goes, except for the appended attributes.
fn check_object(old: &ObjectFacts, new: &ObjectFacts, change: &Change) -> Result<()> {
    if old.kind() != new.kind() || old.uuid() != new.uuid() || old.uuid() != change.owner {
        bail!("the changed descriptor is not the object the attributes sit in");
    }
    let (before, after) = (old.shape(), new.shape());
    if before != after {
        let names: Vec<&str> = before
            .iter()
            .zip(&after)
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a.0)
            .collect();
        bail!(
            "{} {} changes {}: not supported",
            new.kind().label(),
            new.name(),
            names.join(", ")
        );
    }
    let own = |uuid: &String| {
        old.attributes()
            .iter()
            .chain(new.attributes())
            .any(|attribute| &attribute.uuid == uuid)
    };
    let touches_own_attributes = change
        .added
        .iter()
        .chain(&change.removed)
        .chain(&change.retyped)
        .chain(&change.reindexed)
        .any(own);
    old.check_supported(touches_own_attributes)?;
    // The tabular sections: the ones the object has stay, in their order and with their properties, and gain
    // attributes at any place; the new ones stand after them (a table is created for each), with the line
    // number of 5 digits the platform gives every section, used for the items of a hierarchy the default way and
    // with at least one attribute. What is not built: a section removed, moved or renamed, one of the old
    // ones that loses, moves, retypes or re-indexes an attribute.
    let (old_sections, new_sections) = (old.sections(), new.sections());
    let label = |what: &str| {
        format!(
            "{} {} {what}: not supported",
            new.kind().label(),
            new.name()
        )
    };
    if new_sections.len() < old_sections.len() {
        bail!("{}", label("removes a tabular section"));
    }
    for (before, after) in old_sections.iter().zip(new_sections) {
        if before.uuid != after.uuid {
            bail!(
                "{}",
                label("moves a tabular section or puts a new one before the old ones")
            );
        }
        if before.name != after.name
            || before.usage != after.usage
            || before.line_number_length != after.line_number_length
        {
            bail!(
                "{}",
                label(&format!("changes the tabular section {}", before.name))
            );
        }
        let kept: Vec<&AttributeFacts> = after
            .attributes
            .iter()
            .filter(|attribute| !change.added.contains(&attribute.uuid))
            .collect();
        let same = kept.len() == before.attributes.len()
            && kept.iter().zip(&before.attributes).all(|(after, before)| {
                after.uuid == before.uuid
                    && after.pattern == before.pattern
                    && after.indexing == before.indexing
            });
        if !same {
            bail!(
                "{}",
                label(&format!(
                    "removes, moves or changes an attribute of the tabular section {}",
                    before.name
                ))
            );
        }
        // Not traced: a section of a hierarchy used for folders only, or for both, that gets an attribute.
        if before.usage.is_some_and(|usage| usage != 0)
            && after
                .attributes
                .iter()
                .any(|attribute| change.added.contains(&attribute.uuid))
        {
            bail!(
                "{}",
                label(&format!(
                    "adds an attribute to the tabular section {}, whose use is not ForItem",
                    before.name
                ))
            );
        }
    }
    for section in &new_sections[old_sections.len()..] {
        if old_sections.iter().any(|old| old.uuid == section.uuid) {
            bail!("{}", label("moves a tabular section"));
        }
        if section.line_number_length != 5 || section.usage.is_some_and(|usage| usage != 0) {
            bail!(
                "{}",
                label(&format!(
                    "adds the tabular section {} with a line number length other than 5 or another use than ForItem",
                    section.name
                ))
            );
        }
        if section.attributes.is_empty() {
            bail!(
                "{}",
                label(&format!(
                    "adds the tabular section {} with no attributes",
                    section.name
                ))
            );
        }
        if let Some(known) = section
            .attributes
            .iter()
            .find(|attribute| !change.added.contains(&attribute.uuid))
        {
            bail!(
                "{}",
                label(&format!(
                    "puts the attribute {} of the stored image into the new tabular section {}",
                    known.name, section.name
                ))
            );
        }
    }
    // The attributes that stay are the same in both images, in the same order, with the same type (a
    // retyped one is judged by `widening`) and indexing: the stored ones without the removed against the
    // staged ones without the new.
    let kept_before: Vec<&AttributeFacts> = old
        .attributes()
        .iter()
        .filter(|attribute| !change.removed.contains(&attribute.uuid))
        .collect();
    let kept_after: Vec<&AttributeFacts> = new
        .attributes()
        .iter()
        .filter(|attribute| !change.added.contains(&attribute.uuid))
        .collect();
    let same_attributes = kept_after.len() == kept_before.len()
        && kept_after.iter().zip(&kept_before).all(|(after, before)| {
            after.uuid == before.uuid
                && (after.pattern == before.pattern || change.retyped.contains(&after.uuid))
                && (after.indexing == before.indexing || change.reindexed.contains(&after.uuid))
                && after.usage == before.usage
        });
    if !same_attributes {
        bail!(
            "{} {} reorders or changes its stored attributes: not supported",
            new.kind().label(),
            new.name()
        );
    }
    Ok(())
}

/// Every stored attribute has its field in the stored table, and the fields the generator can
/// compute agree with the stored ones.
fn check_stored_fields(
    facts: &ObjectFacts,
    table: &TableView<'_>,
    fields: &[FieldEntry],
    names: &DbNames,
) -> Result<()> {
    let has_folder = fields.iter().any(|field| field.name == "Folder");
    check_attribute_fields(facts.attributes(), table, fields, names, &|attribute| {
        facts.nullable(attribute, has_folder)
    })
}

/// The same for the tabular sections of the stored object: each one has its sub-table (named by its `VT`
/// entry), the line number field (its `LineNo` entry) first and the fields of its attributes after it, none
/// of them nullable.
fn check_stored_sections(
    facts: &ObjectFacts,
    table: &TableView<'_>,
    names: &DbNames,
) -> Result<()> {
    let subtables = table.subtables()?;
    for section in facts.sections() {
        let number = names
            .number_of(&section.uuid, "VT")
            .with_context(|| format!("tabular section {} has no table number", section.name))?;
        let name = format!("VT{number}");
        let sub = subtables
            .iter()
            .find(|sub| sub.name() == name)
            .with_context(|| format!("table {} has no sub-table {name}", table.name()))?;
        let fields = sub.fields()?;
        let line_no = names.number_of(&section.uuid, "LineNo").with_context(|| {
            format!("tabular section {} has no line number entry", section.name)
        })?;
        if fields.first().map(|field| field.name.as_str()) != Some(&format!("LineNo{line_no}")) {
            bail!("the sub-table {name} does not start with LineNo{line_no}");
        }
        check_attribute_fields(&section.attributes, sub, &fields, names, &|_| false)
            .with_context(|| format!("tabular section {}", section.name))?;
    }
    Ok(())
}

fn check_attribute_fields(
    attributes: &[AttributeFacts],
    table: &TableView<'_>,
    fields: &[FieldEntry],
    names: &DbNames,
    nullable: &dyn Fn(&AttributeFacts) -> bool,
) -> Result<()> {
    let mut last = None;
    for attribute in attributes {
        let number = names
            .number_of(&attribute.uuid, "Fld")
            .with_context(|| format!("attribute {} has no number in DBNames", attribute.name))?;
        let field_name = format!("Fld{number}");
        let position = fields
            .iter()
            .position(|field| field.name == field_name)
            .with_context(|| {
                format!(
                    "table {} has no field {field_name} for attribute {}",
                    table.name(),
                    attribute.name
                )
            })?;
        if last.is_some_and(|last| position <= last) {
            bail!(
                "the stored fields of {} are not in metadata order",
                table.name()
            );
        }
        last = Some(position);
        if let Ok(entries) = type_entries(&attribute.pattern_node) {
            let expected_nullable = nullable(attribute);
            let stored = &fields[position];
            if stored.types != entries || stored.nullable != expected_nullable {
                bail!(
                    "the stored field {field_name} of attribute {} differs from what the generator makes of it: {:?} vs {:?}",
                    attribute.name,
                    stored,
                    FieldEntry::new(&field_name, expected_nullable, entries)
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The statements.

/// One statement of the restructure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    pub phase: Phase,
    pub label: String,
    pub sql: String,
    pub params: Vec<StatementParam>,
    /// The rows the statement must change (a `Params` update touches exactly one).
    pub expect_rows: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Guard,
    Create,
    Load,
    Indexes,
    DropOld,
    Rename,
    Publish,
}

/// A parameter of a statement (`@P1`, ...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatementParam {
    Bytes(Vec<u8>),
    Int(i64),
}

impl Plan {
    /// The NG name of a table or index.
    fn ng(name: &str) -> String {
        format!("{name}NG")
    }

    /// The statements in execution order, without the transaction control and the
    /// verification steps `exec` adds between the phases.
    pub fn statements(&self) -> Vec<Statement> {
        let mut out = Vec::new();
        let mut push = |phase: Phase,
                        label: String,
                        sql: String,
                        params: Vec<StatementParam>,
                        expect_rows: Option<u64>| {
            out.push(Statement {
                phase,
                label,
                sql,
                params,
                expect_rows,
            });
        };
        push(
            Phase::Guard,
            "the stored schema is the one the plan was made from".to_owned(),
            format!(
                "IF NOT EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE SchemaID = 0 AND Status = 100 \
                 AND DATALENGTH(NewGenCreated) = {len} AND DATALENGTH(NewGenDropped) = {len} \
                 AND CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2) = '{hash}') \
                 THROW 51000, 'SchemaStorage is not idle or its schema is not the planned one', 1;",
                len = EMPTY_GENERATION.len(),
                hash = self.old_schema_sha256.to_ascii_uppercase()
            ),
            Vec::new(),
            None,
        );
        if self.method == Method::AlterAdd {
            for object in &self.objects {
                for added in &object.alter {
                    let column = &added.column;
                    if column.nullable {
                        push(
                            Phase::Create,
                            format!("alter {} add {}", added.table, column.name),
                            format!(
                                "ALTER TABLE dbo.{} ADD {} {} NULL;",
                                added.table,
                                column.name,
                                column.sql_type.ddl()
                            ),
                            Vec::new(),
                            None,
                        );
                        push(
                            Phase::Create,
                            format!("set {} of the existing rows", column.name),
                            format!(
                                "UPDATE dbo.{} SET {} = {} WHERE _Folder = {};",
                                added.table,
                                column.name,
                                added.value,
                                added.marker.as_deref().unwrap_or("0x01")
                            ),
                            Vec::new(),
                            None,
                        );
                    } else {
                        push(
                            Phase::Create,
                            format!("alter {} add {}", added.table, column.name),
                            format!(
                                "ALTER TABLE dbo.{table} ADD {name} {ty} NOT NULL CONSTRAINT DF_ddl_tmp DEFAULT {value};\nALTER TABLE dbo.{table} DROP CONSTRAINT DF_ddl_tmp;",
                                table = added.table,
                                name = column.name,
                                ty = column.sql_type.ddl(),
                                value = added.value
                            ),
                            Vec::new(),
                            None,
                        );
                    }
                }
            }
        } else {
            for table in self.tables() {
                push(
                    Phase::Create,
                    format!("create {}NG", table.table.name),
                    create_table_sql(&table.table, "NG"),
                    Vec::new(),
                    None,
                );
            }
            for object in &self.objects {
                for (index, table) in object.tables.iter().enumerate() {
                    if table.create {
                        continue;
                    }
                    push(
                        Phase::Load,
                        format!("copy {} into {}NG", table.table.name, table.table.name),
                        format!(
                            "INSERT INTO dbo.{new}NG WITH(TABLOCK) ({columns}) SELECT\n{values}\nFROM dbo.{new} T{alias} WITH(NOLOCK);",
                            new = table.table.name,
                            columns = table.insert_columns.join(", "),
                            values = table.insert_values.join(",\n"),
                            alias = index + 1
                        ),
                        Vec::new(),
                        None,
                    );
                }
            }
            for table in self.tables() {
                for index in &table.table.indexes {
                    push(
                        Phase::Indexes,
                        format!("index {}NG", index.name),
                        create_index_sql(&table.table.name, index, "NG"),
                        Vec::new(),
                        None,
                    );
                }
            }
            for table in self.tables().filter(|table| !table.create) {
                push(
                    Phase::DropOld,
                    format!("drop {}", table.table.name),
                    format!("drop table dbo.{};", table.table.name),
                    Vec::new(),
                    None,
                );
            }
            for table in self.tables() {
                push(
                    Phase::Rename,
                    format!("rename {}NG", table.table.name),
                    format!(
                        "EXEC sp_rename N'{}', N'{}', 'OBJECT';",
                        Self::ng(&table.table.name),
                        table.table.name
                    ),
                    Vec::new(),
                    None,
                );
            }
            for table in self.tables() {
                for index in table
                    .table
                    .indexes
                    .iter()
                    .filter(|index| !index.name.is_empty())
                {
                    push(
                        Phase::Rename,
                        format!("rename index {}NG", index.name),
                        format!(
                            "EXEC sp_rename N'{}.{}', N'{}', 'INDEX';",
                            table.table.name,
                            Self::ng(&index.name),
                            index.name
                        ),
                        Vec::new(),
                        None,
                    );
                }
            }
        }
        push(
            Phase::Publish,
            "SchemaStorage: idle, the new schema, empty generations".to_owned(),
            "UPDATE dbo.SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;"
                .to_owned(),
            vec![
                StatementParam::Bytes(self.new_schema.clone()),
                StatementParam::Bytes(EMPTY_GENERATION.to_vec()),
                StatementParam::Bytes(EMPTY_GENERATION.to_vec()),
            ],
            Some(1),
        );
        push(
            Phase::Publish,
            "DBSchema".to_owned(),
            "UPDATE dbo.DBSchema SET SerializedData = @P1;".to_owned(),
            vec![StatementParam::Bytes(self.new_schema.clone())],
            Some(1),
        );
        for (name, row) in [
            ("DBNames", &self.new_names_row),
            ("DBNamesVersion-DBNames", &self.names_version_row),
        ] {
            push(
                Phase::Publish,
                format!("Params {name}"),
                format!(
                    "UPDATE dbo.Params SET BinaryData = @P1, DataSize = @P2, Modified = DATEADD(YEAR, 2000, SYSDATETIME()) WHERE FileName = N'{name}' AND PartNo = 0;"
                ),
                vec![
                    StatementParam::Bytes(row.clone()),
                    StatementParam::Int(row.len() as i64),
                ],
                Some(1),
            );
        }
        for cache in &self.caches {
            let creation = if cache.set_creation {
                "Creation = DATEADD(YEAR, 2000, SYSDATETIME()), "
            } else {
                ""
            };
            push(
                Phase::Publish,
                format!("Params {}: {}", cache.row_name, cache.what),
                format!(
                    "UPDATE dbo.Params SET BinaryData = @P1, DataSize = @P2, {creation}Modified = DATEADD(YEAR, 2000, SYSDATETIME()) WHERE FileName = N'{}' AND PartNo = 0;",
                    cache.row_name
                ),
                vec![
                    StatementParam::Bytes(cache.row.clone()),
                    StatementParam::Int(cache.row.len() as i64),
                ],
                Some(1),
            );
        }
        push(
            Phase::Publish,
            "Config: the changed staged rows replace the stored ones".to_owned(),
            PROMOTE_CONFIG.to_owned(),
            Vec::new(),
            None,
        );
        push(
            Phase::Publish,
            "ConfigSave: emptied".to_owned(),
            "DELETE FROM dbo.ConfigSave;".to_owned(),
            Vec::new(),
            None,
        );
        out
    }
}

/// The staged files replace the stored ones (a file that is not staged stays; the `deleted` marker is
/// not copied): the parts of a staged file that differ are deleted, the missing ones inserted. The same
/// statements serve a whole-configuration stage and a partial one.
pub const PROMOTE_CONFIG: &str = "\
DELETE c FROM dbo.Config c WHERE c.FileName IN (SELECT FileName FROM dbo.ConfigSave WHERE FileName <> N'deleted') \
AND NOT EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName AND s.PartNo = c.PartNo \
AND s.Creation = c.Creation AND s.Modified = c.Modified AND s.Attributes = c.Attributes AND s.DataSize = c.DataSize AND s.BinaryData = c.BinaryData);
INSERT INTO dbo.Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) \
SELECT s.FileName, s.Creation, s.Modified, s.Attributes, s.DataSize, s.BinaryData, s.PartNo FROM dbo.ConfigSave s \
WHERE s.FileName <> N'deleted' AND NOT EXISTS (SELECT 1 FROM dbo.Config c WHERE c.FileName = s.FileName AND c.PartNo = s.PartNo);";

/// A one-line summary for reports.
impl Plan {
    pub fn summary(&self) -> String {
        self.objects
            .iter()
            .map(|object| {
                format!(
                    "{} {} ({}): {}",
                    object.kind.label(),
                    object.object_name,
                    object.object,
                    object.changes()
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}
