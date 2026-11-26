//! The objects a restructure reads: catalogs and documents, by the facts their tables depend on.
//!
//! Both are `{1,<owner record>,<n>,<collection>...}` rows (see [`crate::restructure::catalog`]); the
//! owner record's slots come from the layout of `metadata_model::objects`, the attribute wrappers differ
//! (a catalog's carries `Use`, a document's does not) and so do the class ids of the collections. What
//! the plan needs from either is the same: the attributes in metadata order with their types, the tabular
//! sections, and a *shape* -- the values of the object's own properties that decide the standard fields
//! and indexes of its table, which must not change in the same stage.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::restructure::catalog::{
    ATTRIBUTES, AttributeFacts, CatalogFacts, DOCUMENT_ATTRIBUTES, DOCUMENT_TABULAR_SECTIONS,
    OwnerRecord, SectionFacts, attribute_body, md_base, number_at, record_positions, section,
};

/// The kinds the plan handles, in the order the platform walks the kinds of a configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    Catalog,
    Document,
}

impl ObjectKind {
    /// The kind in `DBNames` that names the main table (`Reference20`, `Document39`).
    pub fn table_kind(self) -> &'static str {
        match self {
            Self::Catalog => "Reference",
            Self::Document => "Document",
        }
    }

    /// The XDTO type of the object (`CatalogObject.X`).
    pub fn xdto_object(self) -> &'static str {
        match self {
            Self::Catalog => "CatalogObject",
            Self::Document => "DocumentObject",
        }
    }

    /// The properties an object of this kind has before its attributes, in the XDTO model.
    pub fn xdto_standard(self) -> &'static [&'static str] {
        match self {
            Self::Catalog => &[
                "IsFolder",
                "Ref",
                "DeletionMark",
                "Parent",
                "Owner",
                "Code",
                "Description",
                "PredefinedDataName",
            ],
            Self::Document => &["Ref", "DeletionMark", "Date", "Number", "Posted"],
        }
    }

    /// The fields an object of this kind has before its attributes, in `DBSchema`.
    pub fn standard_fields(self) -> &'static [&'static str] {
        match self {
            Self::Catalog => &[
                "ID",
                "Version",
                "Marked",
                "PredefinedID",
                "OwnerID",
                "ParentID",
                "Folder",
                "Code",
                "Description",
            ],
            Self::Document => &[
                "ID",
                "Version",
                "Marked",
                "Date_Time",
                "NumberPrefix",
                "Number",
                "Posted",
            ],
        }
    }

    /// The class of the collection of the object's attributes in the descriptor and in the search
    /// information (`cf4abea7-...` for a catalog, `45e46cbc-...` for a document).
    pub fn attribute_class(self) -> &'static str {
        match self {
            Self::Catalog => ATTRIBUTES,
            Self::Document => DOCUMENT_ATTRIBUTES,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Document => "document",
        }
    }
}

/// A document's descriptor as the table structure needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentFacts {
    pub uuid: String,
    pub name: String,
    pub number_type: i64,
    pub number_length: i64,
    pub number_periodicity: i64,
    pub number_allowed_length: i64,
    pub check_unique: i64,
    pub posting: i64,
    pub data_history: i64,
    pub attributes: Vec<AttributeFacts>,
    pub sections: Vec<SectionFacts>,
}

/// `{<record>,0}` with `record = {v,<body>,<Indexing>,<FullTextSearch>,<DataHistory>[,0,{1,nil}]}`.
fn document_attribute(item: &Brace) -> Result<AttributeFacts> {
    let record = item
        .as_list()
        .and_then(|item| item.first())
        .and_then(Brace::as_list)
        .context("an attribute item has no record")?;
    let body = record.get(1).context("an attribute record has no body")?;
    let mut facts = attribute_body(body)?;
    facts.indexing = Some(number_at(record, 2, "Indexing")?);
    Ok(facts)
}

impl DocumentFacts {
    pub fn parse(row: &Brace) -> Result<Self> {
        let root = row.as_list().context("a document row is not a list")?;
        if root.first().and_then(Brace::as_atom) != Some("1") {
            bail!("not a descriptor row");
        }
        let values = root
            .get(1)
            .and_then(Brace::as_list)
            .context("a document row has no owner record")?;
        let tag: i64 = values
            .first()
            .and_then(Brace::as_atom)
            .and_then(|tag| tag.parse().ok())
            .context("a document row has no tag")?;
        if tag != 40 {
            bail!("not a document row (tag {tag})");
        }
        let (positions, header) = record_positions("Document", tag)?;
        let record = OwnerRecord {
            values,
            positions,
            header,
        };
        let wrapped = values
            .get(record.header)
            .and_then(Brace::as_list)
            .and_then(|wrapped| wrapped.get(1))
            .context("a document row has no header")?;
        let (uuid, name) = md_base(wrapped)?;
        let count: usize = root
            .get(2)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .context("a document row has no collection count")?;
        let mut attributes = Vec::new();
        let mut sections = Vec::new();
        for collection in root.iter().skip(3).take(count) {
            let items = collection.as_list().context("a collection is not a list")?;
            let class = items
                .first()
                .and_then(Brace::as_atom)
                .context("a collection has no class")?;
            if class == DOCUMENT_ATTRIBUTES {
                for item in items.iter().skip(2) {
                    attributes.push(
                        document_attribute(item).with_context(|| format!("document {name}"))?,
                    );
                }
            } else if class == DOCUMENT_TABULAR_SECTIONS {
                for item in items.iter().skip(2) {
                    sections
                        .push(section(item, false).with_context(|| format!("document {name}"))?);
                }
            }
        }
        Ok(Self {
            uuid,
            name,
            number_type: record.number("NumberType")?,
            number_length: record.number("NumberLength")?,
            number_periodicity: record.number("NumberPeriodicity")?,
            number_allowed_length: record.number("NumberAllowedLength")?,
            check_unique: record.number("CheckUnique")?,
            posting: record.number("Posting")?,
            data_history: record.number("DataHistory")?,
            attributes,
            sections,
        })
    }
}

/// A catalog or a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectFacts {
    Catalog(CatalogFacts),
    Document(DocumentFacts),
}

impl ObjectFacts {
    /// Reads a descriptor row (the inflated text, parsed); the record tag tells the kind.
    pub fn parse(row: &Brace) -> Result<Self> {
        let tag = row
            .as_list()
            .and_then(|root| root.get(1))
            .and_then(Brace::as_list)
            .and_then(|values| values.first())
            .and_then(Brace::as_atom)
            .context("a descriptor row has no owner record")?;
        match tag {
            "56" | "57" => CatalogFacts::parse(row).map(Self::Catalog),
            "40" => DocumentFacts::parse(row).map(Self::Document),
            other => bail!("the record tag {other} is neither a catalog nor a document"),
        }
    }

    pub fn kind(&self) -> ObjectKind {
        match self {
            Self::Catalog(_) => ObjectKind::Catalog,
            Self::Document(_) => ObjectKind::Document,
        }
    }

    pub fn uuid(&self) -> &str {
        match self {
            Self::Catalog(facts) => &facts.uuid,
            Self::Document(facts) => &facts.uuid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Catalog(facts) => &facts.name,
            Self::Document(facts) => &facts.name,
        }
    }

    pub fn attributes(&self) -> &[AttributeFacts] {
        match self {
            Self::Catalog(facts) => &facts.attributes,
            Self::Document(facts) => &facts.attributes,
        }
    }

    pub fn sections(&self) -> &[SectionFacts] {
        match self {
            Self::Catalog(facts) => &facts.sections,
            Self::Document(facts) => &facts.sections,
        }
    }

    /// The values of the object's own properties that decide its standard fields and indexes, by
    /// name. Two rows of one object must agree on all of them for the plan to be a plain attribute add.
    pub fn shape(&self) -> Vec<(&'static str, i64)> {
        match self {
            Self::Catalog(facts) => vec![
                ("Hierarchical", i64::from(facts.hierarchical)),
                ("HierarchyType", facts.hierarchy_type),
                ("CodeLength", facts.code_length),
                ("CodeType", facts.code_type),
                ("CodeAllowedLength", facts.code_allowed_length),
                ("DescriptionLength", facts.description_length),
                ("Owners", facts.owners),
                ("DataHistory", facts.data_history),
            ],
            Self::Document(facts) => vec![
                ("NumberType", facts.number_type),
                ("NumberLength", facts.number_length),
                ("NumberPeriodicity", facts.number_periodicity),
                ("NumberAllowedLength", facts.number_allowed_length),
                ("CheckUnique", facts.check_unique),
                ("Posting", facts.posting),
                ("DataHistory", facts.data_history),
            ],
        }
    }

    /// What the plan does not cover: the object is refused before anything is planned. `own_attributes`: the
    /// change touches the object's own attributes (the place of a new field, the nullability of a field), not
    /// only its tabular sections; the owner field of a subordinate catalog is not covered for those.
    pub fn check_supported(&self, own_attributes: bool) -> Result<()> {
        match self {
            Self::Catalog(facts) => {
                if facts.owners != 0 && own_attributes {
                    bail!(
                        "catalog {} is subordinate to owners: its owner field is not covered",
                        facts.name
                    );
                }
                if facts.data_history != 0 {
                    bail!("catalog {} keeps data history: not supported", facts.name);
                }
            }
            Self::Document(facts) => {
                if facts.data_history != 0 {
                    bail!("document {} keeps data history: not supported", facts.name);
                }
            }
        }
        Ok(())
    }

    /// Whether the field of an attribute is nullable. In a hierarchical catalog with folders an
    /// attribute that only items have (`Use` = `ForItem`) is NULL for folders and one that only folders
    /// have (`ForFolder`) is NULL for items; `ForFolderAndItem` is a plain field (measured: 229 of 229
    /// `ForItem` attributes of the БСП, and the types case for `ForFolder`).
    pub fn nullable(&self, attribute: &AttributeFacts, has_folder: bool) -> bool {
        match self {
            Self::Catalog(_) => has_folder && matches!(attribute.usage, Some(0) | Some(1)),
            Self::Document(_) => false,
        }
    }
}
