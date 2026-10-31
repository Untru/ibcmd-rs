use super::*;
use ibcmd_schema::configuration_rights::{
    CONFIGURATION_MODE_GROUP_ANCHOR, CONFIGURATION_RIGHT_ORDER,
    EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START, EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID,
};
// The role-right vocabulary (the right names and uuids, the reference
// families and the standard attribute and tabular section slots) is the
// `ibcmd-1c-vocab` crate's; the decoder reads it by its old names.
use ibcmd_1c_vocab::{
    ROLE_RIGHTS_EXT_DIMENSION_FAMILY as EXT_DIMENSION_FAMILY,
    ROLE_RIGHTS_EXT_DIMENSION_TYPE_FAMILY as EXT_DIMENSION_TYPE_FAMILY,
    ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY as STANDARD_ATTRIBUTE_FAMILY,
    ROLE_RIGHTS_STANDARD_TABULAR_SECTION_FAMILY as STANDARD_TABULAR_SECTION_FAMILY,
    role_right_name, role_standard_attribute_descriptor, role_standard_tabular_section,
    role_standard_tabular_section_attribute,
};

pub(super) struct RoleRights {
    pub(super) set_for_new_objects: bool,
    pub(super) set_for_attributes_by_default: bool,
    pub(super) independent_rights_of_child_objects: bool,
    pub(super) objects: Vec<RoleObjectRights>,
    pub(super) restriction_templates: Vec<RoleRestrictionTemplate>,
}

pub(super) struct RoleObjectRights {
    pub(super) name: String,
    pub(super) rights: Vec<RoleRight>,
    /// The reference names an object the extension adopted (its tuple ends in
    /// the adopted-object marker 3). The platform prints the `View`/`Edit`
    /// rights of such an object's nested objects whatever their value, where
    /// an ordinary object's rights equal to `setForAttributesByDefault` stay
    /// unwritten.
    pub(super) adopted: bool,
    /// True when this object's right-restrictions table carries at least one
    /// conditionless `{RIGHT_UUID,{0}}` entry (see
    /// `RoleRestrictionEntry::Conditionless`). The platform renders such
    /// objects in value-only mode: a right prints exactly when its value is
    /// `true` or it carries a real condition, and an object where nothing
    /// prints is omitted from Rights.xml entirely. Measured over every role
    /// of ERP УХ 3.2.12.6 (2026-08-24): 84/84 objects carrying such an entry
    /// match this rule byte-for-byte, 15/15 all-false ones are absent from
    /// the native file, and no native tree in any of the eight measured
    /// corpora contains an empty `<object>` block. Whether the same rule
    /// extends to objects without such an entry is not proven, so it is
    /// applied only where the marker was observed.
    pub(super) has_conditionless_restrictions: bool,
}

pub(super) struct RoleRight {
    pub(super) name: String,
    pub(super) value: bool,
    /// Every `<restrictionByCondition>` block the platform prints for this
    /// right, in blob order. Usually zero or one; two occurs on exactly six
    /// rights across the whole stand (see `parse_role_restriction_condition`).
    pub(super) restrictions: Vec<RoleRightRestriction>,
}

#[derive(Clone)]
pub(super) struct RoleRightRestriction {
    pub(super) field: Option<String>,
    pub(super) condition: String,
}

/// One parsed entry of an object's right-restrictions table
/// (`{RIGHT_UUID,{…}}`): the wrapper's first field is the number of
/// `<restrictionByCondition>` blocks that follow it (see
/// `parse_role_restriction_condition`).
///
/// A count of `0` carries no condition at all
/// and is accepted only as the exact one-field wrapper `{0}` — measured over
/// every restriction entry of eight corpora (2026-08-24: ERP УХ 3.2.12.6,
/// УТ 11.5.27.75, БСП 3.1.12.297 базовая/демо/international, БСП WE
/// 3.1.10.386 базовая/демо, WMS5 Модуль Web-обмена; 11,149 entries total),
/// kind `0` occurs 161 times, all in ERP УХ, always exactly `{0}`, never on
/// a right that also has a real condition. The native XML never prints a
/// `<restrictionByCondition>` for such an entry; its only observable effect
/// is the object-level value-only rendering mode recorded in
/// `RoleObjectRights::has_conditionless_restrictions`. Any other kind-0
/// arity is unobserved and refused.
pub(super) enum RoleRestrictionEntry {
    Conditionless,
    Conditions(Vec<RoleRightRestriction>),
}

/// An object's parsed right-restrictions table: real conditions by right
/// UUID, plus whether any conditionless `{0}` entry was present.
pub(super) struct RoleRightRestrictions {
    pub(super) by_right: BTreeMap<String, Vec<RoleRightRestriction>>,
    pub(super) has_conditionless: bool,
}

pub(super) struct RoleRestrictionTemplate {
    pub(super) name: String,
    pub(super) condition: String,
}

pub(super) fn parse_role_rights_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
) -> Option<RoleRights> {
    let inflated = ibcmd_compiler::compiler::bodies::rights::decode_compatible_role_rights(
        bytes,
        super::extension::active().is_some(),
    )
    .ok()?
    .plaintext()
    .ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "10" {
        return None;
    }
    let object_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    let count = object_fields.first()?.trim().parse::<usize>().ok()?;
    if object_fields.len() != count + 1 {
        return None;
    }

    // Parsed ahead of the objects loop: the Configuration root's own rights
    // (see `parse_configuration_root_object_rights`) resolve two right UUIDs
    // that carry no recoverable name by checking their value against this
    // role's own `setForNewObjects` flag, so the flag must already be known
    // by the time that object is reached.
    let restriction_templates = parse_role_restriction_templates(fields.get(2)?)?;
    let set_for_new_objects = parse_role_bool_field(fields.get(3)?)?;
    // An extension's adopted role stores `2` here whatever its source says,
    // and 8.3.27.2214 dumps `false` (fixture `adopted/role`).
    let adopted_role = fields.get(4).map(|field| field.trim()) == Some("2");
    let set_for_attributes_by_default = if adopted_role {
        false
    } else {
        parse_role_bool_field_or_default(&fields, 4, true)?
    };
    let independent_rights_of_child_objects = parse_role_bool_field_or_default(&fields, 5, false)?;

    let mut objects = Vec::with_capacity(count);
    for (serialized_index, object_field) in object_fields.iter().skip(1).enumerate() {
        let entry = split_1c_braced_fields(object_field, 0)?;
        if entry.len() != 2 {
            return None;
        }

        let object_ref = split_1c_braced_fields(entry[0], 0)?;
        let object_uuid = object_ref.get(1)?.trim();
        if !is_uuid_text(object_uuid) {
            return None;
        }
        if !object_refs.contains_key(object_uuid) {
            // A reference to metadata that no longer exists. The platform
            // drops the whole `<object>` rather than printing the bare uuid:
            // measured on ERP УХ 3.2.12.6 (2026-08-25), 578 such uuids occur
            // across 13 role Rights blobs, none of them is declared anywhere
            // in the 187,101-uuid `uuid="..."` inventory of the native source
            // tree, and native prints no `<object>` for a single one of them.
            // The platform makes the same admission elsewhere for these very
            // uuids -- `Subsystems/.../Ext/CommandInterface.xml` carries them
            // as unresolved `0:<uuid>` command names -- so this is its own
            // behaviour on a dangling reference, not a guess about ours.
            continue;
        }
        let object_name = role_object_ref_name(&object_ref, object_refs)?;

        let (rights, mut has_conditionless_restrictions) =
            if is_configuration_root_rights_object(&object_name) {
                (
                    parse_configuration_root_object_rights(entry[1], set_for_new_objects)?,
                    false,
                )
            } else {
                parse_role_object_rights(entry[1], field_refs, &object_name)?
            };
        if object_ref.last().map(|field| field.trim()) == Some("3") {
            // An extension's role names an object of the configuration it
            // extends (an adopted object, the root, their attributes) with
            // the reference tail `3`, and 8.3.27.2214 prints the rights it
            // grants there and nothing else, whatever setForNewObjects or
            // setForAttributesByDefault say -- the value-only mode. Fixture
            // `adopted/role` (unset rights stored `2`); a real extension: its
            // adopted full-access role (flag true) prints the root's six
            // client-mode rights and two adopted data processors' Use/View,
            // an own role prints `View` on an adopted catalog's standard
            // attributes against setForAttributesByDefault true.
            has_conditionless_restrictions = true;
        }
        let intra_uuid_order =
            role_rights_object_intra_uuid_order(&object_ref, &object_name).unwrap_or(0);
        objects.push((
            object_uuid.to_string(),
            intra_uuid_order,
            serialized_index,
            RoleObjectRights {
                name: object_name,
                rights,
                adopted: super::extension::active().is_some()
                    && object_ref
                        .last()
                        .is_some_and(|marker| marker.trim() == "3" && object_ref.len() > 3),
                has_conditionless_restrictions,
            },
        ));
    }
    objects.sort_by_key(|(sort_uuid, intra_uuid_order, serialized_index, _)| {
        (sort_uuid.clone(), *intra_uuid_order, *serialized_index)
    });
    let objects = objects
        .into_iter()
        .map(|(_, _, _, object)| object)
        .collect::<Vec<_>>();

    Some(RoleRights {
        set_for_new_objects,
        set_for_attributes_by_default,
        independent_rights_of_child_objects,
        objects,
        restriction_templates,
    })
}

pub(super) fn role_rights_object_intra_uuid_order(
    fields: &[&str],
    object_name: &str,
) -> Option<usize> {
    // Only a braced `{slot, family}` reference carries a print order; a bare
    // integer slot leaves the object in serialized order. Pre-existing rule,
    // kept as it was: every slot code in every ERP УХ 3.2.12.6 role Rights
    // blob is braced, so no measurement here can speak to the bare form.
    if !fields.get(3)?.trim_start().starts_with('{') {
        return None;
    }
    let (kind, _) = object_name.split_once('.')?;
    role_nested_ref_suffix(kind, fields).map(|(_, order)| order)
}

pub(super) fn role_object_ref_name(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let uuid = fields.get(1)?.trim();
    if !is_uuid_text(uuid) {
        return None;
    }
    // Callers skip entries whose uuid is not in `object_refs` (see
    // `parse_role_rights_blob`), so a miss here is a caller bug, not a
    // dangling reference, and is refused rather than named after its uuid.
    let base_name = object_refs.get(uuid).cloned()?;
    let Some((kind, _)) = base_name.split_once('.') else {
        return Some(base_name);
    };
    match role_nested_ref_suffix(kind, fields) {
        Some((suffix, _)) => Some(format!("{base_name}.{suffix}")),
        None => Some(base_name),
    }
}

/// The Rights.xml text the exporter writes for a stored Role rights row, or
/// `None` where the exporter refuses the row: `parse_role_rights_blob` then
/// `format_role_rights_xml`, for callers outside the exporter that need to
/// read a row back the way the export does (the base-free writer's audit and
/// its self-check).
pub(crate) fn role_rights_xml_from_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
) -> Option<String> {
    parse_role_rights_blob(bytes, object_refs, field_refs)
        .map(|rights| format_role_rights_xml(&rights))
}

/// Name suffix (everything after `Kind.Name.`) and intra-owner print order of
/// a nested role-rights reference, or `None` when the reference names the
/// owner itself.
///
/// `fields` is the reference's own braced list: `{1, uuid}` for the owner,
/// `{1, uuid, 1, {slot, family}, …}` for a standard attribute or a standard
/// tabular section, and `{1, uuid, 2, {slot, family}, {slot, family}, …}` for
/// something nested one level deeper (a standard tabular section's own
/// standard attribute, an accounting register's ext-dimension pair).
///
/// ## How the slot tables were measured (ERP УХ 3.2.12.6, 2026-08-25)
///
/// A metadata object's storage element declares its standard attributes in
/// one list, each entry `{slot[,family]},510405d3-2a0c-4fea-960a-7fee59b32f9b`,
/// and the platform writes that same list, in that same order and with the
/// same length, as the object's `<StandardAttributes>` block in its native
/// XML. Pairing the two by position gives slot -> name directly. Checked
/// against the two kinds whose tables were already corpus-hardened here:
/// `Catalogs/Организации` yields exactly the nine `Catalog` rows below
/// (`-13` PredefinedDataName … `-2` Code), and `AccountingRegisters/МСФО`
/// yields the four register rows (`-5` Active, `-4` LineNumber, `-3`
/// Recorder, `-2` Period) -- 13/13 agreements, no disagreement.
/// (`Document`'s existing rows pair the same five slots in the opposite
/// order; nothing in any corpus can tell the two apart -- see the order note
/// below -- so they are left exactly as they were.)
///
/// The `order` half is measured separately and directly, from the native
/// `Roles/*/Ext/Rights.xml` print sequence inside one owner's `<object>`
/// group. It is what the export actually depends on: across all 10,619
/// standard-attribute groups in the whole 2,118-role ERP УХ corpus, exactly
/// one (`Catalog.СоответствиеВнешнимИБ` in `Roles/БазовыеПраваБПУХ`) prints
/// two different right lists inside a group, so which *name* a slot carries
/// is almost never observable, while the order they print in always is.
fn role_nested_ref_suffix(kind: &str, fields: &[&str]) -> Option<(String, usize)> {
    let kind_code = fields.get(2).map(|field| field.trim())?;
    let outer = role_nested_slot(fields.get(3)?)?;
    match kind_code {
        "1" => {
            if outer.family == Some(STANDARD_TABULAR_SECTION_FAMILY) {
                let (section, order) = role_standard_tabular_section(kind, outer.slot)?;
                return Some((format!("StandardTabularSection.{section}"), order));
            }
            let slot_index = usize::try_from(outer.slot).ok();
            let (attribute, order) =
                role_standard_attribute_descriptor(kind, outer.slot, slot_index)?;
            Some((format!("StandardAttribute.{attribute}"), order))
        }
        "2" => {
            let inner = role_nested_slot(fields.get(4)?)?;
            if outer.family == Some(STANDARD_TABULAR_SECTION_FAMILY) {
                let (section, section_order) = role_standard_tabular_section(kind, outer.slot)?;
                let (attribute, position) =
                    role_standard_tabular_section_attribute(kind, inner.slot)?;
                return Some((
                    format!("StandardTabularSection.{section}.StandardAttribute.{attribute}"),
                    section_order.checked_add(position)?,
                ));
            }
            role_ext_dimension_descriptor(kind, outer, inner)
                .map(|(attribute, order)| (format!("StandardAttribute.{attribute}"), order))
        }
        _ => None,
    }
}

/// A nested reference's `{slot, family}` pair, or a bare slot number.
#[derive(Clone, Copy)]
struct RoleNestedSlot<'a> {
    slot: isize,
    family: Option<&'a str>,
}

fn role_nested_slot(slot_code: &str) -> Option<RoleNestedSlot<'_>> {
    let slot_code = slot_code.trim();
    if let Ok(slot) = slot_code.parse::<isize>() {
        return Some(RoleNestedSlot { slot, family: None });
    }
    let fields = split_1c_braced_fields(slot_code, 0)?;
    let slot = fields.first()?.trim().parse::<isize>().ok()?;
    let family = fields
        .get(1)
        .map(|field| field.trim())
        .filter(|field| is_uuid_text(field));
    Some(RoleNestedSlot { slot, family })
}

/// An accounting register's `ExtDimension<n>` / `ExtDimensionType<n>` pair.
///
/// Unlike every other standard attribute these are generated per ext-dimension
/// slot of the register's chart of accounts, so they carry an index rather
/// than a fixed negative slot: outer `{n, STANDARD_ATTRIBUTE_FAMILY}`, inner
/// `{n, EXT_DIMENSION_FAMILY | EXT_DIMENSION_TYPE_FAMILY}`. The family split
/// and the `n -> n + 1` numbering come from pairing
/// `AccountingRegisters/МСФО`'s eleven element entries with its eleven
/// `<StandardAttributes>` names, and again from
/// `AccountingRegisters/МеждународныйБезКорреспонденции`'s twelve; both
/// agree. They print after `Period` (6), interleaved `ExtDimension1`,
/// `ExtDimensionType1`, `ExtDimension2`, … -- see `Roles/ЧтениеДанныхМСФОУХ`.
fn role_ext_dimension_descriptor(
    kind: &str,
    outer: RoleNestedSlot<'_>,
    inner: RoleNestedSlot<'_>,
) -> Option<(String, usize)> {
    if kind != "AccountingRegister" || outer.family != Some(STANDARD_ATTRIBUTE_FAMILY) {
        return None;
    }
    let index = usize::try_from(inner.slot).ok()?;
    if outer.slot != inner.slot {
        return None;
    }
    let (stem, offset) = match inner.family? {
        EXT_DIMENSION_FAMILY => ("ExtDimension", 0),
        EXT_DIMENSION_TYPE_FAMILY => ("ExtDimensionType", 1),
        _ => return None,
    };
    Some((format!("{stem}{}", index + 1), 7 + index * 2 + offset))
}

/// Configuration-root rights suppressed when equal to setForNewObjects.
///
/// Measured over the ERP УХ role corpus (2026-08-24, 1,679 roles whose Rights
/// blob has a Configuration-object entry): the UUID occurs in every one of
/// them, and in every occurrence the value equals that role's own
/// `setForNewObjects` flag — which is exactly the condition under which
/// `role_rights_for_xml` never prints a Configuration-root right at all (see
/// its doc comment). It is therefore structurally invisible in every
/// observed byte of output. The later ITK witness names a diverging value
/// ExclusiveModeTerminationAtSessionStart; that shared schema name and UUID
/// are now also available to the base-free writer.
///
/// The list used to hold a second uuid, `3762abec-…`. Документооборот КОРП
/// 3.0.21.3 writes that one *diverging* from the flag on
/// `Roles/АдминистраторСистемы`, so the platform prints it and its native
/// `Ext/Rights.xml` states the name; it now sits in `ROLE_RIGHT_NAMES` where
/// the bytes put it, and the whole role -- which this refusal used to
/// discard -- is exported again.
const CONFIGURATION_ROOT_TOLERATED_UNNAMED_RIGHT_UUIDS: [&str; 1] =
    [EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID];

/// True for the six Configuration-root rights that pick the client's launch
/// mode (thin/thick client window mode, analytics client). Unlike every
/// other Configuration-root right, their type default is `true`, not the
/// role's own `setForNewObjects` flag.
pub(super) fn is_configuration_mode_right(name: &str) -> bool {
    CONFIGURATION_MODE_RIGHT_NAMES.contains(&name)
}

pub(crate) use ibcmd_schema::configuration_rights::CONFIGURATION_MODE_RIGHT_NAMES;

/// True for the Configuration root's own name (`Configuration.<Name>`), the
/// one object every role's Rights blob may carry administrative (client
/// launch, admin console, `Automation`, …) rights on.
pub(super) fn is_configuration_root_rights_object(name: &str) -> bool {
    name.starts_with("Configuration.")
}

/// Parses the rights list for the Configuration root object.
///
/// This differs from `parse_role_object_rights` in two ways proven over the
/// ERP УХ corpus (2026-08-24, see `role_rights_for_xml` and
/// `CONFIGURATION_ROOT_TOLERATED_UNNAMED_RIGHT_UUIDS`): the six
/// `CONFIGURATION_MODE_RIGHT_NAMES` rights default to `true` and are omitted
/// from the blob entirely (not written as an explicit `false`) whenever a
/// role leaves all six at that default, and two right UUIDs are tolerated
/// without a recoverable name. Administrative rights carry no row-level
/// restriction in any sampled role, so only the plain-pairs blob shape
/// (`{0, uuid, value, uuid, value, …}`) is accepted; the restrictions shape
/// `parse_role_object_rights` also handles is refused here as unproven.
pub(super) fn parse_configuration_root_object_rights(
    value: &str,
    set_for_new_objects: bool,
) -> Option<Vec<RoleRight>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let pairs = &fields[1..];
    if pairs.len() % 2 != 0 {
        return None;
    }
    let pair_count = pairs.len() / 2;

    let mut entries = Vec::with_capacity(pair_count);
    let mut mode_rights_seen = 0usize;
    let mut save_user_data_index = None;
    for index in 0..pair_count {
        let right_uuid = pairs[index * 2].trim();
        if !is_uuid_text(right_uuid) {
            return None;
        }
        let value = parse_role_right_value(pairs[index * 2 + 1].trim())?;
        if CONFIGURATION_ROOT_TOLERATED_UNNAMED_RIGHT_UUIDS.contains(&right_uuid) {
            if value == set_for_new_objects {
                continue;
            }
            // Diverging from the flag, the platform names it: ИТК
            // `Roles/ИТК_СтруктураХранения` and `ИТК_ПоискПоСтруктуреХранения`
            // (flag false, value 1) print `ExclusiveModeTerminationAtSessionStart`
            // `true` at this position.
            entries.push(RoleRight {
                name: EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START.to_string(),
                value,
                restrictions: Vec::new(),
            });
            continue;
        }
        let name = role_right_name(right_uuid)?;
        if is_configuration_mode_right(name) {
            if entries.iter().any(|entry| entry.name == name) {
                return None;
            }
            mode_rights_seen += 1;
        }
        if name == "SaveUserData" {
            save_user_data_index = Some(entries.len());
        }
        entries.push(RoleRight {
            name: name.to_string(),
            value,
            restrictions: Vec::new(),
        });
    }

    // An older root stores the five window modes and not the analytics
    // client the platform added later: the right it does not carry is the
    // type's default, `true`, as for all six when none is stored. ЗУП 3.1.18:
    // 282 such roles, all with `setForNewObjects` false, print it `true`;
    // Монитор's two (`Roles/Пользователь`) have the flag true, where the
    // `true` equals the role's default and is not printed. Any other partial
    // presence stays refused.
    if mode_rights_seen + 1 == CONFIGURATION_MODE_RIGHT_NAMES.len()
        && !entries
            .iter()
            .any(|entry| entry.name == "AnalyticsSystemClient")
    {
        for name in CONFIGURATION_MODE_RIGHT_NAMES {
            if !entries.iter().any(|entry| entry.name == name) {
                entries.push(RoleRight {
                    name: name.to_string(),
                    value: true,
                    restrictions: Vec::new(),
                });
            }
        }
        mode_rights_seen = CONFIGURATION_MODE_RIGHT_NAMES.len();
    }
    match mode_rights_seen {
        0 => {
            // Omitted entirely rather than written `false`: insert the type
            // default (`true`) at the position the platform's own canonical
            // order puts them, immediately before `SaveUserData`. Measured
            // over ERP УХ: 13/13 roles whose blob omits these six show all
            // six as `true` natively, always in this position relative to
            // the rights that are present.
            let insert_at = save_user_data_index?;
            let synthesized = CONFIGURATION_MODE_RIGHT_NAMES.iter().map(|name| RoleRight {
                name: (*name).to_string(),
                value: true,
                restrictions: Vec::new(),
            });
            entries.splice(insert_at..insert_at, synthesized);
        }
        6 => {}
        _ => return None, // partial presence: an unproven shape
    }

    // Stored window modes may be scrambled or synthesized above. Native
    // XML places them before SaveUserData, followed by the termination right
    // if it differs from the role's default. Move the whole group so a
    // termination pair stored before the modes cannot split this sequence.
    if entries
        .iter()
        .any(|entry| entry.name == CONFIGURATION_MODE_GROUP_ANCHOR)
    {
        let (mut ordered, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut entries).into_iter().partition(|entry| {
                is_configuration_mode_right(&entry.name)
                    || entry.name == EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START
            });
        entries = rest;
        ordered.sort_by_key(|entry| {
            CONFIGURATION_RIGHT_ORDER
                .iter()
                .position(|name| *name == entry.name)
        });
        let insert_at = entries
            .iter()
            .position(|entry| entry.name == CONFIGURATION_MODE_GROUP_ANCHOR)?;
        entries.splice(insert_at..insert_at, ordered);
    }

    Some(entries)
}

pub(super) fn parse_role_object_rights(
    value: &str,
    field_refs: &BTreeMap<String, String>,
    object_name: &str,
) -> Option<(Vec<RoleRight>, bool)> {
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first()?.trim() {
        "0" if (fields.len() - 1) % 2 == 0 => {
            parse_role_right_pairs(&fields, 1, (fields.len() - 1) / 2, &BTreeMap::new())
                .map(|rights| (rights, false))
        }
        "0" => None,
        "1" => {
            let count = fields.get(1)?.trim().parse::<usize>().ok()?;
            let pairs_start = 2usize;
            let restrictions_count_index = pairs_start.checked_add(count.checked_mul(2)?)?;
            if fields.len() <= restrictions_count_index {
                return None;
            }
            let restrictions = parse_role_right_restrictions(
                fields.get(restrictions_count_index)?.trim(),
                &fields[restrictions_count_index + 1..],
                field_refs,
                object_name,
            )?;
            parse_role_right_pairs(&fields, pairs_start, count, &restrictions.by_right)
                .map(|rights| (rights, restrictions.has_conditionless))
        }
        _ => None,
    }
}

pub(super) fn parse_role_right_pairs(
    fields: &[&str],
    start: usize,
    count: usize,
    restrictions: &BTreeMap<String, Vec<RoleRightRestriction>>,
) -> Option<Vec<RoleRight>> {
    let mut rights = Vec::with_capacity(count);
    for index in 0..count {
        let offset = start.checked_add(index.checked_mul(2)?)?;
        let right_uuid = fields.get(offset)?.trim();
        if !is_uuid_text(right_uuid) {
            return None;
        }
        let value = parse_role_right_value(fields.get(offset + 1)?.trim())?;
        rights.push(RoleRight {
            name: role_right_name(right_uuid)?.to_string(),
            value,
            restrictions: restrictions.get(right_uuid).cloned().unwrap_or_default(),
        });
    }
    Some(rights)
}

pub(super) fn parse_role_bool_field(value: &str) -> Option<bool> {
    match value.trim() {
        "0" | "4294967295" => Some(false),
        "1" => Some(true),
        // An extension's role stores `false` of a defaulted flag as 2.
        "2" if super::extension::active().is_some() => Some(false),
        _ => None,
    }
}

pub(super) fn parse_role_bool_field_or_default(
    fields: &[&str],
    index: usize,
    default: bool,
) -> Option<bool> {
    fields
        .get(index)
        .map_or(Some(default), |value| parse_role_bool_field(value))
}

pub(super) fn parse_role_right_value(value: &str) -> Option<bool> {
    match value {
        // `2`: an adopted role's right the extension leaves unset (fixture
        // `adopted/role`), printed as `-1` is -- not at all.
        "-1" | "0" | "2" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_role_right_restrictions(
    count_text: &str,
    values: &[&str],
    field_refs: &BTreeMap<String, String>,
    object_name: &str,
) -> Option<RoleRightRestrictions> {
    let count = count_text.parse::<usize>().ok()?;
    let mut restrictions = RoleRightRestrictions {
        by_right: BTreeMap::new(),
        has_conditionless: false,
    };
    let record = |restrictions: &mut RoleRightRestrictions,
                  right_uuid: &str,
                  entry: RoleRestrictionEntry| match entry {
        RoleRestrictionEntry::Conditionless => restrictions.has_conditionless = true,
        RoleRestrictionEntry::Conditions(conditions) => {
            if !conditions.is_empty() {
                restrictions
                    .by_right
                    .insert(right_uuid.to_string(), conditions);
            }
        }
    };
    if count == 0 {
        if !values.is_empty() {
            return None;
        }
        return Some(restrictions);
    }
    if values.len() == count {
        for entry in values {
            let pair = split_1c_braced_fields(entry, 0)?;
            if pair.len() != 2 {
                return None;
            }
            let right_uuid = pair.first()?.trim();
            if !is_uuid_text(right_uuid) {
                return None;
            }
            let condition =
                parse_role_restriction_condition(pair.get(1)?, field_refs, object_name)?;
            record(&mut restrictions, right_uuid, condition);
        }
        return Some(restrictions);
    }
    if values.len() != 1 {
        return None;
    }
    let entries = split_1c_braced_fields(values[0], 0)?;
    if entries.len() == count * 2
        && entries
            .first()
            .is_some_and(|entry| is_uuid_text(entry.trim()))
    {
        for entry in entries.chunks(2) {
            let right_uuid = entry.first()?.trim();
            let condition =
                parse_role_restriction_condition(entry.get(1)?, field_refs, object_name)?;
            record(&mut restrictions, right_uuid, condition);
        }
        return Some(restrictions);
    }
    if entries.len() != count {
        return None;
    }
    for entry in entries {
        let pair = split_1c_braced_fields(entry, 0)?;
        if pair.len() != 2 {
            return None;
        }
        let right_uuid = pair.first()?.trim();
        if !is_uuid_text(right_uuid) {
            return None;
        }
        let condition = parse_role_restriction_condition(pair.get(1)?, field_refs, object_name)?;
        record(&mut restrictions, right_uuid, condition);
    }
    Some(restrictions)
}

/// A right's restriction entry: `{N, block_1, …, block_N}`, where `N` counts
/// the `<restrictionByCondition>` blocks the platform prints for that right.
///
/// The leading field is a count, not a payload kind. Measured on the stand
/// (2026-08-25): across the role trees of all seven corpora exactly six
/// rights print two `<restrictionByCondition>` blocks and 10,439 print one;
/// all six two-block rights live in `Roles/ЧтениеБюджетированиеИОтчетность`
/// and `Roles/ДобавлениеИзменениеБюджетированиеИОтчетность`, whose blobs
/// carry an `N` of 2 with two blocks after it. The earlier reading treated
/// `2` as "condition with a field" and took only the second block, which
/// silently dropped the first -- and refused the whole blob whenever the
/// second block's field payload was a standard-attribute slot rather than a
/// uuid, which is why those two roles produced no `Ext/Rights.xml` at all.
///
/// A block whose condition text is empty is not printed. `{1,"",0}` occurs as
/// the first block of the `InformationRegister.ВерсииОбъектов` `Read`
/// restriction in `Roles/ЧтениеИнформацииОВерсияхОбъектов` -- present in four
/// corpora (ERP УХ, УТ, БСП демо, БСП базовая), and in all four native prints
/// a single block, the second one. No `<restrictionByCondition>` anywhere on
/// the stand carries an empty `<condition>`.
pub(super) fn parse_role_restriction_condition(
    value: &str,
    field_refs: &BTreeMap<String, String>,
    object_name: &str,
) -> Option<RoleRestrictionEntry> {
    let wrapper = split_1c_braced_fields(value, 0)?;
    let count_text = wrapper.first()?.trim();
    // Exactly `{0}`: an entry with no condition payload (see
    // `RoleRestrictionEntry`). A zero-count wrapper with more fields is
    // unobserved in any corpus and stays refused.
    if count_text == "0" {
        return (wrapper.len() == 1).then_some(RoleRestrictionEntry::Conditionless);
    }
    let count = count_text.parse::<usize>().ok()?;
    if wrapper.len() != count.checked_add(1)? {
        return None;
    }
    let mut blocks = Vec::with_capacity(count);
    for block in wrapper.iter().skip(1) {
        let restriction = parse_role_restriction_block(block, field_refs, object_name)?;
        if restriction.condition.is_empty() {
            continue;
        }
        blocks.push(restriction);
    }
    Some(RoleRestrictionEntry::Conditions(blocks))
}

/// One `<restrictionByCondition>` block: `{1, "<condition>"[, 0]}` without a
/// field, `{1, "<condition>", 1, <field payload>}` with one.
pub(super) fn parse_role_restriction_block(
    value: &str,
    field_refs: &BTreeMap<String, String>,
    object_name: &str,
) -> Option<RoleRightRestriction> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let (condition, _) = parse_1c_quoted_string_with_len(fields.get(1)?.trim())?;
    let field = match fields.get(2).map(|field| field.trim()) {
        None | Some("0") => None,
        Some("1") => Some(parse_role_restriction_field(
            fields.get(3)?,
            field_refs,
            object_name,
        )?),
        Some(_) => return None,
    };
    Some(RoleRightRestriction { field, condition })
}

/// Resolves a restriction block's field payload to the field's name.
///
/// The payload names either a declared field by uuid (`{{0},{0,<uuid>}}`,
/// resolved through `field_refs`) or one of the owner's standard attributes
/// by slot (`{{0},{<slot>}}`). The slot form is read through the same
/// per-kind table the nested-object names use: measured on ERP УХ 3.2.12.6
/// (2026-08-25), the six slot-form payloads on the stand are `{-5}` on four
/// `Document` owners and `{-8}` on two `Catalog` owners, and native prints
/// `<field>Ref</field>` for all six -- which is what those two slots carry in
/// the tables (see `role_standard_attribute_descriptor`).
pub(super) fn parse_role_restriction_field(
    value: &str,
    field_refs: &BTreeMap<String, String>,
    object_name: &str,
) -> Option<String> {
    if let Some((_, name)) = field_refs
        .iter()
        .find(|(uuid, _)| value.contains(uuid.as_str()))
    {
        return Some(name.clone());
    }
    if let Some(name) = role_restriction_standard_attribute_field(value, object_name) {
        return Some(name);
    }
    parse_role_restriction_field_payload(value)
}

/// The `{{0},{<slot>}}` payload form: a standard attribute of the restricted
/// object, named by its slot.
fn role_restriction_standard_attribute_field(value: &str, object_name: &str) -> Option<String> {
    let (kind, _) = object_name.split_once('.')?;
    let fields = split_1c_braced_fields(value, 0)?;
    let slot_field = fields.iter().rev().find_map(|field| {
        let inner = split_1c_braced_fields(field.trim(), 0)?;
        if inner.len() != 1 {
            return None;
        }
        inner.first()?.trim().parse::<isize>().ok()
    })?;
    if slot_field >= 0 {
        return None;
    }
    let (attribute, _) = role_standard_attribute_descriptor(kind, slot_field, None)?;
    Some(attribute.to_string())
}

pub(super) fn parse_role_restriction_field_payload(value: &str) -> Option<String> {
    if let Some(value) = parse_1c_quoted_string(value.trim()) {
        return Some(value);
    }
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() == "1" {
        return fields
            .get(1)
            .and_then(|field| parse_1c_quoted_string(field.trim()));
    }
    fields
        .iter()
        .find_map(|field| parse_role_restriction_field_payload(field))
}

pub(super) fn parse_role_restriction_templates(
    value: &str,
) -> Option<Vec<RoleRestrictionTemplate>> {
    let fields = split_1c_braced_fields(value, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if fields.len() != count + 1 {
        return None;
    }
    let mut templates = Vec::with_capacity(count);
    for field in fields.iter().skip(1) {
        let template = split_1c_braced_fields(field, 0)?;
        if template.len() != 2 {
            return None;
        }
        let name = parse_1c_quoted_string_with_len(template.first()?.trim())?.0;
        let condition = parse_1c_quoted_string_with_len(template.get(1)?.trim())?.0;
        templates.push(RoleRestrictionTemplate { name, condition });
    }
    Some(templates)
}

pub(super) fn format_role_rights_xml(rights: &RoleRights) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"Rights\" version=\"2.20\">\r\n\
\t<setForNewObjects>{}</setForNewObjects>\r\n\
\t<setForAttributesByDefault>{}</setForAttributesByDefault>\r\n\
\t<independentRightsOfChildObjects>{}</independentRightsOfChildObjects>\r\n",
        xml_bool(rights.set_for_new_objects),
        xml_bool(rights.set_for_attributes_by_default),
        xml_bool(rights.independent_rights_of_child_objects)
    );
    for object in &rights.objects {
        let object_rights = role_rights_for_xml(rights, object);
        if object_rights.is_empty() {
            // The platform never prints an empty `<object>` block, for any
            // object kind or reason: 0/0 across a direct scan of every
            // `<object>` in the full ERP УХ 3.2.12.6 native corpus
            // (2026-08-25, all 2,118 roles). Originally proven narrower (for
            // the conditionless-restriction value-only mode: 15/15 such
            // objects absent from the native file); generalized here because
            // the same absence-of-counterexamples now holds unconditionally,
            // and the top-level `setForNewObjects`-default rule above can
            // itself empty an object's right list with no marker involved.
            continue;
        }
        xml.push_str("\t<object>\r\n\t\t<name>");
        xml.push_str(&escape_xml_element_text(&object.name));
        xml.push_str("</name>\r\n");
        for right in object_rights {
            xml.push_str("\t\t<right>\r\n\t\t\t<name>");
            xml.push_str(&escape_xml_element_text(&right.name));
            xml.push_str("</name>\r\n\t\t\t<value>");
            xml.push_str(xml_bool(right.value));
            xml.push_str("</value>\r\n");
            for restriction in &right.restrictions {
                xml.push_str("\t\t\t<restrictionByCondition>\r\n");
                if let Some(field) = &restriction.field {
                    xml.push_str("\t\t\t\t<field>");
                    xml.push_str(&escape_xml_element_text(field));
                    xml.push_str("</field>\r\n");
                }
                xml.push_str("\t\t\t\t<condition>");
                // Condition text is handed to `escape_xml_element_text`
                // verbatim, and that escaper's single `\r\n` -> `\n`
                // replacement is the platform's whole rule for it: once, not
                // twice, and never a bare-`\r` pass. Measured on ERP УХ
                // 3.2.12.6 (2026-08-25) by extracting role Rights elements
                // (`<role uuid>.0`) and comparing their inflated bytes with
                // the native `Ext/Rights.xml`. `Roles/ЧтениеПеремещенийОС`
                // stores every condition line break as `\r\n` (3,196 of them,
                // no `\r\r\n`) and native prints a bare `\n`;
                // `Roles/ПросмотрТрансляции` stores 3,219 of its breaks as
                // `\r\r\n` and native prints `\r\n` for each -- dropping one
                // `\r`, keeping the other. One replacement maps both
                // (`"\r\r\n"` -> `"\r\n"`); a second collapse, or a lone-`\r`
                // pass, flattens the second case to `\n`. Four roles in the
                // 2,118-role corpus carry `\r` in a native condition, all as
                // `\r\n`; no native condition prints a lone `\r`.
                xml.push_str(&escape_xml_element_text(&restriction.condition));
                xml.push_str("</condition>\r\n\t\t\t</restrictionByCondition>\r\n");
            }
            xml.push_str("\t\t</right>\r\n");
        }
        xml.push_str("\t</object>\r\n");
    }
    for template in &rights.restriction_templates {
        xml.push_str("\t<restrictionTemplate>\r\n\t\t<name>");
        xml.push_str(&escape_xml_element_text(&template.name));
        xml.push_str("</name>\r\n\t\t");
        if template.condition.is_empty() {
            // An empty restriction-template condition is written as an empty
            // element, not as an open/close pair. One occurrence in the whole
            // stand -- `Roles/ЧтениеКовенантов`, template
            // `ПоЗначениямРасширенный` -- and zero counterexamples: across the
            // role trees of all seven configurations, `<condition/>` appears
            // once and `<condition></condition>` never. The
            // `restrictionByCondition` condition below is never empty in any
            // of them, so nothing measures its empty form and it is left as
            // it was.
            xml.push_str("<condition/>");
        } else {
            xml.push_str("<condition>");
            // Verbatim, for the reason spelled out at the
            // `restrictionByCondition` condition above.
            xml.push_str(&escape_xml_element_text(&template.condition));
            xml.push_str("</condition>");
        }
        xml.push_str("\r\n\t</restrictionTemplate>\r\n");
    }
    xml.push_str("</Rights>");
    xml
}

pub(super) fn role_rights_for_xml<'a>(
    rights: &RoleRights,
    object: &'a RoleObjectRights,
) -> Vec<&'a RoleRight> {
    if is_configuration_root_rights_object(&object.name) && !object.has_conditionless_restrictions {
        // The Configuration root has its own convention, proven over the
        // whole ERP УХ role corpus (2026-08-24, 1,679/1,679 Configuration-
        // scoped Rights blobs matched exactly): a right renders exactly when
        // its value differs from this role's own `setForNewObjects` flag.
        // Ordinary roles leave `setForNewObjects` false, default every
        // administrative right to `false`, and show the explicit grants; the
        // platform's one "full access" role sets `setForNewObjects` true,
        // defaults every right to `true`, and shows the explicit denials
        // instead. Row-level restriction is not a native concept for
        // administrative rights in any sampled role, but the check is kept
        // for symmetry with every other object kind.
        return object
            .rights
            .iter()
            .filter(|right| {
                !right.restrictions.is_empty() || right.value != rights.set_for_new_objects
            })
            .collect();
    }

    if object.has_conditionless_restrictions {
        // Value-only mode, switched on by a conditionless `{0}` restriction
        // entry (see `RoleObjectRights::has_conditionless_restrictions`): a
        // right prints exactly when its value is `true` or it carries a real
        // condition, regardless of the object kind's usual plain-false
        // conventions. Measured over ERP УХ 3.2.12.6 (2026-08-24): 84/84
        // objects, spanning Catalog, Document, DocumentJournal, Information-,
        // Accumulation- and AccountingRegister, match this rule with 0
        // counterexamples (47 true-valued marker rights print bare, 114
        // false-valued ones are hidden, real conditions keep printing).
        return object
            .rights
            .iter()
            .filter(|right| right.value || !right.restrictions.is_empty())
            .collect();
    }

    if is_top_level_rights_object_name(&object.name) {
        // Every top-level object (`Kind.Name`, exactly one dot; not the
        // Configuration root, handled above) follows the *same* rule proven
        // for the Configuration root: a right renders exactly when its value
        // differs from this role's own `setForNewObjects` flag, or it
        // carries a real restriction condition. Proven directly against the
        // full ERP УХ 3.2.12.6 native corpus (2026-08-25): every one of
        // 95,124 printed top-level rights across all 2,118 roles satisfies
        // `restricted || value != setForNewObjects` (0 counterexamples, one
        // direction), and re-deriving the missing side from our own parsed
        // right list against the same corpus's 1,401 then-differing
        // Rights.xml files gives 274,936 confirms and 0 violations (the
        // other direction). The one role that sets `setForNewObjects: true`
        // (`ПолныеПрава`) is the sole source of every "plain `false` shown,
        // no restriction" occurrence anywhere in the corpus (10,532/10,532),
        // confirming the rule inverts there exactly as it does for the
        // Configuration root, not just on it. This replaces the former
        // per-kind, per-name suppression lists (Document's ~20-name list,
        // AccumulationRegister's Edit/Update/View list, and the
        // restriction-gated `should_suppress_plain_false_role_rights`
        // heuristic), none of which extended to Catalog, InformationRegister,
        // Constant or any other top-level kind, and all of which the corpus
        // now shows were special cases of this one rule.
        return object
            .rights
            .iter()
            .filter(|right| {
                !right.restrictions.is_empty() || right.value != rights.set_for_new_objects
            })
            .collect();
    }

    // Nested objects (attributes, standard attributes, tabular-section
    // attributes, commands, resources, addressing attributes, …): only the
    // `View`/`Edit` pair on a non-"action" category follows the
    // `setForAttributesByDefault` rule below; every other right (any name,
    // on any category) keeps the pre-existing "always print" behavior.
    // Proven over a direct scan of the whole ERP УХ 3.2.12.6 native corpus
    // (2026-08-25, all 2,118 roles): restricted to `View`/`Edit` on
    // non-action categories, 146,661/146,661 checks confirm `restricted ||
    // value != setForAttributesByDefault` with 0 violations, both
    // directions (native-printed rights, and our own parsed right list
    // against the same corpus's then-differing files). The category gate
    // (`Command`, `Subsystem`, `Operation`, `URLTemplate`,
    // `IntegrationServiceChannel` — naming a specific command/subsystem/
    // service-operation, not a data attribute) is needed because those
    // categories also use the right name `View` (Command/Subsystem) or
    // `Use` (Operation/URLTemplate/IntegrationServiceChannel) but always
    // print regardless of value (1,887/1,887 confirms). The right-name gate
    // is needed separately: a category outside this proven set can still
    // carry non-`View`/`Edit` rights that must stay "always print" too — a
    // fast-gate regression on `ssl` (`CalculationRegister.…Recalculation.…`,
    // `Read`/`Update`, unseen in the ERP УХ census) showed a first version
    // of this rule that suppressed by category alone was too broad;
    // restricting the rule to `View`/`Edit` specifically closes that gap
    // without reintroducing the Command/Subsystem violation the category
    // gate exists for.
    let action_like_category = object
        .name
        .split('.')
        .nth(2)
        .is_some_and(is_action_like_nested_rights_category);
    object
        .rights
        .iter()
        .filter(|right| {
            if object.adopted
                || action_like_category
                || !matches!(right.name.as_str(), "View" | "Edit")
            {
                return true;
            }
            !right.restrictions.is_empty() || right.value != rights.set_for_attributes_by_default
        })
        .collect()
}

/// The closed set of nested-object category segments (`Kind.Name.<Category>.
/// <Leaf>`) that name a specific command, subsystem, web-service operation,
/// URL template or integration-service channel rather than a data attribute.
/// See `role_rights_for_xml`'s nested-object branch for the corpus proof;
/// every category observed anywhere in the whole ERP УХ 3.2.12.6 role corpus
/// falls into this set or is handled by the `setForAttributesByDefault` rule.
pub(super) fn is_action_like_nested_rights_category(category: &str) -> bool {
    matches!(
        category,
        "Command" | "Subsystem" | "Operation" | "URLTemplate" | "IntegrationServiceChannel"
    )
}

pub(super) fn is_top_level_rights_object_name(name: &str) -> bool {
    name.matches('.').count() == 1
}
