//! `DBSchema`: the table structure of the infobase, and the SQL it stands for.
//!
//! `DBSchema.SerializedData` (equal to `SchemaStorage.CurrentSchema` of `SchemaID 0`) is brace text
//! `{0,{<count>,<table>,...}}`; `SchemaStorage.NewGenCreated` has the same shape. The platform creates
//! the tables and indexes from it, so the text is enough to regenerate them.
//!
//! ```text
//! table    = {"<Name>","N",<num>,"",<fields>,<subtables>,<indexes>,1,"R"|"S",<sepA>,<sepB>,"",0,0}
//! subtable = {"VT<n>","I",0,"<Owner>",<fields>,{0},<indexes>,1,"S",{0},{0},"",0,0}
//! fields   = {<n>,<field>...}       field = {"<name>",<nullable>,{<n>,<type>...},"",0}
//! type     = {"B",len,0,"",0}       binary(len); bit 31 of len set: varbinary(len & 0x7fffffff | max)
//!            {"L",0,0,"",0}         boolean, binary(1)
//!            {"N",p,s,"",0[,1|2]}   numeric(p, s); sixth element 1 and s = 0: int / bigint; 2: identity
//!            {"T",0,0,"",0}         datetime2(0) (dates are stored +2000 years)
//!            {"S",len,0,"",0}       nvarchar(len & 0x7fffffff) if bit 31 is set, else nchar(len); 0: max
//!            {"R",0,0,"<Table>"|"",k}   a reference: typed -> RRef binary(16); untyped -> TRef binary(4) + RRef
//!            {"V",0,0,"",0}         timestamp (row version)
//!            {"E",0,0,"",0}         the discriminator of a composite type, binary(1)
//! index    = {"<name>",<unique>,{<n>,"<field>"...},<clustered>,<flag>,0,{0},0,0}
//! sepA/sepB = {0} | {1,{{<n>,"<field>"...}}}   data-separator columns of the table / of its sub-table indexes
//! ```
//!
//! Tables are read through views over the [`Brace`] tree, so a text the model does not fully understand
//! still round-trips byte for byte; the few edits (a field inserted, a table moved) are tree surgery.

use anyhow::{Context, Result, anyhow, bail};

use crate::brace::{Brace, parse_row, serialize_row};
use crate::brace_list;

// ---------------------------------------------------------------------------
// The whole text.

/// A parsed `DBSchema` (or `NewGenCreated`): the table entries in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbSchema {
    tables: Vec<Brace>,
}

impl DbSchema {
    pub fn empty() -> Self {
        Self { tables: Vec::new() }
    }

    pub fn from_tables(tables: Vec<Brace>) -> Self {
        Self { tables }
    }

    /// Parses the text (BOM optional): `{0,{<count>,<table>...}}`.
    pub fn parse(text: &[u8]) -> Result<Self> {
        let root = parse_row(text).context("DBSchema is not brace text")?;
        let outer = root.as_list().context("DBSchema is not a list")?;
        let [zero, body] = outer else {
            bail!("DBSchema has {} elements, expected 2", outer.len());
        };
        if zero.as_atom() != Some("0") {
            bail!("DBSchema does not start with 0");
        }
        let body = body.as_list().context("DBSchema has no table list")?;
        let Some((count, tables)) = body.split_first() else {
            bail!("DBSchema table list is empty");
        };
        let count: usize = count
            .as_atom()
            .and_then(|count| count.parse().ok())
            .context("DBSchema table count is not a number")?;
        if count != tables.len() {
            bail!("DBSchema counts {count} tables, holds {}", tables.len());
        }
        Ok(Self {
            tables: tables.to_vec(),
        })
    }

    /// The text with its BOM, as the platform lays it out.
    pub fn to_text(&self) -> Vec<u8> {
        let mut body = vec![Brace::atom(self.tables.len())];
        body.extend(self.tables.iter().cloned());
        serialize_row(&brace_list![Brace::num(0), Brace::List(body)])
    }

    pub fn tables(&self) -> &[Brace] {
        &self.tables
    }

    pub fn len(&self) -> usize {
        self.tables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// The position of a top-level table by name (`Reference20`).
    pub fn position(&self, name: &str) -> Option<usize> {
        self.tables
            .iter()
            .position(|table| table_name(table) == Some(name))
    }

    pub fn view(&self, position: usize) -> Result<TableView<'_>> {
        TableView::new(
            self.tables
                .get(position)
                .ok_or_else(|| anyhow!("no table at {position}"))?,
        )
    }

    pub fn named(&self, name: &str) -> Result<TableView<'_>> {
        let position = self
            .position(name)
            .ok_or_else(|| anyhow!("DBSchema has no table {name}"))?;
        self.view(position)
    }

    /// Removes a table and returns its entry.
    pub fn remove(&mut self, name: &str) -> Result<Brace> {
        let position = self
            .position(name)
            .ok_or_else(|| anyhow!("DBSchema has no table {name}"))?;
        Ok(self.tables.remove(position))
    }

    /// Inserts an entry before the table `anchor`, or at the end when there is
    /// no such table.
    pub fn insert_before(&mut self, anchor: &str, entry: Brace) {
        match self.position(anchor) {
            Some(position) => self.tables.insert(position, entry),
            None => self.tables.push(entry),
        }
    }

    /// Puts a table the platform makes or rebuilds where the platform puts it: at the **end** of the list, in the order it
    /// makes them, ahead of `ConfigChngR` when that is the last table. On the БСП the change register is the last table
    /// because the platform rebuilds it in every apply, after the others; on the ERP УХ, where the register is empty, the
    /// platform does not rebuild it, it stays in the middle of the list and the rebuilt tables go to the very end (traced by
    /// track rcheck on the УХ: `b1` positions 16 851-16 856 of 16 857, `c1` the same). Created tables come before the rebuilt
    /// ones (S1-F); both use this place, so the three placements compose.
    pub fn insert_rebuilt(&mut self, entry: Brace) {
        let last = self.tables.len().checked_sub(1);
        match last {
            Some(last) if table_name(&self.tables[last]) == Some("ConfigChngR") => {
                self.tables.insert(last, entry)
            }
            _ => self.tables.push(entry),
        }
    }
}

fn table_name(table: &Brace) -> Option<&str> {
    table.as_list()?.first()?.as_str()
}

// ---------------------------------------------------------------------------
// Entries.

/// One type entry `{tag,a,b,ref,k[,six]}` of a field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeEntry {
    pub tag: String,
    pub a: u64,
    pub b: u64,
    pub reference: String,
    pub k: i64,
    /// The optional sixth element: 1 an integer, 2 an identity integer.
    pub six: Option<i64>,
}

impl TypeEntry {
    pub fn new(tag: &str, a: u64, b: u64, reference: &str, k: i64) -> Self {
        Self {
            tag: tag.to_owned(),
            a,
            b,
            reference: reference.to_owned(),
            k,
            six: None,
        }
    }

    fn parse(node: &Brace) -> Result<Self> {
        let fields = node.as_list().context("a type entry is not a list")?;
        if fields.len() < 5 {
            bail!(
                "a type entry has {} elements, expected 5 or 6",
                fields.len()
            );
        }
        Ok(Self {
            tag: fields[0]
                .as_str()
                .context("a type entry has no tag")?
                .to_owned(),
            a: unsigned(&fields[1], "a type entry length")?,
            b: unsigned(&fields[2], "a type entry scale")?,
            reference: fields[3]
                .as_str()
                .context("a type entry has no reference")?
                .to_owned(),
            k: signed(&fields[4], "a type entry kind")?,
            six: match fields.get(5) {
                Some(six) => Some(signed(six, "a type entry flag")?),
                None => None,
            },
        })
    }

    pub fn to_brace(&self) -> Brace {
        let mut items = vec![
            Brace::str(&self.tag),
            Brace::atom(self.a),
            Brace::atom(self.b),
            Brace::str(&self.reference),
            Brace::num(self.k),
        ];
        if let Some(six) = self.six {
            items.push(Brace::num(six));
        }
        Brace::List(items)
    }
}

/// A field: one or more type entries (several = a composite type).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldEntry {
    pub name: String,
    pub nullable: bool,
    pub types: Vec<TypeEntry>,
}

impl FieldEntry {
    pub fn new(name: &str, nullable: bool, types: Vec<TypeEntry>) -> Self {
        Self {
            name: name.to_owned(),
            nullable,
            types,
        }
    }

    /// `{"<name>",<nullable>,{<n>,<type>...},"",0}`
    pub fn to_brace(&self) -> Brace {
        let mut types = vec![Brace::atom(self.types.len())];
        types.extend(self.types.iter().map(TypeEntry::to_brace));
        brace_list![
            Brace::str(&self.name),
            Brace::flag(self.nullable),
            Brace::List(types),
            Brace::str(""),
            Brace::num(0)
        ]
    }

    fn parse(node: &Brace) -> Result<Self> {
        let fields = node.as_list().context("a field is not a list")?;
        if fields.len() != 5 {
            bail!("a field has {} elements, expected 5", fields.len());
        }
        let name = fields[0].as_str().context("a field has no name")?;
        let types = fields[2]
            .as_list()
            .with_context(|| format!("field {name} has no type list"))?;
        let Some((count, entries)) = types.split_first() else {
            bail!("field {name} has an empty type list");
        };
        if unsigned(count, "a type count")? != entries.len() as u64 {
            bail!("field {name} counts its types wrong");
        }
        Ok(Self {
            name: name.to_owned(),
            nullable: unsigned(&fields[1], "a nullable flag")? == 1,
            types: entries
                .iter()
                .map(TypeEntry::parse)
                .collect::<Result<_>>()
                .with_context(|| format!("field {name}"))?,
        })
    }
}

/// A declared index `{"<name>",<unique>,{<n>,"<field>"...},<clustered>,...}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub name: String,
    pub unique: bool,
    pub fields: Vec<String>,
    pub clustered: bool,
}

impl IndexEntry {
    /// Native declaration for a generated index with the default index options.
    pub fn to_brace(&self) -> Brace {
        let mut keys = vec![Brace::atom(self.fields.len())];
        keys.extend(self.fields.iter().map(Brace::str));
        brace_list![
            Brace::str(&self.name),
            Brace::flag(self.unique),
            Brace::List(keys),
            Brace::flag(self.clustered),
            Brace::num(0),
            Brace::num(0),
            brace_list![Brace::num(0)],
            Brace::num(0),
            Brace::num(0)
        ]
    }

    fn parse(node: &Brace) -> Result<Self> {
        let items = node.as_list().context("an index is not a list")?;
        if items.len() != 9 {
            bail!("an index has {} elements, expected 9", items.len());
        }
        let name = items[0].as_str().context("an index has no name")?;
        let names = items[2]
            .as_list()
            .with_context(|| format!("index {name} has no field list"))?;
        let Some((count, fields)) = names.split_first() else {
            bail!("index {name} has an empty field list");
        };
        if unsigned(count, "an index field count")? != fields.len() as u64 {
            bail!("index {name} counts its fields wrong");
        }
        Ok(Self {
            name: name.to_owned(),
            unique: unsigned(&items[1], "an index unique flag")? == 1,
            fields: fields
                .iter()
                .map(|field| {
                    field
                        .as_str()
                        .map(str::to_owned)
                        .with_context(|| format!("index {name} names a field that is not a string"))
                })
                .collect::<Result<_>>()?,
            clustered: unsigned(&items[3], "an index clustered flag")? == 1,
        })
    }
}

// ---------------------------------------------------------------------------
// A view of one table entry.

/// A read-only view of a table or sub-table entry (14 elements).
#[derive(Clone, Copy, Debug)]
pub struct TableView<'a> {
    items: &'a [Brace],
}

impl<'a> TableView<'a> {
    pub fn new(node: &'a Brace) -> Result<Self> {
        let items = node.as_list().context("a table entry is not a list")?;
        if items.len() != 14 {
            bail!("a table entry has {} elements, expected 14", items.len());
        }
        if items[0].as_str().is_none() || items[4].as_list().is_none() {
            bail!("a table entry has no name or no fields");
        }
        Ok(Self { items })
    }

    /// `Reference20`, or `VT155` for a sub-table.
    pub fn name(&self) -> &'a str {
        self.items[0].as_str().unwrap_or_default()
    }

    /// `N` for an object's table, `I` for a sub-table.
    pub fn kind(&self) -> &'a str {
        self.items[1].as_str().unwrap_or_default()
    }

    /// The owner's name for a sub-table.
    pub fn owner_name(&self) -> &'a str {
        self.items[3].as_str().unwrap_or_default()
    }

    pub fn is_subtable(&self) -> bool {
        self.kind() == "I"
    }

    pub fn fields(&self) -> Result<Vec<FieldEntry>> {
        let list = self.items[4].as_list().unwrap_or_default();
        let Some((count, fields)) = list.split_first() else {
            bail!("table {} has an empty field list", self.name());
        };
        if unsigned(count, "a field count")? != fields.len() as u64 {
            bail!("table {} counts its fields wrong", self.name());
        }
        fields
            .iter()
            .map(FieldEntry::parse)
            .collect::<Result<_>>()
            .with_context(|| format!("table {}", self.name()))
    }

    pub fn subtables(&self) -> Result<Vec<TableView<'a>>> {
        let list = self.items[5]
            .as_list()
            .with_context(|| format!("table {} has no sub-table list", self.name()))?;
        let Some((count, tables)) = list.split_first() else {
            bail!("table {} has an empty sub-table list", self.name());
        };
        if unsigned(count, "a sub-table count")? != tables.len() as u64 {
            bail!("table {} counts its sub-tables wrong", self.name());
        }
        tables.iter().map(TableView::new).collect()
    }

    pub fn indexes(&self) -> Result<Vec<IndexEntry>> {
        let list = self.items[6]
            .as_list()
            .with_context(|| format!("table {} has no index list", self.name()))?;
        let Some((count, indexes)) = list.split_first() else {
            bail!("table {} has an empty index list", self.name());
        };
        if unsigned(count, "an index count")? != indexes.len() as u64 {
            bail!("table {} counts its indexes wrong", self.name());
        }
        indexes
            .iter()
            .map(IndexEntry::parse)
            .collect::<Result<_>>()
            .with_context(|| format!("table {}", self.name()))
    }

    /// The fields named by the table's data-separator list (element 9).
    pub fn separators_a(&self) -> Result<Vec<String>> {
        separator_fields(&self.items[9])
    }

    /// The fields named by the separator list of the sub-table indexes (element 10).
    pub fn separators_b(&self) -> Result<Vec<String>> {
        separator_fields(&self.items[10])
    }
}

/// `{0}` -> nothing; `{1,{{<n>,"<field>"...}}}` -> the names.
fn separator_fields(node: &Brace) -> Result<Vec<String>> {
    let items = node.as_list().context("a separator list is not a list")?;
    match items {
        [zero] if zero.as_atom() == Some("0") => Ok(Vec::new()),
        [one, groups] if one.as_atom() == Some("1") => {
            let mut names = Vec::new();
            for group in groups
                .as_list()
                .context("separator groups are not a list")?
            {
                let group = group.as_list().context("a separator group is not a list")?;
                let Some((count, group)) = group.split_first() else {
                    bail!("a separator group is empty");
                };
                if unsigned(count, "a separator count")? != group.len() as u64 {
                    bail!("a separator group counts its names wrong");
                }
                for name in group {
                    names.push(
                        name.as_str()
                            .context("a separator names a field that is not a string")?
                            .to_owned(),
                    );
                }
            }
            Ok(names)
        }
        _ => bail!("unknown separator list shape"),
    }
}

// ---------------------------------------------------------------------------
// Edits.

/// Inserts a field into a table entry at `at` (0 = first) and fixes the count.
pub fn insert_field(table: &mut Brace, at: usize, field: &FieldEntry) -> Result<()> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let fields = items
        .get_mut(4)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no field list")?;
    if at + 1 > fields.len() {
        bail!("cannot insert a field at {at} of {}", fields.len() - 1);
    }
    fields.insert(at + 1, field.to_brace());
    fields[0] = Brace::atom(fields.len() - 1);
    Ok(())
}

/// Replaces the field of the same name in a table entry (its type entries changed; the place stays).
pub fn replace_field(table: &mut Brace, field: &FieldEntry) -> Result<()> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let fields = items
        .get_mut(4)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no field list")?;
    let position = fields
        .iter()
        .skip(1)
        .position(|node| FieldEntry::parse(node).is_ok_and(|found| found.name == field.name))
        .with_context(|| format!("the table has no field {}", field.name))?
        + 1;
    fields[position] = field.to_brace();
    Ok(())
}

/// Removes the field `name` from a table entry, fixes the count and returns the field.
pub fn remove_field(table: &mut Brace, name: &str) -> Result<FieldEntry> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let fields = items
        .get_mut(4)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no field list")?;
    let position = fields
        .iter()
        .skip(1)
        .position(|node| FieldEntry::parse(node).is_ok_and(|field| field.name == name))
        .with_context(|| format!("the table has no field {name}"))?
        + 1;
    let field = FieldEntry::parse(&fields.remove(position))?;
    fields[0] = Brace::atom(fields.len() - 1);
    Ok(field)
}

/// Removes the declared indexes that name the field `field` from a table entry, fixes the count and
/// returns their names. The indexes the platform makes for an indexed attribute go
/// (`ByFieldFld12`, `ByOwnerFieldFld12`, `ByParentFieldFld12`, `ByField5561`); the date index of a document
/// with an additional-order attribute (`ByDocDate`, which lists the attribute last) stays and loses the field
/// from its list (case b2; the name is reported as `ByDocDate (- Fld12)`); an index of another kind that names
/// the field is refused.
pub fn remove_field_indexes(table: &mut Brace, field: &str) -> Result<Vec<String>> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let indexes = items
        .get_mut(6)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no index list")?;
    let mut removed = Vec::new();
    let mut position = 1;
    while position < indexes.len() {
        let index = IndexEntry::parse(&indexes[position])?;
        if index.fields.iter().any(|name| name == field) {
            if index.name == "ByDocDate" && index.fields.last().is_some_and(|last| last == field) {
                let names = indexes[position]
                    .as_list_mut()
                    .and_then(|entry| entry.get_mut(2))
                    .and_then(Brace::as_list_mut)
                    .context("the date index has no field list")?;
                names.pop();
                names[0] = Brace::atom(names.len() - 1);
                removed.push(format!("ByDocDate (- {field})"));
                position += 1;
                continue;
            }
            if !index.name.starts_with("By") || !index.name.contains("Field") {
                bail!(
                    "index {} names the field {field}: it is not one of the indexes of an indexed attribute",
                    index.name
                );
            }
            removed.push(index.name);
            indexes.remove(position);
        } else {
            position += 1;
        }
    }
    indexes[0] = Brace::atom(indexes.len() - 1);
    Ok(removed)
}

/// A declared index `{"<name>",<unique>,{<n>,"<field>"...},0,0,0,{0},0,0}`.
fn index_brace(name: &str, unique: bool, fields: &[&str]) -> Brace {
    let mut names = vec![Brace::atom(fields.len())];
    names.extend(fields.iter().map(|field| Brace::str(*field)));
    Brace::List(vec![
        Brace::str(name),
        Brace::num(i64::from(unique)),
        Brace::List(names),
        Brace::num(0),
        Brace::num(0),
        Brace::num(0),
        Brace::List(vec![Brace::num(0)]),
        Brace::num(0),
        Brace::num(0),
    ])
}

/// Adds the declared indexes the platform makes for an attribute that gets `Indexing = Index` (or
/// `IndexWithAdditionalOrder` with `additional_order`), fixes the count and returns their names. Traced on the
/// platform (cases d0 and d1):
///
/// * a catalog: `ByFieldFld<n> {2,"Fld<n>","ID"}`; a hierarchical one first gets `ByParentFieldFld<n>
///   {4,"ParentID","Folder","Fld<n>","ID"}` (the prefix is the one of its own `ParentDescr`);
/// * a document: `ByFieldFld<n> {2,"Fld<n>","ID"}`;
/// * with the additional order the key goes on with the order of the object: `Description,ID,Marked` for a
///   catalog (`{6,"ParentID","Folder","Fld<n>","Description","ID","Marked"}` in the parent index), `Date_Time,ID,Marked`
///   for a document, whose date index `ByDocDate` also lists the field last.
///
/// The entries sit among the attribute indexes in the order of their attributes' fields in the table (not by
/// number, not by name); the first attribute index of a table goes after the standard ones. A table that
/// shows anything else is refused: a subordinate catalog, several additional-order attributes in a document,
/// a hierarchical catalog with no `ParentDescr`.
pub fn add_field_indexes(
    table: &mut Brace,
    field: &str,
    additional_order: bool,
) -> Result<Vec<String>> {
    let view = TableView::new(table)?;
    let fields: Vec<String> = view.fields()?.into_iter().map(|entry| entry.name).collect();
    let position_of = |name: &str| fields.iter().position(|candidate| candidate == name);
    let has = |name: &str| position_of(name).is_some();
    let own = position_of(field).with_context(|| format!("the table has no field {field}"))?;
    let existing = view.indexes()?;
    if let Some(index) = existing
        .iter()
        .find(|index| index.fields.iter().any(|name| name == field))
    {
        bail!("index {} already names the field {field}", index.name);
    }
    if has("OwnerID") {
        bail!("an index of an attribute of a subordinate catalog is not traced");
    }
    let document = has("Date_Time");
    let hierarchical = has("ParentID");
    if document && hierarchical {
        bail!("a table with a date and a parent is neither a catalog nor a document");
    }
    let tail: &[&str] = match (document, additional_order) {
        (_, false) => &["ID"],
        (false, true) if has("Description") => &["Description", "ID", "Marked"],
        (false, true) => {
            bail!("an additional order by a catalog with no description is not traced")
        }
        (true, true) => &["Date_Time", "ID", "Marked"],
    };
    let plain = format!("ByField{field}");
    let mut fresh: Vec<(String, Vec<&str>)> = Vec::new();
    if hierarchical {
        let parent = existing
            .iter()
            .find(|index| index.name == "ParentDescr")
            .context(
                "a hierarchical table has no ParentDescr index to take the parent prefix from",
            )?;
        let prefix: Vec<&str> = parent
            .fields
            .iter()
            .take_while(|name| name.as_str() != "Description")
            .map(String::as_str)
            .collect();
        if prefix.is_empty() || prefix.len() == parent.fields.len() {
            bail!("the ParentDescr index has no parent prefix");
        }
        let mut keys = prefix;
        keys.push(field);
        keys.extend(tail);
        fresh.push((format!("ByParentField{field}"), keys));
    }
    let mut keys = vec![field];
    keys.extend(tail);
    fresh.push((plain, keys));

    // Where they go: before the first attribute index whose field sits later in the table.
    let mut before = None;
    let mut last_attribute_index = None;
    for (at, index) in existing.iter().enumerate() {
        if !index.name.starts_with("By") || !index.name.contains("Field") {
            continue;
        }
        last_attribute_index = Some(at);
        let owner = index
            .fields
            .iter()
            .filter(|name| name.starts_with("Fld"))
            .find_map(|name| position_of(name))
            .with_context(|| format!("index {} names no attribute field", index.name))?;
        if owner > own && before.is_none() {
            before = Some(at);
        }
    }
    let at = before
        .or(last_attribute_index.map(|last| last + 1))
        .unwrap_or(existing.len());

    let items = table.as_list_mut().context("a table entry is not a list")?;
    let indexes = items
        .get_mut(6)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no index list")?;
    let mut added = Vec::new();
    for (offset, (name, keys)) in fresh.iter().enumerate() {
        indexes.insert(1 + at + offset, index_brace(name, true, keys));
        added.push(name.clone());
    }
    if document && additional_order {
        // The date index lists the additional-order attribute last.
        let date = indexes
            .iter_mut()
            .skip(1)
            .find(|index| {
                index
                    .as_list()
                    .and_then(|items| items.first())
                    .and_then(Brace::as_str)
                    == Some("ByDocDate")
            })
            .context("a document has no ByDocDate index")?;
        let names = date
            .as_list_mut()
            .and_then(|entry| entry.get_mut(2))
            .and_then(Brace::as_list_mut)
            .context("the date index has no field list")?;
        if names.len() != 4 {
            bail!(
                "the date index already lists an additional-order attribute: several are not traced"
            );
        }
        names.push(Brace::str(field));
        names[0] = Brace::atom(names.len() - 1);
        added.push(format!("ByDocDate (+ {field})"));
    }
    indexes[0] = Brace::atom(indexes.len() - 1);
    Ok(added)
}

/// The entry of a sub-table (the table of a tabular section) that does not exist yet, in the layout the platform
/// stores: `{"VT<n>","I",0,"<Owner>",{<k+1>,<the line number field>,<fields>...},{0},{<indexes>},1,"S",{0},{0},"",0,0}`.
/// `line_no` is the name of the line number field (`LineNo<n+1>`, a `numeric(5,0)`: the section's
/// `LineNumberLength`), `fields` are the attributes' fields, `indexes` the declared indexes of the indexed
/// attributes as (name, fields), which the platform makes not unique (`ByFieldFld12 {2,"Fld12","ID"}`).
pub fn subtable_entry(
    name: &str,
    owner: &str,
    line_no: &str,
    fields: &[FieldEntry],
    indexes: &[(String, Vec<String>)],
) -> Brace {
    let line = FieldEntry::new(line_no, false, vec![TypeEntry::new("N", 5, 0, "", 0)]);
    let mut list = vec![Brace::atom(fields.len() + 1), line.to_brace()];
    list.extend(fields.iter().map(FieldEntry::to_brace));
    let mut declared = vec![Brace::atom(indexes.len())];
    declared.extend(indexes.iter().map(|(index, keys)| {
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        index_brace(index, false, &keys)
    }));
    Brace::List(vec![
        Brace::str(name),
        Brace::str("I"),
        Brace::num(0),
        Brace::str(owner),
        Brace::List(list),
        Brace::List(vec![Brace::num(0)]),
        Brace::List(declared),
        Brace::num(1),
        Brace::str("S"),
        Brace::List(vec![Brace::num(0)]),
        Brace::List(vec![Brace::num(0)]),
        Brace::str(""),
        Brace::num(0),
        Brace::num(0),
    ])
}

/// The entry of the sub-table `name` in the sub-table list of an object's table entry, to edit in place
/// (`insert_field` and `add_subtable_field_index` work on it as on a table entry).
pub fn subtable_mut<'a>(table: &'a mut Brace, name: &str) -> Result<&'a mut Brace> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let list = items
        .get_mut(5)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no sub-table list")?;
    list.iter_mut()
        .skip(1)
        .find(|sub| {
            sub.as_list()
                .and_then(|items| items.first())
                .and_then(Brace::as_str)
                == Some(name)
        })
        .with_context(|| format!("the table has no sub-table {name}"))
}

/// Adds the declared index the platform makes for an indexed attribute of a tabular section, fixes the count and
/// returns its name: `ByField<field> {2,"<field>","ID"}`, not unique whatever the object is (measured on the
/// 72 sub-tables of the БСП that have indexes: every one is `ByField*`, never unique, keyed by the field and the
/// owner's `ID`; traced on catalogs, hierarchical catalogs and documents, cases e1 and e3). The entries sit in
/// the order of their fields in the sub-table. A sub-table that has an index of another kind is refused.
pub fn add_subtable_field_index(table: &mut Brace, field: &str) -> Result<String> {
    let view = TableView::new(table)?;
    if !view.is_subtable() {
        bail!("{} is not a sub-table", view.name());
    }
    let fields: Vec<String> = view.fields()?.into_iter().map(|entry| entry.name).collect();
    let position_of = |name: &str| fields.iter().position(|candidate| candidate == name);
    let own = position_of(field).with_context(|| format!("the sub-table has no field {field}"))?;
    let existing = view.indexes()?;
    let mut at = existing.len();
    for (position, index) in existing.iter().enumerate() {
        if index.fields.iter().any(|name| name == field) {
            bail!("index {} already names the field {field}", index.name);
        }
        let plain = index.name.starts_with("ByFieldFld")
            && index.fields.len() == 2
            && index.fields[1] == "ID";
        if !plain {
            bail!(
                "the sub-table {} has the index {}, which is not the index of an attribute: not traced",
                view.name(),
                index.name
            );
        }
        let indexed = position_of(&index.fields[0])
            .with_context(|| format!("index {} names no field of the sub-table", index.name))?;
        if indexed > own && at == existing.len() {
            at = position;
        }
    }
    let name = format!("ByField{field}");
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let indexes = items
        .get_mut(6)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no index list")?;
    indexes.insert(1 + at, index_brace(&name, false, &[field, "ID"]));
    indexes[0] = Brace::atom(indexes.len() - 1);
    Ok(name)
}

/// Appends a sub-table entry to the sub-table list of an object's table entry (element 5) and fixes the count.
pub fn push_subtable(table: &mut Brace, subtable: Brace) -> Result<()> {
    let items = table.as_list_mut().context("a table entry is not a list")?;
    let list = items
        .get_mut(5)
        .and_then(Brace::as_list_mut)
        .context("a table entry has no sub-table list")?;
    list.push(subtable);
    list[0] = Brace::atom(list.len() - 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// The SQL a table stands for.

/// A SQL Server column type of the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqlType {
    Binary(u32),
    VarBinary(u32),
    VarBinaryMax,
    Numeric(u32, u32),
    Int,
    BigInt,
    DateTime2,
    NVarChar(u32),
    NVarCharMax,
    NChar(u32),
    Timestamp,
}

impl SqlType {
    /// As the platform's DDL spells it (`numeric(7, 0)`).
    pub fn ddl(&self) -> String {
        match self {
            Self::Binary(n) => format!("binary({n})"),
            Self::VarBinary(n) => format!("varbinary({n})"),
            Self::VarBinaryMax => "varbinary(max)".to_owned(),
            Self::Numeric(p, s) => format!("numeric({p}, {s})"),
            Self::Int => "int".to_owned(),
            Self::BigInt => "bigint".to_owned(),
            Self::DateTime2 => "datetime2(0)".to_owned(),
            Self::NVarChar(n) => format!("nvarchar({n})"),
            Self::NVarCharMax => "nvarchar(max)".to_owned(),
            Self::NChar(n) => format!("nchar({n})"),
            Self::Timestamp => "timestamp".to_owned(),
        }
    }
}

/// A column of a physical table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub sql_type: SqlType,
    pub nullable: bool,
    /// An identity column (the type entry's sixth element is 2): a table with
    /// one is not generated here, its values would have to be carried over.
    pub identity: bool,
}

impl Column {
    fn plain(name: String, sql_type: SqlType, nullable: bool) -> Self {
        Self {
            name,
            sql_type,
            nullable,
            identity: false,
        }
    }

    /// `_Fld1 nvarchar(50) not null` (a nullable column has no suffix).
    pub fn ddl(&self) -> String {
        format!(
            "{} {}{}",
            self.name,
            self.sql_type.ddl(),
            if self.nullable { "" } else { " not null" }
        )
    }
}

/// An index of a physical table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexDef {
    /// The SQL name; empty for the inline primary key, which SQL Server names.
    pub name: String,
    pub unique: bool,
    pub clustered: bool,
    pub columns: Vec<String>,
}

/// One physical table with its indexes in the platform's creation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalTable {
    /// `_Reference20`, `_Reference20_VT155`.
    pub name: String,
    pub columns: Vec<Column>,
    /// The column of an inline `primary key` (a table without data separation).
    pub inline_pk: Option<String>,
    pub indexes: Vec<IndexDef>,
    /// How many leading columns are the sub-table's implicit ones (`_<Owner>_IDRRef`, the owner's
    /// separator columns, `_KeyField`); 0 for an object's main table.
    pub implicit_prefix: usize,
}

pub fn sql_type(entry: &TypeEntry) -> Result<SqlType> {
    let a = entry.a;
    Ok(match entry.tag.as_str() {
        "B" => {
            if a & 0x8000_0000 != 0 {
                match (a & 0x7fff_ffff) as u32 {
                    0 => SqlType::VarBinaryMax,
                    n => SqlType::VarBinary(n),
                }
            } else {
                SqlType::Binary(a as u32)
            }
        }
        "L" => SqlType::Binary(1),
        "N" => match entry.six {
            Some(1 | 2) if entry.b == 0 => {
                if a <= 10 {
                    SqlType::Int
                } else {
                    SqlType::BigInt
                }
            }
            _ => SqlType::Numeric(a as u32, entry.b as u32),
        },
        "T" => SqlType::DateTime2,
        "S" => {
            let length = (a & 0x7fff_ffff) as u32;
            if a & 0x8000_0000 != 0 {
                if length == 0 {
                    SqlType::NVarCharMax
                } else {
                    SqlType::NVarChar(length)
                }
            } else if length == 0 {
                SqlType::NVarCharMax
            } else {
                SqlType::NChar(length)
            }
        }
        "V" => SqlType::Timestamp,
        other => bail!("unknown type tag {other:?}"),
    })
}

/// The columns of one field (`_Fld1RRef`, `_Fld2_TYPE`, `_Fld2_S`, ...).
pub fn field_columns(field: &FieldEntry) -> Result<Vec<Column>> {
    let base = format!("_{}", field.name);
    let multi = field.types.len() > 1;
    let mut columns = Vec::new();
    for entry in &field.types {
        let column = |name: String, sql_type: SqlType, identity: bool| Column {
            name,
            sql_type,
            nullable: field.nullable,
            identity,
        };
        match entry.tag.as_str() {
            tag @ ("B" | "L" | "N" | "T" | "S" | "V") => {
                let suffix = if multi {
                    match tag {
                        "B" => "_B",
                        "L" => "_L",
                        "N" => "_N",
                        "T" => "_T",
                        "S" => "_S",
                        _ => "",
                    }
                } else {
                    ""
                };
                columns.push(column(
                    format!("{base}{suffix}"),
                    sql_type(entry).with_context(|| field.name.clone())?,
                    tag == "N" && entry.six == Some(2),
                ));
            }
            "E" => columns.push(column(format!("{base}_TYPE"), SqlType::Binary(1), false)),
            "R" => {
                if entry.reference.is_empty() {
                    columns.push(column(
                        format!("{base}{}", if multi { "_RTRef" } else { "TRef" }),
                        SqlType::Binary(4),
                        false,
                    ));
                }
                columns.push(column(
                    format!("{base}{}", if multi { "_RRRef" } else { "RRef" }),
                    SqlType::Binary(16),
                    false,
                ));
            }
            other => bail!("unknown type tag {other:?} in field {}", field.name),
        }
    }
    Ok(columns)
}

/// The columns a field list stands for, plus the sub-table extras when the
/// table has an owner: `_<Owner>_IDRRef`, the owner's separator fields,
/// `_KeyField`.
pub fn table_columns(table: &TableView<'_>, owner: Option<&TableView<'_>>) -> Result<Vec<Column>> {
    let mut columns = Vec::new();
    if let Some(owner) = owner {
        columns.push(Column::plain(
            format!("_{}_IDRRef", owner.name()),
            SqlType::Binary(16),
            false,
        ));
        columns.extend(separator_columns(owner, &owner.separators_b()?)?);
        columns.push(Column::plain(
            "_KeyField".to_owned(),
            SqlType::Binary(4),
            false,
        ));
    }
    for field in table.fields()? {
        columns.extend(field_columns(&field)?);
    }
    Ok(columns)
}

fn separator_columns(owner: &TableView<'_>, names: &[String]) -> Result<Vec<Column>> {
    let fields = owner.fields()?;
    let mut columns = Vec::new();
    for name in names {
        let field = fields
            .iter()
            .find(|field| &field.name == name)
            .with_context(|| {
                format!("separator field {name} is not a field of {}", owner.name())
            })?;
        columns.extend(field_columns(field)?);
    }
    Ok(columns)
}

fn field_column_names(fields: &[FieldEntry], name: &str) -> Result<Vec<String>> {
    let field = fields
        .iter()
        .find(|field| field.name == name)
        .with_context(|| format!("index names the unknown field {name}"))?;
    Ok(field_columns(field)?
        .into_iter()
        .map(|column| column.name)
        .collect())
}

/// SQL Server allows 16 key columns: a longer index is cut and is no longer unique.
fn cut_index(name: String, unique: bool, clustered: bool, mut columns: Vec<String>) -> IndexDef {
    let unique = if columns.len() > 16 {
        columns.truncate(16);
        false
    } else {
        unique
    };
    IndexDef {
        name,
        unique,
        clustered,
        columns,
    }
}

/// The declared indexes of a table (`_<Table>_<i>`), separator columns in front;
/// a sub-table's own `_SK` key is not among them.
pub fn declared_indexes(
    table: &TableView<'_>,
    owner: Option<&TableView<'_>>,
) -> Result<Vec<IndexDef>> {
    let fields = table.fields()?;
    let mut out = Vec::new();
    match owner {
        None => {
            let sql_name = format!("_{}", table.name());
            let mut prefix = Vec::new();
            for name in table.separators_a()? {
                prefix.extend(field_column_names(&fields, &name)?);
            }
            for (position, index) in table.indexes()?.iter().enumerate() {
                let mut columns = prefix.clone();
                for name in &index.fields {
                    columns.extend(field_column_names(&fields, name)?);
                }
                out.push(cut_index(
                    format!("{sql_name}_{}", position + 1),
                    index.unique,
                    index.clustered,
                    columns,
                ));
            }
        }
        Some(owner) => {
            let sql_name = format!("_{}_{}", owner.name(), table.name());
            let owner_fields = owner.fields()?;
            let mut prefix = Vec::new();
            for name in owner.separators_b()? {
                prefix.extend(field_column_names(&owner_fields, &name)?);
            }
            let owner_key = format!("_{}_IDRRef", owner.name());
            for (position, index) in table.indexes()?.iter().enumerate() {
                let mut columns = prefix.clone();
                for name in &index.fields {
                    if name == "ID" {
                        columns.push(owner_key.clone());
                    } else {
                        columns.extend(field_column_names(&fields, name)?);
                    }
                }
                out.push(cut_index(
                    format!("{sql_name}_{}", position + 1),
                    index.unique,
                    index.clustered,
                    columns,
                ));
            }
        }
    }
    Ok(out)
}

/// The indexes the platform adds beyond the declared ones. For a sub-table: the
/// clustered `_SK`. For an object's table whose `ID` refers to itself: with
/// data separation the clustered `_S_HPK` (and `_S_PK` on `_IDRRef` when the
/// sub-table separators are empty), without it the inline primary key.
pub fn implicit_indexes(
    table: &TableView<'_>,
    owner: Option<&TableView<'_>>,
) -> Result<Vec<IndexDef>> {
    match owner {
        Some(owner) => {
            let owner_fields = owner.fields()?;
            let mut columns = Vec::new();
            for name in owner.separators_b()? {
                columns.extend(field_column_names(&owner_fields, &name)?);
            }
            columns.push(format!("_{}_IDRRef", owner.name()));
            columns.push("_KeyField".to_owned());
            Ok(vec![IndexDef {
                name: format!("_{}_{}_SK", owner.name(), table.name()),
                unique: true,
                clustered: true,
                columns,
            }])
        }
        None => {
            let fields = table.fields()?;
            let Some(id) = fields.iter().find(|field| field.name == "ID") else {
                return Ok(Vec::new());
            };
            let self_reference = id.types.first().is_some_and(|entry| {
                entry.tag == "R" && entry.reference == table.name() && entry.k == 2
            });
            if !self_reference {
                return Ok(Vec::new());
            }
            let mut separators = Vec::new();
            for name in table.separators_a()? {
                separators.extend(field_column_names(&fields, &name)?);
            }
            if separators.is_empty() {
                return Ok(vec![IndexDef {
                    name: String::new(),
                    unique: true,
                    clustered: true,
                    columns: vec!["_IDRRef".to_owned()],
                }]);
            }
            let mut out = vec![IndexDef {
                name: format!("_{}_S_HPK", table.name()),
                unique: true,
                clustered: true,
                columns: separators
                    .iter()
                    .cloned()
                    .chain(std::iter::once("_IDRRef".to_owned()))
                    .collect(),
            }];
            if table.separators_b()?.is_empty() {
                out.push(IndexDef {
                    name: format!("_{}_S_PK", table.name()),
                    unique: true,
                    clustered: false,
                    columns: vec!["_IDRRef".to_owned()],
                });
            }
            Ok(out)
        }
    }
}

/// The physical tables of an object: its main table, then each sub-table, with
/// the indexes in the order the platform creates them: for the main table the
/// declared ones and then the implicit ones, for a sub-table the clustered
/// `_SK` first and then the declared ones (case a of the traces).
pub fn physical_tables(object: &TableView<'_>) -> Result<Vec<PhysicalTable>> {
    let mut out = Vec::new();
    let columns = table_columns(object, None)?;
    let implicit = implicit_indexes(object, None)?;
    let inline_pk = implicit
        .iter()
        .find(|index| index.name.is_empty())
        .map(|index| index.columns[0].clone());
    let mut indexes = declared_indexes(object, None)?;
    indexes.extend(implicit.into_iter().filter(|index| !index.name.is_empty()));
    out.push(PhysicalTable {
        name: format!("_{}", object.name()),
        columns,
        inline_pk,
        indexes,
        implicit_prefix: 0,
    });
    let owner_separators = separator_columns(object, &object.separators_b()?)?.len();
    for sub in object.subtables()? {
        let mut indexes = implicit_indexes(&sub, Some(object))?;
        indexes.extend(declared_indexes(&sub, Some(object))?);
        out.push(PhysicalTable {
            name: format!("_{}_{}", object.name(), sub.name()),
            columns: table_columns(&sub, Some(object))?,
            inline_pk: None,
            indexes,
            implicit_prefix: owner_separators + 2,
        });
    }
    Ok(out)
}

/// `create table dbo._X<suffix> (...)` as the platform writes it.
pub fn create_table_sql(table: &PhysicalTable, suffix: &str) -> String {
    let body = table
        .columns
        .iter()
        .map(|column| {
            let inline = if table.inline_pk.as_deref() == Some(column.name.as_str()) {
                " primary key"
            } else {
                ""
            };
            format!("{}{inline}", column.ddl())
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "create table dbo.{name}{suffix} (\n{body}\n)\n;alter table dbo.{name}{suffix} SET (LOCK_ESCALATION = DISABLE);",
        name = table.name
    )
}

/// `CREATE [UNIQUE] [CLUSTERED] INDEX <name><suffix> ON dbo.<table><suffix> (...) WITH (...)`.
pub fn create_index_sql(table: &str, index: &IndexDef, suffix: &str) -> String {
    format!(
        "CREATE {unique}{clustered}INDEX {name}{suffix} ON dbo.{table}{suffix} ({columns}) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON){storage};",
        unique = if index.unique { "UNIQUE " } else { "" },
        clustered = if index.clustered { "CLUSTERED " } else { "" },
        name = index.name,
        columns = index.columns.join(", "),
        storage = if index.clustered { "" } else { " ON [PRIMARY]" },
    )
}

fn unsigned(node: &Brace, what: &str) -> Result<u64> {
    node.as_atom()
        .and_then(|text| text.parse::<u64>().ok())
        .with_context(|| format!("{what} is not a number"))
}

fn signed(node: &Brace, what: &str) -> Result<i64> {
    node.as_atom()
        .and_then(|text| text.parse::<i64>().ok())
        .with_context(|| format!("{what} is not a number"))
}
