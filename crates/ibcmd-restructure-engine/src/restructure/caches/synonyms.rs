//! `facbfffe-feb2-4d30-8930-a557b185e5c4.si`: the presentations of the objects, by property.
//!
//! The row is `{0,{<n>,<key>,<value>,...},...}`, eight sections in a fixed order, a section a hash map
//! from an object's uuid to the localized string of **one property** (`{1,<languages>,{"ru","text"}...}`),
//! only for the objects where the property is not empty. Measured sections (section -> property):
//! 1 `ListPresentation`, 2 `ExtendedListPresentation`, 3 `ObjectPresentation`,
//! 4 `ExtendedObjectPresentation`, 7 `Explanation`; 0, 5 and 6 are properties of constants and
//! registers. A section holds the objects of several kinds, filled kind by kind in the platform's
//! kind order ([`CLASS_ORDER`]) and each kind in the order of its root collection.
//!
//! So a text like the "synonym" of a new catalog is **not** what a section 3 entry holds -- it is the
//! object presentation, the *second* localized string of the row (measured: the new catalog of case
//! c has the synonym "Демо: Новый справочник" and the presentation "(не используется) Демо: Общее
//! сведение" copied from the catalog it was made from, and section 3 holds the latter).

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::restructure::caches::order::iteration_order;
use crate::restructure::caches::root::Collection;

/// The property a section holds, for the reference objects.
pub const PROPERTY_SECTIONS: &[(&str, usize)] = &[
    ("ListPresentation", 1),
    ("ExtendedListPresentation", 2),
    ("ObjectPresentation", 3),
    ("ExtendedObjectPresentation", 4),
    ("Explanation", 7),
];

/// The kinds of the root in the order the presentations are filled in. Measured as a partial order:
/// ExchangePlan < Catalog < Document < DocumentJournal < InformationRegister < ChartOfAccounts,
/// ChartOfCharacteristicTypes < ChartOfAccounts, Constant < Report < DataProcessor (sections 0-6 of
/// the БСП corpus are reproduced exactly with this list; section 7 is not -- 65 entries of 11 kinds
/// and six keys that are not in the root).
pub const CLASS_ORDER: &[&str] = &[
    "ExchangePlan",
    "Constant",
    "Catalog",
    "Document",
    "DocumentJournal",
    "Enum",
    "Report",
    "DataProcessor",
    "InformationRegister",
    "ChartOfCharacteristicTypes",
    "ChartOfAccounts",
    "ChartOfCalculationTypes",
    "AccumulationRegister",
    "AccountingRegister",
    "CalculationRegister",
    "BusinessProcess",
    "Task",
    "ExternalDataSource",
];

/// One section: key and the value's node, in row order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub entries: Vec<(String, Brace)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Synonyms {
    pub sections: Vec<Section>,
}

/// The keys of a section in the order the platform fills it in: kind by kind ([`CLASS_ORDER`]), each in
/// the order of its root collection. Fails when a key is not an object of a known kind of the root.
pub fn insertion_order<'a>(
    keys: &'a std::collections::BTreeSet<String>,
    collections: &'a [Collection],
    kind_of_class: &dyn Fn(&str) -> Option<&'static str>,
) -> Result<Vec<&'a str>> {
    let mut insertion: Vec<&str> = Vec::new();
    let mut placed = std::collections::BTreeSet::new();
    for kind in CLASS_ORDER {
        for collection in collections {
            if kind_of_class(&collection.class) != Some(kind) {
                continue;
            }
            for member in &collection.objects {
                if keys.contains(member) {
                    insertion.push(member);
                    placed.insert(member.as_str());
                }
            }
        }
    }
    if let Some(stray) = keys.iter().find(|key| !placed.contains(key.as_str())) {
        bail!("{stray} is not an object of a known kind of the root");
    }
    Ok(insertion)
}

impl Synonyms {
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("facbfffe is not brace text")?;
        let outer = tree.as_list().context("facbfffe is not a list")?;
        let Some((zero, sections)) = outer.split_first() else {
            bail!("facbfffe is empty");
        };
        if zero.as_atom() != Some("0") {
            bail!("facbfffe has version {zero:?}");
        }
        let mut out = Vec::with_capacity(sections.len());
        for section in sections {
            let items = section
                .as_list()
                .context("facbfffe holds a non-list section")?;
            let (count, rest) = items
                .split_first()
                .context("facbfffe holds an empty section")?;
            let count: usize = count
                .as_atom()
                .and_then(|count| count.parse().ok())
                .context("facbfffe section has no count")?;
            if rest.len() != 2 * count {
                bail!(
                    "facbfffe section counts {count} entries but has {} items",
                    rest.len()
                );
            }
            let entries = rest
                .chunks(2)
                .map(|pair| {
                    Ok((
                        pair[0]
                            .as_atom()
                            .context("facbfffe key is not a plain token")?
                            .to_owned(),
                        pair[1].clone(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            out.push(Section { entries });
        }
        Ok(Self { sections: out })
    }

    pub fn render(&self) -> Vec<u8> {
        let mut items = vec![Brace::atom("0")];
        for section in &self.sections {
            let mut list = vec![Brace::num(section.entries.len() as i64)];
            for (key, value) in &section.entries {
                list.push(Brace::atom(key));
                list.push(value.clone());
            }
            items.push(Brace::List(list));
        }
        serialize_row(&Brace::List(items))
    }

    /// `{1,<n>,{"ru","text"},...}` for the pairs of a localized string.
    pub fn value(pairs: &[(String, String)]) -> Brace {
        let mut items = vec![Brace::num(1), Brace::num(pairs.len() as i64)];
        items.extend(
            pairs
                .iter()
                .map(|(language, text)| Brace::List(vec![Brace::str(language), Brace::str(text)])),
        );
        Brace::List(items)
    }

    /// Adds the presentation `pairs` of `object` to the section of `property`.
    ///
    /// `collections` is the root **after** the change; `kind_of_class` says which kind a root
    /// collection is (by class uuid). The section is rebuilt in the platform's order from its own
    /// keys plus the new one.
    pub fn add_object(
        &mut self,
        property: &str,
        object: &str,
        pairs: &[(String, String)],
        collections: &[Collection],
        kind_of_class: &dyn Fn(&str) -> Option<&'static str>,
    ) -> Result<()> {
        let &(_, index) = PROPERTY_SECTIONS
            .iter()
            .find(|(name, _)| *name == property)
            .with_context(|| format!("facbfffe has no section for {property}"))?;
        let section = self
            .sections
            .get(index)
            .with_context(|| format!("facbfffe has no section {index}"))?;
        if index == 7 {
            bail!(
                "facbfffe section 7 (Explanation) mixes kinds and keys that the root does not list; its order is not decoded"
            );
        }
        let mut keys: std::collections::BTreeSet<String> =
            section.entries.iter().map(|(key, _)| key.clone()).collect();
        if !keys.insert(object.to_owned()) {
            bail!("facbfffe section {index} has {object} already");
        }
        let mut values: std::collections::BTreeMap<String, Brace> =
            section.entries.iter().cloned().collect();
        values.insert(object.to_owned(), Self::value(pairs));

        let insertion = insertion_order(&keys, collections, kind_of_class)
            .with_context(|| format!("facbfffe section {index}"))?;
        let order = iteration_order(insertion.iter().copied())?;
        self.sections[index].entries = order
            .into_iter()
            .map(|key| (key.to_owned(), values[key].clone()))
            .collect();
        Ok(())
    }
}
