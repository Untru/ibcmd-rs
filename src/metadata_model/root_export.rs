//! The export direction of the Configuration row: the stored row -> the
//! `<Configuration>` element of `Configuration.xml`, and the objects the row
//! lists -- what a name index built from rows needs first.
//!
//! The row is read back field by field the way `root.rs` compiles it: every
//! field `root.rs` derives or keeps constant is checked, so a row this file
//! decodes compiles back to itself, and a row holding a value no corpus has
//! shown is refused instead of printed wrong. Properties the row does not
//! carry at all (`ScriptVariant`, `DataLockControlMode`, ...) are printed at
//! the only value every corpus has, which is what `root.rs` accepts.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};

use super::{
    ConfigurationShape, DESIGN_TIME_REFERENCE_CLASS_ID, FUNCTIONALITIES, SECTIONS,
    SHARE_REQUEST_TYPE_CLASS_ID, USE_PURPOSE_CLASS_ID, Wrapper, permission_flags, permission_table,
    permissions_of,
};
use crate::metadata_model::brace::{Brace, NIL_UUID, serialize};
use crate::metadata_model::export::values::{header, localized_element, reference_name};
use crate::metadata_model::export::{
    Build, ExportContext, ObjectNames, atom, el, item, leaf, list, number, short, string, xml_text,
};
use crate::metadata_model::xml::Element;

/// The row taken apart: its identity, the seven sections' contained objects
/// and slots, and the properties tuple.
struct RootRow<'a> {
    uuid: String,
    /// (section class id, contained object id), in stored order.
    contained: Vec<(&'static str, String)>,
    /// (kind, uuids) of every non-empty slot, in stored order.
    objects: Vec<(&'static str, Vec<String>)>,
    tuple: &'a [Brace],
    shape: ConfigurationShape,
}

fn identity_of(node: &Brace) -> Result<String> {
    // `{1,0,<uuid>}`
    let items = list(node)?;
    if items.len() != 3 || atom(item(items, 0)?)? != "1" || atom(item(items, 1)?)? != "0" {
        bail!("not an object identity: {}", short(node));
    }
    Ok(atom(item(items, 2)?)?.to_ascii_lowercase())
}

/// The stored shape of a Configuration row's properties tuple: `{76,...}`
/// for a configuration kept in compatibility 8.5 or later.
pub(crate) fn stored_shape(row: &Brace) -> Result<ConfigurationShape> {
    Ok(read_row(row)?.shape)
}

/// The configuration uuid the `root` row names: `{2,<uuid>,...}`.
pub(crate) fn root_configuration_uuid(row: &Brace) -> Result<String> {
    let items = list(row)?;
    if atom(item(items, 0)?)? != "2" {
        bail!("the root row does not open with 2");
    }
    Ok(atom(item(items, 1)?)?.to_ascii_lowercase())
}

fn read_row(row: &Brace) -> Result<RootRow<'_>> {
    let items = list(row)?;
    if atom(item(items, 0)?)? != "2" {
        bail!("the Configuration row does not open with 2");
    }
    let uuid = atom(item(list(item(items, 1)?)?, 0)?)?.to_ascii_lowercase();
    let count = number(item(items, 2)?)? as usize;
    if count != SECTIONS.len() || items.len() != 3 + count + 1 {
        bail!(
            "the Configuration row holds {count} sections in {} members; every corpus has 7 in 11",
            items.len()
        );
    }
    let mut contained = Vec::with_capacity(SECTIONS.len());
    let mut objects = Vec::new();
    let mut tuple = None;
    for (index, section) in SECTIONS.iter().enumerate() {
        let node = list(item(items, 3 + index)?)?;
        let class_id = atom(item(node, 0)?)?;
        if class_id != section.class_id {
            bail!(
                "section {index} is {class_id}, expected {}",
                section.class_id
            );
        }
        let body = list(item(node, 1)?)?;
        // (contained object id, slot count at, first slot at, slot list)
        let (object_id, members): (String, &[Brace]) = match section.wrapper {
            Wrapper::Properties => {
                if atom(item(body, 0)?)? != "1" {
                    bail!("the properties section does not open with 1");
                }
                let properties = list(item(body, 1)?)?;
                tuple = Some(properties);
                // The properties header is keyed by the module group.
                let first = list(item(properties, 1)?)?;
                let head = header(item(first, 1)?)?;
                (head.uuid.to_ascii_lowercase(), &body[2..])
            }
            Wrapper::Directory => {
                if atom(item(body, 0)?)? != "6" {
                    bail!("the directory section does not open with 6");
                }
                let inner = list(item(body, 1)?)?;
                if atom(item(inner, 0)?)? != "1" {
                    bail!("the directory section body does not open with 1");
                }
                let identity = list(item(inner, 1)?)?;
                if atom(item(identity, 1)?)? != NIL_UUID {
                    bail!("the directory section keeps a second uuid");
                }
                (identity_of(item(identity, 0)?)?, &inner[2..])
            }
            Wrapper::Zero => {
                if atom(item(body, 0)?)? != "1" {
                    bail!("section {index} does not open with 1");
                }
                let identity = list(item(body, 1)?)?;
                if atom(item(identity, 0)?)? != "0" {
                    bail!("section {index} identity does not open with 0");
                }
                (identity_of(item(identity, 1)?)?, &body[2..])
            }
            Wrapper::Direct => {
                if atom(item(body, 0)?)? != "1" {
                    bail!("section {index} does not open with 1");
                }
                let identity = list(item(body, 1)?)?;
                (identity_of(item(identity, 0)?)?, &body[2..])
            }
        };
        contained.push((section.class_id, object_id));
        let slot_count = number(item(members, 0)?)? as usize;
        let slots = &members[1..];
        if slots.len() != slot_count {
            bail!(
                "section {index} declares {slot_count} slots and holds {}",
                slots.len()
            );
        }
        for slot in slots {
            let slot = list(slot)?;
            let class_id = atom(item(slot, 0)?)?;
            let n = number(item(slot, 1)?)? as usize;
            let uuids = slot[2..]
                .iter()
                .map(|uuid| atom(uuid).map(str::to_ascii_lowercase))
                .collect::<Result<Vec<_>>>()?;
            if uuids.len() != n {
                bail!(
                    "slot {class_id} declares {n} objects and lists {}",
                    uuids.len()
                );
            }
            let known = section
                .slots
                .iter()
                .find(|candidate| candidate.class_id == class_id)
                .ok_or_else(|| anyhow!("section {index} has an unknown slot {class_id}"))?;
            if uuids.is_empty() {
                continue;
            }
            let kind = known.kind.ok_or_else(|| {
                anyhow!("slot {class_id}, which no XML kind fills, lists objects")
            })?;
            objects.push((kind, uuids));
        }
    }
    let tuple = tuple.ok_or_else(|| anyhow!("the Configuration row has no properties"))?;
    let shape = match (atom(item(tuple, 0)?)?, tuple.len()) {
        ("67", 60) => ConfigurationShape::V67,
        ("68", 61) => ConfigurationShape::V68,
        ("76", 77) => ConfigurationShape::V76,
        (tag, len) => {
            bail!("a {{{tag},...}} properties tuple of {len} fields has no known reading")
        }
    };
    Ok(RootRow {
        uuid,
        contained,
        objects,
        tuple,
        shape,
    })
}

/// A Configuration row a later platform staged, in the `{68}` shape, whose
/// extension compatibility mode (field 43) is the edition of that platform
/// while the compatibility mode (field 26) is the configuration's own: the
/// row [`decode`] used to refuse, because no corpus showed which of the two
/// the platform prints. The native export of such a stage shows it: field 26 is
/// `CompatibilityMode` and field 43 `ConfigurationExtensionCompatibilityMode`
/// (`Version8_3_24` and `Version8_3_27` for the БСП demo), and [`decode`] reads
/// the row that way on the platform that writes the shape. The pair of a `{67}`
/// row and this row appears in the
/// staged image of every native import of a configuration kept in an older
/// compatibility mode (`docs/apply/restructuring-check.md`, section 8.9): the
/// import leaves the compatibility mode alone (it decides the record
/// versions) and stores its own edition as the extension one.
///
/// Returns the row with field 43 set to field 26's value (which [`decode`]
/// accepts) and the value field 43 held, for the caller to put back into the
/// decoded `ConfigurationExtensionCompatibilityMode`. `None` when the row is
/// not of that kind (the fields agree, the extension mode is the older one,
/// the shape is another, the row is not a Configuration row): [`decode`] then
/// speaks for it. The comparison of a stage with a tree calls this to read the
/// row as the tree's `{67}`-shaped twin; the export reads the split row itself.
pub(crate) fn fold_split_compatibility(row: &Brace) -> Option<(Brace, u32)> {
    let parsed = read_row(row).ok()?;
    if parsed.shape != ConfigurationShape::V68 {
        return None;
    }
    let compatibility = packed(parsed.tuple, 26).ok()?;
    let extension = packed(parsed.tuple, 43).ok()?;
    if extension <= compatibility {
        return None;
    }
    let section = SECTIONS
        .iter()
        .position(|section| matches!(section.wrapper, Wrapper::Properties))?;
    let mut folded = row.clone();
    let tuple = folded
        .as_list_mut()?
        .get_mut(3 + section)?
        .as_list_mut()?
        .get_mut(1)?
        .as_list_mut()?
        .get_mut(1)?
        .as_list_mut()?;
    *tuple.get_mut(43)? = Brace::atom(compatibility);
    Some((folded, extension))
}

/// (kind, uuid) of every object the configuration lists, in stored order.
pub fn top_level_objects(row: &Brace) -> Result<Vec<(String, String)>> {
    let row = read_row(row)?;
    Ok(row
        .objects
        .into_iter()
        .flat_map(|(kind, uuids)| uuids.into_iter().map(move |uuid| (kind.to_string(), uuid)))
        .collect())
}

/// What the Configuration row contributes to a name index: itself (under
/// the name `Configuration`, as the tree's index keys it). The objects it
/// lists are named by their own rows; `top_level_objects` says which rows
/// those are.
pub(crate) fn names(row: &Brace) -> Result<ObjectNames> {
    let row = read_row(row)?;
    Ok(ObjectNames {
        uuid: row.uuid,
        full_name: "Configuration".to_string(),
        children: Vec::new(),
        types: Vec::new(),
    })
}

/// Fields 67 and 68 of an 8.5 `{76,...}` tuple: `0,0` where БСП 3.2.1.356
/// (8.5.1.1150) keeps `OpenDataInTabs` and `Use`, `1,1` in the configuration
/// 8.5.1.1529 saved from an XML 2.20 tree, which names neither property
/// (`home_page/one_column_v85/input.cf`, `_onecdec/make_home_page_fixtures.py`):
/// the values 8.5 gives a configuration without them, which it prints for a
/// `{68,...}` tuple as `OpenDataInDialogs` and `DontUse` (ERP УХ under
/// 8.5.1.1150). Which of the two fields holds which property is not known;
/// any other pair is refused. `true` for the second pair.
fn v76_defaults_of_8_3(tuple: &[Brace]) -> Result<bool> {
    let pair = (
        serialize(item(tuple, 67)?).replace("\r\n", ""),
        serialize(item(tuple, 68)?).replace("\r\n", ""),
    );
    match (pair.0.as_str(), pair.1.as_str()) {
        ("0", "0") => Ok(false),
        ("1", "1") => Ok(true),
        (a, b) => bail!("Configuration fields 67, 68 hold {a}, {b}; no known configuration does"),
    }
}

/// A field the compiler writes as a constant: checked, not decoded.
fn expect(tuple: &[Brace], index: usize, expected: &str) -> Result<()> {
    let found = serialize(item(tuple, index)?).replace("\r\n", "");
    if found != expected {
        bail!(
            "Configuration field {index} holds {found}; every known configuration holds {expected}"
        );
    }
    Ok(())
}

fn nil_or_reference(
    tuple: &[Brace],
    index: usize,
    kind: &str,
    context: &ExportContext,
) -> Result<String> {
    let uuid = atom(item(tuple, index)?)?.to_ascii_lowercase();
    let name = reference_name(&uuid, &context.names)?;
    if !name.is_empty() && !name.starts_with(&format!("{kind}.")) {
        bail!("Configuration field {index} names {name}, not a {kind}");
    }
    Ok(name)
}

fn flag(tuple: &[Brace], index: usize) -> Result<&'static str> {
    match atom(item(tuple, index)?)? {
        "0" => Ok("false"),
        "1" => Ok("true"),
        other => bail!("Configuration field {index} holds {other}, not a flag"),
    }
}

fn code(tuple: &[Brace], index: usize, table: &[(&'static str, &str)]) -> Result<&'static str> {
    let value = atom(item(tuple, index)?)?;
    table
        .iter()
        .find(|(_, candidate)| *candidate == value)
        .map(|(text, _)| *text)
        .ok_or_else(|| {
            anyhow!("Configuration field {index} holds {value}, which has no known name")
        })
}

/// `80324` -> `Version8_3_24`.
pub(crate) fn version_text(value: u32) -> String {
    format!(
        "Version{}_{}_{}",
        value / 10000,
        (value / 100) % 100,
        value % 100
    )
}

fn packed(tuple: &[Brace], index: usize) -> Result<u32> {
    let value = atom(item(tuple, index)?)?;
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value >= 80000)
        .ok_or_else(|| anyhow!("Configuration field {index} holds {value}, not a platform version"))
}

/// The top-level objects by kind in `<ChildObjects>` order, as XML elements.
fn child_objects(row: &RootRow<'_>, context: &ExportContext) -> Result<Element> {
    let mut groups = Vec::with_capacity(row.objects.len());
    let mut seen = BTreeSet::new();
    for (kind, uuids) in &row.objects {
        let (order, _) = ibcmd_schema::configuration_root::child_xml_kind(kind)
            .ok_or_else(|| anyhow!("no corpus shows where {kind} objects are listed"))?;
        if !seen.insert(*kind) {
            bail!("two slots list {kind} objects");
        }
        groups.push((order, *kind, uuids));
    }
    groups.sort_by_key(|(order, _, _)| *order);
    let mut element = el("ChildObjects");
    for (_, kind, uuids) in groups {
        for uuid in uuids {
            let full = context
                .names
                .name(uuid)
                .ok_or_else(|| anyhow!("no name for the {kind} {uuid}"))?;
            let name = full
                .strip_prefix(kind)
                .and_then(|rest| rest.strip_prefix('.'))
                .filter(|name| !name.is_empty() && !name.contains('.'))
                .ok_or_else(|| anyhow!("{uuid} is {full}, not a top-level {kind}"))?;
            element.children.push(leaf(kind, name));
        }
    }
    Ok(element)
}

/// Decodes the Configuration row into its `<Configuration>` element.
pub(crate) fn decode(row: &Brace, context: &ExportContext) -> Result<Element> {
    let parsed = read_row(row)?;
    let t = parsed.tuple;
    let shape = parsed.shape;
    let xml_2_21 = context.is_xml_2_21();
    // The platform the tree is written for reads the tuple shapes up to the
    // one it writes itself.
    let platform = context.platform();
    let own_shape = ConfigurationShape::for_compatibility(platform.compatibility_packed());
    if shape > own_shape {
        bail!(
            "{platform} does not read a configuration kept in compatibility {}",
            if shape == ConfigurationShape::V76 {
                "8.5"
            } else {
                "newer than its own"
            }
        );
    }

    // The constants `root.rs` writes.
    let nil = NIL_UUID;
    for (index, expected) in [
        (11, nil),
        (17, "1"),
        (18, "{0,0}"),
        (19, "1"),
        (20, "{0,0}"),
        (27, "{0,0}"),
        (34, nil),
        (35, nil),
        (42, "\"\""),
        (44, "1"),
        (45, "0"),
        (46, nil),
        (47, nil),
        (48, nil),
        (49, "1"),
        (50, nil),
        (52, "{0,0}"),
        (54, "{0}"),
        (55, nil),
        (56, "\"\""),
        (57, "0"),
        (58, "1"),
    ] {
        expect(t, index, expected)?;
    }
    match shape {
        ConfigurationShape::V67 => {}
        ConfigurationShape::V68 => expect(t, 60, "1")?,
        ConfigurationShape::V76 => {
            for (index, expected) in [(60, "1"), (61, "0"), (63, "0"), (66, "0")] {
                expect(t, index, expected)?;
            }
            // 67 and 68 are read below (`v76_defaults_of_8_3`).
            v76_defaults_of_8_3(t)?;
            for index in 69..77 {
                expect(t, index, nil)?;
            }
        }
    }

    // The header, keyed by the module group.
    let first = list(item(t, 1)?)?;
    if atom(item(first, 0)?)? != "0" {
        bail!("the Configuration header wrapper does not open with 0");
    }
    let head = header(item(first, 1)?)?;

    // Compatibility: fields 26 and 43 hold the same value in every row
    // `root.rs` writes and in every corpus; the 60-field shape is read at 43,
    // the others at 26.
    //
    // The one row that differs is the one a native import stages for an older
    // configuration: it is rewritten to the shape the platform writes, with the
    // configuration's own compatibility in field 26 and the platform's edition
    // in field 43 (80324 and 80327 in the stage of the БСП demo, twice). The
    // platform prints field 26 as `CompatibilityMode` and field 43 as
    // `ConfigurationExtensionCompatibilityMode`.
    let compat_26 = packed(t, 26)?;
    let compat_43 = packed(t, 43)?;
    let staged = compat_43 > compat_26
        && shape == ConfigurationShape::V68
        && own_shape == ConfigurationShape::V68;
    if compat_26 != compat_43 && !staged {
        bail!(
            "Configuration fields 26 and 43 hold {compat_26} and {compat_43}; no corpus shows which one the platform prints"
        );
    }
    if compat_26.max(compat_43) > platform.compatibility_packed() {
        bail!(
            "compatibility {} is newer than the platform the tree is written for ({platform})",
            compat_26.max(compat_43)
        );
    }
    let compatibility = version_text(compat_26);
    // The extension compatibility the platform prints: its own edition for
    // an older tuple and for ordinary V76; preserve the staged V68 rule.
    let extension_compatibility = if shape < own_shape || shape == ConfigurationShape::V76 {
        // Ordinary V76 at stored 80327/80327 still prints 80501 when read
        // by 8.5 (native 8.5.1.1529). The reader edition is independent of
        // the stored compatibility used by the inverse compiler.
        platform.compatibility_mode()
    } else if staged {
        version_text(compat_43)
    } else {
        compatibility.clone()
    };

    // Default roles; field 12 keeps the first of them.
    let roles_node = list(item(t, 39)?)?;
    if atom(item(roles_node, 0)?)? != "0" {
        bail!("the default roles list does not open with 0");
    }
    let role_count = number(item(roles_node, 1)?)? as usize;
    if roles_node.len() != 2 + role_count {
        bail!(
            "the default roles list declares {role_count} roles and holds {}",
            roles_node.len() - 2
        );
    }
    let mut roles = Vec::with_capacity(role_count);
    let mut role_uuids = Vec::with_capacity(role_count);
    for role in &roles_node[2..] {
        let fields = list(role)?;
        if string(item(fields, 0)?)? != "#"
            || atom(item(fields, 1)?)? != DESIGN_TIME_REFERENCE_CLASS_ID
        {
            bail!(
                "a default role is not a design-time reference: {}",
                short(role)
            );
        }
        let target = list(item(fields, 2)?)?;
        if atom(item(target, 0)?)? != "1" {
            bail!("a default role reference does not open with 1");
        }
        let uuid = atom(item(target, 1)?)?.to_ascii_lowercase();
        let name = reference_name(&uuid, &context.names)?;
        if !name.starts_with("Role.") {
            bail!("default role {uuid} is {name}");
        }
        role_uuids.push(uuid);
        roles.push(name);
    }
    let main_role = role_uuids.first().map_or(NIL_UUID, String::as_str);
    expect(t, 12, main_role)?;

    // Use purposes.
    let purposes = list(item(t, 33)?)?;
    let purpose_count = number(item(purposes, 0)?)? as usize;
    if purposes.len() != 1 + purpose_count {
        bail!(
            "the use purposes declare {purpose_count} and hold {}",
            purposes.len() - 1
        );
    }
    let mut use_purposes = el("UsePurposes");
    for purpose in &purposes[1..] {
        let fields = list(purpose)?;
        if string(item(fields, 0)?)? != "#" || atom(item(fields, 1)?)? != USE_PURPOSE_CLASS_ID {
            bail!("a use purpose is not typed: {}", short(purpose));
        }
        let name = match atom(item(fields, 2)?)? {
            "1" => "PlatformApplication",
            other => bail!("use purpose {other} has no known name"),
        };
        use_purposes
            .children
            .push(leaf("v8:Value", name).attr("type", "app:ApplicationUsePurpose"));
    }

    // Mobile functionalities, and the permission fields derived from them.
    let table = list(item(t, 53)?)?;
    if atom(item(table, 0)?)? != "2" {
        bail!("the mobile functionality table does not open with 2");
    }
    let pairs = number(item(table, 1)?)? as usize;
    let expected_pairs = match shape {
        ConfigurationShape::V67 => FUNCTIONALITIES.len() - 1,
        _ => FUNCTIONALITIES.len(),
    };
    if pairs != expected_pairs || table.len() != 2 + pairs + 1 {
        bail!(
            "the mobile functionality table holds {pairs} pairs in {} members",
            table.len()
        );
    }
    let mut used = BTreeSet::new();
    for ((id, _), pair) in FUNCTIONALITIES.iter().zip(&table[2..2 + pairs]) {
        let fields = list(pair)?;
        if number(item(fields, 0)?)? != i64::from(*id) {
            bail!("mobile functionality {id} is out of order: {}", short(pair));
        }
        match atom(item(fields, 1)?)? {
            "0" => {}
            "1" => {
                used.insert(*id);
            }
            other => bail!("mobile functionality {id} holds {other}"),
        }
    }
    let tail = atom(item(table, 2 + pairs)?)?;
    match (shape, tail) {
        (ConfigurationShape::V67, "1") => {
            used.insert(41);
        }
        (ConfigurationShape::V67, "0") | (_, "0") => {}
        (_, other) => bail!(
            "the mobile functionality table ends with {other}: permission messages are not decoded yet"
        ),
    }
    let permissions = permissions_of(&used);
    let expected_40 = serialize(&permission_table(&permissions)).replace("\r\n", "");
    expect(t, 40, &expected_40)?;
    let expected_51 = serialize(&permission_flags(&permissions)).replace("\r\n", "");
    expect(t, 51, &expected_51)?;
    let mut functionalities = el("UsedMobileApplicationFunctionalities");
    for (id, name) in FUNCTIONALITIES {
        functionalities.children.push(
            el("app:functionality")
                .child(leaf("app:functionality", name))
                .child(leaf(
                    "app:use",
                    if used.contains(&id) { "true" } else { "false" },
                )),
        );
    }

    // Incoming share request types.
    let share = list(item(t, 59)?)?;
    let share_count = number(item(share, 0)?)? as usize;
    if share.len() != 1 + share_count {
        bail!(
            "the share request types declare {share_count} and hold {}",
            share.len() - 1
        );
    }
    let mut share_types = el("AllowedIncomingShareRequestTypes");
    for entry in &share[1..] {
        let typed = list(entry)?;
        if string(item(typed, 0)?)? != "#" || atom(item(typed, 1)?)? != SHARE_REQUEST_TYPE_CLASS_ID
        {
            bail!("a share request type is not typed: {}", short(entry));
        }
        let payload = list(item(typed, 2)?)?;
        if payload.len() != 6 || atom(item(payload, 0)?)? != "0" {
            bail!("a share request type has no known shape: {}", short(entry));
        }
        share_types.children.push(
            el("v8:Value")
                .attr("type", "app:AllowedIncomingShareRequestType")
                .child(leaf("app:mime", string(item(payload, 1)?)?))
                .child(leaf("app:uti", string(item(payload, 2)?)?))
                .child(leaf("app:ext", string(item(payload, 3)?)?))
                .child(
                    leaf("app:processingVariant", atom(item(payload, 4)?)?)
                        .attr("type", "xs:decimal"),
                )
                .child(leaf(
                    "app:isCustom",
                    match atom(item(payload, 5)?)? {
                        "0" => "false",
                        "1" => "true",
                        other => bail!("a share request type's custom flag holds {other}"),
                    },
                )),
        );
    }

    let interface_compatibility = match shape {
        ConfigurationShape::V76 => {
            let first = atom(item(t, 38)?)?;
            let second = atom(item(t, 62)?)?;
            let codes = first.parse::<u8>().ok().zip(second.parse::<u8>().ok());
            let mode = codes.and_then(|(a, b)| {
                // Stored single-byte scalar coordinates, not padded numbers.
                (first.len() == 1 && second.len() == 1)
                    .then(|| ibcmd_schema::configuration_root::V76InterfaceCompatibility::from_stored_codes(a, b))
                    .flatten()
            });
            let mode = mode.ok_or_else(|| {
                anyhow!("interface compatibility {first}/{second} has no known V76 pair")
            })?;
            if !mode.supports_xml_dialect(&context.version) {
                bail!(
                    "Configuration <InterfaceCompatibilityMode> {} requires XML 2.21; selected edition is {}",
                    mode.xml_name(),
                    context.version
                );
            }
            mode.xml_name()
        }
        _ => code(
            t,
            38,
            &[
                ("Version8_2", "0"),
                ("TaxiEnableVersion8_2", "2"),
                ("Taxi", "3"),
            ],
        )?,
    };
    let modes = [("Use", "0"), ("UseWithWarnings", "1"), ("DontUse", "2")];

    let mut p = el("Properties");
    let mut push = |element: Element| p.children.push(element);
    push(leaf("Name", head.name.clone()));
    push(localized_element("Synonym", &head.synonym)?);
    push(leaf("Comment", xml_text(&head.comment)));
    push(leaf("NamePrefix", xml_text(string(item(t, 2)?)?)));
    push(leaf(
        "ConfigurationExtensionCompatibilityMode",
        extension_compatibility,
    ));
    push(leaf(
        "DefaultRunMode",
        code(
            t,
            21,
            &[("OrdinaryApplication", "0"), ("ManagedApplication", "1")],
        )?,
    ));
    push(use_purposes);
    // Field 3 (`1` Russian, `0` English; see the compiler's root layout).
    push(leaf(
        "ScriptVariant",
        code(t, 3, &[("English", "0"), ("Russian", "1")])?,
    ));
    push(
        el("DefaultRoles").children(
            roles
                .into_iter()
                .map(|role| leaf("xr:Item", role).attr("type", "xr:MDObjectRef")),
        ),
    );
    push(leaf("Vendor", xml_text(string(item(t, 14)?)?)));
    push(leaf("Version", xml_text(string(item(t, 15)?)?)));
    push(leaf(
        "UpdateCatalogAddress",
        xml_text(string(item(t, 16)?)?),
    ));
    push(leaf("IncludeHelpInContents", flag(t, 13)?));
    push(leaf("UseManagedFormInOrdinaryApplication", flag(t, 28)?));
    push(leaf("UseOrdinaryFormInManagedApplication", flag(t, 29)?));
    push(el("AdditionalFullTextSearchDictionaries"));
    push(leaf(
        "CommonSettingsStorage",
        nil_or_reference(t, 22, "SettingsStorage", context)?,
    ));
    push(leaf(
        "ReportsUserSettingsStorage",
        nil_or_reference(t, 23, "SettingsStorage", context)?,
    ));
    push(leaf(
        "ReportsVariantsStorage",
        nil_or_reference(t, 24, "SettingsStorage", context)?,
    ));
    push(leaf(
        "FormDataSettingsStorage",
        nil_or_reference(t, 25, "SettingsStorage", context)?,
    ));
    push(el("DynamicListsUserSettingsStorage"));
    push(el("URLExternalDataStorage"));
    push(el("Content"));
    push(leaf(
        "DefaultReportForm",
        nil_or_reference(t, 30, "CommonForm", context)?,
    ));
    push(leaf(
        "DefaultReportVariantForm",
        nil_or_reference(t, 31, "CommonForm", context)?,
    ));
    push(leaf(
        "DefaultReportSettingsForm",
        nil_or_reference(t, 32, "CommonForm", context)?,
    ));
    for name in [
        "DefaultReportAppearanceTemplate",
        "DefaultDynamicListSettingsForm",
    ] {
        push(el(name));
    }
    // Field 37, written by `root.rs` (Управление задачами names
    // `CommonForm.ФормаПоиска` there).
    push(leaf(
        "DefaultSearchForm",
        nil_or_reference(t, 37, "CommonForm", context)?,
    ));
    for name in [
        "DefaultDataHistoryChangeHistoryForm",
        "DefaultDataHistoryVersionDataForm",
        "DefaultDataHistoryVersionDifferencesForm",
        "DefaultCollaborationSystemUsersChoiceForm",
    ] {
        push(el(name));
    }
    if xml_2_21 {
        for name in [
            "AuxiliaryReportForm",
            "AuxiliaryReportVariantForm",
            "AuxiliaryReportSettingsForm",
            "AuxiliaryDynamicListSettingsForm",
            "AuxiliaryDataHistoryChangeHistoryForm",
            "AuxiliaryDataHistoryVersionDataForm",
            "AuxiliaryDataHistoryVersionDifferencesForm",
            "AuxiliaryCollaborationSystemUsersChoiceForm",
        ] {
            push(el(name));
        }
    }
    push(el("RequiredMobileApplicationPermissions"));
    push(functionalities);
    push(el("StandaloneConfigurationRestrictionRoles"));
    push(el("MobileApplicationURLs"));
    push(share_types);
    // A tuple older than 8.5 read by 8.5 gets the defaults 8.5 gives such a
    // configuration (ERP УХ); the 8.5 tuple stores either БСП's set or those
    // same defaults (`v76_defaults_of_8_3`), checked above.
    let tuple_8_5_1 = shape == ConfigurationShape::V76 && !v76_defaults_of_8_3(t)?;
    if xml_2_21 {
        push(leaf(
            "MainClientApplicationWindowInterfaceVariant",
            "NavigationLeft",
        ));
        push(leaf("ClientApplicationTheme", "Auto"));
    }
    push(leaf("MainClientApplicationWindowMode", "Normal"));
    if xml_2_21 {
        push(leaf(
            "ClientApplicationWindowsOpenVariant",
            if tuple_8_5_1 {
                "OpenDataInTabs"
            } else {
                "OpenDataInDialogs"
            },
        ));
    }
    push(el("DefaultInterface"));
    if xml_2_21 {
        if shape == ConfigurationShape::V76 {
            push(localized_element("Caption", item(t, 64)?)?);
            push(localized_element("ShortCaption", item(t, 65)?)?);
        } else {
            push(el("Caption"));
            push(el("ShortCaption"));
        }
    }
    push(leaf(
        "DefaultStyle",
        nil_or_reference(t, 9, "Style", context)?,
    ));
    push(leaf(
        "DefaultLanguage",
        nil_or_reference(t, 10, "Language", context)?,
    ));
    // Field 4 is the detailed text, 5 the brief one.
    push(localized_element("BriefInformation", item(t, 5)?)?);
    push(localized_element("DetailedInformation", item(t, 4)?)?);
    push(localized_element("Copyright", item(t, 6)?)?);
    push(localized_element("VendorInformationAddress", item(t, 7)?)?);
    push(localized_element(
        "ConfigurationInformationAddress",
        item(t, 8)?,
    )?);
    push(leaf("DataLockControlMode", "Managed"));
    push(leaf("ObjectAutonumerationMode", "NotAutoFree"));
    push(leaf("ModalityUseMode", code(t, 36, &modes)?));
    push(leaf(
        "SynchronousPlatformExtensionAndAddInCallUseMode",
        code(t, 41, &modes)?,
    ));
    push(leaf("InterfaceCompatibilityMode", interface_compatibility));
    if xml_2_21 {
        push(leaf(
            "Version85InterfaceMigrationMode",
            if tuple_8_5_1 { "Use" } else { "DontUse" },
        ));
    }
    push(leaf("DatabaseTablespacesUseMode", "DontUse"));
    push(leaf("CompatibilityMode", compatibility));
    push(el("DefaultConstantsForm"));

    let mut internal = el("InternalInfo");
    for (class_id, object_id) in &parsed.contained {
        internal.children.push(
            el("xr:ContainedObject")
                .child(leaf("xr:ClassId", *class_id))
                .child(leaf("xr:ObjectId", object_id.clone())),
        );
    }
    let children = child_objects(&parsed, context)?;
    Ok(el("Configuration")
        .attr("uuid", parsed.uuid)
        .child(internal)
        .child(p)
        .child(children))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::super::brace::parse_row;
    use super::super::super::export::{ExportContext, NameIndex, export_descriptor, object_names};
    use super::super::super::objects::parts::compatibility;
    use super::super::super::{DescriptorContext, compile_descriptor};
    use super::super::{FUNCTIONALITIES, SECTIONS};
    use super::top_level_objects;

    const NAMESPACES: &str = r#"xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" xmlns:cmi="http://v8.1c.ru/8.2/managed-application/cmi" xmlns:ent="http://v8.1c.ru/8.1/data/enterprise" xmlns:lf="http://v8.1c.ru/8.2/managed-application/logform" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:sys="http://v8.1c.ru/8.1/data/ui/fonts/system" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" xmlns:win="http://v8.1c.ru/8.1/data/ui/colors/windows" xmlns:xen="http://v8.1c.ru/8.3/xcf/enums" xmlns:xpr="http://v8.1c.ru/8.3/xcf/predef" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#;

    fn file(lines: &[String]) -> String {
        let mut text = format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject {NAMESPACES} version=\"2.20\">\r\n"
        );
        for line in lines {
            text.push_str(line);
            text.push_str("\r\n");
        }
        text.push_str("</MetaDataObject>");
        text
    }

    /// A configuration goes XML -> row -> XML and comes back byte for byte,
    /// and its row lists the objects it names.
    #[test]
    fn a_configuration_round_trips_through_its_row() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-export-configuration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("Languages")).unwrap();
        let language_uuid = "22222222-2222-2222-2222-222222222222";
        fs::write(
            root.join("Languages").join("Русский.xml"),
            file(&[
                format!("\t<Language uuid=\"{language_uuid}\">"),
                "\t\t<Properties>".into(),
                "\t\t\t<Name>Русский</Name>".into(),
                "\t\t\t<Synonym/>".into(),
                "\t\t\t<Comment/>".into(),
                "\t\t\t<LanguageCode>ru</LanguageCode>".into(),
                "\t\t</Properties>".into(),
                "\t</Language>".into(),
            ]),
        )
        .unwrap();

        let mut lines = vec![
            "\t<Configuration uuid=\"11111111-1111-1111-1111-111111111111\">".to_string(),
            "\t\t<InternalInfo>".into(),
        ];
        for (index, section) in SECTIONS.iter().enumerate() {
            lines.push("\t\t\t<xr:ContainedObject>".into());
            lines.push(format!(
                "\t\t\t\t<xr:ClassId>{}</xr:ClassId>",
                section.class_id
            ));
            lines.push(format!(
                "\t\t\t\t<xr:ObjectId>00000000-0000-0000-0000-00000000000{}</xr:ObjectId>",
                index + 2
            ));
            lines.push("\t\t\t</xr:ContainedObject>".into());
        }
        lines.push("\t\t</InternalInfo>".into());
        for line in [
            "<Properties>",
            "\t<Name>C</Name>",
            "\t<Synonym>",
            "\t\t<v8:item>",
            "\t\t\t<v8:lang>ru</v8:lang>",
            "\t\t\t<v8:content>Конфигурация \"C\"</v8:content>",
            "\t\t</v8:item>",
            "\t</Synonym>",
            "\t<Comment/>",
            "\t<NamePrefix/>",
            "\t<ConfigurationExtensionCompatibilityMode>Version8_3_27</ConfigurationExtensionCompatibilityMode>",
            "\t<DefaultRunMode>ManagedApplication</DefaultRunMode>",
            "\t<UsePurposes>",
            "\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>",
            "\t</UsePurposes>",
            "\t<ScriptVariant>Russian</ScriptVariant>",
            "\t<DefaultRoles/>",
            "\t<Vendor>Фирма</Vendor>",
            "\t<Version>1.0.0.1</Version>",
            "\t<UpdateCatalogAddress/>",
            "\t<IncludeHelpInContents>true</IncludeHelpInContents>",
            "\t<UseManagedFormInOrdinaryApplication>true</UseManagedFormInOrdinaryApplication>",
            "\t<UseOrdinaryFormInManagedApplication>false</UseOrdinaryFormInManagedApplication>",
            "\t<AdditionalFullTextSearchDictionaries/>",
            "\t<CommonSettingsStorage/>",
            "\t<ReportsUserSettingsStorage/>",
            "\t<ReportsVariantsStorage/>",
            "\t<FormDataSettingsStorage/>",
            "\t<DynamicListsUserSettingsStorage/>",
            "\t<URLExternalDataStorage/>",
            "\t<Content/>",
            "\t<DefaultReportForm/>",
            "\t<DefaultReportVariantForm/>",
            "\t<DefaultReportSettingsForm/>",
            "\t<DefaultReportAppearanceTemplate/>",
            "\t<DefaultDynamicListSettingsForm/>",
            "\t<DefaultSearchForm/>",
            "\t<DefaultDataHistoryChangeHistoryForm/>",
            "\t<DefaultDataHistoryVersionDataForm/>",
            "\t<DefaultDataHistoryVersionDifferencesForm/>",
            "\t<DefaultCollaborationSystemUsersChoiceForm/>",
            "\t<RequiredMobileApplicationPermissions/>",
            "\t<UsedMobileApplicationFunctionalities>",
        ] {
            lines.push(format!("\t\t{line}"));
        }
        for (id, name) in FUNCTIONALITIES {
            let used = matches!(id, 0 | 18 | 25);
            lines.push("\t\t\t\t<app:functionality>".into());
            lines.push(format!(
                "\t\t\t\t\t<app:functionality>{name}</app:functionality>"
            ));
            lines.push(format!("\t\t\t\t\t<app:use>{used}</app:use>"));
            lines.push("\t\t\t\t</app:functionality>".into());
        }
        for line in [
            "\t</UsedMobileApplicationFunctionalities>",
            "\t<StandaloneConfigurationRestrictionRoles/>",
            "\t<MobileApplicationURLs/>",
            "\t<AllowedIncomingShareRequestTypes/>",
            "\t<MainClientApplicationWindowMode>Normal</MainClientApplicationWindowMode>",
            "\t<DefaultInterface/>",
            "\t<DefaultStyle/>",
            "\t<DefaultLanguage>Language.Русский</DefaultLanguage>",
            "\t<BriefInformation/>",
            "\t<DetailedInformation/>",
            "\t<Copyright/>",
            "\t<VendorInformationAddress/>",
            "\t<ConfigurationInformationAddress/>",
            "\t<DataLockControlMode>Managed</DataLockControlMode>",
            "\t<ObjectAutonumerationMode>NotAutoFree</ObjectAutonumerationMode>",
            "\t<ModalityUseMode>UseWithWarnings</ModalityUseMode>",
            "\t<SynchronousPlatformExtensionAndAddInCallUseMode>Use</SynchronousPlatformExtensionAndAddInCallUseMode>",
            "\t<InterfaceCompatibilityMode>TaxiEnableVersion8_2</InterfaceCompatibilityMode>",
            "\t<DatabaseTablespacesUseMode>DontUse</DatabaseTablespacesUseMode>",
            "\t<CompatibilityMode>Version8_3_27</CompatibilityMode>",
            "\t<DefaultConstantsForm/>",
            "</Properties>",
            "<ChildObjects>",
            "\t<Language>Русский</Language>",
            "</ChildObjects>",
        ] {
            lines.push(format!("\t\t{line}"));
        }
        lines.push("\t</Configuration>".into());
        let xml = file(&lines);
        let path = root.join("Configuration.xml");
        fs::write(&path, &xml).unwrap();

        let descriptor_context = DescriptorContext::new(&root, "2.20").unwrap();
        let row = compile_descriptor("Configuration", &path, xml.as_bytes(), &descriptor_context)
            .unwrap();
        let context = ExportContext {
            names: NameIndex::from_config_index(&descriptor_context.index),
            version: "2.20".to_string(),
            compat: compatibility(&descriptor_context),
        };
        assert_eq!(
            export_descriptor("Configuration", &row, &context).unwrap(),
            xml
        );
        // The row a native import stages: the compatibility in field 26 is the
        // configuration's own (8.3.24), the extension compatibility in field 43
        // is the platform's edition.
        let text = String::from_utf8(row.clone()).unwrap();
        let staged = text.replacen("80327", "80324", 1);
        assert_ne!(staged, text);
        let exported = export_descriptor("Configuration", staged.as_bytes(), &context).unwrap();
        assert!(exported.contains("<CompatibilityMode>Version8_3_24</CompatibilityMode>"));
        assert!(exported.contains(
            "<ConfigurationExtensionCompatibilityMode>Version8_3_27</ConfigurationExtensionCompatibilityMode>"
        ));
        assert_eq!(exported.replace("Version8_3_24", "Version8_3_27"), xml);
        let tree = parse_row(&row).unwrap();
        let names = object_names("Configuration", &tree).unwrap();
        assert_eq!(names.uuid, "11111111-1111-1111-1111-111111111111");
        assert_eq!(names.full_name, "Configuration");
        assert_eq!(
            top_level_objects(&tree).unwrap(),
            vec![("Language".to_string(), language_uuid.to_string())]
        );
        fs::remove_dir_all(&root).ok();
    }
}
