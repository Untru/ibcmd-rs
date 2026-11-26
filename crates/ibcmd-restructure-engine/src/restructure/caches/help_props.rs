//! `c4629235-4823-4320-b8b5-1d08f4c6d612.si`: properties of the metadata objects the platform looks up
//! by uuid (help references, the type of the reference, a few flags of the object's editor).
//!
//! The row is `{0,{<n>,<key>,<count>,(<property id>,<value>) x count,...}}`, one hash map over the
//! objects **and their nested elements** (2376 keys in the БСП corpus, 4096 buckets). For a
//! catalog the entry is:
//!
//! | id | value | source |
//! |---|---|---|
//! | 2 | `{"N",<EditType>}` | slot `EditType` |
//! | 5 | `{"B",<Hierarchical>}` | slot `Hierarchical` |
//! | 6 | `{"N",<HierarchyType>}` | slot `HierarchyType` |
//! | 21 | `{"N",<SubordinationUse>}` | slot `SubordinationUse` |
//! | 10 | `{"B",1}` | only when `QuickChoice` is 1 |
//! | 0 | `{"S","v8config://v8cfgHelp/mdobject/id<uuid>/038b5c85-fb1c-4082-9c4c-e69f8928bf3a"}` | only when the object has a help page (a `Config` row `<uuid>.1`) |
//! | 3 | `{"#",fc01b5df-97fe-449b-83d4-218a090e681e,<Ref TypeId>}` | the `Ref` generated type |
//! | 24 | `{"#",9cd510d6-...,{0,N,{"#",157fa490-...,{1,<set>}}...}}` | the type sets that mention the object; an object nothing refers to has none |
//!
//! **Order.** The entries are in the iteration order of the map, and that depends on the insertion order
//! of all 2376 keys, which is *not* the root's order (measured: the tail of the row has the
//! nested elements of documents, data processors and registers interleaved). A new catalog changes
//! the position of a few other entries (case c: 5 of 2377 moved, apart from the new one) and these
//! moves are not reproducible without that order, so [`HelpProps::add_entry_approximately`] puts
//! the new key where the *table* puts a key inserted last and reports that the row is not
//! byte-exact. The row is a cache the platform tolerates stale (measured, `docs/apply/restructuring.md`
//! 12.5); it rewrites it at the next native apply.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::order::{bucket_count, uuid_hash};

const TYPE_REF_CLASS: &str = "fc01b5df-97fe-449b-83d4-218a090e681e";
const HELP_PAGE: &str = "038b5c85-fb1c-4082-9c4c-e69f8928bf3a";

/// `{"S","v8config://v8cfgHelp/mdobject/id<uuid>/038b5c85-..."}`
fn help_reference(uuid: &str) -> Brace {
    Brace::List(vec![
        Brace::str("S"),
        Brace::str(format!(
            "v8config://v8cfgHelp/mdobject/id{uuid}/{HELP_PAGE}"
        )),
    ])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub props: Vec<(String, Brace)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpProps {
    pub entries: Vec<Entry>,
}

impl HelpProps {
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("c4629235 is not brace text")?;
        let outer = tree.as_list().context("c4629235 is not a list")?;
        let [zero, body] = outer else {
            bail!("c4629235 has {} elements, expected 2", outer.len());
        };
        if zero.as_atom() != Some("0") {
            bail!("c4629235 has version {zero:?}");
        }
        let body = body.as_list().context("c4629235 has no body")?;
        let (count, mut rest) = body.split_first().context("c4629235 body is empty")?;
        let count: usize = count
            .as_atom()
            .and_then(|count| count.parse().ok())
            .context("c4629235 has no entry count")?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let [key, n, tail @ ..] = rest else {
                bail!("c4629235 ends inside an entry");
            };
            let n: usize = n
                .as_atom()
                .and_then(|n| n.parse().ok())
                .context("c4629235 entry has no property count")?;
            if tail.len() < 2 * n {
                bail!("c4629235 ends inside the properties of an entry");
            }
            let props = tail[..2 * n]
                .chunks(2)
                .map(|pair| {
                    Ok((
                        pair[0]
                            .as_atom()
                            .context("c4629235 property id is not a token")?
                            .to_owned(),
                        pair[1].clone(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            entries.push(Entry {
                key: key
                    .as_atom()
                    .context("c4629235 key is not a token")?
                    .to_owned(),
                props,
            });
            rest = &tail[2 * n..];
        }
        if !rest.is_empty() {
            bail!("c4629235 has {} tokens after its entries", rest.len());
        }
        Ok(Self { entries })
    }

    pub fn render(&self) -> Vec<u8> {
        let mut body = vec![Brace::num(self.entries.len() as i64)];
        for entry in &self.entries {
            body.push(Brace::atom(&entry.key));
            body.push(Brace::num(entry.props.len() as i64));
            for (id, value) in &entry.props {
                body.push(Brace::atom(id));
                body.push(value.clone());
            }
        }
        serialize_row(&Brace::List(vec![Brace::atom("0"), Brace::List(body)]))
    }

    /// The entry of a catalog as the platform writes it, without property 24.
    pub fn catalog_entry(facts: &ObjectFacts, has_help: bool) -> Result<Entry> {
        let tagged = |tag: &str, value: i64| Brace::List(vec![Brace::str(tag), Brace::num(value)]);
        let mut props = vec![
            ("2".to_owned(), tagged("N", facts.number("EditType")?)),
            ("5".to_owned(), tagged("B", facts.number("Hierarchical")?)),
            ("6".to_owned(), tagged("N", facts.number("HierarchyType")?)),
            (
                "21".to_owned(),
                tagged("N", facts.number("SubordinationUse")?),
            ),
        ];
        if facts.number("QuickChoice")? == 1 {
            props.push(("10".to_owned(), tagged("B", 1)));
        }
        if has_help {
            props.push(("0".to_owned(), help_reference(&facts.uuid)));
        }
        props.push((
            "3".to_owned(),
            Brace::List(vec![
                Brace::str("#"),
                Brace::atom(TYPE_REF_CLASS),
                Brace::atom(&facts.generated_type("Ref")?.type_id),
            ]),
        ));
        Ok(Entry {
            key: facts.uuid.clone(),
            props,
        })
    }

    /// The entry of a document as the platform writes it, without property 24: `19` the `Posting`
    /// code, `20` the `RealTimePosting` code, `0` the help reference, `3` the `Ref` type
    /// (measured on all 25 documents of the БСП corpus).
    pub fn document_entry(facts: &ObjectFacts, has_help: bool) -> Result<Entry> {
        let tagged = |tag: &str, value: i64| Brace::List(vec![Brace::str(tag), Brace::num(value)]);
        let mut props = vec![
            ("19".to_owned(), tagged("N", facts.number("Posting")?)),
            (
                "20".to_owned(),
                tagged("N", facts.number("RealTimePosting")?),
            ),
        ];
        if has_help {
            props.push(("0".to_owned(), help_reference(&facts.uuid)));
        }
        props.push((
            "3".to_owned(),
            Brace::List(vec![
                Brace::str("#"),
                Brace::atom(TYPE_REF_CLASS),
                Brace::atom(&facts.generated_type("Ref")?.type_id),
            ]),
        ));
        Ok(Entry {
            key: facts.uuid.clone(),
            props,
        })
    }

    /// Adds an entry where the table puts a key inserted **last**: before the first entry of its
    /// bucket (4096 buckets and up, see [`bucket_count`]), or at the end. Not byte-exact against the
    /// platform, whose order depends on the whole insertion history (see the module docs).
    pub fn add_entry_approximately(&mut self, entry: Entry) -> Result<()> {
        if self.entries.iter().any(|other| other.key == entry.key) {
            bail!("c4629235 has an entry for {} already", entry.key);
        }
        let mask = (bucket_count(self.entries.len() + 1) - 1) as u32;
        let bucket = uuid_hash(&entry.key)? & mask;
        let mut at = self.entries.len();
        for (index, other) in self.entries.iter().enumerate() {
            if uuid_hash(&other.key)? & mask == bucket {
                at = index;
                break;
            }
        }
        self.entries.insert(at, entry);
        Ok(())
    }
}
