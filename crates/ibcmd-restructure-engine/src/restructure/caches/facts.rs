//! What the caches read off a reference object's descriptor row: identity, generated types and the
//! few slots that end up in a cache.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::restructure::caches::root::is_uuid;
use crate::restructure::caches::slots::{
    GeneratedType, RecordMap, md_base, object_identity, owner_record,
};

/// `157fa490-4ce9-11d4-9415-008048da11f9`: the type of a metadata-object reference value.
pub const METADATA_REF: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

/// A reference object's descriptor row, read for the caches.
#[derive(Clone, Debug)]
pub struct ObjectFacts {
    pub kind: String,
    pub uuid: String,
    pub name: String,
    /// The object's synonym, `(language, text)`.
    pub synonym: Vec<(String, String)>,
    /// The generated types in layout order.
    pub generated: Vec<GeneratedType>,
    record: Vec<Brace>,
    map: RecordMap,
    /// The tabular sections of the row (`932159f9-...`), in order.
    pub sections: Vec<SectionFacts>,
}

/// A tabular section of a catalog (or any object with the `932159f9-...` collection).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionFacts {
    pub uuid: String,
    pub name: String,
    /// `TypeId`/`ValueId` of the section itself and of its row, in that order.
    pub types: [(String, String); 2],
}

/// The collection of a catalog's tabular sections.
pub const TABULAR_SECTIONS: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
/// The collection of a document's tabular sections.
pub const DOCUMENT_TABULAR_SECTIONS: &str = "21c53e09-8950-4b5e-a6a0-1054f1bbc274";

/// The class of the tabular-section collection of a kind (and of its section in `2203278d`).
pub fn tabular_class(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" => Some(TABULAR_SECTIONS),
        "Document" => Some(DOCUMENT_TABULAR_SECTIONS),
        _ => None,
    }
}

impl ObjectFacts {
    /// Reads the row of a `kind` object (`Catalog`, `Document`, ...).
    pub fn parse(kind: &str, row: &Brace) -> Result<Self> {
        let (record, tag) = owner_record(row)?;
        let map = RecordMap::new(kind, tag)?;
        let (uuid, name) = object_identity(row, kind)?;
        let synonym = header_synonym(record, &map).with_context(|| format!("{kind} {name}"))?;
        let generated = map.generated_types(record)?;
        let items = row.as_list().context("a descriptor row is not a list")?;
        let count: usize = items
            .get(2)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .context("a descriptor row has no collection count")?;
        let mut sections = Vec::new();
        for collection in items.iter().skip(3).take(count) {
            let list = collection.as_list().context("a collection is not a list")?;
            if tabular_class(kind).is_some()
                && list.first().and_then(Brace::as_atom) == tabular_class(kind)
            {
                for item in list.iter().skip(2) {
                    sections.push(section_facts(item).with_context(|| format!("{kind} {name}"))?);
                }
            }
        }
        Ok(Self {
            kind: kind.to_owned(),
            uuid,
            name,
            synonym,
            generated,
            record: record.to_vec(),
            map,
            sections,
        })
    }

    /// The names of the record's slots.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.map.names.keys().copied()
    }

    /// The slot named `name` of the owner record.
    pub fn slot(&self, name: &str) -> Result<&Brace> {
        self.map.slot(&self.record, name)
    }

    /// A numeric slot.
    pub fn number(&self, name: &str) -> Result<i64> {
        self.slot(name)?
            .as_atom()
            .and_then(|value| value.parse().ok())
            .with_context(|| format!("slot {name} is not a number"))
    }

    /// A localized slot as `(language, text)` pairs; `{0}` is empty.
    pub fn localized(&self, name: &str) -> Result<Vec<(String, String)>> {
        localized_pairs(self.slot(name)?).with_context(|| format!("slot {name}"))
    }

    /// The uuids in a `{0,N,{"#",157fa490-...,{1,<uuid>}}...}` slot.
    pub fn references(&self, name: &str) -> Result<Vec<String>> {
        Ok(metadata_refs(self.slot(name)?))
    }

    /// The generated type of a category.
    pub fn generated_type(&self, category: &str) -> Result<&GeneratedType> {
        self.generated
            .iter()
            .find(|generated| generated.category == category)
            .with_context(|| format!("{} {} has no {category} type", self.kind, self.name))
    }
}

/// The synonym in the md header of an owner record.
fn header_synonym(record: &[Brace], map: &RecordMap) -> Result<Vec<(String, String)>> {
    let header = record
        .get(map.header)
        .context("the record is too short for its header")?;
    let base = match header.as_list() {
        Some([first, second]) if first.as_atom() == Some("0") => second,
        _ => header,
    };
    let node = base
        .as_list()
        .and_then(|items| items.get(3))
        .context("the md header has no synonym")?;
    localized_pairs(node)
}

/// `{N,"lang","text",...}` -> pairs. `{0}` -> none.
pub fn localized_pairs(node: &Brace) -> Result<Vec<(String, String)>> {
    let items = node.as_list().context("a localized string is not a list")?;
    let Some((count, rest)) = items.split_first() else {
        bail!("an empty localized string node");
    };
    let count: usize = count
        .as_atom()
        .and_then(|count| count.parse().ok())
        .context("a localized string has no count")?;
    if rest.len() != 2 * count {
        bail!(
            "a localized string counts {count} languages but has {} items",
            rest.len()
        );
    }
    rest.chunks(2)
        .map(|pair| {
            let language = pair[0].as_str().context("a language is not a string")?;
            let text = pair[1].as_str().context("a text is not a string")?;
            Ok((language.to_owned(), text.to_owned()))
        })
        .collect()
}

/// The object uuids of `{"#",157fa490-...,{1,<uuid>}}` items anywhere below `node`.
pub fn metadata_refs(node: &Brace) -> Vec<String> {
    let mut out = Vec::new();
    collect_refs(node, &mut out);
    out
}

fn collect_refs(node: &Brace, out: &mut Vec<String>) {
    let Some(items) = node.as_list() else { return };
    if let [tag, class, value] = items
        && tag.as_str() == Some("#")
        && class.as_atom() == Some(METADATA_REF)
        && let Some([one, uuid]) = value.as_list()
        && one.as_atom() == Some("1")
        && let Some(uuid) = uuid.as_atom()
        && is_uuid(uuid)
    {
        out.push(uuid.to_ascii_lowercase());
        return;
    }
    for item in items {
        collect_refs(item, out);
    }
}

/// `{{1,{11,<TypeId>,<ValueId>,<TypeId>,<ValueId>,{0,<md base>...}...},...},0}`: a tabular section item.
fn section_facts(item: &Brace) -> Result<SectionFacts> {
    let record = item
        .as_list()
        .and_then(|item| item.first())
        .and_then(Brace::as_list)
        .context("a tabular section item has no record")?;
    let body = record
        .get(1)
        .and_then(Brace::as_list)
        .context("a tabular section record has no body")?;
    let atom = |index: usize| -> Result<String> {
        body.get(index)
            .and_then(Brace::as_atom)
            .filter(|value| is_uuid(value))
            .map(str::to_ascii_lowercase)
            .with_context(|| format!("a tabular section body has no uuid at {index}"))
    };
    let types = [(atom(1)?, atom(2)?), (atom(3)?, atom(4)?)];
    let header = body
        .get(5)
        .and_then(Brace::as_list)
        .and_then(|wrapped| wrapped.get(1))
        .context("a tabular section body has no header")?;
    let (uuid, name) = md_base(header)?;
    Ok(SectionFacts { uuid, name, types })
}
