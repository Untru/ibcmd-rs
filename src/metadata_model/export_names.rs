//! The name index built from the stored rows instead of an XML tree.
//!
//! [`NameIndex::from_config_index`] reverses the load direction's index of a
//! source tree. An export has no tree, only rows, so here the same index is
//! filled from what the rows say: each object row through its kind's
//! [`object_names`], the root row as `Configuration`, and the predefined
//! items from the predefined-data bodies (`<owner>.1c`, `.7`, `.9`, `.2`).
//! Kinds without `object_names` yet are filled by the caller from whatever
//! it has (the old export's indexes); [`compare`] measures the result
//! against the tree's index.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

use anyhow::{Result, bail};

use super::super::brace::Brace;
use super::{ExportContext, NameIndex, atom, decode_object, item, list, object_names};

/// Whether `object_names` reads rows of `kind` (probed once per kind: a kind
/// without a reader answers "not yet").
pub fn has_names(kind: &str) -> bool {
    static KNOWN: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(answer) = known.lock().ok().and_then(|map| map.get(kind).copied()) {
        return answer;
    }
    let answer = match object_names(kind, &Brace::List(Vec::new())) {
        Ok(_) => true,
        Err(error) => !is_not_yet(&error),
    };
    if let Ok(mut map) = known.lock() {
        map.insert(kind.to_string(), answer);
    }
    answer
}

/// Whether `owned_object_names` reads owned rows of `kind`.
pub fn has_owned_names(kind: &str) -> bool {
    static KNOWN: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(answer) = known.lock().ok().and_then(|map| map.get(kind).copied()) {
        return answer;
    }
    let answer = match super::owned_object_names(kind, &Brace::List(Vec::new()), "") {
        Ok(_) => true,
        Err(error) => !is_not_yet(&error),
    };
    if let Ok(mut map) = known.lock() {
        map.insert(kind.to_string(), answer);
    }
    answer
}

/// Whether `decode_object` decodes rows of `kind`.
pub fn has_decoder(kind: &str) -> bool {
    static KNOWN: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(answer) = known.lock().ok().and_then(|map| map.get(kind).copied()) {
        return answer;
    }
    // The probe decodes an empty row: a kind without a decoder answers
    // "not yet" before it reads the context.
    let context = ExportContext {
        names: NameIndex::default(),
        version: "2.20".to_string(),
        compat: super::super::objects::parts::Compat(8, 3, 27),
    };
    let answer = match decode_object(kind, &Brace::List(Vec::new()), &context) {
        Ok(_) => true,
        Err(error) => !is_not_yet(&error),
    };
    if let Ok(mut map) = known.lock() {
        map.insert(kind.to_string(), answer);
    }
    answer
}

fn is_not_yet(error: &anyhow::Error) -> bool {
    error.to_string().starts_with("not yet")
}

impl NameIndex {
    /// Adds a name; an existing one for the same uuid wins.
    pub fn insert_name(&mut self, uuid: &str, full_name: &str) -> bool {
        let key = uuid.to_ascii_lowercase();
        if self.names.contains_key(&key) {
            return false;
        }
        self.names.insert(key, full_name.to_string());
        true
    }

    /// Sets a name, replacing what the uuid had.
    pub fn set_name(&mut self, uuid: &str, full_name: &str) {
        self.names
            .insert(uuid.to_ascii_lowercase(), full_name.to_string());
    }

    /// Adds a generated type name; an existing one for the same id wins.
    pub fn insert_type(&mut self, type_id: &str, name: &str) -> bool {
        let key = type_id.to_ascii_lowercase();
        if self.types.contains_key(&key) {
            return false;
        }
        self.types.insert(key, name.to_string());
        true
    }

    /// Adds a predefined item of `owner` (`Catalog.X`).
    pub fn insert_predefined(&mut self, owner: &str, uuid: &str, name: &str) {
        let items = self
            .predefined
            .entry(uuid.to_ascii_lowercase())
            .or_default();
        if !items.iter().any(|(item_owner, _)| item_owner == owner) {
            items.push((owner.to_string(), name.to_string()));
        }
    }

    pub fn has_name(&self, uuid: &str) -> bool {
        self.names.contains_key(uuid)
    }

    pub fn has_type(&self, type_id: &str) -> bool {
        self.types.contains_key(type_id)
    }

    /// Every (uuid, full name).
    pub fn name_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.names
            .iter()
            .map(|(uuid, name)| (uuid.as_str(), name.as_str()))
    }

    /// Every (type id, generated type name).
    pub fn type_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.types
            .iter()
            .map(|(id, name)| (id.as_str(), name.as_str()))
    }

    /// (names, generated types, predefined items).
    pub fn sizes(&self) -> (usize, usize, usize) {
        (
            self.names.len(),
            self.types.len(),
            self.predefined.values().map(Vec::len).sum(),
        )
    }

    /// Merges what several parallel builders collected; `self` wins.
    pub fn absorb(&mut self, other: NameIndex) {
        for (uuid, name) in other.names {
            self.names.entry(uuid).or_insert(name);
        }
        for (type_id, name) in other.types {
            self.types.entry(type_id).or_insert(name);
        }
        for (uuid, items) in other.predefined {
            for (owner, name) in items {
                self.insert_predefined(&owner, &uuid, &name);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The root row: which kind every top-level object is.

/// The class id each top-level kind's objects are listed under in the root
/// row, `{<class id>,<count>,<uuid>...}` (the load direction's root layout).
const ROOT_CLASSES: &[(&str, &str)] = &[
    ("2deed9b8-0056-4ffe-a473-c20a6c32a0bc", "AccountingRegister"),
    (
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        "AccumulationRegister",
    ),
    ("6e6dc072-b7ac-41e7-8f88-278d25b6da2a", "Bot"),
    ("fcd3404e-1523-48ce-9bc0-ecdb822684a1", "BusinessProcess"),
    (
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
        "CalculationRegister",
    ),
    ("cf4abea6-37b2-11d4-940f-008048da11f9", "Catalog"),
    ("238e7e88-3c5f-48b2-8a3b-81ebbecb20ed", "ChartOfAccounts"),
    (
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
        "ChartOfCalculationTypes",
    ),
    (
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "ChartOfCharacteristicTypes",
    ),
    ("1c57eabe-7349-44b3-b1de-ebfeab67b47d", "CommandGroup"),
    ("15794563-ccec-41f6-a83c-ec5f7b9a5bc1", "CommonAttribute"),
    ("2f1a5187-fb0e-4b05-9489-dc5dd6412348", "CommonCommand"),
    ("07ee8426-87f1-11d5-b99c-0050bae0a95d", "CommonForm"),
    ("0fe48980-252d-11d6-a3c7-0050bae0a776", "CommonModule"),
    ("7dcd43d9-aca5-4926-b549-1842e6a4e8cf", "CommonPicture"),
    ("0c89c792-16c3-11d5-b96b-0050bae0a95d", "CommonTemplate"),
    ("0195e80c-b157-11d4-9435-004095e12fc7", "Constant"),
    ("bf845118-327b-4682-b5c6-285d2a0eb296", "DataProcessor"),
    ("c045099e-13b9-4fb6-9d50-fca00202971e", "DefinedType"),
    ("061d872a-5787-460e-95ac-ed74ea3a3e84", "Document"),
    ("4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d", "DocumentJournal"),
    ("36a8e346-9aaa-4af9-bdbd-83be3c177977", "DocumentNumerator"),
    ("f6a80749-5ad7-400b-8519-39dc5dff2542", "Enum"),
    ("4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc", "EventSubscription"),
    ("857c4a91-e5f4-4fac-86ec-787626f1c108", "ExchangePlan"),
    ("5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5", "ExternalDataSource"),
    ("3e7bfcc0-067d-11d6-a3c7-0050bae0a776", "FilterCriterion"),
    ("af547940-3268-434f-a3e7-e47d6d2638c3", "FunctionalOption"),
    (
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc",
        "FunctionalOptionsParameter",
    ),
    ("0fffc09c-8f4c-47cc-b41c-8d5c5a221d79", "HTTPService"),
    (
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        "InformationRegister",
    ),
    ("bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d", "IntegrationService"),
    ("39bddf6a-0c3c-452b-921c-d99cfa1c2f1b", "Interface"),
    ("9cd510ce-abfc-11d4-9434-004095e12fc7", "Language"),
    ("102f6202-43fa-40b0-8898-acd3876daacb", "PaletteColor"),
    ("631b75a0-29e2-11d6-a3c7-0050bae0a776", "Report"),
    ("09736b02-9cac-4e3f-b4f7-d3e9576ab948", "Role"),
    ("11bdaf85-d5ad-4d91-bb24-aa0eee139052", "ScheduledJob"),
    ("bc587f20-35d9-11d6-a3c7-0050bae0a776", "Sequence"),
    ("24c43748-c938-45d0-8d14-01424a72b11e", "SessionParameter"),
    ("46b4cd97-fd13-4eaa-aba2-3bddd7699218", "SettingsStorage"),
    ("3e5404af-6ef8-4c73-ad11-91bd2dfac4c8", "Style"),
    ("58848766-36ea-4076-8800-e91eb49590d7", "StyleItem"),
    ("37f2fa9a-b276-11d4-9435-004095e12fc7", "Subsystem"),
    ("3e63355c-1378-4953-be9b-1deb5fb6bec5", "Task"),
    ("d26096fb-7a5d-4df9-af63-47d04771fa9b", "WSReference"),
    ("8657032e-7740-4e1d-a3ba-5dd6e8afb78f", "WebService"),
    (
        ibcmd_schema::websocket_client::WebSocketClientLayout::FAMILY_UUID,
        ibcmd_schema::websocket_client::WebSocketClientLayout::KIND,
    ),
    ("cc9df798-7c94-4616-97d2-7aa0b7bc515e", "XDTOPackage"),
];

/// Canonical top-level collection identities already used by the name reader.
pub(crate) fn root_class_kinds() -> &'static [(&'static str, &'static str)] {
    ROOT_CLASSES
}

/// The root row (the configuration's own row) -> the kind of every
/// top-level object it lists, by uuid.
pub fn root_kinds(root: &Brace) -> HashMap<String, &'static str> {
    let classes = ROOT_CLASSES.iter().copied().collect::<HashMap<_, _>>();
    let mut out = HashMap::new();
    collect_root_kinds(root, &classes, &mut out);
    out
}

/// The top-level objects the configuration's own row lists, as (kind, uuid),
/// in the row's order (the configurator's order of each kind's objects).
/// The same lists [`root_kinds`] reads.
pub fn root_members(root: &Brace) -> Vec<(&'static str, String)> {
    let classes = ROOT_CLASSES.iter().copied().collect::<HashMap<_, _>>();
    let mut kinds = HashMap::new();
    let mut out = Vec::new();
    collect_root_members(root, &classes, &mut kinds, &mut out);
    out
}

fn collect_root_members(
    node: &Brace,
    classes: &HashMap<&str, &'static str>,
    seen: &mut HashMap<String, &'static str>,
    out: &mut Vec<(&'static str, String)>,
) {
    let Some(members) = node.as_list() else {
        return;
    };
    if let (Some(class), Some(count)) = (
        members.first().and_then(Brace::as_atom),
        members.get(1).and_then(Brace::as_atom),
    ) && let Some(kind) = classes.get(class)
        && let Ok(count) = count.parse::<usize>()
        && members.len() == count + 2
        && members[2..].iter().all(|member| member.as_atom().is_some())
    {
        for member in &members[2..] {
            if let Some(uuid) = member.as_atom() {
                let uuid = uuid.to_ascii_lowercase();
                if seen.insert(uuid.clone(), kind).is_none() {
                    out.push((*kind, uuid));
                }
            }
        }
        return;
    }
    for member in members {
        collect_root_members(member, classes, seen, out);
    }
}

fn collect_root_kinds(
    node: &Brace,
    classes: &HashMap<&str, &'static str>,
    out: &mut HashMap<String, &'static str>,
) {
    let Some(members) = node.as_list() else {
        return;
    };
    if let (Some(class), Some(count)) = (
        members.first().and_then(Brace::as_atom),
        members.get(1).and_then(Brace::as_atom),
    ) && let Some(kind) = classes.get(class)
        && let Ok(count) = count.parse::<usize>()
        && members.len() == count + 2
        && members[2..].iter().all(|member| member.as_atom().is_some())
    {
        for member in &members[2..] {
            if let Some(uuid) = member.as_atom() {
                out.insert(uuid.to_ascii_lowercase(), *kind);
            }
        }
        return;
    }
    for member in members {
        collect_root_kinds(member, classes, out);
    }
}

// ---------------------------------------------------------------------------
// A row's own header.

/// The object's own md header in a descriptor row, as (uuid, name): the
/// first `{3,{<n>,0,<uuid>},"<name>",<synonym>,"<comment>",...}` of nine
/// members, in document order (the object's header precedes its children's).
pub fn own_header(row: &Brace) -> Option<(String, String)> {
    let members = row.as_list()?;
    if members.len() == 9
        && members.first().and_then(Brace::as_atom) == Some("3")
        && let Some(identity) = members.get(1).and_then(Brace::as_list)
        && identity.len() == 3
        && identity.get(1).and_then(Brace::as_atom) == Some("0")
        && let Some(uuid) = identity.get(2).and_then(Brace::as_atom)
        && let Some(name) = members.get(2).and_then(Brace::as_str)
    {
        return Some((uuid.to_ascii_lowercase(), name.to_string()));
    }
    members.iter().find_map(own_header)
}

// ---------------------------------------------------------------------------
// Owned objects: rows of their own that their owner's row lists by uuid.

/// The class id an owner's row lists each kind of owned object under,
/// `{<class id>,<count>,<uuid>...}` (measured over the four corpora: every
/// form, template, recalculation and nested subsystem of their trees).
const OWNED_CLASSES: &[(&str, &str)] = &[
    ("3daea016-69b7-4ed4-9453-127911372fe6", "Template"),
    ("37f2fa9a-b276-11d4-9435-004095e12fc7", "Subsystem"),
    ("274bf899-db0e-4df6-8ab5-67bf6371ec0b", "Recalculation"),
    // Forms, one class per owner kind.
    ("d3b5d6eb-4ea2-4610-a3e2-624d4e815934", "Form"),
    ("b64d9a44-1642-11d6-a3c7-0050bae0a776", "Form"),
    ("3f7a8120-b71a-4265-98bf-4d9bc09b7719", "Form"),
    ("a2cb086c-db98-43e4-a1a9-0760ab048f8d", "Form"),
    ("fdf816d2-1ead-11d5-b975-0050bae0a95d", "Form"),
    ("5372e285-03db-4f8c-8565-fe56f1aea40e", "Form"),
    ("a7f8f92a-7a4b-484b-937e-42d242e64144", "Form"),
    ("eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94", "Form"),
    ("d5b0e5ed-256d-401c-9c36-f630cafd8a62", "Form"),
    ("fb880e93-47d7-4127-9357-a20e69c17545", "Form"),
    ("ec81ad10-ca07-11d5-b9a5-0050bae0a95d", "Form"),
    ("87c509ab-3d38-4d67-b379-aca796298578", "Form"),
    ("13134204-f60b-11d5-a3c7-0050bae0a776", "Form"),
    ("a3b368c0-29e2-11d6-a3c7-0050bae0a776", "Form"),
    ("b8533c0c-2342-4db3-91a2-c2b08cbf6b23", "Form"),
    ("3f58cbfb-4172-4e54-be49-561a579bb38b", "Form"),
    ("33f2e54b-37ce-4a7a-a569-b648d7aa4634", "Form"),
    ("00867c40-06b1-11d6-a3c7-0050bae0a776", "Form"),
];

/// Whether a stored row's text lists any owned object (a cheap test before
/// parsing it).
pub fn may_own_objects(text: &str) -> bool {
    OWNED_CLASSES.iter().any(|(class, _)| text.contains(class))
}

/// The objects a row owns that have rows of their own, as (kind, uuid).
pub fn owned_objects(row: &Brace) -> Vec<(&'static str, String)> {
    let classes = OWNED_CLASSES.iter().copied().collect::<HashMap<_, _>>();
    let mut out = Vec::new();
    collect_owned(row, &classes, &mut out);
    out
}

fn collect_owned(
    node: &Brace,
    classes: &HashMap<&str, &'static str>,
    out: &mut Vec<(&'static str, String)>,
) {
    let Some(members) = node.as_list() else {
        return;
    };
    if let (Some(class), Some(count)) = (
        members.first().and_then(Brace::as_atom),
        members.get(1).and_then(Brace::as_atom),
    ) && let Some(kind) = classes.get(class)
        && let Ok(count) = count.parse::<usize>()
        && members.len() == count + 2
        && members[2..].iter().all(|member| member.as_atom().is_some())
    {
        for member in &members[2..] {
            if let Some(uuid) = member.as_atom() {
                out.push((*kind, uuid.to_ascii_lowercase()));
            }
        }
        return;
    }
    for member in members {
        collect_owned(member, classes, out);
    }
}

/// One `{<class id>,<count>,<uuid>...}` list of owned objects in a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGroup {
    /// The class id the list is filed under.
    pub class: String,
    pub kind: &'static str,
    /// The listed uuids, lower-cased, in document order.
    pub members: Vec<String>,
}

/// The groups of owned objects a row lists, in document order.
pub fn owned_groups(row: &Brace) -> Vec<OwnedGroup> {
    let classes = OWNED_CLASSES.iter().copied().collect::<HashMap<_, _>>();
    let mut out = Vec::new();
    collect_groups(row, &classes, &mut out);
    out
}

/// The members of an owned-object group: `(class, kind)` and the uuid atoms.
fn group_shape<'a>(
    members: &'a [Brace],
    classes: &HashMap<&str, &'static str>,
) -> Option<(&'a str, &'static str, &'a [Brace])> {
    let class = members.first().and_then(Brace::as_atom)?;
    let count = members
        .get(1)
        .and_then(Brace::as_atom)?
        .parse::<usize>()
        .ok()?;
    let kind = classes.get(class)?;
    (members.len() == count + 2 && members[2..].iter().all(|member| member.as_atom().is_some()))
        .then_some((class, *kind, &members[2..]))
}

fn collect_groups(node: &Brace, classes: &HashMap<&str, &'static str>, out: &mut Vec<OwnedGroup>) {
    let Some(members) = node.as_list() else {
        return;
    };
    if let Some((class, kind, listed)) = group_shape(members, classes) {
        out.push(OwnedGroup {
            class: class.to_owned(),
            kind,
            members: listed
                .iter()
                .filter_map(Brace::as_atom)
                .map(str::to_ascii_lowercase)
                .collect(),
        });
        return;
    }
    for member in members {
        collect_groups(member, classes, out);
    }
}

/// Takes the given owned objects (lower-case uuids) out of every group of a
/// row and keeps each group's count in step; returns how many were removed.
pub fn remove_owned(row: &mut Brace, remove: &std::collections::HashSet<String>) -> usize {
    let classes = OWNED_CLASSES.iter().copied().collect::<HashMap<_, _>>();
    strip_owned(row, &classes, remove)
}

fn strip_owned(
    node: &mut Brace,
    classes: &HashMap<&str, &'static str>,
    remove: &std::collections::HashSet<String>,
) -> usize {
    let is_group = node
        .as_list()
        .is_some_and(|members| group_shape(members, classes).is_some());
    let Some(members) = node.as_list_mut() else {
        return 0;
    };
    if is_group {
        let before = members.len() - 2;
        let mut index = 0;
        members.retain(|member| {
            let keep = index < 2
                || member
                    .as_atom()
                    .is_none_or(|uuid| !remove.contains(&uuid.to_ascii_lowercase()));
            index += 1;
            keep
        });
        let after = members.len() - 2;
        members[1] = Brace::Atom(after.to_string());
        return before - after;
    }
    members
        .iter_mut()
        .map(|member| strip_owned(member, classes, remove))
        .sum()
}

// ---------------------------------------------------------------------------
// Predefined items.

/// `{"#",<this>,{1,<uuid>}}`: a reference to a predefined item.
const PREDEFINED_REF_TYPE: &str = "ae135932-4f94-44df-92c1-c91f15a92848";

/// The body row suffix of each kind that stores predefined items.
pub fn predefined_suffix(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Catalog" => "1c",
        "ChartOfCharacteristicTypes" => "7",
        "ChartOfAccounts" => "9",
        "ChartOfCalculationTypes" => "2",
        _ => return None,
    })
}

/// A predefined-data body -> its items as (uuid, name), in document order.
///
/// - Catalog `{0,<tree>}`: the item reference is column 0, the name column 3;
/// - ChartOfCharacteristicTypes `{1,<tree>}`: columns 1 and 3;
/// - ChartOfAccounts `{2,<tree>}`: columns 0 and 1;
/// - ChartOfCalculationTypes `<table>`: columns 1 and 2.
///
/// A tree is `{1,<columns>,<row data>}`, a table `{9,<columns>,<row data>,
/// {0,0}}`; the row data maps each column id to the position its value has
/// in a row: `{2,N,<position>,<column id>...,{1,<count>,<row>...},X,Y}`,
/// a row `{2,<index>,<value count>,<value>...,0}` or, with children,
/// `{...,1,{1,<count>,<row>...}}`.
///
/// A tree's top row is its root (`Элементы`, `Характеристики`, `Счета`), never
/// an item, even when an edited tree stores a reference in it.
pub fn predefined_items(kind: &str, body: &Brace) -> Result<Vec<(String, String)>> {
    let (container, reference_column, name_column, tree) = match kind {
        "Catalog" => (tagged(body, 0)?, 0, 3, true),
        "ChartOfCharacteristicTypes" => (tagged(body, 1)?, 1, 3, true),
        "ChartOfAccounts" => (tagged(body, 2)?, 0, 1, true),
        "ChartOfCalculationTypes" => (body, 1, 2, false),
        other => bail!("{other} stores no predefined items"),
    };
    let members = list(container)?;
    let row_data = list(item(members, 2)?)?;
    let columns = atom(item(row_data, 1)?)?
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("bad column count"))?;
    let mut positions = HashMap::new();
    for column in 0..columns {
        let position = atom(item(row_data, 2 + 2 * column)?)?;
        let id = atom(item(row_data, 3 + 2 * column)?)?;
        positions.insert(
            id.to_string(),
            position.parse::<usize>().unwrap_or(usize::MAX),
        );
    }
    let position = |id: i64| {
        positions
            .get(&id.to_string())
            .copied()
            .ok_or_else(|| anyhow::anyhow!("no column {id}"))
    };
    let reference_at = position(reference_column)?;
    let name_at = position(name_column)?;
    let rows = item(row_data, 2 + 2 * columns)?;
    let mut out = Vec::new();
    if tree {
        for root in list(rows)?.iter().skip(2) {
            let fields = list(root)?;
            let count = atom(item(fields, 2)?)?
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("bad value count"))?;
            if let Some(flag) = fields.get(3 + count)
                && flag.as_atom() == Some("1")
                && let Some(children) = fields.get(4 + count)
            {
                collect_rows(children, reference_at, name_at, &mut out)?;
            }
        }
    } else {
        collect_rows(rows, reference_at, name_at, &mut out)?;
    }
    Ok(out)
}

/// `{<tag>,<container>}` -> the container.
fn tagged(body: &Brace, tag: i64) -> Result<&Brace> {
    let members = list(body)?;
    if atom(item(members, 0)?)? != tag.to_string() {
        bail!("predefined body does not start with {tag}");
    }
    item(members, 1)
}

fn collect_rows(
    rows: &Brace,
    reference_at: usize,
    name_at: usize,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
    let members = list(rows)?;
    for row in members.iter().skip(2) {
        let fields = list(row)?;
        let count = atom(item(fields, 2)?)?
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("bad value count"))?;
        let value = |at: usize| (at < count).then(|| fields.get(3 + at)).flatten();
        if let Some(uuid) = value(reference_at).and_then(predefined_ref_uuid)
            && !super::is_nil(uuid)
            && let Some(name) = value(name_at).and_then(string_value)
        {
            out.push((uuid.to_ascii_lowercase(), name.to_string()));
        }
        if let Some(flag) = fields.get(3 + count)
            && flag.as_atom() == Some("1")
            && let Some(children) = fields.get(4 + count)
        {
            collect_rows(children, reference_at, name_at, out)?;
        }
    }
    Ok(())
}

fn predefined_ref_uuid(value: &Brace) -> Option<&str> {
    let members = value.as_list()?;
    if members.first()?.as_str()? != "#" || members.get(1)?.as_atom()? != PREDEFINED_REF_TYPE {
        return None;
    }
    members.get(2)?.as_list()?.get(1)?.as_atom()
}

fn string_value(value: &Brace) -> Option<&str> {
    let members = value.as_list()?;
    (members.first()?.as_str()? == "S")
        .then(|| members.get(1)?.as_str())
        .flatten()
}

// ---------------------------------------------------------------------------
// Comparing two indexes.

/// How one part of an index (names, types or predefined items) differs.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CategoryComparison {
    pub expected: usize,
    pub actual: usize,
    pub equal: usize,
    /// In the expected index only.
    pub missing: usize,
    /// In the built index only.
    pub extra: usize,
    /// In both, under another name.
    pub different: usize,
    /// Differences by the kind the name starts with.
    pub by_kind: BTreeMap<String, KindDifferences>,
    pub samples: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct KindDifferences {
    pub missing: usize,
    pub extra: usize,
    pub different: usize,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct IndexComparison {
    pub names: CategoryComparison,
    pub types: CategoryComparison,
    pub predefined: CategoryComparison,
}

impl IndexComparison {
    pub fn is_equal(&self) -> bool {
        [&self.names, &self.types, &self.predefined]
            .iter()
            .all(|part| part.missing == 0 && part.extra == 0 && part.different == 0)
    }
}

/// `expected` (the tree's) against `actual` (the rows').
pub fn compare(expected: &NameIndex, actual: &NameIndex, max_samples: usize) -> IndexComparison {
    IndexComparison {
        names: compare_maps(&expected.names, &actual.names, max_samples),
        types: compare_maps(&expected.types, &actual.types, max_samples),
        predefined: compare_maps(
            &flat_predefined(&expected.predefined),
            &flat_predefined(&actual.predefined),
            max_samples,
        ),
    }
}

/// `owner/uuid` -> `owner.item`: the predefined items keyed per owner.
fn flat_predefined(items: &HashMap<String, Vec<(String, String)>>) -> HashMap<String, String> {
    items
        .iter()
        .flat_map(|(uuid, items)| {
            items
                .iter()
                .map(move |(owner, name)| (format!("{owner}/{uuid}"), format!("{owner}.{name}")))
        })
        .collect()
}

fn kind_of(name: &str) -> String {
    name.split('.').next().unwrap_or_default().to_string()
}

fn compare_maps(
    expected: &HashMap<String, String>,
    actual: &HashMap<String, String>,
    max_samples: usize,
) -> CategoryComparison {
    let mut out = CategoryComparison {
        expected: expected.len(),
        actual: actual.len(),
        ..CategoryComparison::default()
    };
    let mut samples = BTreeMap::<String, Vec<String>>::new();
    let mut sample = |kind: String, text: String| {
        let entry = samples.entry(kind).or_default();
        if entry.len() < max_samples {
            entry.push(text);
        }
    };
    for (key, name) in expected {
        match actual.get(key) {
            Some(found) if found == name => out.equal += 1,
            Some(found) => {
                out.different += 1;
                let kind = kind_of(name);
                out.by_kind.entry(kind.clone()).or_default().different += 1;
                sample(kind, format!("{key}: {name} != {found}"));
            }
            None => {
                out.missing += 1;
                let kind = kind_of(name);
                out.by_kind.entry(kind.clone()).or_default().missing += 1;
                sample(kind, format!("missing {key}: {name}"));
            }
        }
    }
    for (key, found) in actual {
        if !expected.contains_key(key) {
            out.extra += 1;
            let kind = kind_of(found);
            out.by_kind.entry(kind.clone()).or_default().extra += 1;
            sample(kind, format!("extra {key}: {found}"));
        }
    }
    out.samples = samples.into_values().flatten().collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::parse_row;

    #[test]
    fn reads_the_items_of_a_catalog_tree_with_children() {
        let text = concat!(
            "{0,{1,{7,{0,\"\",{\"Pattern\",{\"#\",ae135932-4f94-44df-92c1-c91f15a92848}},\"\",0},",
            "{1,\"\",{\"Pattern\",{\"B\"}},\"\",0},",
            "{2,\"\",{\"Pattern\",{\"#\",ae135932-4f94-44df-92c1-c91f15a92848}},\"\",0},",
            "{3,\"\",{\"Pattern\",{\"S\"}},\"\",0},{4,\"\",{\"Pattern\",{\"S\"}},\"\",0},",
            "{5,\"\",{\"Pattern\",{\"S\"}},\"\",0},{6,\"\",{\"Pattern\",{\"N\"}},\"\",0}},",
            "{2,7,0,0,1,1,2,2,3,3,4,4,5,5,6,6,{1,1,",
            "{2,0,4,{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,00000000-0000-0000-0000-000000000000}},{\"B\",1},",
            "{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,00000000-0000-0000-0000-000000000000}},{\"S\",\"Элементы\"},1,",
            "{1,1,{2,1,7,{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,11111111-1111-1111-1111-111111111111}},{\"B\",1},",
            "{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,00000000-0000-0000-0000-000000000000}},{\"S\",\"Группа\"},{\"S\",\"\"},{\"S\",\"\"},{\"N\",0},1,",
            "{1,1,{2,2,7,{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,22222222-2222-2222-2222-222222222222}},{\"B\",0},",
            "{\"#\",ae135932-4f94-44df-92c1-c91f15a92848,{1,00000000-0000-0000-0000-000000000000}},{\"S\",\"Элемент\"},{\"S\",\"\"},{\"S\",\"\"},{\"N\",0},0}}}}}},-1,2}}}"
        );
        let body = parse_row(text.as_bytes()).unwrap();
        assert_eq!(
            predefined_items("Catalog", &body).unwrap(),
            vec![
                (
                    "11111111-1111-1111-1111-111111111111".to_string(),
                    "Группа".to_string()
                ),
                (
                    "22222222-2222-2222-2222-222222222222".to_string(),
                    "Элемент".to_string()
                ),
            ]
        );
    }

    #[test]
    fn the_root_row_lists_give_each_top_level_object_its_kind() {
        let text = concat!(
            "{2,{\"#\",9fcd25a0-4822-11d4-9414-008048da11f9,",
            "{cf4abea6-37b2-11d4-940f-008048da11f9,2,",
            "aaaaaaaa-0000-0000-0000-000000000001,aaaaaaaa-0000-0000-0000-000000000002},",
            "{f6a80749-5ad7-400b-8519-39dc5dff2542,1,bbbbbbbb-0000-0000-0000-000000000001},",
            "{0195e80c-b157-11d4-9435-004095e12fc7,0}}}"
        );
        let kinds = root_kinds(&parse_row(text.as_bytes()).unwrap());
        assert_eq!(kinds.len(), 3);
        assert_eq!(kinds["aaaaaaaa-0000-0000-0000-000000000002"], "Catalog");
        assert_eq!(kinds["bbbbbbbb-0000-0000-0000-000000000001"], "Enum");
        assert_eq!(
            root_members(&parse_row(text.as_bytes()).unwrap()),
            vec![
                (
                    "Catalog",
                    "aaaaaaaa-0000-0000-0000-000000000001".to_string()
                ),
                (
                    "Catalog",
                    "aaaaaaaa-0000-0000-0000-000000000002".to_string()
                ),
                ("Enum", "bbbbbbbb-0000-0000-0000-000000000001".to_string()),
            ],
            "root_members keeps the row's order"
        );
    }

    #[test]
    fn an_owner_row_lists_its_forms_and_templates_by_class() {
        let text = concat!(
            "{1,{57,{1,0,cccccccc-0000-0000-0000-000000000000},",
            "{fdf816d2-1ead-11d5-b975-0050bae0a95d,1,dddddddd-0000-0000-0000-000000000001},",
            "{3daea016-69b7-4ed4-9453-127911372fe6,2,",
            "eeeeeeee-0000-0000-0000-000000000001,eeeeeeee-0000-0000-0000-000000000002},",
            "{cf4abea7-37b2-11d4-940f-008048da11f9,0}}}"
        );
        assert!(may_own_objects(text));
        let owned = owned_objects(&parse_row(text.as_bytes()).unwrap());
        assert_eq!(
            owned,
            vec![
                ("Form", "dddddddd-0000-0000-0000-000000000001".to_string()),
                (
                    "Template",
                    "eeeeeeee-0000-0000-0000-000000000001".to_string()
                ),
                (
                    "Template",
                    "eeeeeeee-0000-0000-0000-000000000002".to_string()
                ),
            ]
        );
    }

    #[test]
    fn an_owned_row_names_itself_from_its_first_header() {
        let text = concat!(
            "{1,{0,{13,{3,{1,0,aaaaaaaa-0000-0000-0000-000000000001},\"Форма\",{0},\"\",0,0,",
            "00000000-0000-0000-0000-000000000000,0},0,{1,{3,{1,0,bbbbbbbb-0000-0000-0000-000000000001},",
            "\"Вложенная\",{0},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}}}}"
        );
        let row = parse_row(text.as_bytes()).unwrap();
        assert_eq!(
            own_header(&row),
            Some((
                "aaaaaaaa-0000-0000-0000-000000000001".to_string(),
                "Форма".to_string()
            ))
        );
    }

    #[test]
    fn predefined_items_are_kept_per_owner() {
        let mut index = NameIndex::default();
        index.insert_predefined("Catalog.A", "11111111-1111-1111-1111-111111111111", "Один");
        index.insert_predefined(
            "Catalog.B",
            "11111111-1111-1111-1111-111111111111",
            "Другой",
        );
        index.insert_predefined(
            "Catalog.C",
            "22222222-2222-2222-2222-222222222222",
            "Третий",
        );
        assert_eq!(
            index.predefined("11111111-1111-1111-1111-111111111111"),
            None
        );
        assert_eq!(
            index.predefined_in("Catalog.B", "11111111-1111-1111-1111-111111111111"),
            Some("Другой")
        );
        assert_eq!(
            index.predefined("22222222-2222-2222-2222-222222222222"),
            Some("Третий")
        );
        assert_eq!(index.sizes().2, 3);
    }

    #[test]
    fn a_comparison_names_what_is_missing_extra_and_different() {
        let mut expected = NameIndex::default();
        expected.insert_name("a", "Catalog.A");
        expected.insert_name("b", "Document.B");
        expected.insert_name("c", "Enum.C");
        let mut actual = NameIndex::default();
        actual.insert_name("a", "Catalog.A");
        actual.insert_name("b", "Document.X");
        actual.insert_name("d", "Enum.D");
        let comparison = compare(&expected, &actual, 5);
        assert_eq!(comparison.names.equal, 1);
        assert_eq!(comparison.names.different, 1);
        assert_eq!(comparison.names.missing, 1);
        assert_eq!(comparison.names.extra, 1);
        assert!(!comparison.is_equal());
    }
}
