//! `2203278d-ef4f-4f68-98f1-feb257d53ecc.si`: the index of the generated types of every object.
//!
//! The row is `{1,{<sections>,<class>,<count>,<entry>...,<class>,<count>,...}}` -- one section per
//! kind of object, a section a hash map from the object's uuid to its generated types:
//! `<object>,<n>,(<TypeId>,<ValueId>,<index>) x n`, `index` counting the object's generated types in
//! the order of its layout (a catalog: `Object`, `Ref`, `Selection`, `List`, `Manager`; a tabular
//! section: its type and its row type). Without a section entry the client fails on the object's
//! type ("Тип не определен", measured: `docs/apply/restructuring.md` 12.5).
//!
//! The order of the sections and of the entries in them is the iteration order of the platform's hash
//! maps (see [`super::order`]); a section of a class the root lists is filled in the root's order.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};

/// One generated type of an entry: `<TypeId>,<ValueId>,<index>`. The index numbers the *category* of the
/// type (`Object` 0, `Ref` 1, `Selection` 2, `List` 3, `Manager` 4, ...), not the position: a chart of
/// characteristic types stores `0,1,2,3,5,4`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSlot {
    pub type_id: String,
    pub value_id: String,
    pub index: u32,
}

/// One `<object>,<n>,(<TypeId>,<ValueId>,<index>)...` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub object: String,
    /// The object's generated types in the order of its layout.
    pub types: Vec<TypeSlot>,
}

/// The entries of one kind, in row order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub class: String,
    pub entries: Vec<Entry>,
}

/// The parsed row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeIndex {
    pub sections: Vec<Section>,
}

impl TypeIndex {
    /// Parses the inflated row text (BOM optional).
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("2203278d is not brace text")?;
        let outer = tree.as_list().context("2203278d is not a list")?;
        let [version, body] = outer else {
            bail!("2203278d has {} elements, expected 2", outer.len());
        };
        if version.as_atom() != Some("1") {
            bail!("2203278d has version {version:?}");
        }
        let body = body.as_list().context("2203278d has no body")?;
        let (count, mut rest) = body.split_first().context("2203278d body is empty")?;
        let count: usize = count
            .as_atom()
            .and_then(|count| count.parse().ok())
            .context("2203278d has no section count")?;
        let atom = |node: &Brace| -> Result<String> {
            node.as_atom()
                .map(str::to_owned)
                .context("2203278d holds a non-atom token")
        };
        let number = |node: &Brace| -> Result<usize> {
            node.as_atom()
                .and_then(|value| value.parse().ok())
                .context("2203278d holds a bad number")
        };
        let mut sections = Vec::with_capacity(count);
        for _ in 0..count {
            let [class, entries, tail @ ..] = rest else {
                bail!("2203278d ends inside a section");
            };
            let class = atom(class)?;
            let entries = number(entries)?;
            rest = tail;
            let mut section = Section {
                class,
                entries: Vec::with_capacity(entries),
            };
            for _ in 0..entries {
                let [object, n, tail @ ..] = rest else {
                    bail!("2203278d ends inside an entry");
                };
                let n = number(n)?;
                if tail.len() < 3 * n {
                    bail!("2203278d ends inside the types of an entry");
                }
                let mut types = Vec::with_capacity(n);
                for triple in tail[..3 * n].chunks(3) {
                    types.push(TypeSlot {
                        type_id: atom(&triple[0])?,
                        value_id: atom(&triple[1])?,
                        index: u32::try_from(number(&triple[2])?)
                            .context("2203278d holds a bad type index")?,
                    });
                }
                section.entries.push(Entry {
                    object: atom(object)?,
                    types,
                });
                rest = &tail[3 * n..];
            }
            sections.push(section);
        }
        if !rest.is_empty() {
            bail!("2203278d has {} tokens after its sections", rest.len());
        }
        Ok(Self { sections })
    }

    /// The row text with its BOM.
    pub fn render(&self) -> Vec<u8> {
        let mut body = vec![Brace::num(self.sections.len() as i64)];
        for section in &self.sections {
            body.push(Brace::atom(&section.class));
            body.push(Brace::num(section.entries.len() as i64));
            for entry in &section.entries {
                body.push(Brace::atom(&entry.object));
                body.push(Brace::num(entry.types.len() as i64));
                for slot in &entry.types {
                    body.push(Brace::atom(&slot.type_id));
                    body.push(Brace::atom(&slot.value_id));
                    body.push(Brace::num(i64::from(slot.index)));
                }
            }
        }
        serialize_row(&Brace::List(vec![Brace::atom("1"), Brace::List(body)]))
    }

    /// Replaces the section of `class` by its entries plus `added`, in the hash-map order of
    /// `insertion` (the keys of the section in the order the platform filled it in).
    ///
    /// Fails closed when `insertion` is not exactly the keys of the section and the added entries, or
    /// when an added entry has the key of an old one.
    pub fn refill_section(
        &mut self,
        class: &str,
        insertion: &[String],
        added: Vec<Entry>,
    ) -> Result<()> {
        let section = self
            .section_mut(class)
            .with_context(|| format!("2203278d has no section {class}"))?;
        let mut by_object: std::collections::BTreeMap<String, Entry> =
            std::collections::BTreeMap::new();
        for entry in section.entries.drain(..).chain(added) {
            if let Some(clash) = by_object.insert(entry.object.clone(), entry) {
                bail!("2203278d section {class}: {} is there twice", clash.object);
            }
        }
        let order = super::order::iteration_order(insertion.iter().map(String::as_str))?;
        if order.len() != by_object.len() || order.iter().any(|key| !by_object.contains_key(*key)) {
            bail!(
                "2203278d section {class}: the insertion order has {} keys, the section {}",
                order.len(),
                by_object.len()
            );
        }
        section.entries = order
            .into_iter()
            .map(|key| by_object.remove(key).expect("checked above"))
            .collect();
        Ok(())
    }

    pub fn section(&self, class: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.class == class)
    }

    pub fn section_mut(&mut self, class: &str) -> Option<&mut Section> {
        self.sections
            .iter_mut()
            .find(|section| section.class == class)
    }
}
