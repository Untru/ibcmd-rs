//! A managed form stored by an older platform, layout root revision `42`
//! (container `3`), read as 8.3.27.2214 re-saves it (container `4`, root
//! `50`).
//!
//! The readers know the records of roots `49`/`50` only; a `42` root's
//! command bar and items (record `21` and its older nested records) were
//! refused one by one and the form written truncated. Upgrading the whole
//! body first gives every reader the records it knows.
//!
//! Roots `38` and `46` (older still) take the same path.
//!
//! Evidence: 1C:Документооборот holds 217 root-`42`, 56 root-`38` and 53
//! root-`46` forms. Each was loaded into 8.3.27.2214 and saved again
//! (`_onecdec/reserialize.py`); the rules below are what the platform did to
//! every record kind, derived over those pairs (`LEARNED`: a record's new tag,
//! its dropped default visibility tuple, fixed members and appended tail,
//! keyed by the root revision, the parent record and position; the others
//! where a new member depends on an old one). With them 125 of the 217
//! root-`42` bodies equal the platform's re-save byte for byte, and 197 of the
//! 326 forms export to the platform's own `Form.xml` (none did before).
//! Not yet read: root-`46` attribute chains (`{2,{attribute},{field}}`), which
//! number attributes differently from their records; the forms whose
//! conditional appearance binds such a chain are refused, not truncated.

/// One node of a brace record.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Leaf(String),
    List(Vec<Node>),
}

impl Node {
    fn leaf(value: &str) -> Self {
        Self::Leaf(value.to_owned())
    }

    fn as_leaf(&self) -> Option<&str> {
        match self {
            Self::Leaf(value) => Some(value),
            Self::List(_) => None,
        }
    }

    fn as_list(&self) -> Option<&[Node]> {
        match self {
            Self::List(items) => Some(items),
            Self::Leaf(_) => None,
        }
    }

    fn is(&self, value: &str) -> bool {
        self.as_leaf() == Some(value)
    }

    fn render(&self, out: &mut String) {
        match self {
            Self::Leaf(value) => out.push_str(value),
            Self::List(items) => {
                out.push('{');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.render(out);
                }
                out.push('}');
            }
        }
    }

    fn rendered(&self) -> String {
        let mut out = String::new();
        self.render(&mut out);
        out
    }
}

/// The brace list opening at byte `start` and the index after it. Line
/// breaks outside strings are dropped, leaves are trimmed, strings are kept
/// with their quotes.
fn parse(text: &str, start: usize) -> Option<(Node, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut items = Vec::new();
    let mut current: Option<String> = Some(String::new());
    let mut index = start + 1;
    let mut quoted = false;
    while index < bytes.len() {
        let ch = text[index..].chars().next()?;
        let width = ch.len_utf8();
        if quoted {
            current.get_or_insert_with(String::new).push(ch);
            if ch == '"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    current.get_or_insert_with(String::new).push('"');
                    index += 1;
                } else {
                    quoted = false;
                }
            }
        } else {
            match ch {
                '"' => {
                    quoted = true;
                    current.get_or_insert_with(String::new).push(ch);
                }
                '{' => {
                    let (node, next) = parse(text, index)?;
                    items.push(node);
                    current = None;
                    index = next;
                    continue;
                }
                ',' | '}' => {
                    if let Some(value) = current.take() {
                        items.push(Node::Leaf(value.trim().to_owned()));
                    }
                    if ch == '}' {
                        return Some((Node::List(items), index + 1));
                    }
                    current = Some(String::new());
                }
                '\r' | '\n' => {}
                _ => current.get_or_insert_with(String::new).push(ch),
            }
        }
        index += width;
    }
    None
}

fn parse_str(text: &str) -> Node {
    parse(text, 0)
        .map(|(node, _)| node)
        .unwrap_or_else(|| Node::leaf(text))
}

/// A record's version tag: its first member when that is a number.
fn tag(node: &Node) -> Option<&str> {
    let first = node.as_list()?.first()?.as_leaf()?;
    let digits = first.strip_prefix('-').unwrap_or(first);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(first)
}

/// The tag a child's rule is keyed by: an item record (`{tag,{id,class}}`)
/// adds the first eight characters of its class.
fn context_tag(node: &Node) -> Option<String> {
    let tag = tag(node)?;
    let items = node.as_list()?;
    if let Some(identity) = items.get(1).and_then(Node::as_list)
        && identity.len() == 2
        && let Some(class) = identity[1].as_leaf()
        && class.len() == 36
    {
        return Some(format!("{tag}:{}", &class[..8]));
    }
    Some(tag.to_owned())
}

fn is_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok() && value.len() == 36
}

const DEFAULT_VISIBLE: &str = r#"{0,{0,{"B",1},0}}"#;

/// `{N, (uuid, name)*N, 0, 0}`: an event list as the older platform stores it.
fn is_event_list(node: &Node) -> bool {
    is_event_list_shaped(node, false)
}

/// [`is_event_list`], also taking the bare `{0}` when `short` is set.
fn is_event_list_shaped(node: &Node, short: bool) -> bool {
    let Some(items) = node.as_list() else {
        return false;
    };
    let Some(count) = items
        .first()
        .and_then(Node::as_leaf)
        .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return false;
    };
    // Roots `38`/`46` store a non-empty list without its closing `0,0`.
    let shape = (items.len() == 2 * count + 3
        && items[items.len() - 2].is("0")
        && items[items.len() - 1].is("0"))
        || ((count >= 1 || short) && items.len() == 2 * count + 1);
    shape && (0..count).all(|j| items[1 + 2 * j].as_leaf().is_some_and(is_uuid))
}

/// `{N, (uuid, name)*N, 0, 0}` -> `{N, (uuid, name)*N, 1, 0, (uuid, 0, 1)*N}`.
fn upgrade_events(node: Node) -> Node {
    if !is_event_list_shaped(&node, true) {
        return node;
    }
    let Node::List(mut items) = node else {
        return node;
    };
    let count = items[0]
        .as_leaf()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if items.len() == 2 * count + 1 {
        items.extend([Node::leaf("0"), Node::leaf("0")]);
    }
    let count = (items.len() - 3) / 2;
    let handlers = (0..count)
        .map(|j| items[1 + 2 * j].clone())
        .collect::<Vec<_>>();
    let len = items.len();
    items[len - 2] = Node::leaf("1");
    items[len - 1] = Node::leaf("0");
    for handler in handlers {
        items.extend([handler, Node::leaf("0"), Node::leaf("1")]);
    }
    Node::List(items)
}

/// Positions (parent tag, slot) whose empty event list `{0,0,0}` the
/// platform upgrades; everywhere else `{0,0,0}` is not an event list.
const EMPTY_EVENT_SLOTS: &[(&str, usize)] = &[
    ("1", 6),
    ("1", 7),
    ("1", 10),
    ("1", 12),
    ("3", 5),
    ("3", 6),
    ("4", 2),
    ("4", 10),
    ("5", 5),
    ("10", 16),
    ("11", 12),
    ("36", 36),
    ("37", 40),
    ("37", 41),
    ("50", 19),
    ("50", 21),
    ("50", 25),
    ("50", 27),
];

/// Positions whose empty event list roots `37`/`38` store as a bare `{0}`.
const SHORT_EMPTY_EVENT_SLOTS: &[(&str, usize)] = &[
    ("5", 5),
    ("36", 36),
    ("37", 40),
    ("50", 19),
    ("50", 21),
    ("50", 25),
];

/// Drops the default visibility tuple an item record keeps at slot 4 or 5
/// behind its `1` flag.
fn drop_default_visibility(items: &mut Vec<Node>) -> bool {
    if items.len() <= 5 || items[1].as_list().is_none() {
        return false;
    }
    for slot in [4, 3] {
        if items[slot].is("1") && items[slot + 1].rendered() == DEFAULT_VISIBLE {
            items[slot] = Node::leaf("0");
            items.remove(slot + 1);
            return true;
        }
    }
    false
}

/// The members a root `50` closes with, behind its two empty strings and the
/// navigator flag.
const ROOT_50_TRAILER: &[&str] = &[
    "1", "\"\"", "0", "0", "0", "0", "0", "0", "3", "3", "0", "0", "0", "100", "1", "1", "0", "0",
    "0", "{50,0}", "1",
];

/// Roots `37`/`38`/`42`/`46` -> `50`. Behind the root's two empty strings the
/// navigator flag drops to `0` with its item; what follows is a prefix of the
/// `50` trailer, completed from it. Roots `37`/`38` keep four members there (its
/// caption-button items leave) and repeats the third at trailer slot 12.
fn upgrade_root(items: Vec<Node>) -> Vec<Node> {
    let Some(anchor) = (0..items.len().saturating_sub(1))
        .rev()
        .find(|&j| items[j].is("\"\"") && items[j + 1].is("\"\""))
    else {
        return items;
    };
    let old_root = items[0].as_leaf().unwrap_or_default().to_owned();
    let tail_at = anchor + 2;
    let mut rest = items[tail_at..].to_vec();
    let skip = if rest.first().is_some_and(|flag| flag.is("1"))
        && rest.get(1).and_then(Node::as_list).is_some()
    {
        2
    } else {
        1
    };
    rest.drain(..skip.min(rest.len()));
    let trailer = ROOT_50_TRAILER
        .iter()
        .map(|value| parse_str_or_leaf(value))
        .collect::<Vec<_>>();
    let new_rest = if matches!(old_root.as_str(), "37" | "38") && rest.len() >= 4 {
        let mut new_rest = rest[..4].to_vec();
        new_rest.extend_from_slice(&trailer[4..12]);
        new_rest.push(rest[2].clone());
        new_rest.extend_from_slice(&trailer[13..]);
        new_rest
    } else if rest.len() <= trailer.len() {
        let mut new_rest = rest.clone();
        new_rest.extend_from_slice(&trailer[rest.len()..]);
        new_rest
    } else {
        return items;
    };
    let mut out = Vec::with_capacity(tail_at + 1 + new_rest.len());
    out.push(Node::leaf("50"));
    out.extend_from_slice(&items[1..tail_at]);
    out.push(Node::leaf("0"));
    out.extend(new_rest);
    out
}

/// Item `21` -> `22`: the default visibility tuple leaves, a `0` closes it.
fn upgrade_item(mut items: Vec<Node>) -> Vec<Node> {
    items[0] = Node::leaf("22");
    if items[4].is("1") && items[5].rendered() == DEFAULT_VISIBLE {
        items[4] = Node::leaf("0");
        items.remove(5);
    }
    items.push(Node::leaf("0"));
    items
}

/// Usual group `23` -> `29`: old slots 10 and 22 reappear in the tail.
///
/// Slot 10 is the group's behavior and stays where it is, echoed at 24 and
/// closing slot 28: `0` is an explicit `Usual`, which 8.3.27.2214 writes for
/// these groups under compatibility 8.3.21 (1C:Документооборот z34, 120
/// forms). The re-save the other rules come from went through XML dumped
/// under 8.3.17, where `Usual` is not written, and so reads back as the
/// default (`1` at 10, `3` at 28) instead.
fn upgrade_group(mut items: Vec<Node>) -> Vec<Node> {
    let slot10 = items[10].clone();
    let slot22 = items[22].clone();
    let closing = slot10.as_leaf().unwrap_or("3").to_owned();
    let closing = closing.as_str();
    items[0] = Node::leaf("29");
    items.push(parse_str("{3,4,{0}}"));
    items.push(slot10);
    items.extend([Node::leaf("2"), Node::leaf("0")]);
    items.push(slot22);
    items.push(Node::leaf(closing));
    items
}

/// Button `28` -> `31`: slot 49 refines old slot 15 the way
/// `parse_form_button_location_in_command_bar_at_revision` reads the pair.
fn upgrade_button(items: Vec<Node>) -> Vec<Node> {
    let mut upgraded = items.clone();
    drop_default_visibility(&mut upgraded);
    if upgraded.len() != 48 {
        return items;
    }
    let location = match upgraded[15].as_leaf() {
        Some("0") => "1",
        Some("1") => "3",
        _ => "0",
    };
    upgraded[0] = Node::leaf("31");
    upgraded.extend(["0", location, "1", "0"].map(Node::leaf));
    upgraded
}

/// Table `49` -> `55`: its property bag gains `19 = {"S",""}`, the event
/// list after the bag upgrades, eight members close the record.
fn upgrade_table(items: Vec<Node>) -> Vec<Node> {
    let mut upgraded = items.clone();
    drop_default_visibility(&mut upgraded);
    let Some(count) = upgraded
        .get(54)
        .and_then(Node::as_leaf)
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return items;
    };
    let at = 55 + 2 * count;
    if upgraded.len() < at + 1 {
        return items;
    }
    upgraded[0] = Node::leaf("55");
    upgraded[54] = Node::Leaf((count + 1).to_string());
    upgraded.insert(at, parse_str(r#"{"S",""}"#));
    upgraded.insert(at, Node::leaf("19"));
    if let Some(events) = upgraded
        .get_mut(at + 2)
        .filter(|events| is_event_list(events))
    {
        let old = std::mem::replace(events, Node::leaf(""));
        *events = upgrade_events(old);
    }
    upgraded.extend(["0", "1", "0", "0", "0", "0", "0", "0"].map(Node::leaf));
    upgraded
}

/// Dynamic-list table `51` -> `55`: its property bag loses keys 15 and 19
/// and gains `20 = {"B",1}`, the event list after the bag upgrades, five
/// zeros close the record.
fn upgrade_dynamic_list_table(items: Vec<Node>) -> Vec<Node> {
    let mut upgraded = items.clone();
    drop_default_visibility(&mut upgraded);
    let Some(count) = upgraded
        .get(54)
        .and_then(Node::as_leaf)
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return items;
    };
    let bag_end = 55 + 2 * count;
    if upgraded.len() < bag_end {
        return items;
    }
    let mut bag = upgraded[55..bag_end]
        .chunks_exact(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect::<Vec<_>>();
    let had_20 = bag.iter().any(|(key, _)| key.is("20"));
    bag.retain(|(key, _)| !key.is("15") && !key.is("19"));
    if !had_20 {
        bag.push((Node::leaf("20"), parse_str(r#"{"B",1}"#)));
    }
    let mut rest = upgraded[bag_end..].to_vec();
    if let Some(events) = rest.first_mut().filter(|events| is_event_list(events)) {
        let old = std::mem::replace(events, Node::leaf(""));
        *events = upgrade_events(old);
    }
    let mut out = Vec::with_capacity(upgraded.len() + 5);
    out.push(Node::leaf("55"));
    out.extend_from_slice(&upgraded[1..54]);
    out.push(Node::Leaf(bag.len().to_string()));
    for (key, value) in bag {
        out.push(key);
        out.push(value);
    }
    out.extend(rest);
    out.extend(["0", "0", "0", "0", "0"].map(Node::leaf));
    out
}

/// An item's slot 20: `{1,k,X}` with an event list X grows to
/// `{4,k,X,2,0,1}`; a `{4,k,X,2,0,0}` closes with `1`.
fn upgrade_item_slot_20(items: Vec<Node>) -> Vec<Node> {
    match (items.first().and_then(Node::as_leaf), items.len()) {
        (Some("1"), 3) if is_event_list(&items[2]) => vec![
            Node::leaf("4"),
            items[1].clone(),
            items[2].clone(),
            Node::leaf("2"),
            Node::leaf("0"),
            Node::leaf("1"),
        ],
        (Some("4"), 6) if items[3].is("2") && items[4].is("0") => {
            let mut items = items;
            items[5] = Node::leaf("1");
            items
        }
        _ => items,
    }
}

pub(super) struct Learned {
    root: &'static str,
    parent: &'static str,
    slot: Option<usize>,
    tag: &'static str,
    len: usize,
    new_tag: &'static str,
    visible: bool,
    overrides: &'static [(usize, &'static str)],
    tail: &'static [&'static str],
}

/// The rule learned for this record under the form's own root revision, or
/// failing that under root `42`, the revision with the most evidence.
fn learned_rule(
    root: &str,
    parent: Option<&str>,
    slot: Option<usize>,
    tag: &str,
    len: usize,
) -> Option<&'static Learned> {
    let parent = parent?;
    let matches = |rule: &&Learned, root: &str, slot: Option<usize>| {
        rule.root == root
            && rule.parent == parent
            && rule.slot == slot
            && rule.tag == tag
            && rule.len == len
    };
    [root, "42"].into_iter().find_map(|root| {
        LEARNED
            .iter()
            .find(|rule| matches(rule, root, slot))
            .or_else(|| LEARNED.iter().find(|rule| matches(rule, root, None)))
    })
}

fn apply_learned(rule: &Learned, items: Vec<Node>) -> Vec<Node> {
    let mut items = items;
    if rule.visible {
        drop_default_visibility(&mut items);
    }
    items[0] = Node::leaf(rule.new_tag);
    for &(slot, value) in rule.overrides {
        if let Some(member) = items.get_mut(slot) {
            *member = parse_str_or_leaf(value);
        }
    }
    items.extend(rule.tail.iter().map(|value| parse_str_or_leaf(value)));
    items
}

fn parse_str_or_leaf(value: &str) -> Node {
    if value.starts_with('{') {
        parse_str(value)
    } else {
        Node::leaf(value)
    }
}

fn upgrade(
    node: Node,
    root: &str,
    depth: usize,
    parent: Option<&str>,
    slot: Option<usize>,
) -> Node {
    let Node::List(items) = node else {
        return node;
    };
    let own = items
        .first()
        .and_then(Node::as_leaf)
        .map(str::to_owned)
        .unwrap_or_default();
    let is_item = items.len() > 5
        && items[1]
            .as_list()
            .is_some_and(|identity| identity.len() == 2);
    let parent_kind = parent.map(|parent| parent.split(':').next().unwrap_or(parent));
    let items = if depth == 0 && own == "3" {
        let mut items = items;
        items[0] = Node::leaf("4");
        items
    } else if depth == 0 && own == "2" {
        // Container `2` lacks the last two members of `4`.
        let mut items = items;
        items[0] = Node::leaf("4");
        items.extend([Node::leaf("0"), Node::leaf("0")]);
        items
    } else if depth == 1 && matches!(own.as_str(), "37" | "38" | "42" | "46" | "48") {
        upgrade_root(items)
    } else if own == "21" && is_item {
        upgrade_item(items)
    } else if own == "49" && depth > 1 && items.len() > 90 && items[1].as_list().is_some() {
        upgrade_table(items)
    } else if own == "51" && depth > 1 && items.len() > 90 && items[1].as_list().is_some() {
        upgrade_dynamic_list_table(items)
    } else if parent_kind == Some("22") && own == "28" && matches!(items.len(), 48 | 49) {
        upgrade_button(items)
    } else if parent_kind == Some("22") && own == "23" && items.len() == 23 {
        upgrade_group(items)
    } else if parent_kind == Some("22")
        && slot == Some(20)
        && matches!((own.as_str(), items.len()), ("1", 3) | ("4", 6))
    {
        upgrade_item_slot_20(items)
    } else if let Some(rule) = learned_rule(root, parent, slot, &own, items.len()) {
        apply_learned(rule, items)
    } else {
        items
    };
    let this = Node::List(items);
    let context = context_tag(&this);
    let kind = context
        .as_deref()
        .map(|context| context.split(':').next().unwrap_or(context).to_owned());
    let Node::List(items) = this else {
        unreachable!()
    };
    Node::List(
        items
            .into_iter()
            .enumerate()
            .map(|(index, child)| {
                let slot_is = |slots: &[(&str, usize)]| {
                    kind.as_deref()
                        .is_some_and(|kind| slots.contains(&(kind, index)))
                };
                let len = child.as_list().map_or(0, <[Node]>::len);
                let empty = child.as_list().is_some_and(|list| list[0].is("0"));
                let child = if is_event_list_shaped(&child, true)
                    && (!empty
                        || (len == 3 && slot_is(EMPTY_EVENT_SLOTS))
                        || (len == 1 && slot_is(SHORT_EMPTY_EVENT_SLOTS)))
                {
                    upgrade_events(child)
                } else {
                    child
                };
                upgrade(child, root, depth + 1, context.as_deref(), Some(index))
            })
            .collect(),
    )
}

/// The body text of a root-`42` form as 8.3.27.2214 re-saves it; `None` for
/// any other body.
pub(super) fn upgrade_root_42_body(plain: &str) -> Option<String> {
    let start = plain.find('{')?;
    let (body, end) = parse(plain, start)?;
    let items = body.as_list()?;
    let root = items.get(1).and_then(tag)?.to_owned();
    let container_ok = match tag(&body) {
        Some("3") => matches!(root.as_str(), "38" | "42" | "46" | "48"),
        Some("2") => root == "37",
        _ => false,
    };
    if !container_ok {
        return None;
    }
    let upgraded = upgrade(body, &root, 0, None, None);
    let mut out = String::with_capacity(plain.len() + plain.len() / 8);
    out.push_str(&plain[..start]);
    upgraded.render(&mut out);
    out.push_str(&plain[end..]);
    Some(out)
}

include!("form_root_42_rules.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_event_list_gains_its_handler_calls() {
        let events =
            parse_str("{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",0,0}");
        assert_eq!(
            upgrade_events(events).rendered(),
            "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",1,0,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1}"
        );
    }

    #[test]
    fn a_root_42_command_bar_reads_as_the_re_saved_record() {
        // The shape of 1C:Документооборот's root-42 forms: container 3, the
        // root's command bar a record `21` behind the default visibility
        // tuple, a navigator item before the tail.
        let plain = concat!(
            "\u{feff}{3,{42,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,",
            "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",0,0},{0},1,",
            "{21,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},9,\"ФормаКоманднаяПанель\",",
            "{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3},",
            "0,\"\",\"\",1,{21,{0},0,0,0,7,\"Navigator\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3},",
            "1,\"\",0,0,0,0,0,0,3,3,0,0,0},\"\",{0,0},{0,0},{0,0},{0,0},0,0}"
        );
        let upgraded = upgrade_root_42_body(plain).unwrap();
        assert_eq!(
            upgraded,
            concat!(
                "\u{feff}{4,{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,",
                "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",1,0,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1},{0},1,",
                "{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"ФормаКоманднаяПанель\",",
                "{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0},",
                "0,\"\",\"\",0,1,\"\",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{50,0},1},\"\",{0,0},{0,0},{0,0},{0,0},0,0}"
            )
        );
    }

    #[test]
    fn a_body_of_another_root_is_left_alone() {
        assert_eq!(upgrade_root_42_body("{4,{50,0},\"\",0}"), None);
    }
}
