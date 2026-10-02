//! The export direction for the register family: a stored row -> the
//! object's XML DOM, walking the layouts of `registers.rs` backwards, and
//! what the row contributes to a name index.
//!
//! Every kind reads its owner record by slot and writes `<Properties>` in
//! the order the reference trees use (the same for every file of a kind on
//! all four corpora); child collections are written in `<ChildObjects>` by
//! tag, each in stored order.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};

use super::{
    ACCOUNTING_ATTRIBUTES, ACCOUNTING_COMMANDS, ACCOUNTING_DIMENSIONS, ACCOUNTING_FORMS,
    ACCOUNTING_RESOURCES, ACCUM_ATTRIBUTES, ACCUM_COMMANDS, ACCUM_DIMENSIONS, ACCUM_FORMS,
    ACCUM_RESOURCES, CALC_ATTRIBUTES, CALC_COMMANDS, CALC_DIMENSIONS, CALC_FORMS,
    CALC_RECALCULATIONS, CALC_RESOURCES, CALCULATION_REGISTER_PERIODICITY, EXT_DIMENSION,
    EXT_DIMENSION_TYPE, IR_ATTRIBUTES, IR_COMMANDS, IR_DIMENSIONS, IR_FORMS, IR_RESOURCES,
    JOURNAL_COLUMNS, JOURNAL_COMMANDS, JOURNAL_FORMS, RECALCULATION_DIMENSIONS,
    SEQUENCE_DIMENSIONS, TEMPLATES,
};
use super::{
    DATA_HISTORY, DATA_LOCK_CONTROL_MODE, EDIT_TYPE, FULL_TEXT_SEARCH, INDEXING,
    INFORMATION_REGISTER_PERIODICITY, MOVE_BOUNDARY_ON_POSTING, NUMBER_ALLOWED_LENGTH,
    NUMBER_PERIODICITY, NUMBER_TYPE, REGISTER_TYPE, TYPE_REDUCTION_MODE, WRITE_MODE,
};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{
    Header, Owner, attribute_header, attribute_properties, bool_text, code_text, header,
    header_elements, localized_element, reference_name, standard_attribute_codes,
    standard_attribute_elements, type_element,
};
use crate::metadata_model::export::{
    Build, ExportContext, GeneratedTypeName, ObjectNames, atom, el, item, leaf, list, number, short,
};
use crate::metadata_model::objects::parts::Compat;
use crate::metadata_model::standard_pictures::standard_picture_name;
use crate::metadata_model::xml::Element;

/// Rows up to compatibility 8.3.24 keep the older child wrappers (see
/// `registers_parts::Generation`).
fn modern(compat: Compat) -> bool {
    compat >= Compat(8, 3, 25)
}

// ---------------------------------------------------------------------------
// Row access.

/// A descriptor row: `{1,<owner record>,<n>,<collection>...}`.
struct Row<'a> {
    owner: &'a [Brace],
    collections: HashMap<&'a str, &'a [Brace]>,
}

impl<'a> Row<'a> {
    fn new(row: &'a Brace) -> Result<Self> {
        let root = list(row)?;
        if atom(item(root, 0)?)? != "1" {
            bail!("not a descriptor row: {}", short(row));
        }
        let owner = list(item(root, 1)?)?;
        let mut collections = HashMap::new();
        if let Some(count) = root.get(2) {
            let count = number(count)? as usize;
            for collection in root.iter().skip(3).take(count) {
                let fields = list(collection)?;
                collections.insert(atom(item(fields, 0)?)?, fields);
            }
        }
        Ok(Self { owner, collections })
    }

    fn slot(&self, index: usize) -> Result<&'a Brace> {
        item(self.owner, index)
    }

    /// The items of a collection, in stored order.
    fn items(&self, class: &str) -> Result<&'a [Brace]> {
        let fields = self
            .collections
            .get(class)
            .ok_or_else(|| anyhow!("row has no collection {class}"))?;
        let count = number(item(fields, 1)?)? as usize;
        fields
            .get(2..2 + count)
            .ok_or_else(|| anyhow!("collection {class} is shorter than {count}"))
    }

    /// The record of a collection item `{<record>,0}`.
    fn records(&self, class: &str) -> Result<Vec<&'a [Brace]>> {
        self.items(class)?
            .iter()
            .map(|stored| list(item(list(stored)?, 0)?))
            .collect()
    }
}

/// The owner record's md header, wrapped `{0,<md base>}`.
fn wrapped_header(node: &Brace) -> Result<Header> {
    header(item(list(node)?, 1)?)
}

// ---------------------------------------------------------------------------
// Shared element builders.

fn flag(qname: &str, node: &Brace) -> Result<Element> {
    Ok(leaf(qname, bool_text(node)?))
}

fn code(qname: &str, node: &Brace, table: &[(&'static str, i64)]) -> Result<Element> {
    Ok(leaf(qname, code_text(node, table)?))
}

fn reference(qname: &str, node: &Brace, context: &ExportContext) -> Result<Element> {
    Ok(leaf(qname, reference_name(atom(node)?, &context.names)?))
}

/// `{0,<n>,{"#",157fa490-...,{1,<uuid>}}...}` -> `<qname><xr:Item ..>` in
/// stored order.
fn references(qname: &str, node: &Brace, context: &ExportContext) -> Result<Element> {
    let fields = list(node)?;
    let count = number(item(fields, 1)?)? as usize;
    let mut element = el(qname);
    for reference in fields.iter().skip(2).take(count) {
        let payload = list(item(list(reference)?, 2)?)?;
        let uuid = atom(item(payload, 1)?)?;
        element.children.push(
            leaf("xr:Item", reference_name(uuid, &context.names)?).attr("type", "xr:MDObjectRef"),
        );
    }
    Ok(element)
}

/// The XML order of a kind's generated types, with the row position of each
/// (TypeId; ValueId follows it).
fn internal_info(
    type_name: impl Fn(&str) -> String,
    record: &[Brace],
    slots: &[(&str, usize)],
) -> Result<Element> {
    let mut internal = el("InternalInfo");
    for (category, position) in slots {
        internal.children.push(
            el("xr:GeneratedType")
                .attr("name", type_name(category))
                .attr("category", *category)
                .child(leaf("xr:TypeId", atom(item(record, *position)?)?))
                .child(leaf("xr:ValueId", atom(item(record, position + 1)?)?)),
        );
    }
    Ok(internal)
}

/// `<Form>`/`<Template>`/`<Recalculation>` children: the short names of the
/// owned objects a collection lists by uuid.
fn owned(
    row: &Row<'_>,
    class: &str,
    tag: &str,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Vec<Element>> {
    let prefix = format!("{}.{tag}.", owner.full_name);
    row.items(class)?
        .iter()
        .map(|stored| {
            let uuid = atom(stored)?;
            let full = context
                .names
                .name(uuid)
                .ok_or_else(|| anyhow!("no name for {tag} {uuid}"))?;
            let name = full
                .strip_prefix(&prefix)
                .ok_or_else(|| anyhow!("{tag} {full} is not owned by {}", owner.full_name))?;
            Ok(leaf(tag, name))
        })
        .collect()
}

/// A register's `<StandardAttributes>`: `{0}` -> none. An accounting
/// register's `ExtDimensionN`/`ExtDimensionTypeN` are stored under
/// `{N-1,<class>}` markers the shared decoder does not name, so every marker
/// is mapped to a code of its own first.
fn standard_attributes(
    node: &Brace,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Option<Element>> {
    let fields = list(node)?;
    let body = match fields {
        [flag] if atom(flag)? == "0" => return Ok(None),
        [flag, body] if atom(flag)? == "1" => body,
        _ => bail!("bad standard attributes {}", short(node)),
    };
    let codes = standard_attribute_codes(owner.kind)
        .ok_or_else(|| anyhow!("no standard attributes for {}", owner.kind))?;
    let items = list(body)?;
    let count = number(item(items, 1)?)? as usize;
    let mut remapped = items.to_vec();
    let mut table: Vec<(&'static str, i64)> = codes.to_vec();
    for index in 0..count {
        let marker = list(item(items, 2 + 3 * index)?)?;
        if let [number_node, class] = marker {
            let position = number(number_node)?;
            let (names, class_code): (&[&'static str], i64) = match atom(class)? {
                EXT_DIMENSION => (&["ExtDimension1", "ExtDimension2", "ExtDimension3"], 1000),
                EXT_DIMENSION_TYPE => (
                    &[
                        "ExtDimensionType1",
                        "ExtDimensionType2",
                        "ExtDimensionType3",
                    ],
                    2000,
                ),
                other => bail!("unknown standard attribute class {other}"),
            };
            let name = names
                .get(position as usize)
                .ok_or_else(|| anyhow!("unsupported ext dimension {position}"))?;
            let code = class_code + position;
            table.push((name, code));
            remapped[2 + 3 * index] = crate::brace_list![Brace::num(code)];
        }
    }
    let block = Brace::List(remapped);
    Ok(Some(el("StandardAttributes").children(
        standard_attribute_elements(&block, &table, owner, context)?,
    )))
}

// ---------------------------------------------------------------------------
// Attribute-like children.

/// One wrapper slot and where the XML writes it.
enum Tail {
    Flag(&'static str),
    Code(&'static str, &'static [(&'static str, i64)]),
    Reference(&'static str),
    /// A property the older rows do not store: its XML default.
    Default(&'static str, &'static str),
}

/// `{<version>,<body>,<slot>...}` -> `<tag uuid=..><Properties>...`: the
/// body's properties, then the wrapper slots from position 2 on.
fn field(
    tag: &str,
    record: &[Brace],
    with_fill: bool,
    tail: &[Tail],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    let head = attribute_header(body)?;
    let mut elements = Vec::with_capacity(tail.len());
    let mut position = 2;
    for slot in tail {
        let element = match slot {
            Tail::Default(name, value) => {
                elements.push(leaf(name, *value));
                continue;
            }
            Tail::Flag(name) => flag(name, item(record, position)?)?,
            Tail::Code(name, table) => code(name, item(record, position)?, table)?,
            Tail::Reference(name) => reference(name, item(record, position)?, context)?,
        };
        elements.push(element);
        position += 1;
    }
    let properties = attribute_properties(body, with_fill, elements, owner, context)?;
    Ok(el(tag).attr("uuid", head.uuid).child(properties))
}

/// The same fields in the order the XML writes them, from the wrapper slots
/// in the order the row stores them.
fn field_in_order(
    tag: &str,
    record: &[Brace],
    with_fill: bool,
    stored: &[Tail],
    xml_order: &[&str],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    let head = attribute_header(body)?;
    let mut by_name: HashMap<&str, Element> = HashMap::new();
    let mut position = 2;
    for slot in stored {
        let (name, element) = match slot {
            Tail::Default(name, value) => {
                by_name.insert(name, leaf(name, *value));
                continue;
            }
            Tail::Flag(name) => (*name, flag(name, item(record, position)?)?),
            Tail::Code(name, table) => (*name, code(name, item(record, position)?, table)?),
            Tail::Reference(name) => (*name, reference(name, item(record, position)?, context)?),
        };
        by_name.insert(name, element);
        position += 1;
    }
    let mut elements = Vec::with_capacity(xml_order.len());
    for name in xml_order {
        elements.push(
            by_name
                .remove(name)
                .ok_or_else(|| anyhow!("{tag} wrapper has no <{name}>"))?,
        );
    }
    let properties = attribute_properties(body, with_fill, elements, owner, context)?;
    Ok(el(tag).attr("uuid", head.uuid).child(properties))
}

fn children_of(
    row: &Row<'_>,
    class: &str,
    decode: impl Fn(&[Brace]) -> Result<Element>,
) -> Result<Vec<Element>> {
    row.records(class)?
        .into_iter()
        .map(|record| decode(record))
        .collect()
}

// ---------------------------------------------------------------------------
// Commands.

fn builtin_command_group(uuid: &str) -> Option<&'static str> {
    Some(match uuid {
        "77ea1b8f-dd79-4717-9dba-5628e7f348cf" => "NavigationPanelOrdinary",
        "bc80566a-86a5-4e87-acd4-872239385a2e" => "NavigationPanelSeeAlso",
        "1af6d528-0b86-4fba-ab95-bd7475db03ba" => "NavigationPanelImportant",
        "4f499c31-050b-47c5-aa84-d0366c0a0da8" => "ActionsPanelCreate",
        "5b360bff-01a1-49b6-93d2-26e7e8e3a038" => "ActionsPanelReports",
        "aabb34e1-98c1-4bd0-bf7f-243f95437b44" => "ActionsPanelTools",
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0" => "FormCommandBarCreateBasedOn",
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b" => "FormCommandBarImportant",
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => "FormNavigationPanelGoTo",
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8" => "FormNavigationPanelSeeAlso",
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2" => "FormNavigationPanelImportant",
        _ => return None,
    })
}

/// The coded standard pictures (`registers_parts::STANDARD_PICTURE_CODES`).
fn std_picture_by_code(code: i64) -> Option<&'static str> {
    super::parts::STANDARD_PICTURE_CODES
        .iter()
        .find_map(|(name, value)| (*value == code).then_some(*name))
}

/// `{4,<present>,<value>,"",<x>,<y>,<load transparent>,0,""}` -> `<Picture>`.
fn picture(node: &Brace, context: &ExportContext) -> Result<Element> {
    let fields = list(node)?;
    if atom(item(fields, 1)?)? == "0" {
        return Ok(el("Picture"));
    }
    let value = list(item(fields, 2)?)?;
    let reference = match value {
        [code] => {
            let code = number(code)?;
            format!(
                "StdPicture.{}",
                std_picture_by_code(code).ok_or_else(|| anyhow!("unknown picture code {code}"))?
            )
        }
        [_, uuid] => {
            let uuid = atom(uuid)?;
            match standard_picture_name(uuid) {
                Some(name) => name.to_string(),
                None => context
                    .names
                    .name(uuid)
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("no name for picture {uuid}"))?,
            }
        }
        _ => bail!("unsupported picture {}", short(node)),
    };
    let mut element = el("Picture")
        .child(leaf("xr:Ref", reference))
        .child(leaf("xr:LoadTransparent", bool_text(item(fields, 6)?)?));
    let x = atom(item(fields, 4)?)?;
    let y = atom(item(fields, 5)?)?;
    if x != "-1" || y != "-1" {
        element
            .children
            .push(el("xr:TransparentPixel").attr("x", x).attr("y", y));
    }
    Ok(element)
}

/// `{0,<key>,<modifiers>}` -> `Ctrl+Alt+Shift+Key`; empty for `{0,0,0}`.
fn shortcut(node: &Brace) -> Result<String> {
    let fields = list(node)?;
    let code = number(item(fields, 1)?)?;
    let mask = number(item(fields, 2)?)?;
    if code == 0 {
        return Ok(String::new());
    }
    let key = match code {
        8 => "BackSpace".to_string(),
        13 => "Enter".to_string(),
        27 => "Esc".to_string(),
        48..=57 | 65..=90 => char::from(code as u8).to_string(),
        96..=105 => format!("Num {}", code - 96),
        106 => "Num *".to_string(),
        107 => "Num +".to_string(),
        109 => "Num -".to_string(),
        110 => "Num .".to_string(),
        111 => "Num /".to_string(),
        112..=123 => format!("F{}", code - 111),
        other => bail!("unknown shortcut key {other}"),
    };
    let mut parts = Vec::new();
    if mask & 8 != 0 {
        parts.push("Ctrl".to_string());
    }
    if mask & 16 != 0 {
        parts.push("Alt".to_string());
    }
    if mask & 4 != 0 {
        parts.push("Shift".to_string());
    }
    parts.push(key);
    Ok(parts.join("+"))
}

/// `{{0,{1,{2,<uuid>,<command class>},{9,...}}},0}` -> `<Command>`.
fn command(stored: &Brace, context: &ExportContext) -> Result<Element> {
    let names = &context.names;
    let body = item(list(item(list(stored)?, 0)?)?, 1)?;
    let fields = list(body)?;
    let identity = list(item(fields, 1)?)?;
    let uuid = atom(item(identity, 1)?)?;
    let record = list(item(fields, 2)?)?;
    if atom(item(record, 0)?)? != "9" {
        bail!("not a command record: {}", short(item(fields, 2)?));
    }
    let head = header(item(record, 9)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let group_uuid = atom(item(list(item(record, 7)?)?, 1)?)?;
    let group = match builtin_command_group(group_uuid) {
        Some(group) => group.to_string(),
        None => reference_name(group_uuid, names)?,
    };
    let properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(leaf("Group", group))
        .child(type_element(
            "CommandParameterType",
            item(record, 8)?,
            names,
        )?)
        .child(code(
            "ParameterUseMode",
            item(record, 11)?,
            &[("Single", 0), ("Multiple", 1)],
        )?)
        .child(flag("ModifiesData", item(record, 10)?)?)
        .child(code(
            "Representation",
            item(record, 2)?,
            &[
                ("Text", 0),
                ("Picture", 1),
                ("PictureAndText", 2),
                ("Auto", 3),
            ],
        )?)
        .child(localized_element("ToolTip", item(record, 3)?)?)
        .child(picture(item(record, 1)?, context)?)
        .child(leaf("Shortcut", shortcut(item(record, 5)?)?))
        .child(code(
            "OnMainServerUnavalableBehavior",
            item(record, 12)?,
            &[("Auto", 0)],
        )?);
    Ok(el("Command").attr("uuid", uuid).child(properties))
}

fn commands(row: &Row<'_>, class: &str, context: &ExportContext) -> Result<Vec<Element>> {
    row.items(class)?
        .iter()
        .map(|stored| command(stored, context))
        .collect()
}

// ---------------------------------------------------------------------------
// Kinds.

pub(crate) fn decode(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    match kind {
        "InformationRegister" => information_register(row, context),
        "AccumulationRegister" => accumulation_register(row, context),
        "AccountingRegister" => accounting_register(row, context),
        "CalculationRegister" => calculation_register(row, context),
        "Recalculation" => recalculation(row, context),
        "DocumentJournal" => document_journal(row, context),
        "Sequence" => sequence(row, context),
        "DocumentNumerator" => document_numerator(row),
        other => bail!("{other} is not a register kind"),
    }
    .with_context(|| format!("{kind} row"))
}

/// `<Kind uuid=..>` with its InternalInfo, Properties and ChildObjects.
fn object(
    kind: &str,
    uuid: &str,
    internal: Element,
    properties: Element,
    children: Vec<Element>,
) -> Element {
    el(kind)
        .attr("uuid", uuid)
        .child(internal)
        .child(properties)
        .child(el("ChildObjects").children(children))
}

fn information_register(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "InformationRegister";
    let head = wrapped_header(row.slot(15)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };
    let modern = modern(context.compat);
    let internal = internal_info(
        |category| format!("{kind}{category}.{}", head.name),
        row.owner,
        &[
            ("Record", 1),
            ("Manager", 3),
            ("Selection", 5),
            ("List", 7),
            ("RecordSet", 9),
            ("RecordKey", 11),
            ("RecordManager", 13),
        ],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(flag("UseStandardCommands", row.slot(21)?)?)
        .child(code("EditType", row.slot(20)?, EDIT_TYPE)?)
        .child(reference("DefaultRecordForm", row.slot(16)?, context)?)
        .child(reference("DefaultListForm", row.slot(17)?, context)?)
        .child(reference("AuxiliaryRecordForm", row.slot(27)?, context)?)
        .child(reference("AuxiliaryListForm", row.slot(28)?, context)?);
    if let Some(block) = standard_attributes(row.slot(26)?, owner, context)? {
        properties.children.push(block);
    }
    let properties = properties
        .child(code(
            "InformationRegisterPeriodicity",
            row.slot(18)?,
            INFORMATION_REGISTER_PERIODICITY,
        )?)
        .child(code("WriteMode", row.slot(19)?, WRITE_MODE)?)
        .child(flag("MainFilterOnPeriod", row.slot(23)?)?)
        .child(flag("IncludeHelpInContents", row.slot(22)?)?)
        .child(code(
            "DataLockControlMode",
            row.slot(24)?,
            DATA_LOCK_CONTROL_MODE,
        )?)
        .child(code("FullTextSearch", row.slot(25)?, FULL_TEXT_SEARCH)?)
        .child(flag("EnableTotalsSliceFirst", row.slot(35)?)?)
        .child(flag("EnableTotalsSliceLast", row.slot(34)?)?)
        .child(localized_element("RecordPresentation", row.slot(29)?)?)
        .child(localized_element(
            "ExtendedRecordPresentation",
            row.slot(30)?,
        )?)
        .child(localized_element("ListPresentation", row.slot(31)?)?)
        .child(localized_element(
            "ExtendedListPresentation",
            row.slot(32)?,
        )?)
        .child(localized_element("Explanation", row.slot(33)?)?)
        .child(code("DataHistory", row.slot(36)?, DATA_HISTORY)?)
        .child(flag(
            "UpdateDataHistoryImmediatelyAfterWrite",
            row.slot(37)?,
        )?)
        .child(flag(
            "ExecuteAfterWriteDataHistoryVersionProcessing",
            row.slot(38)?,
        )?);
    let plain_tail = [
        Tail::Code("Indexing", INDEXING),
        Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
        Tail::Code("DataHistory", DATA_HISTORY),
    ];
    let mut children = Vec::new();
    children.extend(children_of(&row, IR_RESOURCES, |record| {
        field("Resource", record, true, &plain_tail, owner, context)
    })?);
    children.extend(children_of(&row, IR_ATTRIBUTES, |record| {
        field("Attribute", record, true, &plain_tail, owner, context)
    })?);
    let reduction = if modern {
        Tail::Code("TypeReductionMode", TYPE_REDUCTION_MODE)
    } else {
        Tail::Default("TypeReductionMode", "TransformValues")
    };
    let dimension_slots = [
        Tail::Flag("Master"),
        Tail::Flag("DenyIncompleteValues"),
        Tail::Code("Indexing", INDEXING),
        Tail::Flag("MainFilter"),
        Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
        Tail::Code("DataHistory", DATA_HISTORY),
        reduction,
    ];
    children.extend(children_of(&row, IR_DIMENSIONS, |record| {
        field_in_order(
            "Dimension",
            record,
            true,
            &dimension_slots,
            &[
                "Master",
                "MainFilter",
                "DenyIncompleteValues",
                "Indexing",
                "FullTextSearch",
                "DataHistory",
                "TypeReductionMode",
            ],
            owner,
            context,
        )
    })?);
    children.extend(owned(&row, IR_FORMS, "Form", owner, context)?);
    children.extend(owned(&row, TEMPLATES, "Template", owner, context)?);
    children.extend(commands(&row, IR_COMMANDS, context)?);
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn accumulation_register(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "AccumulationRegister";
    let head = wrapped_header(row.slot(13)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };
    let internal = internal_info(
        |category| format!("{kind}{category}.{}", head.name),
        row.owner,
        &[
            ("Record", 1),
            ("Manager", 3),
            ("Selection", 5),
            ("List", 7),
            ("RecordSet", 9),
            ("RecordKey", 11),
        ],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(flag("UseStandardCommands", row.slot(16)?)?)
        .child(reference("DefaultListForm", row.slot(14)?, context)?)
        .child(reference("AuxiliaryListForm", row.slot(22)?, context)?)
        .child(code("RegisterType", row.slot(15)?, REGISTER_TYPE)?)
        .child(flag("IncludeHelpInContents", row.slot(17)?)?);
    if let Some(block) = standard_attributes(row.slot(21)?, owner, context)? {
        properties.children.push(block);
    }
    let properties = properties
        .child(code(
            "DataLockControlMode",
            row.slot(18)?,
            DATA_LOCK_CONTROL_MODE,
        )?)
        .child(code("FullTextSearch", row.slot(19)?, FULL_TEXT_SEARCH)?)
        .child(flag("EnableTotalsSplitting", row.slot(20)?)?)
        .child(localized_element("ListPresentation", row.slot(23)?)?)
        .child(localized_element(
            "ExtendedListPresentation",
            row.slot(24)?,
        )?)
        .child(localized_element("Explanation", row.slot(25)?)?);
    let mut children = Vec::new();
    children.extend(children_of(&row, ACCUM_RESOURCES, |record| {
        field(
            "Resource",
            record,
            false,
            &[Tail::Code("FullTextSearch", FULL_TEXT_SEARCH)],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, ACCUM_ATTRIBUTES, |record| {
        field(
            "Attribute",
            record,
            false,
            &[
                Tail::Code("Indexing", INDEXING),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, ACCUM_DIMENSIONS, |record| {
        field(
            "Dimension",
            record,
            false,
            &[
                Tail::Flag("DenyIncompleteValues"),
                Tail::Code("Indexing", INDEXING),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
                Tail::Flag("UseInTotals"),
            ],
            owner,
            context,
        )
    })?);
    children.extend(owned(&row, ACCUM_FORMS, "Form", owner, context)?);
    children.extend(owned(&row, TEMPLATES, "Template", owner, context)?);
    children.extend(commands(&row, ACCUM_COMMANDS, context)?);
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn accounting_register(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "AccountingRegister";
    // A register with a period adjustment stores version 22 and a second
    // `22` before the generated types.
    let shift = match atom(row.slot(0)?)? {
        "21" => 0,
        "22" => 1,
        other => bail!("unknown accounting register record version {other}"),
    };
    let at = |index: usize| row.slot(index + shift);
    let head = wrapped_header(at(15)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };
    let internal = internal_info(
        |category| format!("{kind}{category}.{}", head.name),
        row.owner,
        &[
            ("Record", 1 + shift),
            ("ExtDimensions", 3 + shift),
            ("RecordSet", 5 + shift),
            ("RecordKey", 7 + shift),
            ("Selection", 9 + shift),
            ("List", 11 + shift),
            ("Manager", 13 + shift),
        ],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(flag("UseStandardCommands", at(16)?)?)
        .child(flag("IncludeHelpInContents", at(17)?)?)
        .child(reference("ChartOfAccounts", at(18)?, context)?)
        .child(flag("Correspondence", at(20)?)?)
        .child(leaf("PeriodAdjustmentLength", atom(at(29)?)?))
        .child(reference("DefaultListForm", at(19)?, context)?)
        .child(reference("AuxiliaryListForm", at(25)?, context)?);
    if let Some(block) = standard_attributes(at(24)?, owner, context)? {
        properties.children.push(block);
    }
    let properties = properties
        .child(code(
            "DataLockControlMode",
            at(21)?,
            DATA_LOCK_CONTROL_MODE,
        )?)
        .child(flag("EnableTotalsSplitting", at(23)?)?)
        .child(code("FullTextSearch", at(22)?, FULL_TEXT_SEARCH)?)
        .child(localized_element("ListPresentation", at(26)?)?)
        .child(localized_element("ExtendedListPresentation", at(27)?)?)
        .child(localized_element("Explanation", at(28)?)?);
    let mut children = Vec::new();
    children.extend(children_of(&row, ACCOUNTING_DIMENSIONS, |record| {
        field_in_order(
            "Dimension",
            record,
            false,
            &[
                Tail::Flag("Balance"),
                Tail::Reference("AccountingFlag"),
                Tail::Code("Indexing", INDEXING),
                Tail::Flag("DenyIncompleteValues"),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            &[
                "Balance",
                "AccountingFlag",
                "DenyIncompleteValues",
                "Indexing",
                "FullTextSearch",
            ],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, ACCOUNTING_RESOURCES, |record| {
        field(
            "Resource",
            record,
            false,
            &[
                Tail::Flag("Balance"),
                Tail::Reference("AccountingFlag"),
                Tail::Reference("ExtDimensionAccountingFlag"),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, ACCOUNTING_ATTRIBUTES, |record| {
        field(
            "Attribute",
            record,
            false,
            &[
                Tail::Code("Indexing", INDEXING),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            owner,
            context,
        )
    })?);
    children.extend(owned(&row, ACCOUNTING_FORMS, "Form", owner, context)?);
    children.extend(owned(&row, TEMPLATES, "Template", owner, context)?);
    children.extend(commands(&row, ACCOUNTING_COMMANDS, context)?);
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn calculation_register(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "CalculationRegister";
    let head = wrapped_header(row.slot(15)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };
    let internal = internal_info(
        |category| match category {
            "Recalcs" => format!("RecalculationsManager.{}", head.name),
            _ => format!("{kind}{category}.{}", head.name),
        },
        row.owner,
        &[
            ("Record", 1),
            ("Manager", 3),
            ("Selection", 5),
            ("List", 7),
            ("RecordSet", 9),
            ("RecordKey", 11),
            ("Recalcs", 13),
        ],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(flag("UseStandardCommands", row.slot(24)?)?)
        .child(reference("DefaultListForm", row.slot(23)?, context)?)
        .child(reference("AuxiliaryListForm", row.slot(29)?, context)?)
        .child(code(
            "Periodicity",
            row.slot(16)?,
            CALCULATION_REGISTER_PERIODICITY,
        )?)
        .child(flag("ActionPeriod", row.slot(17)?)?)
        .child(flag("BasePeriod", row.slot(18)?)?)
        .child(reference("Schedule", row.slot(19)?, context)?)
        .child(reference("ScheduleValue", row.slot(20)?, context)?)
        .child(reference("ScheduleDate", row.slot(21)?, context)?)
        .child(reference(
            "ChartOfCalculationTypes",
            row.slot(22)?,
            context,
        )?)
        .child(flag("IncludeHelpInContents", row.slot(25)?)?);
    if let Some(block) = standard_attributes(row.slot(28)?, owner, context)? {
        properties.children.push(block);
    }
    let properties = properties
        .child(code(
            "DataLockControlMode",
            row.slot(26)?,
            DATA_LOCK_CONTROL_MODE,
        )?)
        .child(code("FullTextSearch", row.slot(27)?, FULL_TEXT_SEARCH)?)
        .child(localized_element("ListPresentation", row.slot(30)?)?)
        .child(localized_element(
            "ExtendedListPresentation",
            row.slot(31)?,
        )?)
        .child(localized_element("Explanation", row.slot(32)?)?);
    let mut children = Vec::new();
    children.extend(children_of(&row, CALC_RESOURCES, |record| {
        field(
            "Resource",
            record,
            false,
            &[Tail::Code("FullTextSearch", FULL_TEXT_SEARCH)],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, CALC_ATTRIBUTES, |record| {
        field(
            "Attribute",
            record,
            false,
            &[
                Tail::Reference("ScheduleLink"),
                Tail::Code("Indexing", INDEXING),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            owner,
            context,
        )
    })?);
    children.extend(children_of(&row, CALC_DIMENSIONS, |record| {
        field(
            "Dimension",
            record,
            false,
            &[
                Tail::Flag("DenyIncompleteValues"),
                Tail::Flag("BaseDimension"),
                Tail::Reference("ScheduleLink"),
                Tail::Code("Indexing", INDEXING),
                Tail::Code("FullTextSearch", FULL_TEXT_SEARCH),
            ],
            owner,
            context,
        )
    })?);
    children.extend(owned(
        &row,
        CALC_RECALCULATIONS,
        "Recalculation",
        owner,
        context,
    )?);
    children.extend(owned(&row, CALC_FORMS, "Form", owner, context)?);
    children.extend(owned(&row, TEMPLATES, "Template", owner, context)?);
    children.extend(commands(&row, CALC_COMMANDS, context)?);
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn recalculation(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let head = wrapped_header(row.slot(7)?)?;
    // `RecalculationRecord.<register>.<recalculation>`: the register's name
    // comes from the full name the index gives the recalculation.
    let full_name = context
        .names
        .name(&head.uuid)
        .ok_or_else(|| anyhow!("no name for recalculation {}", head.uuid))?
        .to_string();
    let register = full_name
        .strip_prefix("CalculationRegister.")
        .and_then(|rest| rest.split_once(".Recalculation."))
        .map(|(register, _)| register.to_string())
        .ok_or_else(|| anyhow!("{full_name} is not a recalculation"))?;
    let internal = internal_info(
        |category| format!("Recalculation{category}.{register}.{}", head.name),
        row.owner,
        &[("Record", 1), ("Manager", 3), ("RecordSet", 5)],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(code(
            "DataLockControlMode",
            row.slot(8)?,
            DATA_LOCK_CONTROL_MODE,
        )?);
    let children = children_of(&row, RECALCULATION_DIMENSIONS, |record| {
        let head = header(item(record, 1)?)?;
        let [name, synonym, comment] = header_elements(&head)?;
        Ok(el("Dimension").attr("uuid", head.uuid.clone()).child(
            el("Properties")
                .child(name)
                .child(synonym)
                .child(comment)
                .child(reference("RegisterDimension", item(record, 2)?, context)?)
                .child(references(
                    "LeadingRegisterData",
                    item(record, 3)?,
                    context,
                )?),
        ))
    })?;
    Ok(object(
        "Recalculation",
        &head.uuid,
        internal,
        properties,
        children,
    ))
}

fn document_journal(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "DocumentJournal";
    let head = wrapped_header(row.slot(3)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };
    let internal = internal_info(
        |category| format!("{kind}{category}.{}", head.name),
        row.owner,
        &[("Selection", 10), ("List", 1), ("Manager", 8)],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(reference("DefaultForm", row.slot(4)?, context)?)
        .child(reference("AuxiliaryForm", row.slot(13)?, context)?)
        .child(flag("UseStandardCommands", row.slot(5)?)?)
        .child(references("RegisteredDocuments", row.slot(6)?, context)?)
        .child(flag("IncludeHelpInContents", row.slot(7)?)?);
    if let Some(block) = standard_attributes(row.slot(12)?, owner, context)? {
        properties.children.push(block);
    }
    let properties = properties
        .child(localized_element("ListPresentation", row.slot(14)?)?)
        .child(localized_element(
            "ExtendedListPresentation",
            row.slot(15)?,
        )?)
        .child(localized_element("Explanation", row.slot(16)?)?);
    let mut children = children_of(&row, JOURNAL_COLUMNS, |record| {
        let head = header(item(record, 1)?)?;
        let [name, synonym, comment] = header_elements(&head)?;
        Ok(el("Column").attr("uuid", head.uuid.clone()).child(
            el("Properties")
                .child(name)
                .child(synonym)
                .child(comment)
                .child(code("Indexing", item(record, 3)?, INDEXING)?)
                .child(references("References", item(record, 2)?, context)?),
        ))
    })?;
    children.extend(owned(&row, JOURNAL_FORMS, "Form", owner, context)?);
    children.extend(owned(&row, TEMPLATES, "Template", owner, context)?);
    children.extend(commands(&row, JOURNAL_COMMANDS, context)?);
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn sequence(row: &Brace, context: &ExportContext) -> Result<Element> {
    let row = Row::new(row)?;
    let kind = "Sequence";
    let head = wrapped_header(row.slot(7)?)?;
    let internal = internal_info(
        |category| format!("{kind}{category}.{}", head.name),
        row.owner,
        &[("Record", 1), ("Manager", 3), ("RecordSet", 5)],
    )?;
    let [name, synonym, comment] = header_elements(&head)?;
    let properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(code(
            "MoveBoundaryOnPosting",
            row.slot(10)?,
            MOVE_BOUNDARY_ON_POSTING,
        )?)
        .child(references("Documents", row.slot(8)?, context)?)
        .child(references("RegisterRecords", row.slot(9)?, context)?)
        .child(code(
            "DataLockControlMode",
            row.slot(11)?,
            DATA_LOCK_CONTROL_MODE,
        )?);
    let children = children_of(&row, SEQUENCE_DIMENSIONS, |record| {
        let typed = list(item(record, 1)?)?;
        let head = header(item(typed, 1)?)?;
        let [name, synonym, comment] = header_elements(&head)?;
        Ok(el("Dimension").attr("uuid", head.uuid.clone()).child(
            el("Properties")
                .child(name)
                .child(synonym)
                .child(comment)
                .child(type_element("Type", item(typed, 2)?, &context.names)?)
                .child(references("DocumentMap", item(record, 2)?, context)?)
                .child(references("RegisterRecordsMap", item(record, 3)?, context)?),
        ))
    })?;
    Ok(object(kind, &head.uuid, internal, properties, children))
}

fn document_numerator(row: &Brace) -> Result<Element> {
    let root = list(row)?;
    let record = list(item(root, 1)?)?;
    let head = header(item(record, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(code("NumberType", item(record, 2)?, NUMBER_TYPE)?)
        .child(leaf("NumberLength", atom(item(record, 3)?)?))
        .child(code(
            "NumberAllowedLength",
            item(record, 6)?,
            NUMBER_ALLOWED_LENGTH,
        )?)
        .child(code(
            "NumberPeriodicity",
            item(record, 4)?,
            NUMBER_PERIODICITY,
        )?)
        .child(flag("CheckUnique", item(record, 5)?)?);
    Ok(el("DocumentNumerator")
        .attr("uuid", head.uuid)
        .child(properties))
}

// ---------------------------------------------------------------------------
// Names.

/// What a recalculation's row contributes to a name index, under its
/// register's full name `owner` (`CalculationRegister.X`): the recalculation,
/// its dimensions and its generated types
/// (`Recalculation<category>.<register>.<recalculation>`).
pub(crate) fn owned_names(kind: &str, row: &Brace, owner: &str) -> Result<ObjectNames> {
    if kind != "Recalculation" {
        bail!("{kind} is not an owned register kind");
    }
    let register = owner
        .strip_prefix("CalculationRegister.")
        .filter(|name| !name.contains('.'))
        .ok_or_else(|| anyhow!("a recalculation's owner is a calculation register, not {owner}"))?;
    let parsed = Row::new(row)?;
    let head = wrapped_header(parsed.slot(7)?)?;
    let full_name = format!("{owner}.Recalculation.{}", head.name);
    let mut out = ObjectNames {
        uuid: head.uuid.clone(),
        full_name: full_name.clone(),
        children: Vec::new(),
        types: Vec::new(),
    };
    for (category, position) in [("Record", 1), ("Manager", 3), ("RecordSet", 5)] {
        out.types.push(GeneratedTypeName {
            name: format!("Recalculation{category}.{register}.{}", head.name),
            category: category.to_string(),
            type_id: atom(parsed.slot(position)?)?.to_string(),
            value_id: atom(parsed.slot(position + 1)?)?.to_string(),
        });
    }
    for record in parsed.records(RECALCULATION_DIMENSIONS)? {
        let dimension = header(item(record, 1)?)?;
        out.children.push((
            format!("{full_name}.Dimension.{}", dimension.name),
            dimension.uuid,
        ));
    }
    Ok(out)
}

/// What a register-family row contributes to a name index: the object, its
/// dimensions, resources, attributes, columns and commands, its generated
/// types. A recalculation's full name and type names start with its
/// register's name, which its own row does not hold: the register's row
/// lists it only by uuid.
pub(crate) fn names(kind: &str, row: &Brace) -> Result<ObjectNames> {
    if kind == "DocumentNumerator" {
        let head = header(item(list(item(list(row)?, 1)?)?, 1)?)?;
        return Ok(ObjectNames {
            full_name: format!("{kind}.{}", head.name),
            uuid: head.uuid,
            children: Vec::new(),
            types: Vec::new(),
        });
    }
    if kind == "Recalculation" {
        bail!("a recalculation's names start with its register's, which its row does not hold");
    }
    let parsed = Row::new(row)?;
    let shift = match kind {
        "AccountingRegister" if atom(parsed.slot(0)?)? == "22" => 1,
        _ => 0,
    };
    let (header_slot, generated, children): (usize, &[(&str, usize)], &[(&str, &str)]) = match kind
    {
        "InformationRegister" => (
            15,
            &[
                ("Record", 1),
                ("Manager", 3),
                ("Selection", 5),
                ("List", 7),
                ("RecordSet", 9),
                ("RecordKey", 11),
                ("RecordManager", 13),
            ],
            &[
                (IR_RESOURCES, "Resource"),
                (IR_ATTRIBUTES, "Attribute"),
                (IR_DIMENSIONS, "Dimension"),
                (IR_COMMANDS, "Command"),
            ],
        ),
        "AccumulationRegister" => (
            13,
            &[
                ("Record", 1),
                ("Manager", 3),
                ("Selection", 5),
                ("List", 7),
                ("RecordSet", 9),
                ("RecordKey", 11),
            ],
            &[
                (ACCUM_RESOURCES, "Resource"),
                (ACCUM_ATTRIBUTES, "Attribute"),
                (ACCUM_DIMENSIONS, "Dimension"),
                (ACCUM_COMMANDS, "Command"),
            ],
        ),
        "AccountingRegister" => (
            15,
            &[
                ("Record", 1),
                ("ExtDimensions", 3),
                ("RecordSet", 5),
                ("RecordKey", 7),
                ("Selection", 9),
                ("List", 11),
                ("Manager", 13),
            ],
            &[
                (ACCOUNTING_DIMENSIONS, "Dimension"),
                (ACCOUNTING_RESOURCES, "Resource"),
                (ACCOUNTING_ATTRIBUTES, "Attribute"),
                (ACCOUNTING_COMMANDS, "Command"),
            ],
        ),
        "CalculationRegister" => (
            15,
            &[
                ("Record", 1),
                ("Manager", 3),
                ("Selection", 5),
                ("List", 7),
                ("RecordSet", 9),
                ("RecordKey", 11),
                ("Recalcs", 13),
            ],
            &[
                (CALC_RESOURCES, "Resource"),
                (CALC_ATTRIBUTES, "Attribute"),
                (CALC_DIMENSIONS, "Dimension"),
                (CALC_COMMANDS, "Command"),
            ],
        ),
        "DocumentJournal" => (
            3,
            &[("Selection", 10), ("List", 1), ("Manager", 8)],
            &[(JOURNAL_COLUMNS, "Column"), (JOURNAL_COMMANDS, "Command")],
        ),
        "Sequence" => (
            7,
            &[("Record", 1), ("Manager", 3), ("RecordSet", 5)],
            &[(SEQUENCE_DIMENSIONS, "Dimension")],
        ),
        other => bail!("{other} is not a register kind"),
    };
    let head = wrapped_header(parsed.slot(header_slot + shift)?)?;
    let full_name = format!("{kind}.{}", head.name);
    let mut out = ObjectNames {
        uuid: head.uuid.clone(),
        full_name: full_name.clone(),
        children: Vec::new(),
        types: Vec::new(),
    };
    for (category, position) in generated {
        let position = position + shift;
        out.types.push(GeneratedTypeName {
            name: match (kind, *category) {
                ("CalculationRegister", "Recalcs") => {
                    format!("RecalculationsManager.{}", head.name)
                }
                _ => format!("{kind}{category}.{}", head.name),
            },
            category: category.to_string(),
            type_id: atom(parsed.slot(position)?)?.to_string(),
            value_id: atom(parsed.slot(position + 1)?)?.to_string(),
        });
    }
    for (class, tag) in children {
        for stored in parsed.items(class)? {
            let child = match *tag {
                "Command" => {
                    let body = item(list(item(list(stored)?, 0)?)?, 1)?;
                    header(item(list(item(list(body)?, 2)?)?, 9)?)?
                }
                "Column" => header(item(list(item(list(stored)?, 0)?)?, 1)?)?,
                _ if kind == "Sequence" => {
                    let record = list(item(list(stored)?, 0)?)?;
                    header(item(list(item(record, 1)?)?, 1)?)?
                }
                _ => attribute_header(item(list(item(list(stored)?, 0)?)?, 1)?)?,
            };
            out.children
                .push((format!("{full_name}.{tag}.{}", child.name), child.uuid));
        }
    }
    Ok(out)
}
