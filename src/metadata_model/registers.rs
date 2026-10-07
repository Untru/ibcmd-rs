//! Registers and their kin: InformationRegister, AccumulationRegister,
//! AccountingRegister, CalculationRegister, Recalculation, DocumentJournal,
//! Sequence, DocumentNumerator.
//!
//! Every kind is a typed model read from the XML (`from_xml`) and written as
//! the row's brace tree (`to_brace`); the slot table of each kind is on its
//! `to_brace`. The parts they share (standard attributes, owned commands,
//! collections, references) live in `registers_parts`.
//!
//! Register rows come in two generations, chosen by the configuration's
//! compatibility mode rather than by the platform (ERP УХ in compatibility
//! 8.3.27 stores byte-identical register rows on 8.3.27 and on 8.5): up to
//! 8.3.24 the child wrappers are one version older and carry no storage
//! tail, and a standard attribute's bag has no TypeReductionMode.

use anyhow::{Result, anyhow};

#[path = "registers_export.rs"]
pub(crate) mod export;
#[path = "registers_parts.rs"]
pub mod parts;

use self::parts::{
    Generation, StandardAttributes, code, collection, command, generated, item, md_ref_list,
    owned_uuids, reference,
};
use super::attribute;
use super::brace::Brace;
use super::xml::Element;
use super::{DescriptorContext, ObjectXml, localized, md_base, parse_bool};
use crate::brace_list;

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    match object.kind {
        "InformationRegister" => InformationRegister::from_xml(object, context)?.to_brace(),
        "AccumulationRegister" => AccumulationRegister::from_xml(object, context)?.to_brace(),
        "AccountingRegister" => AccountingRegister::from_xml(object, context)?.to_brace(),
        "CalculationRegister" => CalculationRegister::from_xml(object, context)?.to_brace(),
        "Recalculation" => Recalculation::from_xml(object, context)?.to_brace(),
        "DocumentJournal" => DocumentJournal::from_xml(object, context)?.to_brace(),
        "Sequence" => Sequence::from_xml(object, context)?.to_brace(),
        "DocumentNumerator" => DocumentNumerator::from_xml(object)?.to_brace(),
        _ => Err(super::not_yet(object)),
    }
}

// ---------------------------------------------------------------------------
// Code tables.

const DATA_LOCK_CONTROL_MODE: &[(&str, i64)] = &[("Automatic", 0), ("Managed", 1)];
const FULL_TEXT_SEARCH: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1)];
const DATA_HISTORY: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1)];
const INDEXING: &[(&str, i64)] = &[
    ("DontIndex", 0),
    ("Index", 1),
    ("IndexWithAdditionalOrder", 2),
];
const TYPE_REDUCTION_MODE: &[(&str, i64)] = &[("TransformValues", 0), ("DeleteData", 1)];
const INFORMATION_REGISTER_PERIODICITY: &[(&str, i64)] = &[
    ("Nonperiodical", 0),
    ("Year", 1),
    ("Quarter", 2),
    ("Month", 3),
    ("Day", 4),
    ("Second", 5),
    ("RecorderPosition", 6),
];
const WRITE_MODE: &[(&str, i64)] = &[("Independent", 0), ("RecorderSubordinate", 1)];
const EDIT_TYPE: &[(&str, i64)] = &[("InList", 0), ("InDialog", 1), ("BothWays", 2)];
const REGISTER_TYPE: &[(&str, i64)] = &[("Balance", 0), ("Turnovers", 1)];
/// Only `Month` (2) is evidenced; the others follow the platform's enum order.
const CALCULATION_REGISTER_PERIODICITY: &[(&str, i64)] = &[
    ("Day", 0),
    ("Week", 1),
    ("Month", 2),
    ("Quarter", 3),
    ("Year", 4),
];
const MOVE_BOUNDARY_ON_POSTING: &[(&str, i64)] = &[("Move", 0), ("DontMove", 1)];
const NUMBER_TYPE: &[(&str, i64)] = &[("Number", 0), ("String", 1)];
const NUMBER_ALLOWED_LENGTH: &[(&str, i64)] = &[("Fixed", 0), ("Variable", 1)];
const NUMBER_PERIODICITY: &[(&str, i64)] = &[
    ("Nonperiodical", 0),
    ("Year", 1),
    ("Quarter", 2),
    ("Month", 3),
    ("Day", 4),
];

// Collection class ids, in the order the rows keep them (sorted).
const TEMPLATES: &str = "3daea016-69b7-4ed4-9453-127911372fe6";

const IR_RESOURCES: &str = "13134202-f60b-11d5-a3c7-0050bae0a776";
const IR_DIMENSIONS: &str = "13134203-f60b-11d5-a3c7-0050bae0a776";
const IR_FORMS: &str = "13134204-f60b-11d5-a3c7-0050bae0a776";
const IR_ATTRIBUTES: &str = "a2207540-1400-11d6-a3c7-0050bae0a776";
const IR_COMMANDS: &str = "b44ba719-945c-445c-8aab-1088fa4df16e";

const ACCUM_COMMANDS: &str = "99f328af-a77f-4572-a2d8-80ed20c81890";
const ACCUM_RESOURCES: &str = "b64d9a41-1642-11d6-a3c7-0050bae0a776";
const ACCUM_ATTRIBUTES: &str = "b64d9a42-1642-11d6-a3c7-0050bae0a776";
const ACCUM_DIMENSIONS: &str = "b64d9a43-1642-11d6-a3c7-0050bae0a776";
const ACCUM_FORMS: &str = "b64d9a44-1642-11d6-a3c7-0050bae0a776";

const ACCOUNTING_DIMENSIONS: &str = "35b63b9d-0adf-4625-a047-10ae874c19a3";
const ACCOUNTING_RESOURCES: &str = "63405499-7491-4ce3-ac72-43433cbe4112";
const ACCOUNTING_COMMANDS: &str = "7162da60-f7fe-4d78-ad5d-e31700f9af18";
const ACCOUNTING_ATTRIBUTES: &str = "9d28ee33-9c7e-4a1b-8f13-50aa9b36607b";
const ACCOUNTING_FORMS: &str = "d3b5d6eb-4ea2-4610-a3e2-624d4e815934";

const CALC_ATTRIBUTES: &str = "1b304502-2216-440b-960f-60decd04bb5d";
const CALC_RECALCULATIONS: &str = "274bf899-db0e-4df6-8ab5-67bf6371ec0b";
const CALC_RESOURCES: &str = "702b33ad-843e-41aa-8064-112cd38cc92c";
const CALC_FORMS: &str = "a2cb086c-db98-43e4-a1a9-0760ab048f8d";
const CALC_COMMANDS: &str = "acdf0f11-2d59-4e37-9945-c6721871a8fe";
const CALC_DIMENSIONS: &str = "b12fc850-8210-43c8-ae05-89567e698fbb";

const RECALCULATION_DIMENSIONS: &str = "3c456b74-4ea5-4b22-a957-e9fad9133b54";

const JOURNAL_COLUMNS: &str = "5aee69df-0513-4c6c-9815-103102471712";
const JOURNAL_COMMANDS: &str = "a49a35ce-120a-4c80-8eea-b0618479cd70";
const JOURNAL_FORMS: &str = "ec81ad10-ca07-11d5-b9a5-0050bae0a95d";

const SEQUENCE_DIMENSIONS: &str = "437488c0-35e2-11d6-a3c7-0050bae0a776";

/// Existing canonical collection classes for body-owning register families.
pub(crate) fn owned_body_classes(kind: &str) -> Option<Vec<(&'static str, &'static str)>> {
    let form = match kind {
        "InformationRegister" => IR_FORMS,
        "AccumulationRegister" => ACCUM_FORMS,
        "AccountingRegister" => ACCOUNTING_FORMS,
        "CalculationRegister" => CALC_FORMS,
        "DocumentJournal" => JOURNAL_FORMS,
        _ => return None,
    };
    Some(vec![(form, "Form"), (TEMPLATES, "Template")])
}

// ---------------------------------------------------------------------------
// Standard attribute markers by kind.

fn marker(code: i64) -> Brace {
    brace_list![Brace::num(code)]
}

fn information_register_marker(name: &str) -> Option<Brace> {
    Some(marker(match name {
        "Active" => -5,
        "LineNumber" => -4,
        "Recorder" => -3,
        "Period" => -2,
        _ => return None,
    }))
}

fn accumulation_register_marker(name: &str) -> Option<Brace> {
    Some(marker(match name {
        "RecordType" => -9,
        "Active" => -5,
        "LineNumber" => -4,
        "Recorder" => -3,
        "Period" => -2,
        _ => return None,
    }))
}

const EXT_DIMENSION: &str = "91162600-3161-4326-89a0-4a7cecd5092a";
const EXT_DIMENSION_TYPE: &str = "b3b48b29-d652-47ab-9d21-7e06768c31b5";

fn accounting_register_marker(name: &str) -> Option<Brace> {
    let code = match name {
        "PeriodAdjustment" => -30,
        "Account" => -10,
        "RecordType" => -9,
        "Active" => -5,
        "LineNumber" => -4,
        "Recorder" => -3,
        "Period" => -2,
        _ => {
            // ExtDimensionN / ExtDimensionTypeN: `{N-1,<class>}`.
            let (class, number) = match name.strip_prefix("ExtDimensionType") {
                Some(number) => (EXT_DIMENSION_TYPE, number),
                None => (EXT_DIMENSION, name.strip_prefix("ExtDimension")?),
            };
            let number: i64 = number.parse().ok()?;
            return Some(brace_list![Brace::num(number - 1), Brace::atom(class)]);
        }
    };
    Some(marker(code))
}

fn calculation_register_marker(name: &str) -> Option<Brace> {
    Some(marker(match name {
        "RegistrationPeriod" => -13,
        "ReversingEntry" => -11,
        "Active" => -10,
        "EndOfBasePeriod" => -9,
        "BegOfBasePeriod" => -8,
        "EndOfActionPeriod" => -7,
        "BegOfActionPeriod" => -6,
        "ActionPeriod" => -5,
        "CalculationType" => -4,
        "LineNumber" => -3,
        "Recorder" => -2,
        _ => return None,
    }))
}

fn document_journal_marker(name: &str) -> Option<Brace> {
    Some(marker(match name {
        "Type" => -60003,
        "Ref" => -101,
        "Date" => -100,
        "Posted" => -7,
        "DeletionMark" => -4,
        "Number" => -2,
        _ => return None,
    }))
}

// ---------------------------------------------------------------------------
// Attribute-like children: the shared body and the register wrappers.

/// One `<Dimension>`, `<Resource>`, `<Attribute>` or `<Column>` child.
struct Field<'a> {
    element: &'a Element,
    properties: &'a Element,
    uuid: String,
}

impl<'a> Field<'a> {
    fn new(element: &'a Element) -> Result<Self> {
        let properties = element
            .child("Properties")
            .ok_or_else(|| anyhow!("<{}> has no <Properties>", element.name))?;
        Ok(Self {
            element,
            properties,
            uuid: element
                .attr("uuid")
                .unwrap_or_default()
                .to_ascii_lowercase(),
        })
    }
    fn text(&self, name: &str) -> &'a str {
        self.properties.child_text(name).unwrap_or_default().trim()
    }
    fn flag(&self, name: &str) -> Result<Brace> {
        let text = self.properties.child_text(name).ok_or_else(|| {
            anyhow!(
                "{} {} has no <{name}>",
                self.element.name,
                self.text("Name")
            )
        })?;
        Ok(Brace::flag(parse_bool(text.trim())?))
    }
    fn code(&self, name: &str, table: &[(&str, i64)]) -> Result<Brace> {
        code(name, self.text(name), table)
    }
    fn reference(&self, context: &DescriptorContext, name: &str) -> Result<Brace> {
        reference(context, Some(self.text(name)))
    }
    /// The 27-slot attribute body every register wrapper carries (the
    /// simple-objects track's shared encoder).
    fn body(&self, context: &DescriptorContext) -> Result<Brace> {
        attribute::attribute_body(&self.uuid, self.properties, context)
    }
    /// `{2,<md base>,<type>}`: the head of the body, all a sequence
    /// dimension keeps.
    fn typed_header(&self, context: &DescriptorContext) -> Result<Brace> {
        attribute::typed_header(&self.uuid, self.properties, context)
    }
}

/// `0,{1,<nil>}`: the tail modern non-dimension fields carry.
fn storage_tail(items: &mut Vec<Brace>) {
    items.push(Brace::num(0));
    items.push(brace_list![Brace::num(1), Brace::nil_uuid()]);
}

fn fields<'a>(object: &ObjectXml<'a>, tag: &'a str) -> Result<Vec<Field<'a>>> {
    match object.child_objects() {
        Some(children) => children.children_named(tag).map(Field::new).collect(),
        None => Ok(Vec::new()),
    }
}

fn wrap(items: Vec<Brace>) -> Brace {
    item(Brace::List(items))
}

fn commands(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Vec<Brace>> {
    let Some(children) = object.child_objects() else {
        return Ok(Vec::new());
    };
    children
        .children_named("Command")
        .map(|element| command(element, context))
        .collect()
}

/// Property readers of a top-level object.
struct Props<'o, 'a> {
    object: &'o ObjectXml<'a>,
    properties: &'a Element,
}

impl<'o, 'a> Props<'o, 'a> {
    fn new(object: &'o ObjectXml<'a>) -> Result<Self> {
        Ok(Self {
            object,
            properties: object.properties()?,
        })
    }
    fn flag(&self, name: &str) -> Result<Brace> {
        Ok(Brace::flag(self.object.prop_bool(name)?))
    }
    fn code(&self, name: &str, table: &[(&str, i64)]) -> Result<Brace> {
        code(name, self.object.prop_text(name)?.trim(), table)
    }
    fn number(&self, name: &str) -> Result<i64> {
        self.object
            .prop_text(name)?
            .trim()
            .parse::<i64>()
            .map_err(|error| anyhow!("bad <{name}>: {error}"))
    }
    fn reference(&self, context: &DescriptorContext, name: &str) -> Result<Brace> {
        reference(context, self.properties.child_text(name))
    }
    fn localized(&self, name: &str) -> Brace {
        localized(self.properties.child(name))
    }
    fn header(&self) -> Brace {
        md_base(&self.object.uuid, self.properties)
    }
}

// ---------------------------------------------------------------------------
// InformationRegister.

struct InformationRegister {
    generated: Vec<Brace>,
    header: Brace,
    default_record_form: Brace,
    default_list_form: Brace,
    periodicity: Brace,
    write_mode: Brace,
    edit_type: Brace,
    use_standard_commands: Brace,
    include_help_in_contents: Brace,
    main_filter_on_period: Brace,
    data_lock_control_mode: Brace,
    full_text_search: Brace,
    standard_attributes: Brace,
    auxiliary_record_form: Brace,
    auxiliary_list_form: Brace,
    presentations: [Brace; 5],
    enable_totals_slice_last: Brace,
    enable_totals_slice_first: Brace,
    data_history: Brace,
    update_data_history_immediately: Brace,
    execute_after_write_data_history: Brace,
    resources: Vec<Brace>,
    dimensions: Vec<Brace>,
    forms: Vec<Brace>,
    templates: Vec<Brace>,
    attributes: Vec<Brace>,
    commands: Vec<Brace>,
}

impl InformationRegister {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let modern = Generation::of(context).is_modern();
        let props = Props::new(object)?;
        let mut resources = Vec::new();
        for field in fields(object, "Resource")? {
            let mut items = vec![
                Brace::num(if modern { 8 } else { 7 }),
                field.body(context)?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
                field.code("DataHistory", DATA_HISTORY)?,
            ];
            if modern {
                storage_tail(&mut items);
            }
            resources.push(wrap(items));
        }
        let mut attributes = Vec::new();
        for field in fields(object, "Attribute")? {
            let mut items = vec![
                Brace::num(if modern { 5 } else { 4 }),
                field.body(context)?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
                field.code("DataHistory", DATA_HISTORY)?,
            ];
            if modern {
                storage_tail(&mut items);
            }
            attributes.push(wrap(items));
        }
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            let mut items = vec![
                Brace::num(if modern { 10 } else { 9 }),
                field.body(context)?,
                field.flag("Master")?,
                field.flag("DenyIncompleteValues")?,
                field.code("Indexing", INDEXING)?,
                field.flag("MainFilter")?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
                field.code("DataHistory", DATA_HISTORY)?,
            ];
            if modern {
                items.push(field.code("TypeReductionMode", TYPE_REDUCTION_MODE)?);
            }
            dimensions.push(wrap(items));
        }
        Ok(Self {
            generated: generated(
                object,
                &[
                    "Record",
                    "Manager",
                    "Selection",
                    "List",
                    "RecordSet",
                    "RecordKey",
                    "RecordManager",
                ],
            )?,
            header: props.header(),
            default_record_form: props.reference(context, "DefaultRecordForm")?,
            default_list_form: props.reference(context, "DefaultListForm")?,
            periodicity: props.code(
                "InformationRegisterPeriodicity",
                INFORMATION_REGISTER_PERIODICITY,
            )?,
            write_mode: props.code("WriteMode", WRITE_MODE)?,
            edit_type: props.code("EditType", EDIT_TYPE)?,
            use_standard_commands: props.flag("UseStandardCommands")?,
            include_help_in_contents: props.flag("IncludeHelpInContents")?,
            main_filter_on_period: props.flag("MainFilterOnPeriod")?,
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            full_text_search: props.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            standard_attributes: StandardAttributes::from_xml(
                object,
                context,
                information_register_marker,
            )?
            .to_brace(),
            auxiliary_record_form: props.reference(context, "AuxiliaryRecordForm")?,
            auxiliary_list_form: props.reference(context, "AuxiliaryListForm")?,
            presentations: [
                props.localized("RecordPresentation"),
                props.localized("ExtendedRecordPresentation"),
                props.localized("ListPresentation"),
                props.localized("ExtendedListPresentation"),
                props.localized("Explanation"),
            ],
            enable_totals_slice_last: props.flag("EnableTotalsSliceLast")?,
            enable_totals_slice_first: props.flag("EnableTotalsSliceFirst")?,
            data_history: props.code("DataHistory", DATA_HISTORY)?,
            update_data_history_immediately: props
                .flag("UpdateDataHistoryImmediatelyAfterWrite")?,
            execute_after_write_data_history: props
                .flag("ExecuteAfterWriteDataHistoryVersionProcessing")?,
            resources,
            dimensions,
            forms: owned_uuids(object, context, "Form")?,
            templates: owned_uuids(object, context, "Template")?,
            attributes,
            commands: commands(object, context)?,
        })
    }

    /// `{1,{33,<7 generated pairs>,{0,<md base>},DefaultRecordForm,
    /// DefaultListForm,Periodicity,WriteMode,EditType,UseStandardCommands,
    /// IncludeHelpInContents,MainFilterOnPeriod,DataLockControlMode,
    /// FullTextSearch,<standard attributes>,AuxiliaryRecordForm,
    /// AuxiliaryListForm,RecordPresentation,ExtendedRecordPresentation,
    /// ListPresentation,ExtendedListPresentation,Explanation,
    /// EnableTotalsSliceLast,EnableTotalsSliceFirst,DataHistory,
    /// UpdateDataHistoryImmediatelyAfterWrite,
    /// ExecuteAfterWriteDataHistoryVersionProcessing},6,resources,
    /// dimensions,forms,templates,attributes,commands}`.
    /// Resource `{8,<body>,Indexing,FullTextSearch,DataHistory,0,{1,<nil>}}`,
    /// attribute the same with 5, dimension `{10,<body>,Master,
    /// DenyIncompleteValues,Indexing,MainFilter,FullTextSearch,DataHistory,
    /// TypeReductionMode}`; the legacy generation writes 7/4/9 without the
    /// tail and without TypeReductionMode.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(33)];
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([
            self.default_record_form,
            self.default_list_form,
            self.periodicity,
            self.write_mode,
            self.edit_type,
            self.use_standard_commands,
            self.include_help_in_contents,
            self.main_filter_on_period,
            self.data_lock_control_mode,
            self.full_text_search,
            self.standard_attributes,
            self.auxiliary_record_form,
            self.auxiliary_list_form,
        ]);
        fields.extend(self.presentations);
        fields.extend([
            self.enable_totals_slice_last,
            self.enable_totals_slice_first,
            self.data_history,
            self.update_data_history_immediately,
            self.execute_after_write_data_history,
        ]);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(6),
            collection(IR_RESOURCES, self.resources),
            collection(IR_DIMENSIONS, self.dimensions),
            collection(IR_FORMS, self.forms),
            collection(TEMPLATES, self.templates),
            collection(IR_ATTRIBUTES, self.attributes),
            collection(IR_COMMANDS, self.commands),
        ])
    }
}

// ---------------------------------------------------------------------------
// AccumulationRegister.

struct AccumulationRegister {
    generated: Vec<Brace>,
    header: Brace,
    default_list_form: Brace,
    register_type: Brace,
    use_standard_commands: Brace,
    include_help_in_contents: Brace,
    data_lock_control_mode: Brace,
    full_text_search: Brace,
    enable_totals_splitting: Brace,
    standard_attributes: Brace,
    auxiliary_list_form: Brace,
    presentations: [Brace; 3],
    templates: Vec<Brace>,
    commands: Vec<Brace>,
    resources: Vec<Brace>,
    attributes: Vec<Brace>,
    dimensions: Vec<Brace>,
    forms: Vec<Brace>,
}

impl AccumulationRegister {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let modern = Generation::of(context).is_modern();
        let props = Props::new(object)?;
        let mut resources = Vec::new();
        for field in fields(object, "Resource")? {
            resources.push(wrap(vec![
                Brace::num(5),
                field.body(context)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ]));
        }
        let mut attributes = Vec::new();
        for field in fields(object, "Attribute")? {
            let mut items = vec![
                Brace::num(if modern { 4 } else { 3 }),
                field.body(context)?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ];
            if modern {
                storage_tail(&mut items);
            }
            attributes.push(wrap(items));
        }
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            dimensions.push(wrap(vec![
                Brace::num(8),
                field.body(context)?,
                field.flag("DenyIncompleteValues")?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
                field.flag("UseInTotals")?,
            ]));
        }
        Ok(Self {
            generated: generated(
                object,
                &[
                    "Record",
                    "Manager",
                    "Selection",
                    "List",
                    "RecordSet",
                    "RecordKey",
                ],
            )?,
            header: props.header(),
            default_list_form: props.reference(context, "DefaultListForm")?,
            register_type: props.code("RegisterType", REGISTER_TYPE)?,
            use_standard_commands: props.flag("UseStandardCommands")?,
            include_help_in_contents: props.flag("IncludeHelpInContents")?,
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            full_text_search: props.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            enable_totals_splitting: props.flag("EnableTotalsSplitting")?,
            standard_attributes: StandardAttributes::from_xml(
                object,
                context,
                accumulation_register_marker,
            )?
            .to_brace(),
            auxiliary_list_form: props.reference(context, "AuxiliaryListForm")?,
            presentations: [
                props.localized("ListPresentation"),
                props.localized("ExtendedListPresentation"),
                props.localized("Explanation"),
            ],
            templates: owned_uuids(object, context, "Template")?,
            commands: commands(object, context)?,
            resources,
            attributes,
            dimensions,
            forms: owned_uuids(object, context, "Form")?,
        })
    }

    /// `{1,{28,<6 generated pairs>,{0,<md base>},DefaultListForm,
    /// RegisterType,UseStandardCommands,IncludeHelpInContents,
    /// DataLockControlMode,FullTextSearch,EnableTotalsSplitting,
    /// <standard attributes>,AuxiliaryListForm,ListPresentation,
    /// ExtendedListPresentation,Explanation},6,templates,commands,resources,
    /// attributes,dimensions,forms}`.
    /// Resource `{5,<body>,FullTextSearch}`, attribute `{4,<body>,Indexing,
    /// FullTextSearch,0,{1,<nil>}}`, dimension `{8,<body>,
    /// DenyIncompleteValues,Indexing,FullTextSearch,UseInTotals}`.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(28)];
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([
            self.default_list_form,
            self.register_type,
            self.use_standard_commands,
            self.include_help_in_contents,
            self.data_lock_control_mode,
            self.full_text_search,
            self.enable_totals_splitting,
            self.standard_attributes,
            self.auxiliary_list_form,
        ]);
        fields.extend(self.presentations);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(6),
            collection(TEMPLATES, self.templates),
            collection(ACCUM_COMMANDS, self.commands),
            collection(ACCUM_RESOURCES, self.resources),
            collection(ACCUM_ATTRIBUTES, self.attributes),
            collection(ACCUM_DIMENSIONS, self.dimensions),
            collection(ACCUM_FORMS, self.forms),
        ])
    }
}

// ---------------------------------------------------------------------------
// AccountingRegister.

struct AccountingRegister {
    generated: Vec<Brace>,
    header: Brace,
    use_standard_commands: Brace,
    include_help_in_contents: Brace,
    chart_of_accounts: Brace,
    default_list_form: Brace,
    correspondence: Brace,
    full_text_search: Brace,
    data_lock_control_mode: Brace,
    enable_totals_splitting: Brace,
    standard_attributes: Brace,
    auxiliary_list_form: Brace,
    presentations: [Brace; 3],
    period_adjustment_length: i64,
    dimensions: Vec<Brace>,
    templates: Vec<Brace>,
    resources: Vec<Brace>,
    commands: Vec<Brace>,
    attributes: Vec<Brace>,
    forms: Vec<Brace>,
}

impl AccountingRegister {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let modern = Generation::of(context).is_modern();
        let props = Props::new(object)?;
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            dimensions.push(wrap(vec![
                Brace::num(6),
                field.body(context)?,
                field.flag("Balance")?,
                field.reference(context, "AccountingFlag")?,
                field.code("Indexing", INDEXING)?,
                field.flag("DenyIncompleteValues")?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ]));
        }
        let mut resources = Vec::new();
        for field in fields(object, "Resource")? {
            resources.push(wrap(vec![
                Brace::num(2),
                field.body(context)?,
                field.flag("Balance")?,
                field.reference(context, "AccountingFlag")?,
                field.reference(context, "ExtDimensionAccountingFlag")?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ]));
        }
        let mut attributes = Vec::new();
        for field in fields(object, "Attribute")? {
            let mut items = vec![
                Brace::num(if modern { 3 } else { 2 }),
                field.body(context)?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ];
            if modern {
                storage_tail(&mut items);
            }
            attributes.push(wrap(items));
        }
        Ok(Self {
            generated: generated(
                object,
                &[
                    "Record",
                    "ExtDimensions",
                    "RecordSet",
                    "RecordKey",
                    "Selection",
                    "List",
                    "Manager",
                ],
            )?,
            header: props.header(),
            use_standard_commands: props.flag("UseStandardCommands")?,
            include_help_in_contents: props.flag("IncludeHelpInContents")?,
            chart_of_accounts: props.reference(context, "ChartOfAccounts")?,
            default_list_form: props.reference(context, "DefaultListForm")?,
            correspondence: props.flag("Correspondence")?,
            full_text_search: props.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            enable_totals_splitting: props.flag("EnableTotalsSplitting")?,
            standard_attributes: StandardAttributes::from_xml(
                object,
                context,
                accounting_register_marker,
            )?
            .to_brace(),
            auxiliary_list_form: props.reference(context, "AuxiliaryListForm")?,
            presentations: [
                props.localized("ListPresentation"),
                props.localized("ExtendedListPresentation"),
                props.localized("Explanation"),
            ],
            period_adjustment_length: props.number("PeriodAdjustmentLength")?,
            dimensions,
            templates: owned_uuids(object, context, "Template")?,
            resources,
            commands: commands(object, context)?,
            attributes,
            forms: owned_uuids(object, context, "Form")?,
        })
    }

    /// `{1,{21,<7 generated pairs>,{0,<md base>},UseStandardCommands,
    /// IncludeHelpInContents,ChartOfAccounts,DefaultListForm,Correspondence,
    /// DataLockControlMode,FullTextSearch,EnableTotalsSplitting,
    /// <standard attributes>,AuxiliaryListForm,ListPresentation,
    /// ExtendedListPresentation,Explanation,PeriodAdjustmentLength},6,
    /// dimensions,templates,resources,commands,attributes,forms}`.
    /// A register with a period adjustment writes version 22 and a second
    /// `22` before the generated types (ERP УХ `Хозрасчетный` and
    /// `КорректировкиНалоговойБазы`, the only two with a length of 1).
    /// Dimension `{6,<body>,Balance,AccountingFlag,Indexing,
    /// DenyIncompleteValues,FullTextSearch}`, resource `{2,<body>,Balance,
    /// AccountingFlag,ExtDimensionAccountingFlag,FullTextSearch}`, attribute
    /// `{3,<body>,Indexing,FullTextSearch,0,{1,<nil>}}` (legacy `{2,...}`
    /// without the tail).
    fn to_brace(self) -> Result<Brace> {
        let mut fields = if self.period_adjustment_length > 0 {
            vec![Brace::num(22), Brace::num(22)]
        } else {
            vec![Brace::num(21)]
        };
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([
            self.use_standard_commands,
            self.include_help_in_contents,
            self.chart_of_accounts,
            self.default_list_form,
            self.correspondence,
            self.data_lock_control_mode,
            self.full_text_search,
            self.enable_totals_splitting,
            self.standard_attributes,
            self.auxiliary_list_form,
        ]);
        fields.extend(self.presentations);
        fields.push(Brace::num(self.period_adjustment_length));
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(6),
            collection(ACCOUNTING_DIMENSIONS, self.dimensions),
            collection(TEMPLATES, self.templates),
            collection(ACCOUNTING_RESOURCES, self.resources),
            collection(ACCOUNTING_COMMANDS, self.commands),
            collection(ACCOUNTING_ATTRIBUTES, self.attributes),
            collection(ACCOUNTING_FORMS, self.forms),
        ])
    }
}

// ---------------------------------------------------------------------------
// CalculationRegister.

struct CalculationRegister {
    generated: Vec<Brace>,
    header: Brace,
    periodicity: Brace,
    action_period: Brace,
    base_period: Brace,
    schedule: [Brace; 3],
    chart_of_calculation_types: Brace,
    default_list_form: Brace,
    use_standard_commands: Brace,
    include_help_in_contents: Brace,
    data_lock_control_mode: Brace,
    full_text_search: Brace,
    standard_attributes: Brace,
    auxiliary_list_form: Brace,
    presentations: [Brace; 3],
    attributes: Vec<Brace>,
    recalculations: Vec<Brace>,
    templates: Vec<Brace>,
    resources: Vec<Brace>,
    forms: Vec<Brace>,
    commands: Vec<Brace>,
    dimensions: Vec<Brace>,
}

impl CalculationRegister {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let modern = Generation::of(context).is_modern();
        let props = Props::new(object)?;
        let mut attributes = Vec::new();
        for field in fields(object, "Attribute")? {
            let mut items = vec![
                Brace::num(if modern { 3 } else { 2 }),
                field.body(context)?,
                field.reference(context, "ScheduleLink")?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ];
            if modern {
                storage_tail(&mut items);
            }
            attributes.push(wrap(items));
        }
        let mut resources = Vec::new();
        for field in fields(object, "Resource")? {
            resources.push(wrap(vec![
                Brace::num(4),
                field.body(context)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ]));
        }
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            dimensions.push(wrap(vec![
                Brace::num(5),
                field.body(context)?,
                field.flag("DenyIncompleteValues")?,
                field.flag("BaseDimension")?,
                field.reference(context, "ScheduleLink")?,
                field.code("Indexing", INDEXING)?,
                field.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            ]));
        }
        Ok(Self {
            generated: generated(
                object,
                &[
                    "Record",
                    "Manager",
                    "Selection",
                    "List",
                    "RecordSet",
                    "RecordKey",
                    "Recalcs",
                ],
            )?,
            header: props.header(),
            periodicity: props.code("Periodicity", CALCULATION_REGISTER_PERIODICITY)?,
            action_period: props.flag("ActionPeriod")?,
            base_period: props.flag("BasePeriod")?,
            schedule: [
                props.reference(context, "Schedule")?,
                props.reference(context, "ScheduleValue")?,
                props.reference(context, "ScheduleDate")?,
            ],
            chart_of_calculation_types: props.reference(context, "ChartOfCalculationTypes")?,
            default_list_form: props.reference(context, "DefaultListForm")?,
            use_standard_commands: props.flag("UseStandardCommands")?,
            include_help_in_contents: props.flag("IncludeHelpInContents")?,
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            full_text_search: props.code("FullTextSearch", FULL_TEXT_SEARCH)?,
            standard_attributes: StandardAttributes::from_xml(
                object,
                context,
                calculation_register_marker,
            )?
            .to_brace(),
            auxiliary_list_form: props.reference(context, "AuxiliaryListForm")?,
            presentations: [
                props.localized("ListPresentation"),
                props.localized("ExtendedListPresentation"),
                props.localized("Explanation"),
            ],
            attributes,
            recalculations: owned_uuids(object, context, "Recalculation")?,
            templates: owned_uuids(object, context, "Template")?,
            resources,
            forms: owned_uuids(object, context, "Form")?,
            commands: commands(object, context)?,
            dimensions,
        })
    }

    /// `{1,{21,<7 generated pairs>,{0,<md base>},Periodicity,ActionPeriod,
    /// BasePeriod,Schedule,ScheduleValue,ScheduleDate,ChartOfCalculationTypes,
    /// DefaultListForm,UseStandardCommands,IncludeHelpInContents,
    /// DataLockControlMode,FullTextSearch,<standard attributes>,
    /// AuxiliaryListForm,ListPresentation,ExtendedListPresentation,
    /// Explanation},7,attributes,recalculations,templates,resources,forms,
    /// commands,dimensions}`.
    /// Attribute `{3,<body>,ScheduleLink,Indexing,FullTextSearch,0,
    /// {1,<nil>}}`, resource `{4,<body>,FullTextSearch}`, dimension
    /// `{5,<body>,DenyIncompleteValues,BaseDimension,ScheduleLink,Indexing,
    /// FullTextSearch}`.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(21)];
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([self.periodicity, self.action_period, self.base_period]);
        fields.extend(self.schedule);
        fields.extend([
            self.chart_of_calculation_types,
            self.default_list_form,
            self.use_standard_commands,
            self.include_help_in_contents,
            self.data_lock_control_mode,
            self.full_text_search,
            self.standard_attributes,
            self.auxiliary_list_form,
        ]);
        fields.extend(self.presentations);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(7),
            collection(CALC_ATTRIBUTES, self.attributes),
            collection(CALC_RECALCULATIONS, self.recalculations),
            collection(TEMPLATES, self.templates),
            collection(CALC_RESOURCES, self.resources),
            collection(CALC_FORMS, self.forms),
            collection(CALC_COMMANDS, self.commands),
            collection(CALC_DIMENSIONS, self.dimensions),
        ])
    }
}

// ---------------------------------------------------------------------------
// Recalculation.

struct Recalculation {
    generated: Vec<Brace>,
    header: Brace,
    data_lock_control_mode: Brace,
    dimensions: Vec<Brace>,
}

impl Recalculation {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let props = Props::new(object)?;
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            dimensions.push(wrap(vec![
                Brace::num(1),
                md_base(&field.uuid, field.properties),
                field.reference(context, "RegisterDimension")?,
                md_ref_list(context, field.properties.child("LeadingRegisterData"))?,
            ]));
        }
        Ok(Self {
            generated: generated(object, &["Record", "Manager", "RecordSet"])?,
            header: props.header(),
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            dimensions,
        })
    }

    /// `{1,{4,<3 generated pairs>,{0,<md base>},DataLockControlMode},1,
    /// dimensions}`; a dimension is `{1,<md base>,RegisterDimension,
    /// LeadingRegisterData}`.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(4)];
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.push(self.data_lock_control_mode);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(1),
            collection(RECALCULATION_DIMENSIONS, self.dimensions),
        ])
    }
}

// ---------------------------------------------------------------------------
// DocumentJournal.

struct DocumentJournal {
    list: Vec<Brace>,
    manager: Vec<Brace>,
    selection: Vec<Brace>,
    header: Brace,
    default_form: Brace,
    use_standard_commands: Brace,
    registered_documents: Brace,
    include_help_in_contents: Brace,
    standard_attributes: Brace,
    auxiliary_form: Brace,
    presentations: [Brace; 3],
    templates: Vec<Brace>,
    columns: Vec<Brace>,
    commands: Vec<Brace>,
    forms: Vec<Brace>,
}

impl DocumentJournal {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let props = Props::new(object)?;
        let mut columns = Vec::new();
        for field in fields(object, "Column")? {
            columns.push(wrap(vec![
                Brace::num(4),
                md_base(&field.uuid, field.properties),
                md_ref_list(context, field.properties.child("References"))?,
                field.code("Indexing", INDEXING)?,
            ]));
        }
        Ok(Self {
            list: generated(object, &["List"])?,
            manager: generated(object, &["Manager"])?,
            selection: generated(object, &["Selection"])?,
            header: props.header(),
            default_form: props.reference(context, "DefaultForm")?,
            use_standard_commands: props.flag("UseStandardCommands")?,
            registered_documents: md_ref_list(
                context,
                props.properties.child("RegisteredDocuments"),
            )?,
            include_help_in_contents: props.flag("IncludeHelpInContents")?,
            standard_attributes: StandardAttributes::from_xml(
                object,
                context,
                document_journal_marker,
            )?
            .to_brace(),
            auxiliary_form: props.reference(context, "AuxiliaryForm")?,
            presentations: [
                props.localized("ListPresentation"),
                props.localized("ExtendedListPresentation"),
                props.localized("Explanation"),
            ],
            templates: owned_uuids(object, context, "Template")?,
            columns,
            commands: commands(object, context)?,
            forms: owned_uuids(object, context, "Form")?,
        })
    }

    /// `{1,{26,<List pair>,{0,<md base>},DefaultForm,UseStandardCommands,
    /// RegisteredDocuments,IncludeHelpInContents,<Manager pair>,
    /// <Selection pair>,<standard attributes>,AuxiliaryForm,
    /// ListPresentation,ExtendedListPresentation,Explanation},4,templates,
    /// columns,commands,forms}`; a column is `{4,<md base>,References,
    /// Indexing}`.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(26)];
        fields.extend(self.list);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([
            self.default_form,
            self.use_standard_commands,
            self.registered_documents,
            self.include_help_in_contents,
        ]);
        fields.extend(self.manager);
        fields.extend(self.selection);
        fields.push(self.standard_attributes);
        fields.push(self.auxiliary_form);
        fields.extend(self.presentations);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(4),
            collection(TEMPLATES, self.templates),
            collection(JOURNAL_COLUMNS, self.columns),
            collection(JOURNAL_COMMANDS, self.commands),
            collection(JOURNAL_FORMS, self.forms),
        ])
    }
}

// ---------------------------------------------------------------------------
// Sequence.

struct Sequence {
    generated: Vec<Brace>,
    header: Brace,
    documents: Brace,
    register_records: Brace,
    move_boundary_on_posting: Brace,
    data_lock_control_mode: Brace,
    dimensions: Vec<Brace>,
}

impl Sequence {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let props = Props::new(object)?;
        let mut dimensions = Vec::new();
        for field in fields(object, "Dimension")? {
            dimensions.push(wrap(vec![
                Brace::num(0),
                field.typed_header(context)?,
                md_ref_list(context, field.properties.child("DocumentMap"))?,
                md_ref_list(context, field.properties.child("RegisterRecordsMap"))?,
            ]));
        }
        Ok(Self {
            generated: generated(object, &["Record", "Manager", "RecordSet"])?,
            header: props.header(),
            documents: md_ref_list(context, props.properties.child("Documents"))?,
            register_records: md_ref_list(context, props.properties.child("RegisterRecords"))?,
            move_boundary_on_posting: props
                .code("MoveBoundaryOnPosting", MOVE_BOUNDARY_ON_POSTING)?,
            data_lock_control_mode: props.code("DataLockControlMode", DATA_LOCK_CONTROL_MODE)?,
            dimensions,
        })
    }

    /// `{1,{6,<3 generated pairs>,{0,<md base>},Documents,RegisterRecords,
    /// MoveBoundaryOnPosting,DataLockControlMode},1,dimensions}`; a dimension
    /// is `{0,{2,<md base>,<type>},DocumentMap,RegisterRecordsMap}`.
    fn to_brace(self) -> Result<Brace> {
        let mut fields = vec![Brace::num(6)];
        fields.extend(self.generated);
        fields.push(brace_list![Brace::num(0), self.header]);
        fields.extend([
            self.documents,
            self.register_records,
            self.move_boundary_on_posting,
            self.data_lock_control_mode,
        ]);
        Ok(brace_list![
            Brace::num(1),
            Brace::List(fields),
            Brace::num(1),
            collection(SEQUENCE_DIMENSIONS, self.dimensions),
        ])
    }
}

// ---------------------------------------------------------------------------
// DocumentNumerator.

struct DocumentNumerator {
    header: Brace,
    number_type: Brace,
    number_length: i64,
    number_periodicity: Brace,
    check_unique: Brace,
    number_allowed_length: Brace,
}

impl DocumentNumerator {
    fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let props = Props::new(object)?;
        Ok(Self {
            header: props.header(),
            number_type: props.code("NumberType", NUMBER_TYPE)?,
            number_length: props.number("NumberLength")?,
            number_periodicity: props.code("NumberPeriodicity", NUMBER_PERIODICITY)?,
            check_unique: props.flag("CheckUnique")?,
            number_allowed_length: props.code("NumberAllowedLength", NUMBER_ALLOWED_LENGTH)?,
        })
    }

    /// `{1,{3,<md base>,NumberType,NumberLength,NumberPeriodicity,
    /// CheckUnique,NumberAllowedLength},0}`
    fn to_brace(self) -> Result<Brace> {
        Ok(brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(3),
                self.header,
                self.number_type,
                Brace::num(self.number_length),
                self.number_periodicity,
                self.check_unique,
                self.number_allowed_length,
            ],
            Brace::num(0),
        ])
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::parts::StandardAttributes;
    use super::*;
    use crate::metadata_model::brace::serialize;
    use crate::metadata_model::index::ConfigIndex;
    use crate::metadata_model::xml::MetadataXml;
    use crate::module_blob::MetadataSourceContext;

    const NAMESPACES: &str = r#"xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#;

    fn context(compatibility: &str) -> DescriptorContext {
        let index = ConfigIndex {
            compatibility_mode: Some(compatibility.to_string()),
            ..ConfigIndex::default()
        };
        DescriptorContext {
            root: PathBuf::new(),
            index,
            source: MetadataSourceContext::new(PathBuf::new()),
            version: "2.20".to_string(),
        }
    }

    fn object<'a>(doc: &'a MetadataXml, kind: &'a str) -> ObjectXml<'a> {
        let element = doc.object().unwrap();
        ObjectXml {
            element,
            kind,
            uuid: element.attr("uuid").unwrap().to_ascii_lowercase(),
            name: element
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default(),
            path: Path::new("test.xml"),
        }
    }

    #[test]
    fn numerator_matches_the_stored_row() {
        let xml = format!(
            r#"<MetaDataObject {NAMESPACES} version="2.20"><DocumentNumerator uuid="5c85fe94-66e5-42be-852a-9c90f6242257"><Properties><Name>СчетаФактурыВыданные</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Счета-фактуры выданные</v8:content></v8:item><v8:item><v8:lang>en</v8:lang><v8:content>Issued tax invoices</v8:content></v8:item></Synonym><Comment/><NumberType>String</NumberType><NumberLength>12</NumberLength><NumberAllowedLength>Fixed</NumberAllowedLength><NumberPeriodicity>Year</NumberPeriodicity><CheckUnique>false</CheckUnique></Properties></DocumentNumerator></MetaDataObject>"#
        );
        let doc = MetadataXml::parse(xml.as_bytes()).unwrap();
        let tree = compile(
            &object(&doc, "DocumentNumerator"),
            &context("Version8_3_27"),
        )
        .unwrap();
        assert_eq!(
            serialize(&tree),
            "{1,\r\n{3,\r\n{3,\r\n{1,0,5c85fe94-66e5-42be-852a-9c90f6242257},\"СчетаФактурыВыданные\",\r\n{2,\"ru\",\"Счета-фактуры выданные\",\"en\",\"Issued tax invoices\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},1,12,1,0,0},0}"
        );
    }

    #[test]
    fn standard_attribute_bag_follows_the_compatibility_mode() {
        let xml = format!(
            r#"<MetaDataObject {NAMESPACES} version="2.20"><InformationRegister uuid="9706f479-a13c-4fed-b046-9fa602b3a01e"><Properties><Name>R</Name><StandardAttributes><xr:StandardAttribute name="Period"><xr:LinkByType/><xr:FillChecking>ShowError</xr:FillChecking><xr:MultiLine>false</xr:MultiLine><xr:FillFromFillingValue>false</xr:FillFromFillingValue><xr:CreateOnInput>Auto</xr:CreateOnInput><xr:TypeReductionMode>TransformValues</xr:TypeReductionMode><xr:MaxValue xsi:nil="true"/><xr:ToolTip><v8:item><v8:lang>ru</v8:lang><v8:content>Дата</v8:content></v8:item></xr:ToolTip><xr:ExtendedEdit>false</xr:ExtendedEdit><xr:Format/><xr:ChoiceForm/><xr:QuickChoice>Auto</xr:QuickChoice><xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput><xr:EditFormat/><xr:PasswordMode>false</xr:PasswordMode><xr:DataHistory>Use</xr:DataHistory><xr:MarkNegatives>false</xr:MarkNegatives><xr:MinValue xsi:nil="true"/><xr:Synonym/><xr:Comment/><xr:FullTextSearch>Use</xr:FullTextSearch><xr:ChoiceParameterLinks/><xr:FillValue xsi:nil="true"/><xr:Mask/><xr:ChoiceParameters/></xr:StandardAttribute></StandardAttributes></Properties></InformationRegister></MetaDataObject>"#
        );
        let doc = MetadataXml::parse(xml.as_bytes()).unwrap();
        let object = object(&doc, "InformationRegister");
        for (mode, header, keys) in [("Version8_3_24", "13", "24"), ("Version8_3_27", "14", "25")] {
            let tree =
                StandardAttributes::from_xml(&object, &context(mode), information_register_marker)
                    .unwrap()
                    .to_brace();
            // {1,{1,1,{-2},510405d3-...,<bag>}}
            assert_eq!(tree.at(&[1, 2]), Some(&brace_list![Brace::num(-2)]));
            let bag = tree.at(&[1, 4]).unwrap();
            assert_eq!(bag.at(&[0]).and_then(Brace::as_atom), Some(header));
            assert_eq!(bag.at(&[1]).and_then(Brace::as_atom), Some(keys));
            let items = bag.as_list().unwrap();
            let tool_tip = items
                .iter()
                .position(|item| item.as_atom() == Some("4690ff70-e3fa-4914-9127-6a9acc5fc949"))
                .unwrap();
            assert_eq!(
                items[tool_tip + 1],
                brace_list![
                    Brace::str("#"),
                    Brace::atom("87024738-fc2a-4436-ada1-df79d395c424"),
                    brace_list![Brace::num(1), Brace::str("ru"), Brace::str("Дата")],
                ]
            );
        }
    }

    #[test]
    fn shortcuts_sum_the_modifiers() {
        use super::parts::shortcut;
        assert_eq!(serialize(&shortcut("Ctrl+Shift+S").unwrap()), "{0,83,12}");
        assert_eq!(serialize(&shortcut("F5").unwrap()), "{0,116,0}");
        assert_eq!(serialize(&shortcut("").unwrap()), "{0,0,0}");
        assert!(shortcut("Ctrl+Space").is_err());
    }
}
