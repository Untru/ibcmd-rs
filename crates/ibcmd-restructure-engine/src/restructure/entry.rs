//! The `DBSchema` entry of a new catalog or document (S1-F, `docs/apply/new-object.md` 1.2 and 3.2).
//!
//! The entry is generated from the object's descriptor, the numbers `DBNames` gives its tables and fields
//! and the common attributes that apply to it, and must be what the platform stores, byte for byte: the
//! test `tests_entry.rs` rebuilds the stored entry of **every** catalog and document of the БСП and compares
//! the text. What the builder does not cover it refuses (an error that says why), never guesses.
//!
//! ```text
//! table    = {"<Table><n>","N",<n>,"",<fields>,<sub-tables>,<indexes>,1,"R",<sepA>,<sepB>,"",0,0}
//! subtable = {"VT<n>","I",0,"<Table><n>",<fields>,{0},<indexes>,1,"S",{0},{0},"",0,0}
//! ```
//!
//! **Fields** of a catalog: `ID`, `Version`, `Marked`, `PredefinedID`, [`OwnerID`: a typed reference for one owner,
//! `E` and an untyped reference for several], [`ParentID`, and `Folder` for a hierarchy of folders and items],
//! `Code`, `Description`, the attributes, the common attributes that apply. Of a document: `ID`, `Version`,
//! `Marked`, `Date_Time`, `NumberPrefix`, `Number`, `Posted`, the attributes, the common attributes.
//!
//! **Declared indexes** of a catalog, in this order: `ByPredefinedIDNotUniq`; `OwnerCode`, `OwnerDescr`;
//! `ParentCode`, `ParentDescr`; `Code`, `Descr`; then for each attribute and each common attribute that is
//! indexed `ByOwnerField<f>`, `ByParentField<f>`, `ByField<f>`. Of a document: `ByDocNumPrefix`, `ByDocNum`,
//! `ByDocDate`, `ByField<f>`. The key of an index with `IndexWithAdditionalOrder` goes on with the order of
//! the object (`Description`, or `Code` when there is none, and `ID`, `Marked`; `Date_Time`, `ID`, `Marked`
//! for a document, whose `ByDocDate` also lists the field).

use anyhow::{Context, Result, bail};

use crate::brace_list;
use crate::metadata_model::brace::Brace;
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::{
    Member, Members, SectionMember, catalog_attribute_nullable,
};
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::catalog::type_entries;
use crate::restructure::common::CommonAttributes;
use crate::restructure::names::DbNames;
use crate::restructure::schema::{FieldEntry, TypeEntry};

/// Catalog or document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Catalog,
    Document,
}

impl Kind {
    pub fn parse(kind: &str) -> Result<Self> {
        match kind {
            "Catalog" => Ok(Self::Catalog),
            "Document" => Ok(Self::Document),
            other => bail!("{other}: only catalogs and documents have an entry builder"),
        }
    }

    /// The family of the main table's name in `DBNames` (`Reference20`, `Document39`).
    pub fn table_kind(self) -> &'static str {
        match self {
            Self::Catalog => "Reference",
            Self::Document => "Document",
        }
    }
}

/// The tables the reference types and the owners of the configuration stand for, from the names and tables
/// row `a07b62f0`: the `Ref` type id of an object -> its main table (`Reference572`), and the object's uuid
/// -> the same.
#[derive(Clone, Debug, Default)]
pub struct RefTables {
    by_type: std::collections::BTreeMap<String, String>,
    by_object: std::collections::BTreeMap<String, String>,
    /// A defined type's type id -> the items of its pattern.
    defined: std::collections::BTreeMap<String, Vec<Brace>>,
}

impl RefTables {
    pub fn build(names: &NamesTables) -> Self {
        let mut tables = Self::default();
        for entry in &names.entries {
            if let Some(table) = &entry.table {
                tables
                    .by_type
                    .insert(table.type_id.to_ascii_lowercase(), table.sql.clone());
                tables
                    .by_object
                    .insert(entry.object.to_ascii_lowercase(), table.sql.clone());
            }
        }
        tables
    }

    pub fn get(&self, type_id: &str) -> Option<&str> {
        self.by_type
            .get(&type_id.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Adds a defined type from its descriptor row `{1,{0,<TypeId>,<ValueId>,<header>,{"Pattern",...}},0}`.
    pub fn add_defined(&mut self, row: &Brace) -> Result<()> {
        let record = row
            .as_list()
            .and_then(|root| root.get(1))
            .and_then(Brace::as_list)
            .context("a defined type row has no record")?;
        let type_id = record
            .get(1)
            .and_then(Brace::as_atom)
            .context("a defined type has no type id")?
            .to_ascii_lowercase();
        let pattern = record
            .get(4)
            .and_then(Brace::as_list)
            .context("a defined type has no pattern")?;
        if pattern.first().and_then(Brace::as_str) != Some("Pattern") {
            bail!("a defined type has no type pattern");
        }
        self.defined.insert(type_id, pattern[1..].to_vec());
        Ok(())
    }

    /// The items of a pattern with the defined types replaced by their own items.
    fn expand(&self, items: &[Brace], depth: usize) -> Result<Vec<Brace>> {
        if depth > 8 {
            bail!("defined types nest deeper than 8 levels");
        }
        let mut out = Vec::new();
        for item in items {
            let defined = item
                .as_list()
                .filter(|fields| fields.first().and_then(Brace::as_str) == Some("#"))
                .and_then(|fields| fields.get(1))
                .and_then(Brace::as_atom)
                .and_then(|id| self.defined.get(&id.to_ascii_lowercase()));
            match defined {
                Some(inner) => out.extend(self.expand(inner, depth + 1)?),
                None => out.push(item.clone()),
            }
        }
        Ok(out)
    }

    /// Adds a table by hand (the object, its `Ref` type id and its table).
    pub fn with_table(mut self, object: &str, type_id: &str, table: &str) -> Self {
        self.by_type
            .insert(type_id.to_ascii_lowercase(), table.to_owned());
        self.by_object
            .insert(object.to_ascii_lowercase(), table.to_owned());
        self
    }

    pub fn of_object(&self, uuid: &str) -> Option<&str> {
        self.by_object
            .get(&uuid.to_ascii_lowercase())
            .map(String::as_str)
    }
}

/// Everything the entry is made of.
pub struct EntryInput<'a> {
    pub kind: Kind,
    /// The object's descriptor row, read for the slots (properties) of its owner record.
    pub facts: &'a ObjectFacts,
    /// Its attributes and tabular sections.
    pub members: &'a Members,
    /// `DBNames` holding the numbers of the object, its attributes, its sections and the common attributes.
    pub names: &'a DbNames,
    pub common: &'a CommonAttributes,
    /// The tables of the reference types and of the owners.
    pub refs: &'a RefTables,
}

fn field(name: &str, nullable: bool, types: Vec<TypeEntry>) -> Brace {
    FieldEntry::new(name, nullable, types).to_brace()
}

fn one(tag: &str, a: u64, b: u64, reference: &str, k: i64) -> Vec<TypeEntry> {
    vec![TypeEntry::new(tag, a, b, reference, k)]
}

fn list(items: Vec<Brace>) -> Brace {
    let mut all = vec![Brace::atom(items.len())];
    all.extend(items);
    Brace::List(all)
}

/// `{"<name>",<unique>,{<n>,"<field>"...},0,0,0,{0},0,0}`
fn index(name: &str, unique: bool, fields: &[&str]) -> Brace {
    let names = list(fields.iter().map(|field| Brace::str(*field)).collect());
    Brace::List(vec![
        Brace::str(name),
        Brace::num(i64::from(unique)),
        names,
        Brace::num(0),
        Brace::num(0),
        Brace::num(0),
        brace_list![Brace::num(0)],
        Brace::num(0),
        Brace::num(0),
    ])
}

/// `{0}` or `{1,{{<n>,"<field>"...}}}`
fn separators(names: &[String]) -> Brace {
    if names.is_empty() {
        return brace_list![Brace::num(0)];
    }
    let group = list(names.iter().map(|name| Brace::str(name.as_str())).collect());
    brace_list![Brace::num(1), Brace::List(vec![group])]
}

fn fld(names: &DbNames, uuid: &str, what: &str) -> Result<String> {
    let number = names
        .number_of(uuid, "Fld")
        .with_context(|| format!("DBNames has no field number for {what}"))?;
    Ok(format!("Fld{number}"))
}

/// The order of the primitive parts of a composite type in a field.
fn part_order(tag: &str) -> u8 {
    match tag {
        "L" => 0,
        "N" => 1,
        "T" => 2,
        "S" => 3,
        _ => 4,
    }
}

/// The type entries of an attribute: a primitive type, a value storage, a uuid, a reference type of the
/// configuration (`R` with the table) or a composite type (`E`, the primitive parts in the fixed order
/// `L N T S B`, and one untyped `R` when a reference is among them).
pub fn attribute_types(member: &Member, refs: &RefTables) -> Result<Vec<TypeEntry>> {
    let items = member
        .pattern
        .as_list()
        .with_context(|| format!("attribute {} has no pattern", member.name))?;
    let items = refs.expand(&items[1..], 0)?;
    let items = items.as_slice();
    let reference = |item: &Brace| -> Option<String> {
        let fields = item.as_list()?;
        if fields.first().and_then(Brace::as_str) != Some("#") {
            return None;
        }
        refs.get(fields.get(1)?.as_atom()?).map(str::to_owned)
    };
    let single = |item: &Brace| -> Result<Vec<TypeEntry>> {
        let pattern = Brace::List(vec![Brace::str("Pattern"), item.clone()]);
        type_entries(&pattern)
    };
    if let [item] = items {
        if let Some(table) = reference(item) {
            return Ok(one("R", 0, 0, &table, 3));
        }
        return single(item).with_context(|| {
            format!(
                "attribute {} is not covered by the entry builder",
                member.name
            )
        });
    }
    let mut parts: Vec<TypeEntry> = Vec::new();
    let mut references = false;
    for item in items {
        if reference(item).is_some() {
            references = true;
            continue;
        }
        let mut entries = single(item).with_context(|| {
            format!(
                "attribute {} is not covered by the entry builder",
                member.name
            )
        })?;
        parts.append(&mut entries);
    }
    parts.sort_by_key(|part| part_order(&part.tag));
    let mut out = vec![TypeEntry::new("E", 0, 0, "", 0)];
    out.append(&mut parts);
    if references {
        out.push(TypeEntry::new("R", 0, 0, "", 4));
    }
    Ok(out)
}

/// What the indexes of a table are made of.
struct Layout {
    owner: bool,
    hierarchical: bool,
    folders: bool,
    /// The field the additional order of an index goes on with (`Description`, `Code`, `Date_Time`).
    order_field: Option<&'static str>,
}

impl Layout {
    /// The declared indexes an indexed field (`mode` 1 `Index`, 2 `IndexWithAdditionalOrder`) makes.
    fn field_indexes(
        &self,
        name: &str,
        mode: i64,
        extras: &[String],
        out: &mut Vec<Brace>,
    ) -> Result<()> {
        let additional = match mode {
            0 => return Ok(()),
            1 => false,
            2 => true,
            other => bail!("Indexing {other} of {name} is not known"),
        };
        let tail: Vec<&str> = if additional {
            let order = self.order_field.with_context(|| {
                format!("an additional order of {name} in an object with no code and no description is not covered")
            })?;
            if extras.len() > 1 {
                bail!(
                    "{name} and {} other fields have an additional order: the order of their indexes' extra keys is not covered",
                    extras.len()
                );
            }
            let mut tail = vec![order, "ID", "Marked"];
            // the index goes on with the other additional-order attribute, when there is exactly one
            tail.extend(extras.iter().map(String::as_str));
            tail
        } else {
            vec!["ID"]
        };
        if self.owner {
            let mut keys = vec!["OwnerID", name];
            keys.extend(&tail);
            out.push(index(&format!("ByOwnerField{name}"), true, &keys));
        }
        if self.hierarchical {
            let mut keys = Vec::new();
            if self.owner {
                keys.push("OwnerID");
            }
            keys.push("ParentID");
            if self.folders {
                keys.push("Folder");
            }
            keys.push(name);
            keys.extend(&tail);
            out.push(index(&format!("ByParentField{name}"), true, &keys));
        }
        let mut keys = vec![name];
        keys.extend(&tail);
        out.push(index(&format!("ByField{name}"), true, &keys));
        Ok(())
    }
}

/// The entry of the object's main table with its sub-tables.
pub fn main_entry(input: &EntryInput<'_>) -> Result<Brace> {
    let kind = input.kind;
    let facts = input.facts;
    let members = input.members;
    let number = input
        .names
        .number_of(&facts.uuid, kind.table_kind())
        .with_context(|| {
            format!(
                "DBNames has no {} number for {}",
                kind.table_kind(),
                facts.name
            )
        })?;
    let table = format!("{}{number}", kind.table_kind());

    let mut fields: Vec<Brace> = vec![
        field("ID", false, one("R", 0, 0, &table, 2)),
        field("Version", false, one("V", 0, 0, "", 0)),
        field("Marked", false, one("L", 0, 0, "", 0)),
    ];
    let mut declared: Vec<Brace> = Vec::new();
    let mut layout = Layout {
        owner: false,
        hierarchical: false,
        folders: false,
        order_field: None,
    };
    let mut date_index: Option<usize> = None;
    match kind {
        Kind::Catalog => {
            fields.push(field("PredefinedID", false, one("B", 16, 0, "", 0)));
            let owners = facts.references("Owners")?;
            if !owners.is_empty() {
                layout.owner = true;
                let types = if let [owner] = owners.as_slice() {
                    let table = input.refs.of_object(owner).with_context(|| {
                        format!("the owner {owner} of {} has no table", facts.name)
                    })?;
                    one("R", 0, 0, table, 3)
                } else {
                    vec![
                        TypeEntry::new("E", 0, 0, "", 0),
                        TypeEntry::new("R", 0, 0, "", 4),
                    ]
                };
                fields.push(field("OwnerID", false, types));
            }
            layout.hierarchical = facts.number("Hierarchical")? == 1;
            layout.folders = layout.hierarchical && facts.number("HierarchyType")? == 0;
            if layout.hierarchical {
                fields.push(field("ParentID", false, one("R", 0, 0, &table, 3)));
                if layout.folders {
                    fields.push(field("Folder", false, one("L", 0, 0, "", 0)));
                }
            }
            let code = facts.number("CodeLength")?;
            if code > 0 {
                let code = code as u64;
                let types = if facts.number("CodeType")? == 1 {
                    let variable = facts.number("CodeAllowedLength")? == 1;
                    one(
                        "S",
                        if variable { 0x8000_0000 | code } else { code },
                        0,
                        "",
                        0,
                    )
                } else {
                    one("N", code, 0, "", 0)
                };
                fields.push(field("Code", false, types));
                layout.order_field = Some("Code");
            }
            let description = facts.number("DescriptionLength")?;
            if description > 0 {
                fields.push(field(
                    "Description",
                    false,
                    one("S", 0x8000_0000 | description as u64, 0, "", 0),
                ));
                layout.order_field = Some("Description");
            }
            // the declared indexes of the standard fields
            declared.push(index("ByPredefinedIDNotUniq", false, &["PredefinedID"]));
            let folders = layout.folders;
            let mut push = |name: &str, own: bool, parent: bool, field: &str| {
                let mut keys: Vec<&str> = Vec::new();
                if own {
                    keys.push("OwnerID");
                }
                if parent {
                    keys.push("ParentID");
                    if folders {
                        keys.push("Folder");
                    }
                }
                keys.push(field);
                keys.push("ID");
                declared.push(index(name, true, &keys));
            };
            if layout.owner && code > 0 {
                push("OwnerCode", true, false, "Code");
            }
            if layout.owner && description > 0 {
                push("OwnerDescr", true, false, "Description");
            }
            if layout.hierarchical && code > 0 {
                push("ParentCode", layout.owner, true, "Code");
            }
            if layout.hierarchical && description > 0 {
                push("ParentDescr", layout.owner, true, "Description");
            }
            if code > 0 {
                push("Code", false, false, "Code");
            }
            if description > 0 {
                push("Descr", false, false, "Description");
            }
        }
        Kind::Document => {
            fields.push(field("Date_Time", false, one("T", 0, 0, "", 0)));
            let periodic = facts.number("NumberPeriodicity")? != 0;
            if periodic {
                fields.push(field("NumberPrefix", false, one("T", 0, 0, "", 0)));
            }
            let length = facts.number("NumberLength")? as u64;
            let types = if facts.number("NumberType")? == 1 {
                let variable = facts.number("NumberAllowedLength")? == 1;
                one(
                    "S",
                    if variable {
                        0x8000_0000 | length
                    } else {
                        length
                    },
                    0,
                    "",
                    0,
                )
            } else {
                one("N", length, 0, "", 0)
            };
            fields.push(field("Number", false, types));
            fields.push(field("Posted", false, one("L", 0, 0, "", 0)));
            layout.order_field = Some("Date_Time");
            if periodic {
                declared.push(index(
                    "ByDocNumPrefix",
                    true,
                    &["NumberPrefix", "Number", "ID"],
                ));
            }
            declared.push(index("ByDocNum", true, &["Number", "ID"]));
            date_index = Some(declared.len());
            declared.push(index("ByDocDate", true, &["Date_Time", "ID", "Marked"]));
        }
    }

    // the attributes and their indexes
    let additional: Vec<String> = members
        .attributes
        .iter()
        .filter(|attribute| attribute.indexing == Some(2))
        .map(|attribute| fld(input.names, &attribute.uuid, &attribute.name))
        .collect::<Result<_>>()?;
    let mut date_extra: Vec<String> = Vec::new();
    for attribute in &members.attributes {
        let name = fld(
            input.names,
            &attribute.uuid,
            &format!("attribute {}", attribute.name),
        )?;
        let nullable = kind == Kind::Catalog
            && catalog_attribute_nullable(
                layout.hierarchical,
                if layout.folders { 0 } else { 1 },
                attribute.usage,
            );
        let types = attribute_types(attribute, input.refs)?;
        let mode = attribute.indexing.unwrap_or(0);
        fields.push(field(&name, nullable, types));
        let others: Vec<String> = additional
            .iter()
            .filter(|other| **other != name)
            .cloned()
            .collect();
        layout.field_indexes(&name, mode, &others, &mut declared)?;
        if kind == Kind::Document && mode == 2 {
            date_extra.push(name);
        }
    }

    // the common attributes that apply
    let mut separated: Vec<String> = Vec::new();
    let mut independent: Vec<String> = Vec::new();
    for attribute in input.common.applying(&facts.uuid) {
        let name = fld(
            input.names,
            &attribute.uuid,
            &format!("common attribute {}", attribute.name),
        )?;
        fields.push(field(&name, false, attribute.types.clone()));
        layout.field_indexes(&name, attribute.indexing, &additional, &mut declared)?;
        if kind == Kind::Document && attribute.indexing == 2 {
            date_extra.push(name.clone());
        }
        if attribute.separates_data {
            separated.push(name.clone());
            if attribute.independent {
                independent.push(name);
            }
        }
    }
    if let Some(at) = date_index {
        if date_extra.len() > 1 {
            bail!("several additional-order fields in a document are not covered");
        }
        if let Some(extra) = date_extra.first() {
            // the date index lists the additional-order field last
            declared[at] = index(
                "ByDocDate",
                true,
                &["Date_Time", "ID", "Marked", extra.as_str()],
            );
        }
    }

    // the sub-tables
    let mut subtables: Vec<Brace> = Vec::new();
    for section in &members.sections {
        subtables.push(subtable(input.names, input.refs, kind, &table, section)?);
    }

    Ok(Brace::List(vec![
        Brace::str(&table),
        Brace::str("N"),
        Brace::atom(number),
        Brace::str(""),
        list(fields),
        list(subtables),
        list(declared),
        Brace::num(1),
        Brace::str("R"),
        separators(&separated),
        separators(&independent),
        Brace::str(""),
        Brace::num(0),
        Brace::num(0),
    ]))
}

/// A tabular section's sub-table.
pub fn subtable(
    names: &DbNames,
    refs: &RefTables,
    kind: Kind,
    owner: &str,
    section: &SectionMember,
) -> Result<Brace> {
    let number = names
        .number_of(&section.uuid, "VT")
        .with_context(|| format!("DBNames has no VT number for section {}", section.name))?;
    let line = names
        .number_of(&section.uuid, "LineNo")
        .with_context(|| format!("DBNames has no LineNo number for section {}", section.name))?;
    let mut fields = vec![field(
        &format!("LineNo{line}"),
        false,
        one("N", 5, 0, "", 0),
    )];
    let mut declared = Vec::new();
    for attribute in &section.attributes {
        let name = fld(
            names,
            &attribute.uuid,
            &format!("attribute {} of {}", attribute.name, section.name),
        )?;
        let types = attribute_types(attribute, refs)?;
        // The platform indexes some references to documents in a document's tabular section although the
        // flag says `DontIndex` (two attributes of the БСП demo): what decides it is not known.
        if types
            .iter()
            .any(|entry| entry.tag == "R" && entry.reference.starts_with("Document"))
            && kind == Kind::Document
        {
            bail!(
                "attribute {} of section {} refers to a document: whether the platform indexes it is not known",
                attribute.name,
                section.name
            );
        }
        fields.push(field(&name, false, types));
        match attribute.indexing.unwrap_or(0) {
            0 => {}
            // the index of a sub-table is not unique (its key is the row of the owner, not the field)
            1 => declared.push(index(&format!("ByField{name}"), false, &[&name, "ID"])),
            other => bail!(
                "attribute {} of section {} has Indexing {other}: not covered",
                attribute.name,
                section.name
            ),
        }
    }
    Ok(Brace::List(vec![
        Brace::str(format!("VT{number}")),
        Brace::str("I"),
        Brace::num(0),
        Brace::str(owner),
        list(fields),
        brace_list![Brace::num(0)],
        if declared.is_empty() {
            brace_list![Brace::num(0)]
        } else {
            list(declared)
        },
        Brace::num(1),
        Brace::str("S"),
        brace_list![Brace::num(0)],
        brace_list![Brace::num(0)],
        Brace::str(""),
        Brace::num(0),
        Brace::num(0),
    ]))
}
