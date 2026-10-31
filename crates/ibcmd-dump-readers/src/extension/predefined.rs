//! `<ExtensionState>` of the predefined items of an adopted object, which
//! 8.3.27.2214 prints last in each item (fixture `adopted/predefined`; a real
//! extension's three catalogs).
//!
//! The items row (`<uuid>.1c`) names its columns `{2,N,(column id,value
//! offset)×N,…}`; an object whose predefined items the extension extends has
//! a column 6 whose number is the state: `0` an item of the extension
//! (`Native`), `2` an adopted one (`AdoptedCheck`). An object that only
//! controls them has no column 6, and every item is `AdoptedCheck`.

use std::collections::BTreeMap;

use anyhow::{Result, bail};

use crate::external::brace;

const STATE_COLUMN: &str = "6";

/// Item id → state, from the items row `text`.
pub fn states(text: &str) -> Result<BTreeMap<String, &'static str>> {
    let offset = state_offset(text);
    let mut out = BTreeMap::new();
    let mut search = 0;
    while let Some(relative) = text[search..].find("{2,") {
        let start = search + relative;
        search = start + 1;
        let Some((fields, _)) = brace::fields(text, start) else {
            continue;
        };
        let Some(id) = item_id(&fields) else {
            continue;
        };
        let state = match offset {
            None => "AdoptedCheck",
            Some(offset) => {
                let count: usize = fields
                    .get(2)
                    .and_then(|count| count.parse().ok())
                    .unwrap_or(0);
                let value = (offset < count).then(|| fields.get(3 + offset)).flatten();
                match value.map(|value| value.split_whitespace().collect::<String>()) {
                    Some(value) if value == "{\"N\",0}" => "Native",
                    Some(value) if value == "{\"N\",2}" => "AdoptedCheck",
                    other => bail!("predefined item {id}: extension state {other:?} is not known"),
                }
            }
        };
        out.insert(id, state);
    }
    Ok(out)
}

/// The item's id: its first value `{"#",<class>,{1,<id>}}`, the nil id (the
/// root) excluded.
fn item_id(fields: &[&str]) -> Option<String> {
    if fields.first() != Some(&"2") {
        return None;
    }
    let (reference, _) = brace::fields(fields.get(3)?, 0)?;
    if reference.first() != Some(&"\"#\"") {
        return None;
    }
    let (target, _) = brace::fields(reference.get(2)?, 0)?;
    let id = target.get(1)?;
    (target.first() == Some(&"1") && id.len() == 36 && *id != super::adoption::NIL_UUID)
        .then(|| (*id).to_owned())
}

/// The value offset of the state column, when the row has one.
fn state_offset(text: &str) -> Option<usize> {
    let mut search = 0;
    while let Some(relative) = text[search..].find("{2,") {
        let start = search + relative;
        search = start + 1;
        let Some((fields, _)) = brace::fields(text, start) else {
            continue;
        };
        let Some(count) = fields
            .get(1)
            .and_then(|count| count.parse::<usize>().ok())
            .filter(|count| *count <= fields.len())
        else {
            continue;
        };
        // `{2,N,pairs,{items},-1,2}` (the tail after the items varies).
        let Some(pairs) = fields.get(2..2 + 2 * count) else {
            continue;
        };
        if count == 0
            || fields.len() < 3 + 2 * count
            || !pairs.iter().all(|value| value.parse::<i64>().is_ok())
            || !fields[2 + 2 * count].starts_with('{')
        {
            continue;
        }
        return pairs
            .chunks_exact(2)
            .find(|pair| pair[0] == STATE_COLUMN)
            .and_then(|pair| pair[1].parse().ok());
    }
    None
}

/// `xml` (the pipeline's `Predefined.xml`) with each listed item's state
/// after its own properties, before its `<ChildItems>`.
pub fn rewrite(xml: &str, states: &BTreeMap<String, &'static str>) -> Result<String> {
    let mut out = String::with_capacity(xml.len() + 64 * states.len());
    let mut open: Vec<(String, Option<&'static str>)> = Vec::new();
    for line in xml.split_inclusive('\n') {
        let trimmed = line.trim_start_matches('\t');
        let indent = &line[..line.len() - trimmed.len()];
        if let Some(rest) = trimmed.strip_prefix("<Item id=\"") {
            let id = rest.split('"').next().unwrap_or_default();
            open.push((indent.to_owned(), states.get(id).copied()));
        } else if trimmed.starts_with("<ChildItems>") || trimmed.starts_with("</Item>") {
            let own = open.last().is_some_and(|(item_indent, _)| {
                if trimmed.starts_with("</Item>") {
                    item_indent == indent
                } else {
                    indent.len() == item_indent.len() + 1
                }
            });
            if own && let Some((item_indent, state)) = open.last_mut() {
                if let Some(state) = state.take() {
                    out.push_str(&format!(
                        "{item_indent}\t<ExtensionState>{state}</ExtensionState>\r\n"
                    ));
                }
                if trimmed.starts_with("</Item>") {
                    open.pop();
                }
            }
        }
        out.push_str(line);
    }
    if !open.is_empty() {
        bail!("unbalanced predefined items");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_beyond_the_fields_is_no_state_column() {
        assert_eq!(
            state_offset("{2,18446744073709551615,1,2,{1,1,x},-1,2}"),
            None
        );
    }

    #[test]
    fn reads_the_state_column_or_its_absence() {
        // Fixture `adopted/predefined`, shortened: columns, then
        // `{2,7,pairs,{1,1,root},-1,2}`; the root lists two items.
        let item = |order: u8, id: &str, state: u8| {
            format!(
                "{{2,{order},7,{{\"#\",t,{{1,{id}}}}},{{\"B\",0}},x,{{\"S\",\"a\"}},{{\"S\",\"1\"}},{{\"S\",\"a\"}},{{\"N\",{state}}},0}}"
            )
        };
        let with = format!(
            "{{0,{{1,{{7}},{{2,7,0,0,1,1,2,2,3,3,4,4,5,5,6,6,{{1,1,{{2,0,4,{{\"#\",t,{{1,{NIL}}}}},{{\"B\",1}},x,\
             {{\"S\",\"\"}},1,{{1,2,{},{}}}}}}},-1,2}}}}}}",
            item(1, "ba000000-0000-4000-8000-000000000002", 2),
            item(2, "ea000000-0000-4000-8000-000000000003", 0),
            NIL = super::super::adoption::NIL_UUID,
        );
        let states = states(&with).unwrap();
        assert_eq!(
            states["ba000000-0000-4000-8000-000000000002"],
            "AdoptedCheck"
        );
        assert_eq!(states["ea000000-0000-4000-8000-000000000003"], "Native");
        let without = with.replace(
            "{2,7,0,0,1,1,2,2,3,3,4,4,5,5,6,6,",
            "{2,6,0,0,1,1,2,2,3,3,4,4,5,5,",
        );
        let states = super::states(&without).unwrap();
        assert!(states.values().all(|state| *state == "AdoptedCheck"));
    }

    #[test]
    fn writes_the_state_before_the_child_items() {
        let xml = "<PredefinedData>\r\n\t<Item id=\"a\">\r\n\t\t<Name>A</Name>\r\n\t\t<ChildItems>\r\n\
                   \t\t\t<Item id=\"b\">\r\n\t\t\t\t<Name>B</Name>\r\n\t\t\t</Item>\r\n\t\t</ChildItems>\r\n\t</Item>\r\n</PredefinedData>";
        let states = BTreeMap::from([("a".to_owned(), "Native"), ("b".to_owned(), "AdoptedCheck")]);
        assert_eq!(
            rewrite(xml, &states).unwrap(),
            "<PredefinedData>\r\n\t<Item id=\"a\">\r\n\t\t<Name>A</Name>\r\n\t\t<ExtensionState>Native</ExtensionState>\r\n\
             \t\t<ChildItems>\r\n\t\t\t<Item id=\"b\">\r\n\t\t\t\t<Name>B</Name>\r\n\
             \t\t\t\t<ExtensionState>AdoptedCheck</ExtensionState>\r\n\t\t\t</Item>\r\n\t\t</ChildItems>\r\n\
             \t</Item>\r\n</PredefinedData>"
        );
    }
}
