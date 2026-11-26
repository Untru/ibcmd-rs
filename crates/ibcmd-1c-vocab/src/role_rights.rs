//! The role-right vocabulary of a stored Role rights row: the right names a
//! `Rights.xml` writes and the uuids the row stores them by, the families a
//! nested reference's `{slot, family}` pair carries, and the standard
//! attributes and standard tabular sections a slot stands for, with the
//! inverse lookups the base-free writer places a name with.
//!
//! It moved here verbatim, but for visibility, from the root's
//! `mssql_dump::role_rights` (the exporter's decoder, which reads it from
//! here) so that the base-free writer (`compiler::bodies::role_rights_writer`)
//! and `module_blob` read it without the exporter.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Family uuid a nested reference's slot carries for an ordinary standard
/// attribute, and for a standard tabular section.
///
/// Both are platform-fixed: every `{slot, family}` pair observed in an ERP УХ
/// 3.2.12.6 role Rights blob uses `STANDARD_ATTRIBUTE_FAMILY` except the
/// chart-of-accounts `ExtDimensionTypes` section, which uses
/// `STANDARD_TABULAR_SECTION_FAMILY`.
const STANDARD_ATTRIBUTE_FAMILY: &str = "03f171e8-326f-41c6-9fa5-932a0b12cddf";
const STANDARD_TABULAR_SECTION_FAMILY: &str = "28db313d-dbc2-4b83-8c4a-d2aeee708062";
pub const ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY: &str = STANDARD_ATTRIBUTE_FAMILY;
pub const ROLE_RIGHTS_STANDARD_TABULAR_SECTION_FAMILY: &str = STANDARD_TABULAR_SECTION_FAMILY;
pub const ROLE_RIGHTS_EXT_DIMENSION_FAMILY: &str = EXT_DIMENSION_FAMILY;
pub const ROLE_RIGHTS_EXT_DIMENSION_TYPE_FAMILY: &str = EXT_DIMENSION_TYPE_FAMILY;

/// The two families an accounting register's ext-dimension pair is split
/// across: `{n, EXT_DIMENSION_FAMILY}` is `ExtDimension<n+1>`,
/// `{n, EXT_DIMENSION_TYPE_FAMILY}` is `ExtDimensionType<n+1>`.
const EXT_DIMENSION_FAMILY: &str = "91162600-3161-4326-89a0-4a7cecd5092a";
const EXT_DIMENSION_TYPE_FAMILY: &str = "b3b48b29-d652-47ab-9d21-7e06768c31b5";

/// Every slot a braced `{slot, family}` reference is observed to carry for a
/// standard attribute or a standard tabular section, and a margin around
/// them: the inverse lookups below search this range and refuse a name that
/// two slots would claim.
fn role_rights_slot_candidates() -> impl Iterator<Item = isize> {
    (-120..=-1).rev().chain(std::iter::once(-60003))
}

/// The braced slot and print order a standard attribute of `kind` is written
/// with -- the inverse of `role_standard_attribute_descriptor` over the
/// negative-slot form, which is the only form a role Rights blob uses (see
/// `role_rights_object_intra_uuid_order`). `None` when no slot, or more than
/// one, carries the name.
pub fn role_standard_attribute_slot(kind: &str, name: &str) -> Option<(isize, usize)> {
    unique_slot(role_rights_slot_candidates(), |slot| {
        role_standard_attribute_descriptor(kind, slot, None)
            .filter(|(candidate, _)| *candidate == name)
            .map(|(_, order)| order)
    })
}

/// The inverse of `role_standard_tabular_section`.
pub fn role_standard_tabular_section_slot(kind: &str, name: &str) -> Option<(isize, usize)> {
    unique_slot(role_rights_slot_candidates(), |slot| {
        role_standard_tabular_section(kind, slot)
            .filter(|(candidate, _)| *candidate == name)
            .map(|(_, order)| order)
    })
}

/// The inverse of `role_standard_tabular_section_attribute`.
pub fn role_standard_tabular_section_attribute_slot(
    kind: &str,
    name: &str,
) -> Option<(isize, usize)> {
    unique_slot(role_rights_slot_candidates(), |slot| {
        role_standard_tabular_section_attribute(kind, slot)
            .filter(|(candidate, _)| *candidate == name)
            .map(|(_, position)| position)
    })
}

fn unique_slot(
    candidates: impl Iterator<Item = isize>,
    matches: impl Fn(isize) -> Option<usize>,
) -> Option<(isize, usize)> {
    let mut found = None;
    for slot in candidates {
        if let Some(order) = matches(slot) {
            if found.is_some() {
                return None;
            }
            found = Some((slot, order));
        }
    }
    found
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
pub fn role_standard_tabular_section(kind: &str, slot: isize) -> Option<(&'static str, usize)> {
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
pub fn role_standard_tabular_section_attribute(
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

pub fn role_standard_attribute_descriptor(
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

pub fn role_right_name(uuid: &str) -> Option<&'static str> {
    ibcmd_schema::configuration_rights::termination_right_name(uuid)
        .or_else(|| ROLE_RIGHT_NAMES_BY_UUID.get(uuid).copied())
}

/// The right uuid a `Rights.xml` right name stands for: the same table read
/// the other way. Every name in it is unique, so the inverse is exact.
pub fn role_right_uuid(name: &str) -> Option<&'static str> {
    ibcmd_schema::configuration_rights::termination_right_uuid(name)
        .or_else(|| ROLE_RIGHT_UUIDS_BY_NAME.get(name).copied())
}

static ROLE_RIGHT_NAMES_BY_UUID: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| ROLE_RIGHT_NAMES.iter().copied().collect());

static ROLE_RIGHT_UUIDS_BY_NAME: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        ROLE_RIGHT_NAMES
            .iter()
            .map(|(uuid, name)| (*name, *uuid))
            .collect()
    });

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

#[cfg(test)]
mod tests {
    use super::*;

    /// `role_right_uuid` is `role_right_name` read the other way: every uuid
    /// and every name of the table is unique, and each answers the other.
    #[test]
    fn right_names_and_uuids_answer_each_other() {
        let uuids = ROLE_RIGHT_NAMES
            .iter()
            .map(|(uuid, _)| *uuid)
            .collect::<std::collections::BTreeSet<_>>();
        let names = ROLE_RIGHT_NAMES
            .iter()
            .map(|(_, name)| *name)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(uuids.len(), ROLE_RIGHT_NAMES.len());
        assert_eq!(names.len(), ROLE_RIGHT_NAMES.len());
        for (uuid, name) in ROLE_RIGHT_NAMES {
            assert_eq!(role_right_name(uuid), Some(*name), "{uuid}");
            assert_eq!(role_right_uuid(name), Some(*uuid), "{name}");
        }
        assert_eq!(
            role_right_name("00000000-0000-0000-0000-000000000000"),
            None
        );
        assert_eq!(role_right_uuid("NoSuchRight"), None);
    }

    /// The inverse lookups place a name at the slot the forward tables give
    /// it, and refuse a name no slot carries.
    #[test]
    fn slot_lookups_invert_the_slot_tables() {
        assert_eq!(
            role_standard_attribute_slot("Catalog", "Owner"),
            Some((-5, 6))
        );
        assert_eq!(
            role_standard_attribute_descriptor("Catalog", -5, None),
            Some(("Owner", 6))
        );
        assert_eq!(
            role_standard_attribute_slot("ChartOfAccounts", "OffBalance"),
            Some((-11, 8))
        );
        assert_eq!(
            role_standard_attribute_slot("Catalog", "NoSuchAttribute"),
            None
        );
        assert_eq!(
            role_standard_tabular_section_slot("ChartOfAccounts", "ExtDimensionTypes"),
            Some((-12, 3))
        );
        assert_eq!(
            role_standard_tabular_section_slot("ChartOfCalculationTypes", "BaseCalculationTypes"),
            Some((-10, 10))
        );
        assert_eq!(
            role_standard_tabular_section_attribute_slot("ChartOfCalculationTypes", "LineNumber"),
            Some((-100, 3))
        );
        assert_eq!(
            role_standard_tabular_section_attribute_slot("ChartOfAccounts", "LineNumber"),
            Some((-12, 4))
        );
        assert_eq!(
            role_standard_tabular_section_slot("Catalog", "ExtDimensionTypes"),
            None
        );
    }
}
