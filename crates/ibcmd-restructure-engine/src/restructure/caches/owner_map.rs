//! `42ed49cc-765d-4314-bc2d-af425af7bf13.si`: which objects a catalog is subordinate to.
//!
//! The row is `{0,{<n>,<entries>},{<n>,<entries>},{<n>,<entries>}}`: three hash maps. The one of the
//! catalogs is `<catalog>,<count>,<owner>...` for every catalog in the configuration (a catalog without
//! owners is `<catalog>,0`), filled in the root's order of the catalogs. The other two (the registers'
//! dimensions and a two-level map of the composite types) do not change when a catalog is added; the
//! first one's insertion order is not the root's (7 entries, not decoded), so the section is found by
//! its keys and the others are kept as they are.

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::restructure::caches::order::iteration_order;

/// One `<key>,<n>,<owner>...` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub object: String,
    pub owners: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerMap {
    /// The three sections as parsed lists, in row order.
    sections: Vec<Vec<Brace>>,
}

impl OwnerMap {
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("42ed49cc is not brace text")?;
        let outer = tree.as_list().context("42ed49cc is not a list")?;
        let Some((zero, sections)) = outer.split_first() else {
            bail!("42ed49cc is empty");
        };
        if zero.as_atom() != Some("0") {
            bail!("42ed49cc has version {zero:?}");
        }
        let sections = sections
            .iter()
            .map(|section| {
                section
                    .as_list()
                    .map(<[Brace]>::to_vec)
                    .context("42ed49cc holds a non-list section")
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { sections })
    }

    pub fn render(&self) -> Vec<u8> {
        let mut items = vec![Brace::atom("0")];
        items.extend(self.sections.iter().cloned().map(Brace::List));
        serialize_row(&Brace::List(items))
    }

    /// The entries of a one-level section (`key,n,owner...`), or `None` when the section is not of
    /// that shape.
    fn flat_entries(section: &[Brace]) -> Option<Vec<Entry>> {
        let (count, mut rest) = section.split_first()?;
        let count: usize = count.as_atom()?.parse().ok()?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let [key, n, tail @ ..] = rest else {
                return None;
            };
            let n: usize = n.as_atom()?.parse().ok()?;
            if tail.len() < n {
                return None;
            }
            let owners = tail[..n]
                .iter()
                .map(|owner| owner.as_atom().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?;
            entries.push(Entry {
                object: key.as_atom()?.to_owned(),
                owners,
            });
            rest = &tail[n..];
        }
        rest.is_empty().then_some(entries)
    }

    fn render_flat(entries: &[Entry]) -> Vec<Brace> {
        let mut section = vec![Brace::num(entries.len() as i64)];
        for entry in entries {
            section.push(Brace::atom(&entry.object));
            section.push(Brace::num(entry.owners.len() as i64));
            section.extend(entry.owners.iter().map(Brace::atom));
        }
        section
    }

    /// Adds a catalog. `catalogs` are the objects of the root's catalog collection **after** the
    /// change, in the root's order (the new one included); `owners` are the uuids the new catalog is
    /// subordinate to.
    pub fn add_catalog(
        &mut self,
        catalogs: &[String],
        object: &str,
        owners: &[String],
    ) -> Result<()> {
        let old: std::collections::BTreeSet<&str> = catalogs
            .iter()
            .map(String::as_str)
            .filter(|catalog| *catalog != object)
            .collect();
        let mut target = None;
        for (index, section) in self.sections.iter().enumerate() {
            let Some(entries) = Self::flat_entries(section) else {
                continue;
            };
            let keys: std::collections::BTreeSet<&str> =
                entries.iter().map(|entry| entry.object.as_str()).collect();
            if !entries.is_empty() && keys == old {
                if target.is_some() {
                    bail!("42ed49cc has two sections with the keys of the catalogs");
                }
                target = Some((index, entries));
            }
        }
        let (index, mut entries) = target
            .context("42ed49cc has no section with the keys of the catalogs (a stale row?)")?;
        entries.push(Entry {
            object: object.to_owned(),
            owners: owners.to_vec(),
        });
        let order = iteration_order(catalogs.iter().map(String::as_str))?;
        let by_object: std::collections::BTreeMap<&str, &Entry> = entries
            .iter()
            .map(|entry| (entry.object.as_str(), entry))
            .collect();
        let ordered: Vec<Entry> = order
            .iter()
            .map(|key| {
                by_object
                    .get(key)
                    .map(|entry| (*entry).clone())
                    .with_context(|| format!("42ed49cc has no entry for {key}"))
            })
            .collect::<Result<_>>()?;
        self.sections[index] = Self::render_flat(&ordered);
        Ok(())
    }
}
