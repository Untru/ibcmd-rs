//! A catalog's descriptor row, as far as the table structure needs it.
//!
//! A catalog row is `{1,<owner record>,<n>,<collection>...}`; the owner record is a flat list of slots
//! (the layout of `metadata_model::objects`), each collection `{<class uuid>,<count>,<item>...}`. The
//! record versions follow the configuration's compatibility mode -- a staged row written by a newer
//! record version differs from the stored one in almost every record (tag 56 -> 57, an attribute wrapper
//! `5` -> `6` with a constant tail) without any structural change, so rows are never compared as text:
//! the *facts* the structure depends on are read from each and compared.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};

use crate::metadata_model::brace::Brace;
use crate::metadata_model::objects::{layout, parts::Slot};

/// The collection of a catalog's attributes.
pub const ATTRIBUTES: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
/// The collection of a catalog's tabular sections.
pub const TABULAR_SECTIONS: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
/// The collection of a document's attributes and of its tabular sections.
pub const DOCUMENT_ATTRIBUTES: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
pub const DOCUMENT_TABULAR_SECTIONS: &str = "21c53e09-8950-4b5e-a6a0-1054f1bbc274";

/// `v8:ValueStorage` and `v8:UUID`, the only type ids of a pattern that map to a field on their own.
const VALUE_STORAGE_TYPE: &str = "e199ca70-93cf-46ce-a54b-6edc88c3a296";
const UUID_TYPE: &str = "fc01b5df-97fe-449b-83d4-218a090e681e";

/// One attribute of a catalog (or of a tabular section) as its row states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeFacts {
    pub uuid: String,
    pub name: String,
    /// The `{"Pattern",...}` text of the type.
    pub pattern: String,
    /// `Indexing` (0 DontIndex, 1 Index, 2 IndexWithAdditionalOrder). A tabular section's attribute
    /// has it in the same place of its wrapper (`{v,<body>,<Indexing>,<FullTextSearch>,<DataHistory>}`).
    pub indexing: Option<i64>,
    /// `Use` (0 ForItem, 1 ForFolder, 2 ForFolderAndItem) of a catalog attribute.
    pub usage: Option<i64>,
    /// The parsed `{"Pattern",...}`.
    pub pattern_node: Brace,
    /// The synonyms as the search information lists them: `(language, text)`, the text as it stands
    /// between the quotes (quotes doubled).
    pub synonyms: Vec<(String, String)>,
}

/// What a tabular section contributes to the structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionFacts {
    pub uuid: String,
    pub name: String,
    /// The synonyms as the search information lists them.
    pub synonyms: Vec<(String, String)>,
    /// `Use` (0 ForItem, 1 ForFolder, 2 ForFolderAndItem) of a catalog's section; `None` for a document's.
    pub usage: Option<i64>,
    /// `LineNumberLength`; the line number field is a `numeric(5,0)` whatever it says, so a section of any
    /// other length is not something the plan can build.
    pub line_number_length: i64,
    pub attributes: Vec<AttributeFacts>,
}

/// The structure-relevant facts of a catalog row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFacts {
    pub uuid: String,
    pub name: String,
    pub hierarchical: bool,
    /// 0 `HierarchyFoldersAndItems`, 1 `HierarchyOfItems`.
    pub hierarchy_type: i64,
    pub code_length: i64,
    /// 0 `Number`, 1 `String`.
    pub code_type: i64,
    /// 0 `Fixed`, 1 `Variable`.
    pub code_allowed_length: i64,
    pub description_length: i64,
    /// How many owners the catalog is subordinate to.
    pub owners: i64,
    /// 0 `DontUse`, 1 `Use`.
    pub data_history: i64,
    pub attributes: Vec<AttributeFacts>,
    pub sections: Vec<SectionFacts>,
}

/// The value of a slot of the owner record by property name.
pub struct OwnerRecord<'a> {
    pub values: &'a [Brace],
    pub positions: HashMap<&'static str, usize>,
    pub header: usize,
}

impl OwnerRecord<'_> {
    pub fn atom(&self, name: &str) -> Result<&str> {
        let position = *self
            .positions
            .get(name)
            .ok_or_else(|| anyhow!("the catalog layout has no slot {name}"))?;
        self.values
            .get(position)
            .and_then(Brace::as_atom)
            .with_context(|| format!("slot {name} is not a plain value"))
    }

    pub fn number(&self, name: &str) -> Result<i64> {
        self.atom(name)?
            .parse()
            .with_context(|| format!("slot {name} is not a number"))
    }

    /// The count of `{0,N,...}` (`References`).
    pub fn reference_count(&self, name: &str) -> Result<i64> {
        let position = *self
            .positions
            .get(name)
            .ok_or_else(|| anyhow!("the catalog layout has no slot {name}"))?;
        let list = self
            .values
            .get(position)
            .and_then(Brace::as_list)
            .with_context(|| format!("slot {name} is not a list"))?;
        list.get(1)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .with_context(|| format!("slot {name} has no count"))
    }
}

/// Walks the catalog layout for the positions of the named slots in a record
/// of the given tag (`56` before compatibility 8.3.27, `57` from it).
pub fn record_positions(kind: &str, tag: i64) -> Result<(HashMap<&'static str, usize>, usize)> {
    let layout = layout(kind).with_context(|| format!("no {kind} layout"))?;
    let Some(Slot::Tag(old, new, latest)) = layout.slots.first().copied() else {
        bail!("the {kind} layout does not start with its tag");
    };
    let modern = tag != old;
    let since_8_5_1 = tag == latest && latest != new;
    let mut positions = HashMap::new();
    let mut header = None;
    let mut position = 0usize;
    for slot in layout.slots {
        walk(
            slot,
            modern,
            since_8_5_1,
            &mut position,
            &mut positions,
            &mut header,
        );
    }
    Ok((
        positions,
        header.with_context(|| format!("the {kind} layout has no header slot"))?,
    ))
}

fn walk(
    slot: &Slot,
    modern: bool,
    since_8_5_1: bool,
    position: &mut usize,
    positions: &mut HashMap<&'static str, usize>,
    header: &mut Option<usize>,
) {
    match slot {
        Slot::Modern(inner) => {
            if modern {
                walk(inner, modern, since_8_5_1, position, positions, header);
            }
        }
        Slot::Since8_5_1(inner) => {
            if since_8_5_1 {
                walk(inner, modern, since_8_5_1, position, positions, header);
            }
        }
        Slot::Generated(_) => *position += 2,
        Slot::Header | Slot::WrappedHeader => {
            header.get_or_insert(*position);
            *position += 1;
        }
        Slot::Flag(name)
        | Slot::Number(name)
        | Slot::Code(name, _)
        | Slot::Text(name)
        | Slot::Localized(name)
        | Slot::Reference(name)
        | Slot::References(name)
        | Slot::Fields(name)
        | Slot::TypePattern(name) => {
            positions.insert(*name, *position);
            *position += 1;
        }
        _ => *position += 1,
    }
}

/// `{3,{1,0,<uuid>},"Name",{<n>,"ru","text",...},...}` -> the synonyms `(language, text)`.
pub fn md_synonyms(node: &Brace) -> Vec<(String, String)> {
    let Some(block) = node
        .as_list()
        .and_then(|items| items.get(3))
        .and_then(Brace::as_list)
    else {
        return Vec::new();
    };
    block
        .get(1..)
        .unwrap_or_default()
        .chunks(2)
        .filter_map(
            |pair| match (pair.first()?.as_str(), pair.get(1)?.as_str()) {
                (Some(language), Some(text)) => {
                    Some((language.to_owned(), text.replace('"', "\"\"")))
                }
                _ => None,
            },
        )
        .collect()
}

/// `{3,{1,0,<uuid>},"Name",...}` -> (uuid, name).
pub fn md_base(node: &Brace) -> Result<(String, String)> {
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
    Ok((uuid.to_ascii_lowercase(), name.to_owned()))
}

impl CatalogFacts {
    /// Reads a catalog row (the inflated text, parsed).
    pub fn parse(row: &Brace) -> Result<Self> {
        let root = row.as_list().context("a catalog row is not a list")?;
        if root.first().and_then(Brace::as_atom) != Some("1") {
            bail!("not a descriptor row");
        }
        let values = root
            .get(1)
            .and_then(Brace::as_list)
            .context("a catalog row has no owner record")?;
        let tag: i64 = values
            .first()
            .and_then(Brace::as_atom)
            .and_then(|tag| tag.parse().ok())
            .context("a catalog row has no tag")?;
        if tag != 56 && tag != 57 {
            bail!("not a catalog row (tag {tag})");
        }
        let (positions, header) = record_positions("Catalog", tag)?;
        let record = OwnerRecord {
            values,
            positions,
            header,
        };
        let wrapped = values
            .get(record.header)
            .and_then(Brace::as_list)
            .and_then(|wrapped| wrapped.get(1))
            .context("a catalog row has no header")?;
        let (uuid, name) = md_base(wrapped)?;

        let count: usize = root
            .get(2)
            .and_then(Brace::as_atom)
            .and_then(|count| count.parse().ok())
            .context("a catalog row has no collection count")?;
        let mut attributes = Vec::new();
        let mut sections = Vec::new();
        for collection in root.iter().skip(3).take(count) {
            let items = collection.as_list().context("a collection is not a list")?;
            let class = items
                .first()
                .and_then(Brace::as_atom)
                .context("a collection has no class")?;
            if class == ATTRIBUTES {
                for item in items.iter().skip(2) {
                    attributes
                        .push(catalog_attribute(item).with_context(|| format!("catalog {name}"))?);
                }
            } else if class == TABULAR_SECTIONS {
                for item in items.iter().skip(2) {
                    sections.push(section(item, true).with_context(|| format!("catalog {name}"))?);
                }
            }
        }
        Ok(Self {
            uuid,
            name,
            hierarchical: record.atom("Hierarchical")? == "1",
            hierarchy_type: record.number("HierarchyType")?,
            code_length: record.number("CodeLength")?,
            code_type: record.number("CodeType")?,
            code_allowed_length: record.number("CodeAllowedLength")?,
            description_length: record.number("DescriptionLength")?,
            owners: record.reference_count("Owners")?,
            data_history: record.number("DataHistory")?,
            attributes,
            sections,
        })
    }
}

/// `{<record>,0}` with `record = {v,<body>,<Indexing>,<Use>,<FullTextSearch>,<DataHistory>[,0,{1,nil}]}`.
fn catalog_attribute(item: &Brace) -> Result<AttributeFacts> {
    let record = item
        .as_list()
        .and_then(|item| item.first())
        .and_then(Brace::as_list)
        .context("an attribute item has no record")?;
    let body = record.get(1).context("an attribute record has no body")?;
    let mut facts = attribute_body(body)?;
    facts.indexing = Some(number_at(record, 2, "Indexing")?);
    facts.usage = Some(number_at(record, 3, "Use")?);
    Ok(facts)
}

pub fn number_at(record: &[Brace], index: usize, what: &str) -> Result<i64> {
    record
        .get(index)
        .and_then(Brace::as_atom)
        .and_then(|value| value.parse().ok())
        .with_context(|| format!("attribute record has no {what}"))
}

/// `{27,{2,<md base>,<pattern>},...}`.
pub fn attribute_body(body: &Brace) -> Result<AttributeFacts> {
    let slots = body.as_list().context("an attribute body is not a list")?;
    if slots.first().and_then(Brace::as_atom) != Some("27") {
        bail!("not an attribute body");
    }
    let typed = slots
        .get(1)
        .and_then(Brace::as_list)
        .context("an attribute body has no typed header")?;
    let header = typed.get(1).context("no md header in an attribute body")?;
    let (uuid, name) = md_base(header)?;
    let pattern = typed.get(2).context("an attribute body has no type")?;
    if pattern
        .as_list()
        .and_then(|items| items.first())
        .and_then(Brace::as_str)
        != Some("Pattern")
    {
        bail!("attribute {name} has no type pattern");
    }
    Ok(AttributeFacts {
        uuid,
        name,
        pattern: crate::metadata_model::brace::serialize(pattern),
        indexing: None,
        usage: None,
        pattern_node: pattern.clone(),
        synonyms: md_synonyms(header),
    })
}

/// A tabular section item: `{<wrapper>,1,<collection of attributes>}`, the wrapper `{v,<record>,<Use>,
/// <LineNumberLength>}` of a catalog's section and `{v,<record>,<LineNumberLength>}` of a document's (the
/// older record versions have neither the length nor, for a document, anything after the record); its own
/// md header and every attribute below it.
pub fn section(item: &Brace, catalog: bool) -> Result<SectionFacts> {
    let mut bodies = Vec::new();
    let mut headers = Vec::new();
    collect(item, &mut bodies, &mut headers);
    let (uuid, name) = headers
        .first()
        .map(|header| md_base(header))
        .transpose()?
        .context("a tabular section has no md header")?;
    let synonyms = headers
        .first()
        .map(|header| md_synonyms(header))
        .unwrap_or_default();
    let wrapper = item
        .as_list()
        .and_then(|items| items.first())
        .and_then(Brace::as_list)
        .with_context(|| format!("tabular section {name} has no wrapper"))?;
    let (usage, length_at) = if catalog {
        (Some(number_at(wrapper, 2, "Use")?), 3)
    } else {
        (None, 2)
    };
    // The older record versions (`{1,<record>[,<Use>]}`) have no length: it is the 5 every section has there.
    let line_number_length = match wrapper.get(length_at) {
        Some(_) => number_at(wrapper, length_at, "LineNumberLength")
            .with_context(|| format!("tabular section {name}"))?,
        None => 5,
    };
    let attributes = bodies
        .into_iter()
        .map(|(body, indexing)| {
            let mut facts = attribute_body(body)?;
            facts.indexing = indexing;
            Ok(facts)
        })
        .collect::<Result<Vec<_>>>()
        .with_context(|| format!("tabular section {name}"))?;
    Ok(SectionFacts {
        uuid,
        name,
        synonyms,
        usage,
        line_number_length,
        attributes,
    })
}

fn is_attribute_body(node: &Brace) -> bool {
    node.as_list().is_some_and(|items| {
        items.first().and_then(Brace::as_atom) == Some("27")
            && items
                .get(1)
                .and_then(Brace::as_list)
                .is_some_and(|typed| typed.first().and_then(Brace::as_atom) == Some("2"))
    })
}

/// Depth-first: every attribute body `{27,{2,...}}` with the `Indexing` of the wrapper `{v,<body>,<Indexing>,
/// ...}` it sits in, and the first md header `{3,{1,0,<uuid>},"Name",...}` outside of the bodies.
fn collect<'a>(
    node: &'a Brace,
    bodies: &mut Vec<(&'a Brace, Option<i64>)>,
    headers: &mut Vec<&'a Brace>,
) {
    let Some(items) = node.as_list() else {
        return;
    };
    if let Some(body) = items.get(1).filter(|body| is_attribute_body(body)) {
        let indexing = items
            .get(2)
            .and_then(Brace::as_atom)
            .and_then(|indexing| indexing.parse().ok());
        bodies.push((body, indexing));
        return;
    }
    if is_attribute_body(node) {
        bodies.push((node, None));
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
        collect(child, bodies, headers);
    }
}

// ---------------------------------------------------------------------------
// The type of an attribute -> the field's type entries.

/// The field type entries of an attribute pattern. Only the types whose columns were
/// verified against the platform (case h of the traces, the types case) are mapped: a boolean, a
/// string, a number, a date, a date and time or a time, a value storage, a uuid. A reference, a
/// composite type or a defined type is refused.
pub fn type_entries(pattern: &Brace) -> Result<Vec<crate::restructure::schema::TypeEntry>> {
    use crate::restructure::schema::TypeEntry;

    let items = pattern.as_list().context("a pattern is not a list")?;
    if items.first().and_then(Brace::as_str) != Some("Pattern") {
        bail!("not a type pattern");
    }
    let [_, item] = items else {
        bail!(
            "a composite type ({} items) is not mapped to fields yet",
            items.len() - 1
        );
    };
    let fields = item.as_list().context("a pattern item is not a list")?;
    let tag = fields
        .first()
        .and_then(Brace::as_str)
        .context("a pattern item has no tag")?;
    let atom = |index: usize| {
        fields
            .get(index)
            .and_then(Brace::as_atom)
            .and_then(|value| value.parse::<u64>().ok())
    };
    Ok(vec![match tag {
        "B" => TypeEntry::new("L", 0, 0, "", 0),
        "S" => {
            let length = if fields.len() >= 3 {
                atom(1).context("a string length")?
            } else {
                0
            };
            let variable = fields.len() < 3 || fields.get(2).and_then(Brace::as_atom) != Some("0");
            if length > 0x7fff_ffff {
                bail!("a string longer than {length} is not a SQL Server column");
            }
            if variable {
                TypeEntry::new("S", 0x8000_0000 | length, 0, "", 0)
            } else if length == 0 {
                bail!("a fixed string of length 0");
            } else {
                TypeEntry::new("S", length, 0, "", 0)
            }
        }
        "N" => {
            let digits = atom(1).context("number digits")?;
            let fraction = atom(2).context("number fraction digits")?;
            if digits == 0 || digits > 38 || fraction > digits {
                bail!("numeric({digits}, {fraction}) is not a SQL Server type");
            }
            TypeEntry::new("N", digits, fraction, "", 0)
        }
        // Unqualified BinaryData (`{"R"}`) is varbinary(max), measured in
        // the synthetic extension update on both 8.3.27 and 8.5 (#582).
        "R" if fields.len() == 1 => TypeEntry::new("B", 0x8000_0000, 0, "", 0),
        // A date, a date and time and a time alone are the same field (`datetime2(0)`; measured).
        "D" => TypeEntry::new("T", 0, 0, "", 0),
        "#" => match fields.get(1).and_then(Brace::as_atom) {
            Some(VALUE_STORAGE_TYPE) => TypeEntry::new("B", 0x8000_0000, 0, "", 0),
            Some(UUID_TYPE) => TypeEntry::new("B", 16, 0, "", 0),
            other => bail!(
                "the type {other:?} is a reference or a platform type that is not mapped to fields yet"
            ),
        },
        other => bail!("the pattern item {other:?} is not mapped to fields yet"),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::parse_row;
    use crate::restructure::names::inflate;

    const OLD: &[u8] = include_bytes!(
        "../../../../tests/fixtures/native-evidence/restructure/catalog-reference20-old.deflate"
    );
    const NEW: &[u8] = include_bytes!(
        "../../../../tests/fixtures/native-evidence/restructure/catalog-reference20-new.deflate"
    );

    fn facts(row: &[u8]) -> CatalogFacts {
        CatalogFacts::parse(&parse_row(&inflate(row).unwrap()).unwrap()).unwrap()
    }

    #[test]
    fn a_stored_and_a_staged_row_have_the_same_facts_but_the_new_attribute() {
        let old = facts(OLD);
        let new = facts(NEW);
        assert_eq!(old.uuid, "5eab8a1b-070f-4dcf-bdcc-a259c62c3693");
        assert_eq!(old.name, "_ДемоПартнеры");
        assert!(old.hierarchical);
        assert_eq!(old.hierarchy_type, 0);
        assert_eq!(old.owners, 0);
        assert_eq!(old.data_history, 0);
        // The record versions differ (56 vs 57, wrapper 5 vs 6), the facts do not.
        assert_eq!(old.sections, new.sections);
        assert_eq!(
            (
                old.code_length,
                old.code_type,
                old.code_allowed_length,
                old.description_length
            ),
            (
                new.code_length,
                new.code_type,
                new.code_allowed_length,
                new.description_length
            )
        );
        assert_eq!(old.attributes.len(), 6);
        assert_eq!(new.attributes.len(), 7);
        assert_eq!(&new.attributes[..6], &old.attributes[..]);
        let added = &new.attributes[6];
        assert_eq!(added.uuid, "c60cdc87-198a-4f6e-8f17-76bcb1b1914b");
        assert_eq!(added.name, "ДемоНовыйРеквизит");
        assert_eq!(added.pattern, "{\"Pattern\",\r\n{\"S\",50,1}\r\n}");
        assert_eq!((added.indexing, added.usage), (Some(0), Some(0)));
        // Two tabular sections with 3 and 13 attributes (VT155 and VT159 of the schema).
        assert_eq!(new.sections.len(), 2);
        assert_eq!(new.sections[0].attributes.len(), 3);
        assert_eq!(new.sections[1].attributes.len(), 13);
    }

    #[test]
    fn the_pattern_of_the_new_attribute_is_a_variable_string_of_50() {
        let new = facts(NEW);
        let entries = type_entries(&new.attributes[6].pattern_node).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].to_brace(),
            crate::brace_list![
                Brace::str("S"),
                Brace::atom(2147483698u64),
                Brace::num(0),
                Brace::str(""),
                Brace::num(0)
            ]
        );
    }

    #[test]
    fn patterns_are_mapped_or_refused() {
        let parse = |text: &str| parse_row(text.as_bytes()).unwrap();
        let entry = |text: &str| type_entries(&parse(text)).unwrap().remove(0);
        assert_eq!(entry("{\"Pattern\",{\"B\"}}").tag, "L");
        let fixed = entry("{\"Pattern\",{\"S\",20,0}}");
        assert_eq!((fixed.tag.as_str(), fixed.a), ("S", 20));
        let unlimited = entry("{\"Pattern\",{\"S\",0,1}}");
        assert_eq!(unlimited.a, 0x8000_0000);
        let number = entry("{\"Pattern\",{\"N\",12,3,0}}");
        assert_eq!((number.tag.as_str(), number.a, number.b), ("N", 12, 3));
        assert_eq!(entry("{\"Pattern\",{\"D\",\"DT\"}}").tag, "T");
        assert_eq!(entry("{\"Pattern\",{\"D\",\"D\"}}").tag, "T");
        assert_eq!(entry("{\"Pattern\",{\"D\",\"T\"}}").tag, "T");
        assert_eq!(
            entry("{\"Pattern\",{\"#\",fc01b5df-97fe-449b-83d4-218a090e681e}}").a,
            16
        );
        assert_eq!(
            entry("{\"Pattern\",{\"#\",e199ca70-93cf-46ce-a54b-6edc88c3a296}}").a,
            0x8000_0000
        );
        for refused in [
            "{\"Pattern\",{\"S\",10,1},{\"N\",5,0,0}}",
            "{\"Pattern\",{\"#\",a8034afe-1aa5-41d3-a773-0abf601f51ca}}",
            "{\"Pattern\",{\"N\",0,0,0}}",
            "{\"Pattern\"}",
        ] {
            assert!(type_entries(&parse(refused)).is_err(), "{refused}");
        }
    }
}
