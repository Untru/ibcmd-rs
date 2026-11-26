//! The attributes and tabular sections a reference object's descriptor row lists, with what the caches
//! keep of each: identity, synonym, type pattern, the flags that decide the XDTO property.
//!
//! A descriptor row is `{1,<owner record>,<n>,<collection>...}`; a collection is `{<class>,<count>,
//! <item>...}`. Of the classes an object has, the caches of this module know three: the attributes of a
//! catalog (`cf4abea7-...`) or a document (`45e46cbc-...`), its tabular sections (`932159f9-...` /
//! `21c53e09-...`) and the attributes of a section (`888744e1-...`). A collection of any other class that
//! is not empty (forms, templates, commands, ...) is reported in [`Members::others`], and the writers
//! refuse such an object: its records would reach rows this module does not build.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::restructure::caches::facts::{localized_pairs, tabular_class};

/// The attribute collection of a catalog.
pub const CATALOG_ATTRIBUTES: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
/// The attribute collection of a document.
pub const DOCUMENT_ATTRIBUTES: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
/// The attribute collection of a tabular section (catalogs and documents alike).
pub const SECTION_ATTRIBUTES: &str = "888744e1-b616-11d4-9436-004095e12fc7";

/// The class of the attribute collection of an object kind.
pub fn attribute_class(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" => Some(CATALOG_ATTRIBUTES),
        "Document" => Some(DOCUMENT_ATTRIBUTES),
        _ => None,
    }
}

/// Whether the field of a catalog attribute is nullable, and so its XDTO property has `lowerBound="0"`:
/// an attribute of a hierarchy of folders and items that is used for items only or for folders only
/// (`Use` 0 or 1). Attributes of flat catalogs, of documents and of tabular sections are not.
pub fn catalog_attribute_nullable(
    hierarchical: bool,
    hierarchy_type: i64,
    usage: Option<i64>,
) -> bool {
    hierarchical && hierarchy_type == 0 && matches!(usage, Some(0) | Some(1))
}

/// One attribute of an object or of a tabular section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub uuid: String,
    pub name: String,
    /// `(language, text)`, unescaped.
    pub synonyms: Vec<(String, String)>,
    /// The `{"Pattern",...}` of the attribute's type.
    pub pattern: Brace,
    /// `Use` of a catalog attribute (0 `ForItem`, 1 `ForFolder`, 2 `ForFolderAndItem`).
    pub usage: Option<i64>,
    /// `Indexing`: 0 `DontIndex`, 1 `Index`, 2 `IndexWithAdditionalOrder`.
    pub indexing: Option<i64>,
}

/// A tabular section and its attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionMember {
    pub uuid: String,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub attributes: Vec<Member>,
}

/// What an object's descriptor lists below its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Members {
    pub attributes: Vec<Member>,
    pub sections: Vec<SectionMember>,
    /// `(class, count)` of every other collection that is not empty.
    pub others: Vec<(String, usize)>,
}

impl Members {
    /// Reads the collections of a `kind` object's descriptor row.
    pub fn parse(kind: &str, row: &Brace) -> Result<Self> {
        let attributes_class =
            attribute_class(kind).with_context(|| format!("no attribute class for {kind}"))?;
        let sections_class =
            tabular_class(kind).with_context(|| format!("no sections for {kind}"))?;
        let items = row.as_list().context("a descriptor row is not a list")?;
        let count: usize = items
            .get(2)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .context("a descriptor row has no collection count")?;
        let mut out = Self {
            attributes: Vec::new(),
            sections: Vec::new(),
            others: Vec::new(),
        };
        for collection in items.iter().skip(3).take(count) {
            let list = collection.as_list().context("a collection is not a list")?;
            let class = list
                .first()
                .and_then(Brace::as_atom)
                .context("a collection has no class")?
                .to_ascii_lowercase();
            let members = list.get(2..).unwrap_or(&[]);
            if class == attributes_class {
                for item in members {
                    let mut attribute = member(item)?;
                    if kind != "Catalog" {
                        attribute.usage = None;
                    }
                    out.attributes.push(attribute);
                }
            } else if class == sections_class {
                for item in members {
                    out.sections.push(section(item)?);
                }
            } else if !members.is_empty() {
                out.others.push((class, members.len()));
            }
        }
        Ok(out)
    }
}

/// Depth first: the first attribute body `{27,{2,<md base>,<pattern>},...}` below `node` (not looking
/// inside a body), and the first md header outside of the bodies.
fn find<'a>(node: &'a Brace, bodies: &mut Vec<&'a Brace>, headers: &mut Vec<&'a Brace>) {
    let Some(items) = node.as_list() else { return };
    let is_body = items.first().and_then(Brace::as_atom) == Some("27")
        && items
            .get(1)
            .and_then(Brace::as_list)
            .is_some_and(|typed| typed.first().and_then(Brace::as_atom) == Some("2"));
    if is_body {
        bodies.push(node);
        return;
    }
    if headers.is_empty()
        && items.first().and_then(Brace::as_atom) == Some("3")
        && items.get(1).and_then(Brace::as_list).is_some_and(|inner| {
            inner.len() == 3 && inner.first().and_then(Brace::as_atom) == Some("1")
        })
        && items.get(2).and_then(Brace::as_str).is_some()
    {
        headers.push(node);
    }
    for child in items {
        find(child, bodies, headers);
    }
}

/// `{3,{1,0,<uuid>},"Name",{<n>,"ru","text",...},...}` -> (uuid, name, synonyms).
/// `(language, text)` pairs.
type Pairs = Vec<(String, String)>;

fn header(node: &Brace) -> Result<(String, String, Pairs)> {
    let items = node.as_list().context("an md header is not a list")?;
    let uuid = items
        .get(1)
        .and_then(Brace::as_list)
        .and_then(|inner| inner.get(2))
        .and_then(Brace::as_atom)
        .context("an md header has no uuid")?;
    let name = items
        .get(2)
        .and_then(Brace::as_str)
        .context("an md header has no name")?;
    let synonyms = localized_pairs(items.get(3).context("an md header has no synonym")?)
        .with_context(|| format!("the synonym of {name}"))?;
    Ok((uuid.to_ascii_lowercase(), name.to_owned(), synonyms))
}

/// An attribute item `{<record>,0}`, `record = {<v>,<body>,...}` (catalogs: `{v,body,Indexing,Use,...}`).
fn member(item: &Brace) -> Result<Member> {
    let record = item
        .as_list()
        .and_then(|item| item.first())
        .and_then(Brace::as_list)
        .context("an attribute item has no record")?;
    let body = record.get(1).context("an attribute record has no body")?;
    let (mut bodies, mut headers) = (Vec::new(), Vec::new());
    find(body, &mut bodies, &mut headers);
    let [body] = bodies.as_slice() else {
        bail!("an attribute record holds {} bodies", bodies.len());
    };
    let typed = body
        .as_list()
        .and_then(|items| items.get(1))
        .and_then(Brace::as_list)
        .context("an attribute body has no typed header")?;
    let (uuid, name, synonyms) =
        header(typed.get(1).context("no md header in an attribute body")?)?;
    let pattern = typed
        .get(2)
        .context("an attribute body has no type")?
        .clone();
    if pattern
        .as_list()
        .and_then(|items| items.first())
        .and_then(Brace::as_str)
        != Some("Pattern")
    {
        bail!("attribute {name} has no type pattern");
    }
    let usage = record
        .get(3)
        .and_then(Brace::as_atom)
        .and_then(|value| value.parse().ok());
    let indexing = record
        .get(2)
        .and_then(Brace::as_atom)
        .and_then(|value| value.parse().ok());
    Ok(Member {
        uuid,
        name,
        synonyms,
        pattern,
        usage,
        indexing,
    })
}

/// A tabular section item: its own md header and every attribute below it.
fn section(item: &Brace) -> Result<SectionMember> {
    let (mut bodies, mut headers) = (Vec::new(), Vec::new());
    find(item, &mut bodies, &mut headers);
    let (uuid, name, synonyms) = header(
        headers
            .first()
            .context("a tabular section has no md header")?,
    )?;
    let mut attributes = Vec::new();
    // the attributes are the items of the section's own attribute collection; a body is one of them
    let mut collections = Vec::new();
    collect_collections(item, &mut collections);
    for list in collections {
        for attribute in list.get(2..).unwrap_or(&[]) {
            let mut attribute =
                member(attribute).with_context(|| format!("tabular section {name}"))?;
            attribute.usage = None;
            attributes.push(attribute);
        }
    }
    if attributes.len() != bodies.len() {
        bail!(
            "tabular section {name} holds {} attribute bodies but {} attribute items",
            bodies.len(),
            attributes.len()
        );
    }
    Ok(SectionMember {
        uuid,
        name,
        synonyms,
        attributes,
    })
}

/// The `{888744e1-...,<count>,<item>...}` collections below a section item.
fn collect_collections<'a>(node: &'a Brace, out: &mut Vec<&'a [Brace]>) {
    let Some(items) = node.as_list() else { return };
    if items.first().and_then(Brace::as_atom) == Some(SECTION_ATTRIBUTES) {
        out.push(items);
        return;
    }
    for child in items {
        collect_collections(child, out);
    }
}
