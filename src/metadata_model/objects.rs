//! Reference objects: Catalog, Document, ExchangePlan, the three charts,
//! BusinessProcess, Task, Report, DataProcessor, Enum.
//!
//! Every kind is `{1,<owner record>,<n>,<collection>...}`: the owner record
//! is a flat list of slots (generated types, the md header, properties) and
//! each collection is `{<class uuid>,<count>,<item>...}`, the collections in
//! uuid order. A kind is a [`Layout`]: its slots named by the XML property
//! each carries, and its collections -- the model a row is compiled from and,
//! read the other way, decoded into. The parts every kind composes live in
//! `objects_parts.rs`: standard attributes, commands, tabular sections,
//! attribute wrappers, characteristics, field and metadata references.
//!
//! Record versions follow the configuration's compatibility mode, not the
//! platform: 8.3.24 stores the older versions; 8.3.27 adds
//! `LineNumberLength` to tabular sections, `TypeReductionMode` to standard
//! attributes and a constant tail to attributes; 8.5.1 adds
//! `AuxiliaryVariantForm` to reports and colours to enum values.

#[path = "objects_export.rs"]
pub(crate) mod export;
#[path = "objects_parts.rs"]
pub(crate) mod parts;

use anyhow::{Result, anyhow};

use self::parts::Slot::*;
use self::parts::{
    AttributeWrapper, Codes, Coll, CommandWrapper, Layout, Obj, StandardSections, TsWrapper,
    Versioned, restore_crlf,
};
use super::brace::Brace;
use super::{DescriptorContext, ObjectXml};

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let layout =
        layout(object.kind).ok_or_else(|| anyhow!("{} is not a reference object", object.kind))?;
    let mut tree = Obj::new(object, context)?.compile(layout)?;
    restore_crlf(&mut tree);
    Ok(tree)
}

/// The layout of a reference kind.
pub(crate) fn layout(kind: &str) -> Option<&'static Layout> {
    Some(match kind {
        "Catalog" => &CATALOG,
        "Document" => &DOCUMENT,
        "ExchangePlan" => &EXCHANGE_PLAN,
        "ChartOfCharacteristicTypes" => &CHART_OF_CHARACTERISTIC_TYPES,
        "ChartOfAccounts" => &CHART_OF_ACCOUNTS,
        "ChartOfCalculationTypes" => &CHART_OF_CALCULATION_TYPES,
        "BusinessProcess" => &BUSINESS_PROCESS,
        "Task" => &TASK,
        "Report" => &REPORT,
        "DataProcessor" => &DATA_PROCESSOR,
        "Enum" => &ENUM,
        _ => return None,
    })
}

/// Body-owning collection identities from the existing typed layouts.
/// This reader does not alter encoding or admit a new dynamic body family.
pub(crate) fn owned_body_classes(kind: &str) -> Option<Vec<(&'static str, &'static str)>> {
    if let Some(layout) = layout(kind) {
        return Some(
            layout
                .collections
                .iter()
                .filter_map(|(class, role)| match role {
                    Coll::Forms => Some((*class, "Form")),
                    Coll::Templates => Some((*class, "Template")),
                    _ => None,
                })
                .collect(),
        );
    }
    super::registers::owned_body_classes(kind).or_else(|| super::simple::owned_body_classes(kind))
}

// Collection class uuids (the platform's, not the configuration's).
pub(crate) const TEMPLATES: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
pub(crate) const TS_ATTRIBUTES: &str = "888744e1-b616-11d4-9436-004095e12fc7";

// Enumerations as the rows store them.
pub(crate) const EDIT_TYPE: Codes = &[("InList", 0), ("InDialog", 1), ("BothWays", 2)];
pub(crate) const CHOICE_MODE: Codes = &[("FromForm", 0), ("QuickChoice", 1), ("BothWays", 2)];
pub(crate) const CODE_TYPE: Codes = &[("Number", 0), ("String", 1)];
pub(crate) const DEFAULT_PRESENTATION: Codes = &[("AsCode", 0), ("AsDescription", 1)];
pub(crate) const DATA_LOCK_CONTROL_MODE: Codes = &[("Automatic", 0), ("Managed", 1)];
pub(crate) const USE: Codes = &[("DontUse", 0), ("Use", 1)];
pub(crate) const ALLOWED_LENGTH: Codes = &[("Fixed", 0), ("Variable", 1)];
pub(crate) const CREATE_ON_INPUT: Codes = &[("Auto", 0), ("DontUse", 1), ("Use", 2)];
pub(crate) const PREDEFINED_DATA_UPDATE: Codes =
    &[("Auto", 0), ("AutoUpdate", 1), ("DontAutoUpdate", 2)];
pub(crate) const CHOICE_HISTORY_ON_INPUT: Codes = &[("Auto", 0), ("DontUse", 1)];
pub(crate) const HIERARCHY_TYPE: Codes =
    &[("HierarchyFoldersAndItems", 0), ("HierarchyOfItems", 1)];
pub(crate) const SUBORDINATION_USE: Codes =
    &[("ToItems", 0), ("ToFolders", 1), ("ToFoldersAndItems", 2)];
pub(crate) const CATALOG_CODE_SERIES: Codes = &[
    ("WholeCatalog", 0),
    ("WithinSubordination", 1),
    ("WithinOwnerSubordination", 2),
];
pub(crate) const CCT_CODE_SERIES: Codes =
    &[("WholeCharacteristicKind", 0), ("WithinSubordination", 1)];
pub(crate) const COA_CODE_SERIES: Codes =
    &[("WholeChartOfAccounts", 0), ("WithinSubordination", 1)];
pub(crate) const POSTING: Codes = &[("Allow", 0), ("Deny", 1)];
pub(crate) const REGISTER_RECORDS_DELETION: Codes = &[
    ("AutoDelete", 0),
    ("AutoDeleteOff", 1),
    ("AutoDeleteOnUnpost", 2),
];
pub(crate) const REGISTER_RECORDS_WRITING: Codes = &[("WriteSelected", 0), ("WriteModified", 1)];
pub(crate) const SEQUENCE_FILLING: Codes = &[("AutoFill", 0), ("AutoFillOff", 1)];
pub(crate) const NUMBER_PERIODICITY: Codes = &[
    ("Nonperiodical", 0),
    ("Year", 1),
    ("Quarter", 2),
    ("Month", 3),
    ("Day", 4),
];
/// A business process spells its number type and periodicity its own way.
pub(crate) const BP_NUMBER_TYPE: Codes = &[("String", 0), ("Number", 1)];
pub(crate) const BP_NUMBER_PERIODICITY: Codes = &[
    ("Year", 0),
    ("Nonperiodical", 1),
    ("Quarter", 2),
    ("Month", 3),
    ("Day", 4),
];
pub(crate) const TASK_NUMBER_AUTO_PREFIX: Codes = &[("BusinessProcessNumber", 0), ("DontUse", 1)];
pub(crate) const DEPENDENCE_ON_CALCULATION_TYPES: Codes =
    &[("DontUse", 0), ("OnActionPeriod", 1), ("OnBasePeriod", 2)];

// Standard-attribute markers per family, root level.
pub(crate) const CATALOG_STANDARD: Codes = &[
    ("PredefinedDataName", -13),
    ("Predefined", -10),
    ("Ref", -8),
    ("DeletionMark", -7),
    ("IsFolder", -6),
    ("Owner", -5),
    ("Parent", -4),
    ("Description", -3),
    ("Code", -2),
];
pub(crate) const DOCUMENT_STANDARD: Codes = &[
    ("Posted", -7),
    ("Ref", -5),
    ("DeletionMark", -4),
    ("Date", -3),
    ("Number", -2),
];
pub(crate) const EXCHANGE_PLAN_STANDARD: Codes = &[
    ("ExchangeDate", -14),
    ("ThisNode", -13),
    ("ReceivedNo", -10),
    ("SentNo", -9),
    ("Ref", -6),
    ("DeletionMark", -4),
    ("Description", -3),
    ("Code", -2),
];
pub(crate) const CCT_STANDARD: Codes = &[
    ("PredefinedDataName", -14),
    ("ValueType", -11),
    ("Description", -9),
    ("Code", -8),
    ("IsFolder", -7),
    ("Parent", -6),
    ("Predefined", -5),
    ("DeletionMark", -4),
    ("Ref", -2),
];
pub(crate) const COA_STANDARD: Codes = &[
    ("PredefinedDataName", -28),
    ("Order", -17),
    ("OffBalance", -11),
    ("Type", -10),
    ("Description", -8),
    ("Code", -7),
    ("Parent", -6),
    ("Predefined", -5),
    ("DeletionMark", -4),
    ("Ref", -2),
];
pub(crate) const CCALC_STANDARD: Codes = &[
    ("PredefinedDataName", -11),
    ("Predefined", -8),
    ("Ref", -6),
    ("DeletionMark", -5),
    ("ActionPeriodIsBasic", -4),
    ("Description", -3),
    ("Code", -2),
];
pub(crate) const BUSINESS_PROCESS_STANDARD: Codes = &[
    ("Started", -9),
    ("HeadTask", -8),
    ("Completed", -7),
    ("Ref", -5),
    ("DeletionMark", -4),
    ("Date", -3),
    ("Number", -2),
];
pub(crate) const TASK_STANDARD: Codes = &[
    ("Executed", -10),
    ("Description", -9),
    ("RoutePoint", -8),
    ("BusinessProcess", -7),
    ("Ref", -5),
    ("DeletionMark", -4),
    ("Date", -3),
    ("Number", -2),
];
pub(crate) const ENUM_STANDARD: Codes = &[("Order", -3), ("Ref", -2)];
pub(crate) const INFORMATION_REGISTER_STANDARD: Codes = &[
    ("Active", -5),
    ("LineNumber", -4),
    ("Recorder", -3),
    ("Period", -2),
];

/// The charts' predefined tabular sections: name, marker, attribute markers.
const COA_TABULAR: StandardSections = &[(
    "ExtDimensionTypes",
    -12,
    &[
        ("TurnoversOnly", -15),
        ("Predefined", -14),
        ("ExtDimensionType", -13),
        ("LineNumber", -12),
    ],
)];
const CALCULATION_TYPE_ROWS: Codes = &[
    ("Predefined", -102),
    ("CalculationType", -101),
    ("LineNumber", -100),
];
const CCALC_TABULAR: StandardSections = &[
    ("LeadingCalculationTypes", -30, CALCULATION_TYPE_ROWS),
    ("DisplacingCalculationTypes", -20, CALCULATION_TYPE_ROWS),
    ("BaseCalculationTypes", -10, CALCULATION_TYPE_ROWS),
];

/// The root-level standard attribute markers of a family, by kind name.
pub(crate) fn standard_markers(kind: &str) -> Option<Codes> {
    Some(match kind {
        "Catalog" => CATALOG_STANDARD,
        "Document" => DOCUMENT_STANDARD,
        "ExchangePlan" => EXCHANGE_PLAN_STANDARD,
        "ChartOfCharacteristicTypes" => CCT_STANDARD,
        "ChartOfAccounts" => COA_STANDARD,
        "ChartOfCalculationTypes" => CCALC_STANDARD,
        "BusinessProcess" => BUSINESS_PROCESS_STANDARD,
        "Task" => TASK_STANDARD,
        "Enum" => ENUM_STANDARD,
        "InformationRegister" => INFORMATION_REGISTER_STANDARD,
        _ => return None,
    })
}

/// The `LineNumber` marker of a family's tabular sections.
pub(crate) fn line_number_marker(kind: &str) -> i64 {
    match kind {
        "Report" | "DataProcessor" => -3,
        "ChartOfCalculationTypes" => -100,
        _ => -10,
    }
}

const fn versioned<T: Copy>(old: T, modern: T) -> Versioned<T> {
    Versioned { old, modern }
}

/// Tabular sections of the reference families (catalog-like attributes).
const fn sections(old: TsWrapper, modern: TsWrapper) -> Coll {
    Coll::TabularSections {
        wrapper: versioned(old, modern),
        attribute_class: TS_ATTRIBUTES,
        attribute: AttributeWrapper::TabularSection,
    }
}

const fn attributes(old: AttributeWrapper, modern: AttributeWrapper) -> Coll {
    Coll::Children("Attribute", versioned(old, modern))
}

/// Catalog, tag 56 (compatibility 8.3.24) or 57, 61 slots.
static CATALOG: Layout = Layout {
    slots: &[
        Tag(56, 57, 57),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        WrappedHeader,
        Number("LevelCount"),
        Code("EditType", EDIT_TYPE),
        References("Owners"),
        Flag("FoldersOnTop"),
        Flag("CheckUnique"),
        Flag("Autonumbering"),
        Code("CodeSeries", CATALOG_CODE_SERIES),
        Number("CodeLength"),
        Code("CodeType", CODE_TYPE),
        Number("DescriptionLength"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Reference("DefaultObjectForm"),
        Reference("DefaultFolderForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Reference("DefaultFolderChoiceForm"),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryFolderForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Reference("AuxiliaryFolderChoiceForm"),
        Flag("UseStandardCommands"),
        References("BasedOn"),
        Flag("IncludeHelpInContents"),
        Generated("Manager"),
        Code("HierarchyType", HIERARCHY_TYPE),
        Flag("Hierarchical"),
        Flag("LimitLevelCount"),
        Code("SubordinationUse", SUBORDINATION_USE),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        Fields("InputByString"),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(CATALOG_STANDARD),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("CodeAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        Code("PredefinedDataUpdate", PREDEFINED_DATA_UPDATE),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (TEMPLATES, Coll::Templates),
        (
            "4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6",
            Coll::Commands(CommandWrapper::Owner),
        ),
        (
            "932159f9-95b2-4e76-a8dd-8849fe5c5ded",
            sections(TsWrapper::Use(1), TsWrapper::UseAndLength(2)),
        ),
        (
            "cf4abea7-37b2-11d4-940f-008048da11f9",
            attributes(
                AttributeWrapper::Hierarchical(5),
                AttributeWrapper::HierarchicalModern(6),
            ),
        ),
        ("fdf816d2-1ead-11d5-b975-0050bae0a95d", Coll::Forms),
    ],
};

/// Document, tag 40, 53 slots.
static DOCUMENT: Layout = Layout {
    slots: &[
        Tag(40, 40, 40),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        WrappedHeader,
        Reference("Numerator"),
        Code("NumberType", CODE_TYPE),
        Number("NumberLength"),
        Code("NumberPeriodicity", NUMBER_PERIODICITY),
        Flag("CheckUnique"),
        Flag("Autonumbering"),
        Reference("DefaultObjectForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("Posting", POSTING),
        Code("RegisterRecordsDeletion", REGISTER_RECORDS_DELETION),
        Code("RealTimePosting", POSTING),
        References("BasedOn"),
        Flag("UseStandardCommands"),
        References("RegisterRecords"),
        Flag("IncludeHelpInContents"),
        Generated("Manager"),
        Code("SequenceFilling", SEQUENCE_FILLING),
        Fields("InputByString"),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(DOCUMENT_STANDARD),
        Flag("PostInPrivilegedMode"),
        Flag("UnpostInPrivilegedMode"),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("RegisterRecordsWritingOnPost", REGISTER_RECORDS_WRITING),
        Code("NumberAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (
            "21c53e09-8950-4b5e-a6a0-1054f1bbc274",
            sections(TsWrapper::Bare(1), TsWrapper::Length(2)),
        ),
        (TEMPLATES, Coll::Templates),
        (
            "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
            attributes(AttributeWrapper::Plain(5), AttributeWrapper::PlainModern(6)),
        ),
        (
            "b544fc6a-2ba3-4885-8fb2-cb289fb6d65e",
            Coll::Commands(CommandWrapper::Owner),
        ),
        ("fb880e93-47d7-4127-9357-a20e69c17545", Coll::Forms),
    ],
};

/// ExchangePlan, tag 36 (50 slots) or 37 (51 slots).
static EXCHANGE_PLAN: Layout = Layout {
    slots: &[
        Tag(36, 37, 37),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Manager"),
        ThisNode,
        Header,
        Flag("UseStandardCommands"),
        Reference("DefaultObjectForm"),
        Number("CodeLength"),
        Const(0),
        Number("DescriptionLength"),
        Flag("IncludeHelpInContents"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("EditType", EDIT_TYPE),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        References("BasedOn"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Flag("DistributedInfoBase"),
        Fields("InputByString"),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(EXCHANGE_PLAN_STANDARD),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("CodeAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Flag("IncludeConfigurationExtensions"),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
        Const(0),
        Modern(&Const(1)),
    ],
    collections: &[
        (
            "1a1b4fea-e093-470d-94ff-1d2f16cda2ab",
            attributes(AttributeWrapper::Plain(3), AttributeWrapper::PlainModern(4)),
        ),
        (TEMPLATES, Coll::Templates),
        (
            "52293f4b-f98c-43ea-a80f-41047ae7ab58",
            sections(TsWrapper::Bare(0), TsWrapper::Length(1)),
        ),
        ("87c509ab-3d38-4d67-b379-aca796298578", Coll::Forms),
        (
            "d5207c64-11d5-4d46-bba2-55b7b07ff4eb",
            Coll::Commands(CommandWrapper::Owner),
        ),
    ],
};

/// ChartOfCharacteristicTypes, tag 34, 59 slots.
static CHART_OF_CHARACTERISTIC_TYPES: Layout = Layout {
    slots: &[
        Tag(34, 34, 34),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Characteristic"),
        Generated("Manager"),
        WrappedHeader,
        Flag("UseStandardCommands"),
        References("BasedOn"),
        Flag("IncludeHelpInContents"),
        Reference("CharacteristicExtValues"),
        TypePattern("Type"),
        Flag("Hierarchical"),
        Flag("FoldersOnTop"),
        Number("CodeLength"),
        Flag("Autonumbering"),
        Number("DescriptionLength"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Code("EditType", EDIT_TYPE),
        Reference("DefaultObjectForm"),
        Reference("DefaultFolderForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Reference("DefaultFolderChoiceForm"),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        Fields("InputByString"),
        Flag("CheckUnique"),
        Code("CodeSeries", CCT_CODE_SERIES),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(CCT_STANDARD),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryFolderForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Reference("AuxiliaryFolderChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("CodeAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        Code("PredefinedDataUpdate", PREDEFINED_DATA_UPDATE),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (
            "31182525-9346-4595-81f8-6f91a72ebe06",
            attributes(
                AttributeWrapper::Hierarchical(2),
                AttributeWrapper::HierarchicalModern(3),
            ),
        ),
        (TEMPLATES, Coll::Templates),
        (
            "54e36536-7863-42fd-bea3-c5edd3122fdc",
            sections(TsWrapper::Use(0), TsWrapper::UseAndLength(1)),
        ),
        (
            "95b5e1d4-abfa-4a16-818d-a5b07b7d3f73",
            Coll::Commands(CommandWrapper::Owner),
        ),
        ("eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94", Coll::Forms),
    ],
};

/// ChartOfAccounts, tag 32, 57 slots, seven collections.
static CHART_OF_ACCOUNTS: Layout = Layout {
    slots: &[
        Tag(32, 32, 32),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Manager"),
        Generated("ExtDimensionTypes"),
        Generated("ExtDimensionTypesRow"),
        WrappedHeader,
        Flag("UseStandardCommands"),
        Flag("IncludeHelpInContents"),
        References("BasedOn"),
        Reference("ExtDimensionTypes"),
        Number("MaxExtDimensionCount"),
        Text("CodeMask"),
        Number("CodeLength"),
        Number("DescriptionLength"),
        // The chart of an extension separates the next slot from EditType
        // (AutoOrderByCode false, EditType InDialog): slot 24 is the flag and
        // slot 27 the edit type, `1` (InDialog) on every chart on record.
        Flag("AutoOrderByCode"),
        Number("OrderLength"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Code("EditType", EDIT_TYPE),
        Reference("DefaultObjectForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        Fields("InputByString"),
        Flag("CheckUnique"),
        Code("CodeSeries", COA_CODE_SERIES),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(COA_STANDARD),
        StandardTabularSections(COA_TABULAR),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        Code("PredefinedDataUpdate", PREDEFINED_DATA_UPDATE),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (
            "0df30176-6865-4787-9fc8-609eb144174f",
            Coll::Commands(CommandWrapper::Owner),
        ),
        (TEMPLATES, Coll::Templates),
        ("4c7fec95-d1bd-4508-8a01-f1db090d9af8", Coll::Empty),
        ("5372e285-03db-4f8c-8565-fe56f1aea40e", Coll::Forms),
        (
            "6e65cbf5-daa8-4d8d-bef8-59723f4e5777",
            attributes(AttributeWrapper::Plain(2), AttributeWrapper::PlainModern(3)),
        ),
        (
            "78bd1243-c4df-46c3-8138-e147465cb9a4",
            Coll::Children(
                "AccountingFlag",
                versioned(AttributeWrapper::Flag(6), AttributeWrapper::Flag(6)),
            ),
        ),
        (
            "c70ca527-5042-4cad-a315-dcb4007e32a3",
            Coll::Children(
                "ExtDimensionAccountingFlag",
                versioned(AttributeWrapper::Flag(6), AttributeWrapper::Flag(6)),
            ),
        ),
    ],
};

/// ChartOfCalculationTypes, tag 35, 63 slots.
static CHART_OF_CALCULATION_TYPES: Layout = Layout {
    slots: &[
        Tag(35, 35, 35),
        WrappedHeader,
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Manager"),
        Generated("DisplacingCalculationTypes"),
        Generated("DisplacingCalculationTypesRow"),
        Generated("BaseCalculationTypes"),
        Generated("BaseCalculationTypesRow"),
        Generated("LeadingCalculationTypes"),
        Generated("LeadingCalculationTypesRow"),
        Flag("UseStandardCommands"),
        Number("CodeLength"),
        Code("CodeType", CODE_TYPE),
        // The chart of an extension separates slot 27 from EditType
        // (DependenceOnCalculationTypes DontUse, EditType InDialog): the
        // dependence rides slot 27 and the edit type slot 35, `1` (InDialog)
        // on every chart on record.
        Code(
            "DependenceOnCalculationTypes",
            DEPENDENCE_ON_CALCULATION_TYPES,
        ),
        References("BaseCalculationTypes"),
        Flag("ActionPeriodUse"),
        Number("DescriptionLength"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Reference("DefaultObjectForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("EditType", EDIT_TYPE),
        References("BasedOn"),
        Flag("IncludeHelpInContents"),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        Fields("InputByString"),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Code("FullTextSearch", USE),
        StandardAttributes(CCALC_STANDARD),
        StandardTabularSections(CCALC_TABULAR),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("CodeAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        Code("PredefinedDataUpdate", PREDEFINED_DATA_UPDATE),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (
            "054aa8cf-faa6-4634-aef4-1087ca0d88fc",
            sections(TsWrapper::Bare(0), TsWrapper::Length(1)),
        ),
        (
            "0dc22ad2-476a-4794-afae-cfa7ed251752",
            attributes(AttributeWrapper::Plain(3), AttributeWrapper::PlainModern(4)),
        ),
        (
            "2e90c75b-2f0c-4899-a7d4-5426eaefc96e",
            Coll::Commands(CommandWrapper::Owner),
        ),
        (TEMPLATES, Coll::Templates),
        ("a7f8f92a-7a4b-484b-937e-42d242e64144", Coll::Forms),
    ],
};

/// BusinessProcess, tag 30, 49 slots.
static BUSINESS_PROCESS: Layout = Layout {
    slots: &[
        Tag(30, 30, 30),
        Header,
        Flag("UseStandardCommands"),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Manager"),
        Generated("RoutePointRef"),
        References("BasedOn"),
        Code("EditType", EDIT_TYPE),
        Code("NumberType", BP_NUMBER_TYPE),
        Number("NumberLength"),
        Code("CreateOnInput", CREATE_ON_INPUT),
        Flag("CheckUnique"),
        Flag("Autonumbering"),
        Reference("DefaultObjectForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Reference("Task"),
        Flag("IncludeHelpInContents"),
        Fields("InputByString"),
        Code("NumberAllowedLength", ALLOWED_LENGTH),
        Flag("CreateTaskInPrivilegedMode"),
        StandardAttributes(BUSINESS_PROCESS_STANDARD),
        Code("NumberPeriodicity", BP_NUMBER_PERIODICITY),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        Characteristics,
        Code("FullTextSearch", USE),
        Fields("DataLockFields"),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (TEMPLATES, Coll::Templates),
        ("3f7a8120-b71a-4265-98bf-4d9bc09b7719", Coll::Forms),
        (
            "7a3e533c-f232-40d5-a932-6a311d2480bf",
            Coll::Commands(CommandWrapper::Owner),
        ),
        (
            "87c988de-ecbf-413b-87b0-b9516df05e28",
            attributes(AttributeWrapper::Plain(2), AttributeWrapper::PlainModern(3)),
        ),
        (
            "a3fe6537-d787-40f7-8a06-419d2f0c1cfd",
            sections(TsWrapper::Bare(0), TsWrapper::Length(1)),
        ),
    ],
};

/// Task, tag 33, 52 slots, six collections. Slots 13 and 14 are nil in
/// every row but one (BSP 8.5 `ЗадачаИсполнителя`), and no XML carries them.
static TASK: Layout = Layout {
    slots: &[
        Tag(33, 33, 33),
        Header,
        Flag("UseStandardCommands"),
        Generated("Object"),
        Generated("Ref"),
        Generated("Selection"),
        Generated("List"),
        Generated("Manager"),
        Nil,
        Nil,
        Reference("DefaultObjectForm"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("NumberType", CODE_TYPE),
        Number("NumberLength"),
        Code("EditType", EDIT_TYPE),
        Flag("CheckUnique"),
        Number("DescriptionLength"),
        Flag("Autonumbering"),
        Flag("IncludeHelpInContents"),
        Reference("Addressing"),
        Reference("MainAddressingAttribute"),
        Code("DefaultPresentation", DEFAULT_PRESENTATION),
        Fields("InputByString"),
        Reference("CurrentPerformer"),
        References("BasedOn"),
        Code("TaskNumberAutoPrefix", TASK_NUMBER_AUTO_PREFIX),
        Code("FullTextSearch", USE),
        Code("DataLockControlMode", DATA_LOCK_CONTROL_MODE),
        StandardAttributes(TASK_STANDARD),
        Reference("AuxiliaryObjectForm"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ObjectPresentation"),
        Localized("ExtendedObjectPresentation"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        Code("NumberAllowedLength", ALLOWED_LENGTH),
        Characteristics,
        Code("CreateOnInput", CREATE_ON_INPUT),
        Fields("DataLockFields"),
        InputModes,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
        Code("DataHistory", USE),
        Flag("UpdateDataHistoryImmediatelyAfterWrite"),
        Flag("ExecuteAfterWriteDataHistoryVersionProcessing"),
    ],
    collections: &[
        (TEMPLATES, Coll::Templates),
        ("3f58cbfb-4172-4e54-be49-561a579bb38b", Coll::Forms),
        (
            "8ddfb495-c5fc-46b9-bdc5-bcf58341bff0",
            attributes(AttributeWrapper::Plain(2), AttributeWrapper::PlainModern(3)),
        ),
        (
            "e97c0570-251c-4566-b0f1-10686820f143",
            Coll::Children(
                "AddressingAttribute",
                versioned(
                    AttributeWrapper::Addressing(4),
                    AttributeWrapper::Addressing(4),
                ),
            ),
        ),
        (
            "ee865d4b-a458-48a0-b38f-5a26898feeb0",
            sections(TsWrapper::Bare(0), TsWrapper::Length(1)),
        ),
        (
            "f27c2152-a2c9-4c30-adb1-130f5eb2590f",
            Coll::Commands(CommandWrapper::Owner),
        ),
    ],
};

/// Report, tag 19 (18 slots) or, from compatibility 8.5.1, 20 (19 slots).
static REPORT: Layout = Layout {
    slots: &[
        Tag(19, 19, 20),
        Generated("Object"),
        WrappedHeader,
        Reference("DefaultForm"),
        Reference("MainDataCompositionSchema"),
        Reference("DefaultSettingsForm"),
        Flag("UseStandardCommands"),
        Reference("VariantsStorage"),
        Reference("SettingsStorage"),
        Reference("DefaultVariantForm"),
        Flag("IncludeHelpInContents"),
        Generated("Manager"),
        Reference("AuxiliaryForm"),
        Localized("ExtendedPresentation"),
        Localized("Explanation"),
        Reference("AuxiliarySettingsForm"),
        Since8_5_1(&Reference("AuxiliaryVariantForm")),
    ],
    collections: &[
        (TEMPLATES, Coll::Templates),
        (
            "7e7123e0-29e2-11d6-a3c7-0050bae0a776",
            attributes(AttributeWrapper::Bare, AttributeWrapper::Bare),
        ),
        ("a3b368c0-29e2-11d6-a3c7-0050bae0a776", Coll::Forms),
        (
            "b077d780-29e2-11d6-a3c7-0050bae0a776",
            Coll::TabularSections {
                wrapper: versioned(TsWrapper::Bare(0), TsWrapper::Bare(0)),
                attribute_class: "c339c860-29e2-11d6-a3c7-0050bae0a776",
                attribute: AttributeWrapper::Bare,
            },
        ),
        (
            "e7ff38c0-ec3c-47a0-ae90-20c73ca72246",
            Coll::Commands(CommandWrapper::Bare),
        ),
    ],
};

/// DataProcessor, tag 17, 12 slots.
static DATA_PROCESSOR: Layout = Layout {
    slots: &[
        Tag(17, 17, 17),
        Generated("Object"),
        WrappedHeader,
        Reference("DefaultForm"),
        Flag("UseStandardCommands"),
        Flag("IncludeHelpInContents"),
        Generated("Manager"),
        Reference("AuxiliaryForm"),
        Localized("ExtendedPresentation"),
        Localized("Explanation"),
    ],
    collections: &[
        (
            "2bcef0d1-0981-11d6-b9b8-0050bae0a95d",
            Coll::TabularSections {
                wrapper: versioned(TsWrapper::Bare(0), TsWrapper::Bare(0)),
                attribute_class: "5d24a9d1-098e-11d6-b9b8-0050bae0a95d",
                attribute: AttributeWrapper::Bare,
            },
        ),
        (TEMPLATES, Coll::Templates),
        (
            "45556acb-826a-4f73-898a-6025fc9536e1",
            Coll::Commands(CommandWrapper::Bare),
        ),
        ("d5b0e5ed-256d-401c-9c36-f630cafd8a62", Coll::Forms),
        (
            "ec6bb5e5-b7a8-4d75-bec9-658107a699cf",
            attributes(AttributeWrapper::Bare, AttributeWrapper::Bare),
        ),
    ],
};

/// Enum, tag 20, 21 slots, four collections.
static ENUM: Layout = Layout {
    slots: &[
        Tag(20, 20, 20),
        Generated("Ref"),
        Generated("Manager"),
        WrappedHeader,
        Flag("UseStandardCommands"),
        Generated("List"),
        Reference("DefaultListForm"),
        Reference("DefaultChoiceForm"),
        Code("ChoiceMode", CHOICE_MODE),
        Flag("QuickChoice"),
        Reference("AuxiliaryListForm"),
        Reference("AuxiliaryChoiceForm"),
        Localized("ListPresentation"),
        Localized("ExtendedListPresentation"),
        Localized("Explanation"),
        StandardAttributes(ENUM_STANDARD),
        Characteristics,
        Code("ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT),
    ],
    collections: &[
        ("33f2e54b-37ce-4a7a-a569-b648d7aa4634", Coll::Forms),
        (TEMPLATES, Coll::Templates),
        (
            "6d8d73a7-ba29-401d-9032-3872ec2d6433",
            Coll::Commands(CommandWrapper::Owner),
        ),
        ("bee0a08c-07eb-40c0-8544-5c364c171465", Coll::EnumValues),
    ],
};

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::{DescriptorContext, compile_descriptor};

    const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#;

    /// An enum compiles to its stored row with no base: owner record,
    /// generated types, the four collections, the value records; the
    /// 8.5.1 compatibility adds the value colour.
    #[test]
    fn compiles_an_enum_without_a_base_row() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-objects-enum-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("Enums")).unwrap();
        let enum_xml = format!(
            r#"{HEAD}
<Enum uuid="0D2189F1-306C-4236-836A-1A4BFC0CFF13">
<InternalInfo>
<xr:GeneratedType name="EnumRef.E" category="Ref"><xr:TypeId>e4253022-3bfb-430b-bd60-e507622d28c9</xr:TypeId><xr:ValueId>2b90a755-c5f0-4075-b543-5eaf9c0f9b54</xr:ValueId></xr:GeneratedType>
<xr:GeneratedType name="EnumManager.E" category="Manager"><xr:TypeId>6a120ac5-26bf-4c49-b7e5-37917a0c787e</xr:TypeId><xr:ValueId>e8eb2b7a-fbde-45c9-9e8a-1a2a0a0832bf</xr:ValueId></xr:GeneratedType>
<xr:GeneratedType name="EnumList.E" category="List"><xr:TypeId>3a7ddce0-9165-4ff2-ab59-f91506e04201</xr:TypeId><xr:ValueId>6d76ebe3-a08e-4c5e-ad5d-c9f739008a98</xr:ValueId></xr:GeneratedType>
</InternalInfo>
<Properties><Name>E</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Два
строки</v8:content></v8:item></Synonym><Comment/>
<UseStandardCommands>false</UseStandardCommands><Characteristics/><QuickChoice>true</QuickChoice>
<ChoiceMode>BothWays</ChoiceMode><DefaultListForm/><DefaultChoiceForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/>
<ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput></Properties>
<ChildObjects><EnumValue uuid="a03d3535-090a-46f5-a411-3567c8c12bc1"><Properties><Name>V</Name><Synonym/><Comment/><Color>pal:Red</Color></Properties></EnumValue></ChildObjects>
</Enum></MetaDataObject>"#
        );
        let path = root.join("Enums").join("E.xml");
        fs::write(&path, &enum_xml).unwrap();
        let compile = |mode: &str| {
            fs::write(
                root.join("Configuration.xml"),
                format!(
                    "{HEAD}<Configuration uuid=\"11111111-1111-1111-1111-111111111111\"><Properties><Name>C</Name><CompatibilityMode>{mode}</CompatibilityMode></Properties></Configuration></MetaDataObject>"
                ),
            )
            .unwrap();
            let context = DescriptorContext::new(&root, "2.20").unwrap();
            let row = compile_descriptor("Enum", &path, enum_xml.as_bytes(), &context).unwrap();
            String::from_utf8(row).unwrap()
        };
        let row = compile("Version8_3_24");
        let nil = "00000000-0000-0000-0000-000000000000";
        let expected = format!(
            "\u{feff}{{1,{{20,e4253022-3bfb-430b-bd60-e507622d28c9,2b90a755-c5f0-4075-b543-5eaf9c0f9b54,\
             6a120ac5-26bf-4c49-b7e5-37917a0c787e,e8eb2b7a-fbde-45c9-9e8a-1a2a0a0832bf,\
             {{0,{{3,{{1,0,0d2189f1-306c-4236-836a-1a4bfc0cff13}},\"E\",{{1,\"ru\",\"Два\r\nстроки\"}},\"\",0,0,{nil},0}}}},\
             0,3a7ddce0-9165-4ff2-ab59-f91506e04201,6d76ebe3-a08e-4c5e-ad5d-c9f739008a98,{nil},{nil},2,1,{nil},{nil},\
             {{0}},{{0}},{{0}},{{0}},{{0,{{0}}}},0}},4,\
             {{33f2e54b-37ce-4a7a-a569-b648d7aa4634,0}},{{3daea016-69b7-4ed4-9453-127911372fe6,0}},\
             {{6d8d73a7-ba29-401d-9032-3872ec2d6433,0}},\
             {{bee0a08c-07eb-40c0-8544-5c364c171465,1,{{{{0,{{3,{{1,0,a03d3535-090a-46f5-a411-3567c8c12bc1}},\"V\",{{0}},\"\",0,0,{nil},0}}}},0}}}}}}"
        );
        // Layout breaks aside (they follow the one serializer rule), the
        // row is the stored one.
        assert_eq!(strip_layout(&row), expected);
        // The 8.5.1 compatibility spells the value colour.
        let row = compile("Version8_5_1");
        assert!(row.contains("{1,\r\n{3,\r\n{1,0,a03d3535-090a-46f5-a411-3567c8c12bc1}"));
        assert!(strip_layout(&row).contains(&format!("{nil},0}},{{4,4,{{2}},5}}}},0}}")));
        fs::remove_dir_all(&root).ok();
    }

    /// Drops the CRLF the serializer puts before nested lists and closing
    /// braces, keeping CRLF inside strings.
    fn strip_layout(row: &str) -> String {
        let mut out = String::with_capacity(row.len());
        let mut in_string = false;
        let mut chars = row.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '"' {
                in_string = !in_string;
            }
            if !in_string && ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
                continue;
            }
            out.push(ch);
        }
        out
    }
}
