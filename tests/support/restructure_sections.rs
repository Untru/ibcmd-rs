//! Shared fixture helpers for the planner and its root apply gate tests.

use crate::restructure::names::{deflate, inflate};
use crate::restructure::plan::Inputs;
use crate::restructure::tests_plan::{OLD_ROW, inputs};
use ibcmd_restructure::brace::{Brace, parse_row, serialize_row};
use std::collections::HashMap;

pub(super) const SECTIONS: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";

pub(super) const SECTION_ATTRIBUTES: &str = "888744e1-b616-11d4-9436-004095e12fc7";

pub(super) fn tree() -> Brace {
    parse_row(&inflate(OLD_ROW).unwrap()).unwrap()
}

pub(super) fn stage(tree: &Brace) -> Vec<u8> {
    deflate(&serialize_row(tree)).unwrap()
}

/// The items of the collection of `class` in the descriptor (`{<class>,<count>,<item>...}`).
pub(super) fn collection<'a>(root: &'a mut Brace, class: &str) -> &'a mut Vec<Brace> {
    root.as_list_mut()
        .unwrap()
        .iter_mut()
        .filter_map(Brace::as_list_mut)
        .find(|items| items.first().and_then(Brace::as_atom) == Some(class))
        .unwrap()
}

pub(super) fn recount(items: &mut [Brace]) {
    items[1] = Brace::num(items.len() as i64 - 2);
}

pub(super) fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(at, c)| {
            matches!(at, 8 | 13 | 18 | 23) == (c == '-') && (c == '-' || c.is_ascii_hexdigit())
        })
        && text != "00000000-0000-0000-0000-000000000000"
}

/// A copy of `node` with every uuid replaced by another (the same one everywhere in the copy) and, when
/// `primitive`, every type pattern by a string of 10 characters.
pub(super) fn fresh(node: &Brace, seed: u32, primitive: bool) -> Brace {
    fn walk(node: &mut Brace, seed: u32, map: &mut HashMap<String, String>, primitive: bool) {
        match node {
            // the class of a collection is a uuid too, and stays
            Brace::Atom(text) if is_uuid(text) && text != SECTION_ATTRIBUTES => {
                let next = map.len() as u32;
                let mapped = map
                    .entry(text.clone())
                    .or_insert_with(|| format!("e{seed:03x}{next:04x}-{}", &text[9..]));
                *text = mapped.clone();
            }
            Brace::List(items) => {
                if primitive && items.first().and_then(Brace::as_str) == Some("Pattern") {
                    *node = parse_row(b"{\"Pattern\",{\"S\",10,1}}").unwrap();
                    return;
                }
                for item in items {
                    walk(item, seed, map, primitive);
                }
            }
            _ => {}
        }
    }
    let mut copy = node.clone();
    walk(&mut copy, seed, &mut HashMap::new(), primitive);
    copy
}

pub(super) fn rename(node: &mut Brace, from: &str, to: &str) {
    match node {
        Brace::Str(text) if text == from => *text = to.to_owned(),
        Brace::List(items) => items.iter_mut().for_each(|item| rename(item, from, to)),
        _ => {}
    }
}

/// The attribute items of a section item: `{<wrapper>,1,{888744e1-...,<n>,{{<record>},0}...}}`.
pub(super) fn attributes_of(section: &mut Brace) -> &mut Vec<Brace> {
    section
        .as_list_mut()
        .unwrap()
        .iter_mut()
        .filter_map(Brace::as_list_mut)
        .find(|items| items.first().and_then(Brace::as_atom) == Some(SECTION_ATTRIBUTES))
        .unwrap()
}

pub(super) fn staged(tree: &Brace) -> Inputs {
    inputs(OLD_ROW, &stage(tree))
}

/// The row with a copy of its first section (3 attributes) added as the last one, primitive types only.
pub(super) fn with_new_section() -> Brace {
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    let mut copy = fresh(&sections[2], 1, true);
    rename(&mut copy, "ДополнительныеРеквизиты", "НоваяТЧ");
    sections.push(copy);
    recount(sections);
    root
}

/// `Indexing` of an attribute item `{{v,<body>,<Indexing>,...},0}`.
pub(super) fn set_indexing(attribute: &mut Brace, indexing: i64) {
    attribute.as_list_mut().unwrap()[0].as_list_mut().unwrap()[2] = Brace::num(indexing);
}
