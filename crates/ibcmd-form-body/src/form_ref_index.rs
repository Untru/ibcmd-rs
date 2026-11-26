//! Reverse lookups over the object-reference index a form is parsed against.
//!
//! A dynamic list's field universe asks that index three questions its keys
//! do not answer: whether a reference exists, which children a table declares
//! and which common attributes there are. Walking every value for each of them
//! cost a pass over the whole index (ERP УХ: about 190 000 references) several
//! times per dynamic-list attribute -- most of the form converter's time.
//! Built once per export, each answer is a lookup. It stands only for the very
//! map it was built from; for any other map the readers walk the map as
//! before.

use std::collections::{BTreeMap, HashMap, HashSet};

pub struct FormObjectRefIndex<'a> {
    source: &'a BTreeMap<String, String>,
    references: HashSet<&'a str>,
    /// `<owner>.<kind>.<name>` references, `kind` and `name` without a dot,
    /// by owner, in the map's key order: `(kind, name, reference)`.
    children: HashMap<&'a str, Vec<(&'a str, &'a str, &'a str)>>,
    /// `<name>` of every `CommonAttribute.<name>` reference without a further
    /// dot, in the map's key order.
    common_attributes: Vec<&'a str>,
}

impl<'a> FormObjectRefIndex<'a> {
    pub fn new(source: &'a BTreeMap<String, String>) -> Self {
        let mut references = HashSet::with_capacity(source.len());
        let mut children = HashMap::<&str, Vec<(&str, &str, &str)>>::new();
        let mut common_attributes = Vec::new();
        for reference in source.values() {
            let reference = reference.as_str();
            references.insert(reference);
            if let Some((rest, name)) = reference.rsplit_once('.')
                && let Some((owner, kind)) = rest.rsplit_once('.')
            {
                children
                    .entry(owner)
                    .or_default()
                    .push((kind, name, reference));
            }
            if let Some(name) = reference.strip_prefix("CommonAttribute.")
                && !name.is_empty()
                && !name.contains('.')
            {
                common_attributes.push(name);
            }
        }
        Self {
            source,
            references,
            children,
            common_attributes,
        }
    }

    /// `index` when it was built from `object_refs` itself.
    pub fn of<'b>(
        index: Option<&'b FormObjectRefIndex<'a>>,
        object_refs: &BTreeMap<String, String>,
    ) -> Option<&'b FormObjectRefIndex<'a>> {
        index.filter(|index| std::ptr::eq(index.source, object_refs))
    }

    /// Whether some key maps to exactly `reference`.
    pub fn contains(&self, reference: &str) -> bool {
        self.references.contains(reference)
    }

    /// The `<owner>.<kind>.<name>` references of `owner`, in key order.
    pub fn children(&self, owner: &str) -> &[(&'a str, &'a str, &'a str)] {
        self.children.get(owner).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Common attribute names, in key order.
    pub fn common_attributes(&self) -> &[&'a str] {
        &self.common_attributes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_what_a_walk_over_the_values_answers() {
        let map = [
            ("1", "Catalog.X"),
            ("2", "Catalog.X.Attribute.A"),
            ("3", "Catalog.X.TabularSection.T"),
            ("4", "Catalog.X.TabularSection.T.Attribute.B"),
            ("5", "CommonAttribute.Область"),
            ("6", "CommonAttribute.Нет.Дальше"),
            ("7", "owner-value:Catalog.X:u"),
            ("0", "Catalog.X.Attribute.A"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
        let index = FormObjectRefIndex::new(&map);
        assert!(index.contains("Catalog.X"));
        assert!(!index.contains("Catalog.Y"));
        let children = index
            .children("Catalog.X")
            .iter()
            .map(|(kind, name, _)| format!("{kind}.{name}"))
            .collect::<Vec<_>>();
        assert_eq!(children, ["Attribute.A", "Attribute.A", "TabularSection.T"]);
        assert_eq!(
            index.children("Catalog.X.TabularSection.T")[0].2,
            "Catalog.X.TabularSection.T.Attribute.B"
        );
        assert_eq!(index.common_attributes(), ["Область"]);
        assert!(FormObjectRefIndex::of(Some(&index), &map).is_some());
        let other = map.clone();
        assert!(FormObjectRefIndex::of(Some(&index), &other).is_none());
    }
}
