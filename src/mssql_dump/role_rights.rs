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

        let (rights, has_conditionless_restrictions) =
            if is_configuration_root_rights_object(&object_name) {
                (
                    parse_configuration_root_object_rights(entry[1], set_for_new_objects)?,
                    false,
                )
            } else {
                parse_role_object_rights(entry[1], field_refs, &object_name)?
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

/// Family uuid a nested reference's slot carries for an ordinary standard
/// attribute, and for a standard tabular section.
///
/// Both are platform-fixed: every `{slot, family}` pair observed in an ERP УХ
/// 3.2.12.6 role Rights blob uses `STANDARD_ATTRIBUTE_FAMILY` except the
/// chart-of-accounts `ExtDimensionTypes` section, which uses
/// `STANDARD_TABULAR_SECTION_FAMILY`.
const STANDARD_ATTRIBUTE_FAMILY: &str = "03f171e8-326f-41c6-9fa5-932a0b12cddf";
const STANDARD_TABULAR_SECTION_FAMILY: &str = "28db313d-dbc2-4b83-8c4a-d2aeee708062";

/// The two families an accounting register's ext-dimension pair is split
/// across: `{n, EXT_DIMENSION_FAMILY}` is `ExtDimension<n+1>`,
/// `{n, EXT_DIMENSION_TYPE_FAMILY}` is `ExtDimensionType<n+1>`.
const EXT_DIMENSION_FAMILY: &str = "91162600-3161-4326-89a0-4a7cecd5092a";
const EXT_DIMENSION_TYPE_FAMILY: &str = "b3b48b29-d652-47ab-9d21-7e06768c31b5";

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

/// The standard tabular sections a role's rights can name, with the print
/// order of the section's own entry.
///
/// Section slots come from the owner's storage element, where each section
/// block is introduced by its slot: the chart of accounts' single
/// `ExtDimensionTypes` is `-12`, and a chart of calculation types' three are
/// `-30`, `-20`, `-10` in the order its `<StandardTabularSections>` block
/// declares them -- Leading, Displacing, Base
/// (`ChartsOfCalculationTypes/Начисления` and `.../Удержания` agree).
/// Orders are read from the native Rights print sequence
/// (`Roles/БазовыеПраваУХ`, `Roles/БазовыеПраваБПУХ`), where a section prints
/// immediately before its own attributes.
fn role_standard_tabular_section(kind: &str, slot: isize) -> Option<(&'static str, usize)> {
    match (kind, slot) {
        ("ChartOfAccounts", -12) => Some(("ExtDimensionTypes", 3)),
        ("ChartOfCalculationTypes", -30) => Some(("LeadingCalculationTypes", 1)),
        ("ChartOfCalculationTypes", -20) => Some(("DisplacingCalculationTypes", 5)),
        ("ChartOfCalculationTypes", -10) => Some(("BaseCalculationTypes", 10)),
        _ => None,
    }
}

/// Standard attributes of a standard tabular section, with their position
/// inside it. Slots come from the tail of the owner element's
/// `<StandardAttributes>` list, paired positionally as everywhere else. The
/// printed order is the section's own order plus this position, which is how
/// native lays both kinds out: chart of accounts section 3, attributes 4..=7;
/// chart of calculation types sections 1/5/10, attributes 2..=4, 6..=8 and
/// 11..=13.
fn role_standard_tabular_section_attribute(
    kind: &str,
    slot: isize,
) -> Option<(&'static str, usize)> {
    match (kind, slot) {
        ("ChartOfAccounts", -15) => Some(("TurnoversOnly", 1)),
        ("ChartOfAccounts", -14) => Some(("Predefined", 2)),
        ("ChartOfAccounts", -13) => Some(("ExtDimensionType", 3)),
        ("ChartOfAccounts", -12) => Some(("LineNumber", 4)),
        ("ChartOfCalculationTypes", -102) => Some(("Predefined", 1)),
        ("ChartOfCalculationTypes", -101) => Some(("CalculationType", 2)),
        ("ChartOfCalculationTypes", -100) => Some(("LineNumber", 3)),
        _ => None,
    }
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
                // Paired positionally, like every other kind here:
                // `Documents/ЗаказКлиента`'s element declares `-7 -5 -4 -3 -2`
                // in the order its `<StandardAttributes>` block lists Posted,
                // Ref, DeletionMark, Date, Number. The pairing is directly
                // observable for `-5`: the restriction field payload
                // `{{0},{-5}}` on `Document.КорректировкаЗначенийПоказателей`
                // and `Document.НастраиваемыйОтчет` prints as
                // `<field>Ref</field>` in
                // `Roles/ЧтениеБюджетированиеИОтчетность` and
                // `Roles/ДобавлениеИзменениеБюджетированиеИОтчетность` (4
                // occurrences, 0 counterexamples), and the same payload form
                // `{{0},{-8}}` on two `Catalog` owners prints `Ref` too --
                // which is what `-8` already carries in the Catalog rows.
                // These slots previously ran the other way round; the print
                // order is unaffected either way, because the order values
                // follow the names.
                -7 => Some(("Posted", 1)),
                -5 => Some(("Ref", 2)),
                -4 => Some(("DeletionMark", 3)),
                -3 => Some(("Date", 4)),
                -2 => Some(("Number", 5)),
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
        "AccountingRegister" => match slot {
            // Slots from `AccountingRegisters/МСФО` and
            // `.../МеждународныйБезКорреспонденции`, paired positionally with
            // their `<StandardAttributes>` blocks (11/11 and 12/12); order
            // from the native Rights print sequence, which puts `Account` and
            // `RecordType` ahead of the four a register shares with the other
            // kinds, then the ext-dimension pairs (7 onward, see
            // `role_ext_dimension_descriptor`). `Roles/ЧтениеДанныхМСФОУХ`
            // prints the 11-member group, `Roles/БазовыеПраваБПУХ` the
            // 12-member one with `RecordType`.
            -10 => Some(("Account", 1)),
            -9 => Some(("RecordType", 2)),
            0 | -5 => Some(("Active", 3)),
            3 | -4 => Some(("LineNumber", 4)),
            2 | -3 => Some(("Recorder", 5)),
            1 | -2 => Some(("Period", 6)),
            _ => None,
        },
        "InformationRegister" => match slot {
            // Confirmed positionally against
            // `InformationRegisters/ВерсииОбъектов`: its element declares
            // `-5 -4 -3 -2` in the order its `<StandardAttributes>` block
            // lists Active, LineNumber, Recorder, Period (4/4).
            0 | -5 => Some(("Active", 1)),
            1 | -2 => Some(("Period", 4)),
            2 | -3 => Some(("Recorder", 3)),
            3 | -4 => Some(("LineNumber", 2)),
            _ => None,
        },
        "CalculationRegister" => match slot {
            // A calculation register shares none of the other registers'
            // layout: eleven standard attributes, no `Period` at all.
            // `CalculationRegisters/Начисления` and `.../Удержания` both
            // declare these eleven slots in exactly the order their
            // `<StandardAttributes>` blocks list the names (11/11 each), and
            // `Roles/БазовыеПраваБПУХ` prints the group in that same order.
            // The four rows this kind used to share with the other registers
            // were wrong in both directions: they named `-2` `Period` (a name
            // the kind does not have -- two such objects were printed that
            // native does not print) and left the other seven unnamed.
            -13 => Some(("RegistrationPeriod", 1)),
            -11 => Some(("ReversingEntry", 2)),
            -10 => Some(("Active", 3)),
            -9 => Some(("EndOfBasePeriod", 4)),
            -8 => Some(("BegOfBasePeriod", 5)),
            -7 => Some(("EndOfActionPeriod", 6)),
            -6 => Some(("BegOfActionPeriod", 7)),
            -5 => Some(("ActionPeriod", 8)),
            -4 => Some(("CalculationType", 9)),
            -3 => Some(("LineNumber", 10)),
            -2 => Some(("Recorder", 11)),
            _ => None,
        },
        "BusinessProcess" => match slot {
            // `BusinessProcesses/ЗаявкаСотрудникаНалоговыйВычет`, 9/9 against
            // its `<StandardAttributes>` block; order confirmed against the
            // sixteen business-process groups `Roles/БазовыеПраваБПУХ`
            // prints, all in this sequence.
            -9 => Some(("Started", 1)),
            -8 => Some(("HeadTask", 2)),
            -7 => Some(("Completed", 3)),
            -5 => Some(("Ref", 4)),
            -4 => Some(("DeletionMark", 5)),
            -3 => Some(("Date", 6)),
            -2 => Some(("Number", 7)),
            _ => None,
        },
        "ChartOfCalculationTypes" => match slot {
            // `ChartsOfCalculationTypes/Начисления` (23/23) and
            // `.../Удержания` (19/19). Orders 1..=13 belong to the three
            // standard tabular sections and their attributes (see
            // `role_standard_tabular_section`), which native interleaves with
            // these: the group prints Leading + its three, Displacing + its
            // three, `PredefinedDataName`, Base + its three, then the rest.
            -11 => Some(("PredefinedDataName", 9)),
            -8 => Some(("Predefined", 14)),
            -6 => Some(("Ref", 15)),
            -5 => Some(("DeletionMark", 16)),
            -4 => Some(("ActionPeriodIsBasic", 17)),
            -3 => Some(("Description", 18)),
            -2 => Some(("Code", 19)),
            _ => None,
        },
        "DocumentJournal" => match slot {
            // `DocumentJournals/ДокументыБюджетирования` and
            // `.../ОперацииСЦеннымиБумагами` both declare the same six slots
            // in the same order as their `<StandardAttributes>` block, and
            // native prints the group in that order too -- all 124
            // DocumentJournal groups in the corpus print all six.
            -60003 => Some(("Type", 1)),
            -101 => Some(("Ref", 2)),
            -100 => Some(("Date", 3)),
            -7 => Some(("Posted", 4)),
            -4 => Some(("DeletionMark", 5)),
            -2 => Some(("Number", 6)),
            _ => None,
        },
        "Task" => match slot {
            // `Tasks/БюджетнаяЗадача` and `Tasks/ЗадачаИсполнителя` agree on
            // all eight; order confirmed against
            // `Roles/ЧтениеВыполнениеБюджетныхЗадач`.
            -10 => Some(("Executed", 1)),
            -9 => Some(("Description", 2)),
            -8 => Some(("RoutePoint", 3)),
            -7 => Some(("BusinessProcess", 4)),
            -5 => Some(("Ref", 5)),
            -4 => Some(("DeletionMark", 6)),
            -3 => Some(("Date", 7)),
            -2 => Some(("Number", 8)),
            _ => None,
        },
        "ChartOfAccounts" => match slot {
            // `ChartsOfAccounts/МСФО` and `.../Хозрасчетный` declare the same
            // fourteen entries; the first ten are these, the last four belong
            // to the `ExtDimensionTypes` section (see
            // `role_standard_tabular_section_attribute`). Orders 3..=7 are
            // that section and its attributes, which native prints between
            // `Order` and `OffBalance` -- see `Roles/БазовыеПраваУХ`.
            -28 => Some(("PredefinedDataName", 1)),
            -17 => Some(("Order", 2)),
            -11 => Some(("OffBalance", 8)),
            -10 => Some(("Type", 9)),
            -8 => Some(("Description", 10)),
            -7 => Some(("Code", 11)),
            -6 => Some(("Parent", 12)),
            -5 => Some(("Predefined", 13)),
            -4 => Some(("DeletionMark", 14)),
            -2 => Some(("Ref", 15)),
            _ => None,
        },
        _ => None,
    }
}

/// Right UUIDs the platform writes on the Configuration root that carry no
/// name in `ROLE_RIGHT_NAMES`.
///
/// Measured over the ERP УХ role corpus (2026-08-24, 1,679 roles whose Rights
/// blob has a Configuration-object entry): the UUID occurs in every one of
/// them, and in every occurrence the value equals that role's own
/// `setForNewObjects` flag — which is exactly the condition under which
/// `role_rights_for_xml` never prints a Configuration-root right at all (see
/// its doc comment). It is therefore structurally invisible in every
/// observed byte of output, on any value it takes. Parsing tolerates it
/// only while the equality holds, because no observed byte proves what name
/// the platform would print if it ever didn't: a role where the value
/// diverges from the flag is refused rather than guessed.
///
/// The list used to hold a second uuid, `3762abec-…`. Документооборот КОРП
/// 3.0.21.3 writes that one *diverging* from the flag on
/// `Roles/АдминистраторСистемы`, so the platform prints it and its native
/// `Ext/Rights.xml` states the name; it now sits in `ROLE_RIGHT_NAMES` where
/// the bytes put it, and the whole role -- which this refusal used to
/// discard -- is exported again.
const CONFIGURATION_ROOT_TOLERATED_UNNAMED_RIGHT_UUIDS: [&str; 1] =
    ["4df6d046-3bf8-4dda-991c-53ba664296a5"];

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
            restrictions: Vec::new(),
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
                restrictions: Vec::new(),
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
    // Named from the platform's own bytes, not inferred: Документооборот
    // КОРП 3.0.21.3's `Roles/АдминистраторСистемы` writes this uuid `1` while
    // the role's `setForNewObjects` is false, so the platform prints it -- and
    // its native `Ext/Rights.xml` names it here, in the one position the
    // blob's order leaves for it, between `TechnicalSpecialistMode` and
    // `MainWindowModeNormal`. Every other of that object's 26 pairs maps
    // one-to-one onto a named right in the same order, so the alignment
    // admits no other reading.
    (
        "3762abec-3836-446a-83ce-3e05001bca8b",
        "CollaborationSystemInfoBaseRegistration",
    ),
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
            if action_like_category || !matches!(right.name.as_str(), "View" | "Edit") {
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
