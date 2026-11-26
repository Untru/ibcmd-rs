//! A semantic diff of two metadata objects, each the XML element tree the
//! metadata model decodes a row into (or that a source tree holds).
//!
//! Children are matched by identity, not by position: a child object by its
//! `uuid`, a named element (`xr:StandardAttribute name="Code"`,
//! `xr:GeneratedType name="CatalogRef.X"`) by its name, anything else by its
//! tag and its place among the siblings of that tag. A change is reported at
//! the shallowest element it can be pinned to: a child that exists on one
//! side only is one `Added`/`Removed` change, not one per leaf.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::metadata_model::xml::Element;

/// One step of the path from the object's element to a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seg {
    /// The element's local name (`Attribute`, `Properties`, `Type`), or
    /// `@type` for an attribute of the element before it.
    pub name: String,
    /// Which child of that name: the object name for a child object, the
    /// `name` attribute for a named element.
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOp {
    Added,
    Removed,
    Modified {
        old: String,
        new: String,
    },
    /// The common children of one tag changed their relative order.
    Reordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: Vec<Seg>,
    pub op: ChangeOp,
}

impl Change {
    /// The element names along the path.
    pub fn names(&self) -> Vec<&str> {
        self.path.iter().map(|seg| seg.name.as_str()).collect()
    }

    /// `ChildObjects/Attribute[Код]/Properties/Type`
    pub fn display_path(&self) -> String {
        self.path
            .iter()
            .map(|seg| match &seg.label {
                Some(label) => format!("{}[{}]", seg.name, label),
                None => seg.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// `9 -> 12`, `added`, `removed`, `reordered`.
    pub fn describe(&self) -> String {
        match &self.op {
            ChangeOp::Added => "added".to_string(),
            ChangeOp::Removed => "removed".to_string(),
            ChangeOp::Reordered => "reordered".to_string(),
            ChangeOp::Modified { old, new } => format!("{} -> {}", shorten(old), shorten(new)),
        }
    }
}

fn shorten(text: &str) -> String {
    const LIMIT: usize = 80;
    let flat = text.replace(['\r', '\n'], " ");
    if flat.chars().count() > LIMIT {
        let head = flat.chars().take(LIMIT).collect::<String>();
        format!("{head:?}...")
    } else if flat.is_empty() {
        "\"\"".to_string()
    } else {
        flat
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Uuid(String, String),
    Named(String, String),
    /// An item of a list of plain values (`<CommonModule>Name</CommonModule>`
    /// among its siblings): the value is its identity, and the number says
    /// which of equal values it is.
    Text(String, String, usize),
    Seq(String, usize),
}

impl Key {
    fn tag(&self) -> &str {
        match self {
            Key::Uuid(tag, _) | Key::Named(tag, _) | Key::Text(tag, _, _) | Key::Seq(tag, _) => tag,
        }
    }

    fn is_identified(&self) -> bool {
        !matches!(self, Key::Seq(..))
    }
}

/// The text of a leaf. A leaf that holds nothing but a line break and
/// indentation is an empty container written the long way
/// (`<ChildObjects>` newline `</ChildObjects>`), which is the same as
/// `<ChildObjects/>`; a value made of blanks alone stays a value.
fn value(text: &str) -> &str {
    if text.contains('\n') && text.trim().is_empty() {
        ""
    } else {
        text
    }
}

/// A child that is nothing but its text (`<CommonModule>Name</CommonModule>`).
fn plain_value(child: &Element) -> bool {
    child.attrs.is_empty() && child.children.is_empty() && !child.text.trim().is_empty()
}

/// The tags whose plain-value children form a list on either side of a
/// comparison. Both sides are asked, so that a list that shrinks to one item
/// is still compared by value.
fn list_tags<'a>(old: &'a Element, new: &'a Element) -> HashSet<&'a str> {
    let mut lists = HashSet::new();
    for parent in [old, new] {
        let mut counts = HashMap::<&str, usize>::new();
        for child in parent.children.iter().filter(|child| plain_value(child)) {
            *counts.entry(child.name.as_str()).or_default() += 1;
        }
        lists.extend(
            counts
                .into_iter()
                .filter(|(_, n)| *n >= 2)
                .map(|(tag, _)| tag),
        );
    }
    lists
}

fn keyed_children<'a>(parent: &'a Element, lists: &HashSet<&str>) -> Vec<(Key, &'a Element)> {
    let mut counters = HashMap::<&str, usize>::new();
    let mut values = HashMap::<(&str, &str), usize>::new();
    let mut out = Vec::with_capacity(parent.children.len());
    for child in &parent.children {
        let tag = child.name.as_str();
        let key = if let Some(uuid) = child.attr("uuid") {
            Key::Uuid(tag.to_string(), uuid.to_ascii_lowercase())
        } else if let Some(name) = child.attr("name") {
            Key::Named(tag.to_string(), name.to_string())
        } else if lists.contains(tag) && plain_value(child) {
            // One of a list of plain values: an item added in the middle is
            // one change, not a shift of every item after it.
            let number = values.entry((tag, child.text.as_str())).or_default();
            let key = Key::Text(tag.to_string(), child.text.clone(), *number);
            *number += 1;
            key
        } else {
            let index = counters.entry(tag).or_default();
            let key = Key::Seq(tag.to_string(), *index);
            *index += 1;
            key
        };
        out.push((key, child));
    }
    out
}

fn label_of(key: &Key, child: &Element) -> Option<String> {
    match key {
        Key::Uuid(_, uuid) => Some(
            child
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| uuid.clone()),
        ),
        Key::Named(_, name) => Some(name.clone()),
        Key::Text(_, text, _) => Some(text.trim().to_string()),
        Key::Seq(..) => None,
    }
}

/// Every difference between two objects' element trees (`old` and `new` are
/// the object elements, `<Catalog uuid=...>`).
pub fn diff(old: &Element, new: &Element) -> Vec<Change> {
    let mut out = Vec::new();
    let mut path = Vec::new();
    diff_element(old, new, &mut path, &mut out);
    out
}

fn diff_element(old: &Element, new: &Element, path: &mut Vec<Seg>, out: &mut Vec<Change>) {
    // The element's own attributes.
    let mut attrs = BTreeMap::<&str, (Option<&str>, Option<&str>)>::new();
    for (key, value) in &old.attrs {
        attrs.entry(key).or_default().0 = Some(value);
    }
    for (key, value) in &new.attrs {
        attrs.entry(key).or_default().1 = Some(value);
    }
    for (key, (before, after)) in attrs {
        if before != after {
            let mut at = path.clone();
            at.push(Seg {
                name: format!("@{key}"),
                label: None,
            });
            out.push(Change {
                path: at,
                op: ChangeOp::Modified {
                    old: before.unwrap_or_default().to_string(),
                    new: after.unwrap_or_default().to_string(),
                },
            });
        }
    }
    // Text: only a leaf's counts (an element with children has layout
    // whitespace at most).
    if old.children.is_empty() && new.children.is_empty() && value(&old.text) != value(&new.text) {
        out.push(Change {
            path: path.clone(),
            op: ChangeOp::Modified {
                old: old.text.clone(),
                new: new.text.clone(),
            },
        });
    }
    if old.children.is_empty() && new.children.is_empty() {
        return;
    }

    let lists = list_tags(old, new);
    let old_children = keyed_children(old, &lists);
    let new_children = keyed_children(new, &lists);
    let new_index = new_children
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (key, index))
        .collect::<HashMap<_, _>>();
    let old_index = old_children
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (key, index))
        .collect::<HashMap<_, _>>();

    for (key, child) in &old_children {
        let seg = Seg {
            name: child.name.clone(),
            label: label_of(key, child),
        };
        path.push(seg);
        match new_index.get(key) {
            Some(&index) => diff_element(child, new_children[index].1, path, out),
            None => out.push(Change {
                path: path.clone(),
                op: ChangeOp::Removed,
            }),
        }
        path.pop();
    }
    for (key, child) in &new_children {
        if old_index.contains_key(key) {
            continue;
        }
        path.push(Seg {
            name: child.name.clone(),
            label: label_of(key, child),
        });
        out.push(Change {
            path: path.clone(),
            op: ChangeOp::Added,
        });
        path.pop();
    }

    // The order of the identified children that both sides have, by tag.
    let mut tags = Vec::<&str>::new();
    for (key, _) in &old_children {
        if key.is_identified() && !tags.contains(&key.tag()) {
            tags.push(key.tag());
        }
    }
    for tag in tags {
        let common = |children: &[(Key, &Element)], other: &HashMap<&Key, usize>| {
            children
                .iter()
                .filter(|(key, _)| {
                    key.tag() == tag && key.is_identified() && other.contains_key(key)
                })
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>()
        };
        if common(&old_children, &new_index) != common(&new_children, &old_index) {
            let mut at = path.clone();
            at.push(Seg {
                name: tag.to_string(),
                label: None,
            });
            out.push(Change {
                path: at,
                op: ChangeOp::Reordered,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::xml::parse_element_tree;

    fn tree(xml: &str) -> Element {
        parse_element_tree(xml.as_bytes()).unwrap()
    }

    fn changes(old: &str, new: &str) -> Vec<String> {
        diff(&tree(old), &tree(new))
            .into_iter()
            .map(|change| format!("{}: {}", change.display_path(), change.describe()))
            .collect()
    }

    const CATALOG: &str = r#"<Catalog uuid="11111111-1111-1111-1111-111111111111">
        <Properties><Name>C</Name><CodeLength>9</CodeLength><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Кассы</v8:content></v8:item></Synonym></Properties>
        <ChildObjects>
          <Attribute uuid="22222222-2222-2222-2222-222222222222"><Properties><Name>Код</Name><Indexing>DontIndex</Indexing></Properties></Attribute>
          <Attribute uuid="33333333-3333-3333-3333-333333333333"><Properties><Name>Имя</Name></Properties></Attribute>
          <Form>ФормаЭлемента</Form>
        </ChildObjects>
      </Catalog>"#;

    #[test]
    fn identical_trees_have_no_changes() {
        assert!(changes(CATALOG, CATALOG).is_empty());
    }

    #[test]
    fn an_empty_container_written_the_long_way_is_the_empty_one() {
        let short = "<Catalog uuid=\"a\"><Properties/><ChildObjects/></Catalog>";
        let long =
            "<Catalog uuid=\"a\"><Properties/><ChildObjects>\r\n\t\t</ChildObjects></Catalog>";
        assert!(changes(short, long).is_empty());
        assert!(changes(long, short).is_empty());
        // Blanks alone, on one line, are a value.
        let blank = "<Catalog uuid=\"a\"><Properties><Comment> </Comment></Properties></Catalog>";
        let empty = "<Catalog uuid=\"a\"><Properties><Comment/></Properties></Catalog>";
        assert_eq!(changes(empty, blank).len(), 1);
    }

    #[test]
    fn a_leaf_change_is_pinned_to_its_element() {
        let new = CATALOG.replace("<CodeLength>9</CodeLength>", "<CodeLength>12</CodeLength>");
        assert_eq!(
            changes(CATALOG, &new),
            vec!["Properties/CodeLength: 9 -> 12"]
        );
    }

    #[test]
    fn a_child_object_is_one_change_and_is_named_by_its_object_name() {
        let new = CATALOG.replace(
            "<Form>ФормаЭлемента</Form>",
            r#"<Attribute uuid="44444444-4444-4444-4444-444444444444"><Properties><Name>Цена</Name></Properties></Attribute><Form>ФормаЭлемента</Form>"#,
        );
        assert_eq!(
            changes(CATALOG, &new),
            vec!["ChildObjects/Attribute[Цена]: added"]
        );
        assert_eq!(
            changes(&new, CATALOG),
            vec!["ChildObjects/Attribute[Цена]: removed"]
        );
    }

    #[test]
    fn a_property_of_a_child_object_is_reported_inside_it() {
        let new = CATALOG.replace(
            "<Indexing>DontIndex</Indexing>",
            "<Indexing>Index</Indexing>",
        );
        assert_eq!(
            changes(CATALOG, &new),
            vec!["ChildObjects/Attribute[Код]/Properties/Indexing: DontIndex -> Index"]
        );
    }

    #[test]
    fn a_renamed_child_object_is_a_change_of_its_name_not_a_new_child() {
        let new = CATALOG.replace("<Name>Имя</Name>", "<Name>Наименование</Name>");
        assert_eq!(
            changes(CATALOG, &new),
            vec!["ChildObjects/Attribute[Имя]/Properties/Name: Имя -> Наименование"]
        );
    }

    #[test]
    fn a_reordered_collection_is_one_change() {
        let swapped = r#"<Catalog uuid="11111111-1111-1111-1111-111111111111">
        <Properties><Name>C</Name><CodeLength>9</CodeLength><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Кассы</v8:content></v8:item></Synonym></Properties>
        <ChildObjects>
          <Attribute uuid="33333333-3333-3333-3333-333333333333"><Properties><Name>Имя</Name></Properties></Attribute>
          <Attribute uuid="22222222-2222-2222-2222-222222222222"><Properties><Name>Код</Name><Indexing>DontIndex</Indexing></Properties></Attribute>
          <Form>ФормаЭлемента</Form>
        </ChildObjects>
      </Catalog>"#;
        assert_eq!(
            changes(CATALOG, swapped),
            vec!["ChildObjects/Attribute: reordered"]
        );
    }

    #[test]
    fn a_localized_string_is_compared_item_by_item() {
        let new = CATALOG.replace("Кассы", "Кассы (2)");
        assert_eq!(
            changes(CATALOG, &new),
            vec!["Properties/Synonym/item/content: Кассы -> Кассы (2)"]
        );
    }

    #[test]
    fn a_list_of_plain_values_is_compared_by_value() {
        let old = "<X><Properties><Owners><Item>A</Item><Item>B</Item></Owners></Properties></X>";
        let new = "<X><Properties><Owners><Item>A</Item></Owners></Properties></X>";
        assert_eq!(
            changes(old, new),
            vec!["Properties/Owners/Item[B]: removed"]
        );
        assert_eq!(changes(new, old), vec!["Properties/Owners/Item[B]: added"]);

        // An item in the middle of a long list is one change, not a shift of
        // every item after it.
        let long = "<X><ChildObjects><Module>A</Module><Module>C</Module><Module>D</Module></ChildObjects></X>";
        let longer = "<X><ChildObjects><Module>A</Module><Module>B</Module><Module>C</Module><Module>D</Module></ChildObjects></X>";
        assert_eq!(changes(long, longer), vec!["ChildObjects/Module[B]: added"]);
        assert_eq!(
            changes(longer, long),
            vec!["ChildObjects/Module[B]: removed"]
        );

        // Reordered values are one change; equal values are told apart by number.
        let swapped = "<X><ChildObjects><Module>A</Module><Module>C</Module><Module>B</Module><Module>D</Module></ChildObjects></X>";
        assert_eq!(
            changes(longer, swapped),
            vec!["ChildObjects/Module: reordered"]
        );
        let twice = "<X><P><V>A</V><V>A</V></P></X>";
        let once = "<X><P><V>A</V></P></X>";
        assert_eq!(changes(twice, once), vec!["P/V[A]: removed"]);
    }

    #[test]
    fn a_property_that_is_alone_is_modified_not_replaced() {
        // A single leaf is a property: its value changes, it is not removed and added.
        let old = "<X><Properties><CodeLength>9</CodeLength></Properties></X>";
        let new = "<X><Properties><CodeLength>12</CodeLength></Properties></X>";
        assert_eq!(changes(old, new), vec!["Properties/CodeLength: 9 -> 12"]);
    }

    #[test]
    fn a_changed_attribute_of_an_element_is_reported_under_the_element() {
        let old = r#"<X><Properties><FillValue type="xs:string"/></Properties></X>"#;
        let new = r#"<X><Properties><FillValue type="xs:boolean"/></Properties></X>"#;
        assert_eq!(
            changes(old, new),
            vec!["Properties/FillValue/@type: xs:string -> xs:boolean"]
        );
    }

    #[test]
    fn named_elements_are_matched_by_their_name() {
        let old = r#"<X><Properties><StandardAttributes>
            <StandardAttribute name="Code"><FillChecking>DontCheck</FillChecking></StandardAttribute>
            <StandardAttribute name="Ref"><FillChecking>DontCheck</FillChecking></StandardAttribute>
        </StandardAttributes></Properties></X>"#;
        let new = old.replace(
            r#"<StandardAttribute name="Ref"><FillChecking>DontCheck"#,
            r#"<StandardAttribute name="Ref"><FillChecking>ShowError"#,
        );
        assert_eq!(
            changes(old, &new),
            vec![
                "Properties/StandardAttributes/StandardAttribute[Ref]/FillChecking: DontCheck -> ShowError"
            ]
        );
    }
}
