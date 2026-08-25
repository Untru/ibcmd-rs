use super::*;

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
    pub(super) restriction_by_condition: Option<RoleRightRestriction>,
}

#[derive(Clone)]
pub(super) struct RoleRightRestriction {
    pub(super) field: Option<String>,
    pub(super) condition: String,
}

/// One parsed entry of an object's right-restrictions table
/// (`{RIGHT_UUID,{…}}`): the wrapper's first field selects the payload kind.
///
/// Kinds `1` (plain condition text) and `2` (condition with a field
/// reference) carry a real condition. Kind `0` carries no condition at all
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
    Condition(RoleRightRestriction),
}

/// An object's parsed right-restrictions table: real conditions by right
/// UUID, plus whether any conditionless `{0}` entry was present.
pub(super) struct RoleRightRestrictions {
    pub(super) by_right: BTreeMap<String, RoleRightRestriction>,
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
    let inflated = crate::compiler::bodies::rights::decode_compatible_role_rights(bytes)
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
    let set_for_attributes_by_default = parse_role_bool_field_or_default(&fields, 4, true)?;
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
        let object_name = role_object_ref_name(&object_ref, object_refs)?;

        let (rights, has_conditionless_restrictions) =
            if is_configuration_root_rights_object(&object_name) {
                (
                    parse_configuration_root_object_rights(entry[1], set_for_new_objects)?,
                    false,
                )
            } else {
                parse_role_object_rights(entry[1], field_refs)?
            };
        let intra_uuid_order =
            role_rights_object_intra_uuid_order(&object_ref, &object_name).unwrap_or(0);
        objects.push((
            object_uuid.to_string(),
            intra_uuid_order,
            serialized_index,
            RoleObjectRights {
                name: object_name,
                rights,
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
    let kind_code = fields.get(2).map(|field| field.trim());
    let slot_code = fields.get(3).map(|field| field.trim())?;
    role_standard_attribute_sort_order(object_name, kind_code, slot_code)
}

pub(super) fn role_object_ref_name(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let uuid = fields.get(1)?.trim();
    if !is_uuid_text(uuid) {
        return None;
    }
    let base_name = object_refs
        .get(uuid)
        .cloned()
        .unwrap_or_else(|| uuid.to_string());
    let kind_code = fields.get(2).map(|field| field.trim());
    let slot_code = fields.get(3).map(|field| field.trim());
    role_standard_attribute_ref_name(&base_name, kind_code, slot_code).or(Some(base_name))
}

pub(super) fn role_standard_attribute_ref_name(
    base_name: &str,
    kind_code: Option<&str>,
    slot_code: Option<&str>,
) -> Option<String> {
    if kind_code != Some("1") {
        return None;
    }
    let slot = role_standard_attribute_slot(slot_code?)?;
    let slot_index = usize::try_from(slot).ok();
    let (kind, _) = base_name.split_once('.')?;
    let attribute = role_standard_attribute_descriptor(kind, slot, slot_index)?.0;
    Some(format!("{base_name}.StandardAttribute.{attribute}"))
}

pub(super) fn role_standard_attribute_slot(slot_code: &str) -> Option<isize> {
    slot_code.parse::<isize>().ok().or_else(|| {
        split_1c_braced_fields(slot_code, 0)?
            .first()?
            .trim()
            .parse::<isize>()
            .ok()
    })
}

pub(super) fn role_standard_attribute_sort_order(
    base_name: &str,
    kind_code: Option<&str>,
    slot_code: &str,
) -> Option<usize> {
    if kind_code != Some("1") || !slot_code.trim_start().starts_with('{') {
        return None;
    }
    let slot = role_standard_attribute_slot(slot_code)?;
    let slot_index = usize::try_from(slot).ok();
    let (kind, _) = base_name.split_once('.')?;
    role_standard_attribute_descriptor(kind, slot, slot_index).map(|(_, order)| order)
}

pub(super) fn role_standard_attribute_descriptor(
    kind: &str,
    slot: isize,
    slot_index: Option<usize>,
) -> Option<(&'static str, usize)> {
    match kind {
        "Catalog" => match slot {
            -13 => Some(("PredefinedDataName", 1)),
            -10 => Some(("Predefined", 2)),
            -8 => Some(("Ref", 3)),
            -7 => Some(("DeletionMark", 4)),
            -6 => Some(("IsFolder", 5)),
            -5 => Some(("Owner", 6)),
            -4 => Some(("Parent", 7)),
            -3 => Some(("Description", 8)),
            -2 => Some(("Code", 9)),
            _ => [
                ("PredefinedDataName", 1),
                ("Predefined", 2),
                ("Ref", 3),
                ("DeletionMark", 4),
                ("IsFolder", 5),
                ("Owner", 6),
                ("Parent", 7),
                ("Description", 8),
                ("Code", 9),
            ]
            .get(slot_index?)
            .copied(),
        },
        "ChartOfCharacteristicTypes" => match slot {
            -14 => Some(("PredefinedDataName", 1)),
            -11 => Some(("ValueType", 2)),
            -9 => Some(("Description", 3)),
            -8 => Some(("Code", 4)),
            -7 => Some(("IsFolder", 5)),
            -6 => Some(("Parent", 6)),
            -5 => Some(("Predefined", 7)),
            -4 => Some(("DeletionMark", 8)),
            -2 => Some(("Ref", 9)),
            _ => [
                ("PredefinedDataName", 1),
                ("ValueType", 2),
                ("Description", 3),
                ("Code", 4),
                ("IsFolder", 5),
                ("Parent", 6),
                ("Predefined", 7),
                ("DeletionMark", 8),
                ("Ref", 9),
            ]
            .get(slot_index?)
            .copied(),
        },
        "Document" => slot_index
            .and_then(|index| {
                [
                    ("Posted", 1),
                    ("Ref", 2),
                    ("DeletionMark", 3),
                    ("Date", 4),
                    ("Number", 5),
                ]
                .get(index)
                .copied()
            })
            .or(match slot {
                -7 => Some(("Number", 5)),
                -5 => Some(("Date", 4)),
                -4 => Some(("DeletionMark", 3)),
                -3 => Some(("Ref", 2)),
                -2 => Some(("Posted", 1)),
                _ => None,
            }),
        "ExchangePlan" => match slot {
            -14 => Some(("ExchangeDate", 1)),
            -13 => Some(("ThisNode", 2)),
            -10 => Some(("ReceivedNo", 3)),
            -9 => Some(("SentNo", 4)),
            -6 => Some(("Ref", 5)),
            -4 => Some(("DeletionMark", 6)),
            -3 => Some(("Description", 7)),
            -2 => Some(("Code", 8)),
            _ => [
                ("ExchangeDate", 1),
                ("ThisNode", 2),
                ("ReceivedNo", 3),
                ("SentNo", 4),
                ("Ref", 5),
                ("DeletionMark", 6),
                ("Description", 7),
                ("Code", 8),
            ]
            .get(slot_index?)
            .copied(),
        },
        "AccumulationRegister" => match slot {
            -9 => Some(("RecordType", 1)),
            0 | -5 => Some(("Active", 2)),
            1 | -2 => Some(("Period", 5)),
            2 | -3 => Some(("Recorder", 4)),
            3 | -4 => Some(("LineNumber", 3)),
            _ => None,
        },
        "AccountingRegister" | "CalculationRegister" | "InformationRegister" => match slot {
            0 | -5 => Some(("Active", 1)),
            1 | -2 => Some(("Period", 4)),
            2 | -3 => Some(("Recorder", 3)),
            3 | -4 => Some(("LineNumber", 2)),
            _ => None,
        },
        _ => None,
    }
}

/// Right UUIDs the platform writes on the Configuration root that carry no
/// name in `ROLE_RIGHT_NAMES`.
///
/// Measured over the ERP УХ role corpus (2026-08-24, 1,679 roles whose Rights
/// blob has a Configuration-object entry): both UUIDs occur in every one of
/// them, and in every occurrence the value equals that role's own
/// `setForNewObjects` flag — which is exactly the condition under which
/// `role_rights_for_xml` never prints a Configuration-root right at all (see
/// its doc comment). They are therefore structurally invisible in every
/// observed byte of output, on any value they take. Parsing tolerates them
/// only while the equality holds, because no observed byte proves what name
/// the platform would print if it ever didn't: a role where either value
/// diverges from the flag is refused rather than guessed.
const CONFIGURATION_ROOT_TOLERATED_UNNAMED_RIGHT_UUIDS: [&str; 2] = [
    "3762abec-3836-446a-83ce-3e05001bca8b",
    "4df6d046-3bf8-4dda-991c-53ba664296a5",
];

/// True for the six Configuration-root rights that pick the client's launch
/// mode (thin/thick client window mode, analytics client). Unlike every
/// other Configuration-root right, their type default is `true`, not the
/// role's own `setForNewObjects` flag.
pub(super) fn is_configuration_mode_right(name: &str) -> bool {
    CONFIGURATION_MODE_RIGHT_NAMES.contains(&name)
}

const CONFIGURATION_MODE_RIGHT_NAMES: [&str; 6] = [
    "MainWindowModeNormal",
    "MainWindowModeWorkplace",
    "MainWindowModeEmbeddedWorkplace",
    "MainWindowModeFullscreenWorkplace",
    "MainWindowModeKiosk",
    "AnalyticsSystemClient",
];

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
            if value != set_for_new_objects {
                return None;
            }
            continue;
        }
        let name = role_right_name(right_uuid)?;
        if is_configuration_mode_right(name) {
            mode_rights_seen += 1;
        }
        if name == "SaveUserData" {
            save_user_data_index = Some(entries.len());
        }
        entries.push(RoleRight {
            name: name.to_string(),
            value,
            restriction_by_condition: None,
        });
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
                restriction_by_condition: None,
            });
            entries.splice(insert_at..insert_at, synthesized);
        }
        6 => {}
        _ => return None, // partial presence: an unproven shape
    }

    Some(entries)
}

pub(super) fn parse_role_object_rights(
    value: &str,
    field_refs: &BTreeMap<String, String>,
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
    restrictions: &BTreeMap<String, RoleRightRestriction>,
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
            restriction_by_condition: restrictions.get(right_uuid).cloned(),
        });
    }
    Some(rights)
}

pub(super) fn parse_role_bool_field(value: &str) -> Option<bool> {
    match value.trim() {
        "0" | "4294967295" => Some(false),
        "1" => Some(true),
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
        "-1" | "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_role_right_restrictions(
    count_text: &str,
    values: &[&str],
    field_refs: &BTreeMap<String, String>,
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
        RoleRestrictionEntry::Condition(condition) => {
            restrictions
                .by_right
                .insert(right_uuid.to_string(), condition);
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
            let condition = parse_role_restriction_condition(pair.get(1)?, field_refs)?;
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
            let condition = parse_role_restriction_condition(entry.get(1)?, field_refs)?;
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
        let condition = parse_role_restriction_condition(pair.get(1)?, field_refs)?;
        record(&mut restrictions, right_uuid, condition);
    }
    Some(restrictions)
}

pub(super) fn parse_role_restriction_condition(
    value: &str,
    field_refs: &BTreeMap<String, String>,
) -> Option<RoleRestrictionEntry> {
    let wrapper = split_1c_braced_fields(value, 0)?;
    match wrapper.first()?.trim() {
        // Exactly `{0}`: an entry with no condition payload (see
        // `RoleRestrictionEntry`). A kind-0 wrapper with more fields is
        // unobserved in any corpus and stays refused.
        "0" if wrapper.len() == 1 => Some(RoleRestrictionEntry::Conditionless),
        "1" => parse_role_restriction_condition_body(wrapper.get(1)?)
            .map(RoleRestrictionEntry::Condition),
        "2" => {
            let mut restriction = parse_role_restriction_condition_body(wrapper.get(2)?)?;
            let field = parse_role_restriction_field(wrapper.get(2)?, field_refs)?;
            restriction.field = Some(field);
            Some(RoleRestrictionEntry::Condition(restriction))
        }
        _ => None,
    }
}

pub(super) fn parse_role_restriction_condition_body(value: &str) -> Option<RoleRightRestriction> {
    let condition_fields = split_1c_braced_fields(value, 0)?;
    if condition_fields.first()?.trim() != "1" {
        return None;
    }
    parse_1c_quoted_string_with_len(condition_fields.get(1)?.trim()).map(|(condition, _)| {
        RoleRightRestriction {
            field: None,
            condition,
        }
    })
}

pub(super) fn parse_role_restriction_field(
    value: &str,
    field_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some((_, name)) = field_refs
        .iter()
        .find(|(uuid, _)| value.contains(uuid.as_str()))
    {
        return Some(name.clone());
    }
    if let Some(field) = parse_role_restriction_condition_body_field(value) {
        return Some(field);
    }
    let field_wrapper = split_1c_braced_fields(value, 0)?;
    if field_wrapper.first()?.trim() != "0" {
        return None;
    }
    let field_fields = split_1c_braced_fields(field_wrapper.get(1)?, 0)?;
    if field_fields.first()?.trim() != "1" {
        return None;
    }
    parse_1c_quoted_string_with_len(field_fields.get(1)?.trim()).map(|(value, _)| value)
}

pub(super) fn parse_role_restriction_condition_body_field(value: &str) -> Option<String> {
    let condition_fields = split_1c_braced_fields(value, 0)?;
    if condition_fields.first()?.trim() != "1" {
        return None;
    }
    let field_payload = condition_fields.get(3)?;
    parse_role_restriction_field_payload(field_payload)
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

pub(super) fn role_right_name(uuid: &str) -> Option<&'static str> {
    ROLE_RIGHT_NAMES_BY_UUID.get(uuid).copied()
}

static ROLE_RIGHT_NAMES_BY_UUID: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ROLE_RIGHT_NAMES.iter().copied().collect());

const ROLE_RIGHT_NAMES: &[(&str, &str)] = &[
    ("fd05f656-7a23-43a4-8996-f480a806fb97", "ActiveUsers"),
    ("900e3c92-6e18-4874-846a-b28780b5b54c", "Administration"),
    (
        "f7c6a0bb-bca6-4cd3-9146-832971cd7073",
        "AnalyticsSystemClient",
    ),
    ("07ef4641-f7da-417a-bd75-35c40a17c2f7", "Automation"),
    (
        "399d7390-8d83-4a57-b4d7-c902c15b701f",
        "ConfigurationExtensionsAdministration",
    ),
    ("10b8ce49-ae3d-4a2e-afe7-1e3648bd59f7", "DataAdministration"),
    ("c0028105-4cc1-41ca-aef1-bfbd8fc8f8c4", "Delete"),
    ("b7bab52d-c1b1-4bd8-8276-02db08d42352", "Edit"),
    (
        "8497054a-ffd1-4ca7-bdfe-340b9ddc050a",
        "EditDataHistoryVersionComment",
    ),
    ("1c799cf9-342d-4bf7-9b6f-951a009228ce", "EventLog"),
    ("8fb221e3-0d4f-43f2-ad71-1984cad63375", "ExclusiveMode"),
    ("74fd69fa-368e-4292-956a-65eb2f9877bd", "Execute"),
    ("02119c69-f08a-4142-9426-3725d74b7719", "ExternalConnection"),
    ("499e8968-ca89-43f0-9955-8756058b1b53", "Get"),
    ("b5f861d3-d9c5-45ec-98bf-0ed4d489a351", "InputByString"),
    ("33200740-82b0-4de7-8556-d3fb25ca4328", "Insert"),
    (
        "3b869658-ebc9-49ff-9bb3-e7c59686f538",
        "InteractiveActivate",
    ),
    (
        "b0c0cbfc-f2cc-4b80-8460-5d5d7a599d9d",
        "InteractiveChangeOfPosted",
    ),
    (
        "798cf688-ad74-44fe-a464-236b49e910e0",
        "InteractiveClearDeletionMark",
    ),
    (
        "e7f9daf9-eac2-4ada-9c26-c380858f3589",
        "InteractiveClearDeletionMarkPredefinedData",
    ),
    ("b53db6ed-6e5b-4035-8d24-f10083d646ed", "InteractiveDelete"),
    (
        "fa6dbe86-856a-4ac4-b8ac-bce99f8b8b22",
        "InteractiveDeleteMarked",
    ),
    (
        "65e5f92c-40ff-4130-9652-c0e7612d0609",
        "InteractiveDeleteMarkedPredefinedData",
    ),
    (
        "013a262e-165f-4815-bdae-7a1bed6a68e4",
        "InteractiveDeletePredefinedData",
    ),
    ("fb88c756-91c9-4351-9cdf-e027879886c6", "InteractiveInsert"),
    (
        "7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6",
        "InteractiveOpenExtDataProcessors",
    ),
    (
        "eb29e198-c338-4a20-a253-be6fc3dd44d9",
        "InteractiveOpenExtReports",
    ),
    ("5d167fcc-b11f-403a-9a37-1eda64c19df1", "InteractivePosting"),
    (
        "21b4742a-d335-4234-bf0f-a3074a0e31ac",
        "InteractivePostingRegular",
    ),
    (
        "d76b72ba-5388-4b7f-af64-1b351f63a1e1",
        "InteractiveSetDeletionMark",
    ),
    (
        "408c56c0-e210-4e2e-8e82-610050a08a39",
        "InteractiveSetDeletionMarkPredefinedData",
    ),
    (
        "4d0d77ec-8511-430d-bd77-8407f27bc8f4",
        "InteractiveUndoPosting",
    ),
    ("5e664189-f0ee-439c-bdc5-eb81cca41ddf", "InteractiveExecute"),
    (
        "b9b44b51-3ac9-47cd-8b5a-df51afdcceb0",
        "MainWindowModeEmbeddedWorkplace",
    ),
    (
        "818fc6c3-4691-44e3-a80c-e8d424730ead",
        "MainWindowModeFullscreenWorkplace",
    ),
    (
        "155a0b35-4343-4047-989b-d385373b063e",
        "MainWindowModeKiosk",
    ),
    (
        "d066966a-ff6a-4a41-bd68-6191cab083bc",
        "MainWindowModeNormal",
    ),
    (
        "f6168734-8b8d-4a88-ab39-ef6b51758e83",
        "MainWindowModeWorkplace",
    ),
    ("1e50809b-73ed-4935-bb77-2616c4cabdf5", "MobileClient"),
    ("31c3d4f6-7d02-4654-a14e-06aacafcb4fa", "Output"),
    ("e060de25-bffd-42fd-bb09-f3a788d65760", "Posting"),
    ("1c87578f-9e09-4ec0-a991-5629c87b1588", "Read"),
    ("64319ca1-f3d8-472e-82ce-5da233e6daaa", "ReadDataHistory"),
    (
        "1b762bf9-df7f-4255-bbe6-f7578f41368d",
        "ReadDataHistoryOfMissingData",
    ),
    ("d8682bbb-7800-4aa0-8590-d3cb11fe2a29", "SaveUserData"),
    ("1d306db2-d97e-4b57-9b28-5d21e838cd9e", "Set"),
    ("65b6855f-85d5-4d33-ab75-be4485326dd5", "Start"),
    ("84487e82-eb6c-4c51-ae16-3a6db17e886d", "InteractiveStart"),
    (
        "479a42c0-c3e9-4ae7-bf4a-75cebc14fec4",
        "SwitchToDataHistoryVersion",
    ),
    (
        "265eec41-3ce1-4a07-bc3b-253d44c9a4f4",
        "TechnicalSpecialistMode",
    ),
    ("29da0973-3b85-40e5-89da-bce02dbab08e", "ThickClient"),
    ("3c00c6ee-844e-4620-85e4-671e72f114d9", "ThinClient"),
    ("24abfe06-289a-48c5-8bb4-032c733e45c5", "TotalsControl"),
    ("f55a8f7f-2c65-404f-b530-093d9006adba", "UndoPosting"),
    ("287b74b8-3a66-4a76-ba27-4f1f6a93770e", "Update"),
    (
        "4d87a22d-ca7f-40ba-a367-a4eae62f4a7f",
        "UpdateDataBaseConfiguration",
    ),
    ("b162ff57-0296-483e-9af8-dc37576802cb", "UpdateDataHistory"),
    (
        "c4ab1331-e58d-4a46-ad2e-fe6d80b72aa4",
        "UpdateDataHistoryOfMissingData",
    ),
    (
        "a679c969-8ea1-4b8b-9e61-8a414ba448f4",
        "UpdateDataHistorySettings",
    ),
    (
        "5b3ea0e2-fdb9-41f6-bf6c-25747906b4cb",
        "UpdateDataHistoryVersionComment",
    ),
    ("c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1", "Use"),
    ("aa6448f2-be0f-42ea-ba26-1af7f52b5b65", "View"),
    ("9342b152-a7ae-4c79-9b7b-f4f028a36479", "ViewDataHistory"),
    ("bd33c881-192c-4ef7-a51d-b146e38c5078", "WebClient"),
];

pub(super) fn normalize_role_condition_text(condition: &str) -> String {
    condition.replace("\r\n", "\n").replace('\r', "\n")
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
            if let Some(restriction) = &right.restriction_by_condition {
                xml.push_str("\t\t\t<restrictionByCondition>\r\n");
                if let Some(field) = &restriction.field {
                    xml.push_str("\t\t\t\t<field>");
                    xml.push_str(&escape_xml_element_text(field));
                    xml.push_str("</field>\r\n");
                }
                xml.push_str("\t\t\t\t<condition>");
                xml.push_str(&escape_xml_element_text(&normalize_role_condition_text(
                    &restriction.condition,
                )));
                xml.push_str("</condition>\r\n\t\t\t</restrictionByCondition>\r\n");
            }
            xml.push_str("\t\t</right>\r\n");
        }
        xml.push_str("\t</object>\r\n");
    }
    for template in &rights.restriction_templates {
        xml.push_str("\t<restrictionTemplate>\r\n\t\t<name>");
        xml.push_str(&escape_xml_element_text(&template.name));
        xml.push_str("</name>\r\n\t\t<condition>");
        xml.push_str(&escape_xml_element_text(&normalize_role_condition_text(
            &template.condition,
        )));
        xml.push_str("</condition>\r\n\t</restrictionTemplate>\r\n");
    }
    xml.push_str("</Rights>");
    xml
}

pub(super) fn role_rights_for_xml<'a>(
    rights: &RoleRights,
    object: &'a RoleObjectRights,
) -> Vec<&'a RoleRight> {
    if is_configuration_root_rights_object(&object.name) {
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
                right.restriction_by_condition.is_some()
                    || right.value != rights.set_for_new_objects
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
            .filter(|right| right.value || right.restriction_by_condition.is_some())
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
                right.restriction_by_condition.is_some()
                    || right.value != rights.set_for_new_objects
            })
            .collect();
    }

    // Nested objects (attributes, standard attributes, tabular-section
    // attributes, commands, resources, addressing attributes, …) are left at
    // the pre-existing "print every right regardless of value" behavior:
    // measured over the same corpus, `Edit`/`View` rights on 366,965/354,209
    // nested objects show both `true` and `false` freely, under both
    // `setForNewObjects` values, with no value-based suppression pattern like
    // the top-level rule above (unlike the top-level case, this is not a
    // 0-counterexample proof — a residual ~137-file gap on nested `View`
    // rights, unrelated to this change, is not addressed here).
    object.rights.iter().collect()
}

pub(super) fn is_top_level_rights_object_name(name: &str) -> bool {
    name.matches('.').count() == 1
}
