//! The common attributes of the configuration and the fields they add to the tables of catalogs and
//! documents (S1-F, `docs/apply/new-object.md` 1.2).
//!
//! A common attribute is a descriptor `{1,{5,<body>,<Content>,<Indexing>,<FullTextSearch>,<DataSeparation>,
//! <AutoUse>,{1,<DataSeparationValue>},{1,<DataSeparationUse>},{1,<ConditionalSeparation>},<UsersSeparation>,
//! <AuthenticationSeparation>,<SeparatedDataUse>,<ConfigurationExtensionsSeparation>,<DataHistory>},0}` with
//! `Content = {3,<n>,<object uuid>,{2,<use>,<condition>},...}` (`use`: 0 Auto, 1 Use, 2 DontUse),
//! `DataSeparation` 0 Separate / 1 DontUse, `AutoUse` 0 Use / 1 DontUse. It becomes a field of the table
//! of every catalog and document it applies to, after the attributes, in the configuration's order:
//!
//! - an object the `Content` lists with `Use` has it, with `DontUse` has not;
//! - an object it lists with `Auto`, or does not list, has it when `AutoUse` is `Use`.
//!
//! The `Fld<n>` of the field is the number `DBNames` holds for the common attribute's uuid; its type is the
//! common attribute's own type pattern. The two separation lists of the table entry name the fields of the
//! applying common attributes that separate data (`DataSeparation` = Separate), the second one only those
//! that are used `Independently` (`SeparatedDataUse` = 0): all the БСП needs (checked on its 139 entries).

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::restructure::catalog::type_entries;
use crate::restructure::schema::TypeEntry;

/// The root collection of the common attributes.
pub const COMMON_ATTRIBUTE_CLASS: &str = "15794563-ccec-41f6-a83c-ec5f7b9a5bc1";

/// How a common attribute's content lists an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentUse {
    Auto,
    Use,
    DontUse,
}

/// One common attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonAttribute {
    pub uuid: String,
    pub name: String,
    /// The type entries of its field.
    pub types: Vec<TypeEntry>,
    /// The objects the content lists.
    pub content: BTreeMap<String, ContentUse>,
    pub auto_use: bool,
    pub separates_data: bool,
    pub independent: bool,
    /// `Indexing`: 0 `DontIndex`, 1 `Index`, 2 `IndexWithAdditionalOrder`.
    pub indexing: i64,
}

impl CommonAttribute {
    /// Reads a descriptor row.
    pub fn parse(row: &Brace) -> Result<Self> {
        let root = row
            .as_list()
            .context("a common attribute row is not a list")?;
        let record = root
            .get(1)
            .and_then(Brace::as_list)
            .context("a common attribute row has no record")?;
        if record.first().and_then(Brace::as_atom) != Some("5") || record.len() != 15 {
            bail!(
                "a common attribute record of {} elements starting with {:?} is not the layout this reads",
                record.len(),
                record.first()
            );
        }
        let typed = record[1]
            .as_list()
            .and_then(|body| body.get(1))
            .and_then(Brace::as_list)
            .context("a common attribute has no typed header")?;
        let header = typed
            .get(1)
            .and_then(Brace::as_list)
            .context("a common attribute has no md header")?;
        let uuid = header
            .get(1)
            .and_then(Brace::as_list)
            .and_then(|inner| inner.get(2))
            .and_then(Brace::as_atom)
            .context("a common attribute has no uuid")?
            .to_ascii_lowercase();
        let name = header
            .get(2)
            .and_then(Brace::as_str)
            .context("a common attribute has no name")?
            .to_owned();
        let pattern = typed.get(2).context("a common attribute has no type")?;
        let types = type_entries(pattern).with_context(|| format!("common attribute {name}"))?;

        let items = record[2]
            .as_list()
            .context("a common attribute has no content")?;
        let count: usize = items
            .get(1)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .context("a common attribute content has no count")?;
        let mut content = BTreeMap::new();
        for pair in items[2..].chunks(2) {
            let [object, mode] = pair else {
                bail!("the content of {name} ends inside an item");
            };
            let object = object
                .as_atom()
                .context("a content item has no object")?
                .to_ascii_lowercase();
            let mode = mode
                .as_list()
                .and_then(|mode| mode.get(1))
                .and_then(Brace::as_atom)
                .context("a content item has no use")?;
            let mode = match mode {
                "0" => ContentUse::Auto,
                "1" => ContentUse::Use,
                "2" => ContentUse::DontUse,
                other => bail!("content use {other} of {name} is not known"),
            };
            content.insert(object, mode);
        }
        if content.len() != count {
            bail!(
                "the content of {name} counts {count} and holds {}",
                content.len()
            );
        }
        let flag = |at: usize| -> Result<i64> {
            record[at]
                .as_atom()
                .and_then(|value| value.parse().ok())
                .with_context(|| format!("element {at} of common attribute {name} is not a number"))
        };
        let auto_use = flag(6)? == 0;
        let separates_data = flag(5)? == 0;
        let independent = flag(12)? == 0;
        let indexing = flag(3)?;
        Ok(Self {
            uuid,
            name,
            types,
            content,
            auto_use,
            separates_data,
            independent,
            indexing,
        })
    }

    /// Whether the attribute is a field of the object's table.
    pub fn applies_to(&self, object: &str) -> bool {
        match self.content.get(&object.to_ascii_lowercase()) {
            Some(ContentUse::Use) => true,
            Some(ContentUse::DontUse) => false,
            Some(ContentUse::Auto) | None => self.auto_use,
        }
    }
}

/// The common attributes of a configuration, in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommonAttributes {
    pub attributes: Vec<CommonAttribute>,
}

impl CommonAttributes {
    /// From the descriptor rows of the root collection's uuids (`rows(uuid)` gives the parsed row).
    pub fn from_rows(order: &[String], rows: &dyn Fn(&str) -> Option<Brace>) -> Result<Self> {
        let mut attributes = Vec::new();
        for uuid in order {
            let row = rows(uuid)
                .with_context(|| format!("no descriptor row for common attribute {uuid}"))?;
            attributes.push(CommonAttribute::parse(&row)?);
        }
        Ok(Self { attributes })
    }

    /// The attributes that are fields of the object's table, in order.
    pub fn applying(&self, object: &str) -> Vec<&CommonAttribute> {
        self.attributes
            .iter()
            .filter(|attribute| attribute.applies_to(object))
            .collect()
    }

    /// The common attributes a **new** object gets on their own, without being listed: those with
    /// `AutoUse = Use`.
    pub fn automatic(&self) -> Vec<&CommonAttribute> {
        self.attributes
            .iter()
            .filter(|attribute| attribute.auto_use)
            .collect()
    }
}
