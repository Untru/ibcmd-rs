//! Parts the register family shares: layout generation, generated types,
//! references and collections, standard attributes, owned commands.

use anyhow::{Context, Result, anyhow};

use crate::metadata_model::attribute::{choice_parameter_links, choice_parameters, link_by_type};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::types::{type_pattern, typed_value};
use crate::metadata_model::xml::Element;
use crate::metadata_model::{
    DescriptorContext, ObjectXml, localized, md_base, native_text, parse_bool,
};
use ibcmd_restructure::brace_list;

/// Which layout generation the rows follow. It is the configuration's
/// compatibility mode that decides, not the platform: БСП in 8.3.24
/// compatibility writes the legacy rows on 8.3.27, ERP УХ in 8.3.27
/// compatibility writes the same modern rows on 8.3.27 and 8.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    /// Compatibility 8.3.24 and older.
    Legacy,
    /// Compatibility 8.3.25 and newer (8.3.27 and 8.5.1 are measured), or
    /// none.
    Modern,
}

impl Generation {
    pub fn of(context: &DescriptorContext) -> Self {
        match context.index.compatibility_version() {
            Some(version) if version < (8, 3, 25) => Generation::Legacy,
            _ => Generation::Modern,
        }
    }
    pub fn is_modern(self) -> bool {
        self == Generation::Modern
    }
}

/// A code from a `(name, code)` table.
pub fn code(name: &str, value: &str, table: &[(&str, i64)]) -> Result<Brace> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, code)| Brace::num(*code))
        .ok_or_else(|| anyhow!("unsupported <{name}> value {value:?}"))
}

/// `{<class id>,N,item...}`
pub fn collection(class: &str, items: Vec<Brace>) -> Brace {
    let mut list = Vec::with_capacity(items.len() + 2);
    list.push(Brace::atom(class));
    list.push(Brace::num(items.len() as i64));
    list.extend(items);
    Brace::List(list)
}

/// A collection item: `{<object>,0}`.
pub fn item(object: Brace) -> Brace {
    brace_list![object, Brace::num(0)]
}

/// TypeId/ValueId pairs of the object's generated types, in the order given.
pub fn generated(object: &ObjectXml<'_>, categories: &[&str]) -> Result<Vec<Brace>> {
    let info = object
        .element
        .child("InternalInfo")
        .ok_or_else(|| anyhow!("{} {} has no <InternalInfo>", object.kind, object.name))?;
    let mut out = Vec::with_capacity(categories.len() * 2);
    for category in categories {
        let generated = info
            .children_named("GeneratedType")
            .find(|generated| generated.attr("category") == Some(category))
            .ok_or_else(|| {
                anyhow!(
                    "{} {} has no generated type {category}",
                    object.kind,
                    object.name
                )
            })?;
        for id in ["TypeId", "ValueId"] {
            out.push(Brace::uuid(
                generated.child_text(id).unwrap_or_default().trim(),
            ));
        }
    }
    Ok(out)
}

/// The uuid a full metadata name resolves to; the nil uuid for an empty or
/// absent reference.
pub fn reference(context: &DescriptorContext, text: Option<&str>) -> Result<Brace> {
    let text = text.unwrap_or_default().trim();
    if text.is_empty() {
        return Ok(Brace::nil_uuid());
    }
    context
        .index
        .uuid_of(text)
        .map(Brace::uuid)
        .ok_or_else(|| anyhow!("unresolved reference {text}"))
}

/// The object's full name (`InformationRegister.X`,
/// `CalculationRegister.R.Recalculation.X`).
pub fn full_name(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<String> {
    context
        .index
        .objects_by_uuid
        .get(&object.uuid)
        .cloned()
        .ok_or_else(|| anyhow!("{} {} is not in the index", object.kind, object.name))
}

/// Uuids of the object's own `<Form>`, `<Template>`, `<Recalculation>`
/// children (listed by name), in document order.
pub fn owned_uuids(
    object: &ObjectXml<'_>,
    context: &DescriptorContext,
    tag: &str,
) -> Result<Vec<Brace>> {
    let Some(children) = object.child_objects() else {
        return Ok(Vec::new());
    };
    let owner = full_name(object, context)?;
    children
        .children_named(tag)
        .map(|child| {
            let name = format!("{owner}.{tag}.{}", child.text.trim());
            context
                .index
                .uuid_of(&name)
                .map(Brace::uuid)
                .ok_or_else(|| anyhow!("unresolved {name}"))
        })
        .collect()
}

const METADATA_REF: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

/// `{"#",157fa490-...,{1,<uuid>}}`: a metadata object reference value.
pub fn md_ref(uuid: Brace) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::atom(METADATA_REF),
        brace_list![Brace::num(1), uuid],
    ]
}

/// `{0,N,<md ref>...}` from a list of `<xr:Item>` full names.
pub fn md_ref_list(context: &DescriptorContext, items: Option<&Element>) -> Result<Brace> {
    let mut list = vec![Brace::num(0)];
    let mut refs = Vec::new();
    if let Some(items) = items {
        for item in items.children_named("Item") {
            refs.push(md_ref(reference(context, Some(&item.text))?));
        }
    }
    list.push(Brace::num(refs.len() as i64));
    list.extend(refs);
    Ok(Brace::List(list))
}

// ---------------------------------------------------------------------------
// Standard attributes.

const STANDARD_ATTRIBUTE_CLASS: &str = "510405d3-2a0c-4fea-960a-7fee59b32f9b";
const LOCALIZED_STRING: &str = "87024738-fc2a-4436-ada1-df79d395c424";

/// The `<StandardAttributes>` of a register or journal:
/// `{1,{1,N,<marker>,510405d3-...,<bag>,...}}`, `{0}` when there are none.
pub struct StandardAttributes {
    entries: Vec<(Brace, StandardAttribute)>,
}

impl StandardAttributes {
    pub fn from_xml(
        object: &ObjectXml<'_>,
        context: &DescriptorContext,
        marker: fn(&str) -> Option<Brace>,
    ) -> Result<Self> {
        let generation = Generation::of(context);
        let mut entries = Vec::new();
        if let Some(list) = object.properties()?.child("StandardAttributes") {
            for element in list.children_named("StandardAttribute") {
                let name = element.attr("name").unwrap_or_default();
                let code = marker(name).ok_or_else(|| {
                    anyhow!("unknown standard attribute {name} of {}", object.kind)
                })?;
                let attribute = StandardAttribute::from_xml(element, context, generation, marker)
                    .with_context(|| format!("standard attribute {name}"))?;
                entries.push((code, attribute));
            }
        }
        Ok(Self { entries })
    }

    pub fn to_brace(self) -> Brace {
        if self.entries.is_empty() {
            return brace_list![Brace::num(0)];
        }
        let mut list = vec![Brace::num(1), Brace::num(self.entries.len() as i64)];
        for (marker, attribute) in self.entries {
            list.push(marker);
            list.push(Brace::atom(STANDARD_ATTRIBUTE_CLASS));
            list.push(attribute.to_brace());
        }
        brace_list![Brace::num(1), Brace::List(list)]
    }
}

/// One standard attribute's property bag: `{13,24,<key>,<value>,...}` with
/// the keys in uuid order, `{14,25,...}` when the modern generation adds
/// TypeReductionMode.
pub struct StandardAttribute {
    generation: Generation,
    link_by_type: Brace,
    choice_parameter_links: Brace,
    choice_parameters: Brace,
    fill_checking: i64,
    multi_line: bool,
    fill_from_filling_value: bool,
    create_on_input: i64,
    type_reduction_mode: i64,
    max_value: Brace,
    tool_tip: Brace,
    extended_edit: bool,
    format: Brace,
    choice_form: Brace,
    quick_choice: i64,
    choice_history_on_input: i64,
    edit_format: Brace,
    password_mode: bool,
    data_history: i64,
    mark_negatives: bool,
    min_value: Brace,
    synonym: Brace,
    comment: String,
    full_text_search: i64,
    fill_value: Brace,
    mask: String,
}

fn enum_value(name: &str, value: &str, table: &[(&str, i64)]) -> Result<i64> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, code)| *code)
        .ok_or_else(|| anyhow!("unsupported <{name}> value {value:?}"))
}

impl StandardAttribute {
    fn from_xml(
        element: &Element,
        context: &DescriptorContext,
        generation: Generation,
        marker: fn(&str) -> Option<Brace>,
    ) -> Result<Self> {
        let text = |name: &str| -> &str { element.child_text(name).unwrap_or_default().trim() };
        let flag = |name: &str| -> Result<bool> {
            match element.child_text(name) {
                Some(value) => parse_bool(value.trim()),
                None => Ok(false),
            }
        };
        let choice = |name: &str, table: &[(&str, i64)]| enum_value(name, text(name), table);
        // A link to another standard attribute of the same register is the
        // register's own marker (`AccountingRegister.X.StandardAttribute.Account`
        // -> `{-10}`); anything else is a plain field data path.
        let link = element.child("LinkByType");
        let path = link
            .and_then(|link| link.child_text("DataPath"))
            .unwrap_or_default()
            .trim();
        let link_by_type = match path.rsplit_once(".StandardAttribute.") {
            Some((_, attribute)) => {
                let target = marker(attribute)
                    .ok_or_else(|| anyhow!("unsupported LinkByType data path {path}"))?;
                let item = link
                    .and_then(|link| link.child_text("LinkItem"))
                    .unwrap_or("0")
                    .trim();
                brace_list![Brace::num(3), Brace::num(1), target, Brace::atom(item)]
            }
            None => link_by_type(link, context)?,
        };
        Ok(Self {
            generation,
            link_by_type,
            choice_parameter_links: choice_parameter_links(
                element.child("ChoiceParameterLinks"),
                context,
            )?,
            choice_parameters: choice_parameters(element.child("ChoiceParameters"), context)?,
            fill_checking: choice(
                "FillChecking",
                &[("DontCheck", 0), ("ShowError", 1), ("ShowWarning", 2)],
            )?,
            multi_line: flag("MultiLine")?,
            fill_from_filling_value: flag("FillFromFillingValue")?,
            create_on_input: choice("CreateOnInput", &[("Auto", 0), ("DontUse", 1), ("Use", 2)])?,
            type_reduction_mode: match element.child_text("TypeReductionMode") {
                Some(value) => enum_value(
                    "TypeReductionMode",
                    value.trim(),
                    &[("TransformValues", 0), ("DeleteData", 1)],
                )?,
                None => 0,
            },
            max_value: typed_value(element.child("MaxValue"), context)?,
            tool_tip: localized(element.child("ToolTip")),
            extended_edit: flag("ExtendedEdit")?,
            format: localized(element.child("Format")),
            choice_form: reference(context, element.child_text("ChoiceForm"))?,
            quick_choice: choice("QuickChoice", &[("Use", 0), ("DontUse", 1), ("Auto", 2)])?,
            choice_history_on_input: choice(
                "ChoiceHistoryOnInput",
                &[("Auto", 0), ("DontUse", 1)],
            )?,
            edit_format: localized(element.child("EditFormat")),
            password_mode: flag("PasswordMode")?,
            data_history: choice("DataHistory", &[("DontUse", 0), ("Use", 1)])?,
            mark_negatives: flag("MarkNegatives")?,
            min_value: typed_value(element.child("MinValue"), context)?,
            synonym: localized(element.child("Synonym")),
            comment: native_text(element.child_text("Comment").unwrap_or_default()),
            full_text_search: choice("FullTextSearch", &[("DontUse", 0), ("Use", 1)])?,
            fill_value: typed_value(element.child("FillValue"), context)?,
            mask: native_text(element.child_text("Mask").unwrap_or_default()),
        })
    }

    pub fn to_brace(self) -> Brace {
        fn hash(class: &str, value: Brace) -> Brace {
            brace_list![Brace::str("#"), Brace::atom(class), value]
        }
        fn nested(class: &str, code: i64) -> Brace {
            hash(class, brace_list![Brace::atom(class), Brace::num(code)])
        }
        fn boolean(value: bool) -> Brace {
            brace_list![Brace::str("B"), Brace::flag(value)]
        }
        fn string(value: String) -> Brace {
            brace_list![Brace::str("S"), Brace::str(value)]
        }
        let modern = self.generation.is_modern();
        let mut entries: Vec<(&str, Brace)> = vec![
            (
                "1183c14f-f814-49c6-9233-a3c26b3f64cf",
                hash("9ad557b1-249e-48dc-824b-3e149ecf10a6", self.link_by_type),
            ),
            (
                "2723eb98-b4c1-498a-a6f3-70444757902f",
                hash(
                    "98ea8e5a-b586-442b-b944-6e3447734aa7",
                    Brace::num(self.fill_checking),
                ),
            ),
            (
                "2bbba66b-fabf-4863-8ba3-54b3c64c896e",
                boolean(self.multi_line),
            ),
            (
                "2c8143d5-4248-4c43-8bfb-307c0be2e415",
                boolean(self.fill_from_filling_value),
            ),
            (
                "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9",
                nested("ad3615c5-aae6-4725-89be-91827523abd9", self.create_on_input),
            ),
        ];
        if modern {
            entries.push((
                "3b10624f-1e3d-495d-8093-25225efc5313",
                nested(
                    "502b7765-f89c-4fd0-924f-0a28d3dc09b7",
                    self.type_reduction_mode,
                ),
            ));
        }
        entries.extend([
            ("3eaf5a8b-06d6-47b0-ac7d-a9698247f499", self.max_value),
            (
                "4690ff70-e3fa-4914-9127-6a9acc5fc949",
                hash(LOCALIZED_STRING, self.tool_tip),
            ),
            (
                "4de03908-56f4-4396-a61e-17253afca9ac",
                boolean(self.extended_edit),
            ),
            (
                "580c29e2-8af4-4258-882a-7cf8073e61c8",
                hash(LOCALIZED_STRING, self.format),
            ),
            (
                "6c4f7074-e7d4-48eb-b31b-132873666262",
                md_ref(self.choice_form),
            ),
            (
                "6e3a1131-37a3-4da5-8895-572d9d0c9db6",
                nested("ace3fd07-11b2-477e-ab7f-36f0ea37c8dd", self.quick_choice),
            ),
            (
                "7ba608f2-e654-42a3-8885-334fe88ca910",
                hash(
                    "12ca4003-ac70-450e-b897-37faf86bd313",
                    Brace::num(self.choice_history_on_input),
                ),
            ),
            (
                "88149a78-9448-4767-867b-0e650d165d2e",
                hash(LOCALIZED_STRING, self.edit_format),
            ),
            (
                "90ae4b5d-e0fd-49ef-a008-d67c1e75038c",
                boolean(self.password_mode),
            ),
            (
                "9288a8ed-b259-46d0-a8e3-70d87956ff2d",
                nested("d46ea122-3201-4e5e-bed4-e669c6e463c8", self.data_history),
            ),
            (
                "b02800e9-a8d1-42ab-9a12-f673e92be968",
                boolean(self.mark_negatives),
            ),
            ("c65a541f-0b91-4f33-bc88-fbaaa57f9992", self.min_value),
            (
                "cf4abea3-37b2-11d4-940f-008048da11f9",
                hash(LOCALIZED_STRING, self.synonym),
            ),
            ("cf4abea4-37b2-11d4-940f-008048da11f9", string(self.comment)),
            (
                "d4232326-022b-421e-b6d3-88e418f74327",
                nested(
                    "3b8e6bdd-d648-49d5-af2f-d46d84f87dd5",
                    self.full_text_search,
                ),
            ),
            (
                "e3da683b-c54a-457a-a243-b9b4f9bf76dd",
                hash(
                    "b76a58b9-2a56-4e46-bb31-8e04ad9f31ae",
                    self.choice_parameter_links,
                ),
            ),
            ("e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208", self.fill_value),
            ("f49e4ced-4033-4e6c-8755-9fbaaccd6078", string(self.mask)),
            (
                "fcf503b8-1c06-454a-970c-06413e64aee5",
                hash(
                    "f2eaae14-91a7-47b9-9d69-097877f41580",
                    self.choice_parameters,
                ),
            ),
        ]);
        let mut list = vec![
            Brace::num(if modern { 14 } else { 13 }),
            Brace::num(entries.len() as i64),
        ];
        for (key, value) in entries {
            list.push(Brace::atom(key));
            list.push(value);
        }
        Brace::List(list)
    }
}

// ---------------------------------------------------------------------------
// Owned commands.

const COMMAND_CLASS: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

/// A `<Command>` child: `{{0,{1,{2,<uuid>,078a6af8-...},{9,<picture>,
/// Representation,ToolTip,1,Shortcut,0,{1,Group},CommandParameterType,
/// <md base>,ModifiesData,ParameterUseMode,OnMainServerUnavalableBehavior}}},0}`.
pub fn command(element: &Element, context: &DescriptorContext) -> Result<Brace> {
    let properties = element
        .child("Properties")
        .ok_or_else(|| anyhow!("<Command> has no <Properties>"))?;
    let uuid = element
        .attr("uuid")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = |name: &str| -> &str { properties.child_text(name).unwrap_or_default().trim() };
    let name = text("Name");
    let group = command_group(text("Group"), context).with_context(|| format!("command {name}"))?;
    let body = brace_list![
        Brace::num(9),
        picture(properties.child("Picture"), context).with_context(|| format!("command {name}"))?,
        code(
            "Representation",
            text("Representation"),
            &[
                ("Text", 0),
                ("Picture", 1),
                ("PictureAndText", 2),
                ("Auto", 3)
            ],
        )?,
        localized(properties.child("ToolTip")),
        Brace::num(1),
        shortcut(text("Shortcut"))?,
        Brace::num(0),
        brace_list![Brace::num(1), group],
        type_pattern(properties.child("CommandParameterType"), context)
            .with_context(|| format!("command {name}"))?,
        md_base(&uuid, properties),
        Brace::flag(parse_bool(text("ModifiesData"))?),
        code(
            "ParameterUseMode",
            text("ParameterUseMode"),
            &[("Single", 0), ("Multiple", 1)],
        )?,
        code(
            "OnMainServerUnavalableBehavior",
            text("OnMainServerUnavalableBehavior"),
            &[("Auto", 0)],
        )?,
    ];
    Ok(item(brace_list![
        Brace::num(0),
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(2),
                Brace::uuid(&uuid),
                Brace::atom(COMMAND_CLASS)
            ],
            body,
        ],
    ]))
}

/// A command's `<Group>`: a platform group by name or a `CommandGroup.X`.
fn command_group(name: &str, context: &DescriptorContext) -> Result<Brace> {
    let platform = match name {
        "NavigationPanelOrdinary" => "77ea1b8f-dd79-4717-9dba-5628e7f348cf",
        "NavigationPanelSeeAlso" => "bc80566a-86a5-4e87-acd4-872239385a2e",
        "NavigationPanelImportant" => "1af6d528-0b86-4fba-ab95-bd7475db03ba",
        "ActionsPanelCreate" => "4f499c31-050b-47c5-aa84-d0366c0a0da8",
        "ActionsPanelReports" => "5b360bff-01a1-49b6-93d2-26e7e8e3a038",
        "ActionsPanelTools" => "aabb34e1-98c1-4bd0-bf7f-243f95437b44",
        "FormCommandBarCreateBasedOn" => "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0",
        "FormCommandBarImportant" => "cb50f5c0-8013-4262-93a2-f0db379d6b6b",
        "FormNavigationPanelGoTo" => "eacad741-96b9-4b3a-bf79-dde9ecead1a1",
        "FormNavigationPanelSeeAlso" => "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8",
        "FormNavigationPanelImportant" => "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2",
        _ => return reference(context, Some(name)),
    };
    Ok(Brace::atom(platform))
}

/// Standard pictures stored by code rather than by uuid.
#[doc(hidden)]
pub const STANDARD_PICTURE_CODES: &[(&str, i64)] = &[
    ("CheckAll", -10),
    ("Clear", -200),
    ("InputFieldCalculator", -6),
    ("InputFieldCalendar", -5),
    ("InputFieldChooseType", -14),
    ("InputFieldClear", -2),
    ("InputFieldOpen", -7),
    ("InputFieldSelect", -1),
    ("MoveDown", -4),
    ("MoveLeft", -8),
    ("MoveRight", -9),
    ("MoveUp", -3),
    ("Print", -13),
    ("Select", -100),
    ("UncheckAll", -11),
    ("ZoomIn", -16),
    ("ZoomOut", -15),
];

/// `{4,0,{0},"",-1,-1,1,0,""}` for no picture, `{4,1,{0,<uuid>},...}` for a
/// common or uuid-stored standard picture, `{4,1,{<code>},...}` for a coded
/// standard picture; slot 6 is LoadTransparent.
fn picture(element: Option<&Element>, context: &DescriptorContext) -> Result<Brace> {
    let reference_text = element
        .and_then(|element| element.child_text("Ref"))
        .unwrap_or_default()
        .trim();
    let tail = |present: bool, value: Brace, transparent: bool| {
        brace_list![
            Brace::num(4),
            Brace::flag(present),
            value,
            Brace::str(""),
            Brace::num(-1),
            Brace::num(-1),
            Brace::flag(transparent),
            Brace::num(0),
            Brace::str(""),
        ]
    };
    if reference_text.is_empty() {
        return Ok(tail(false, brace_list![Brace::num(0)], true));
    }
    let transparent = match element.and_then(|element| element.child_text("LoadTransparent")) {
        Some(value) => parse_bool(value.trim())?,
        None => false,
    };
    let value = if let Some(name) = reference_text.strip_prefix("StdPicture.") {
        if let Some((_, code)) = STANDARD_PICTURE_CODES
            .iter()
            .find(|(candidate, _)| *candidate == name)
        {
            brace_list![Brace::num(*code)]
        } else {
            let uuid = ibcmd_1c_vocab::standard_picture_uuid(reference_text)
                .ok_or_else(|| anyhow!("unknown standard picture {reference_text}"))?;
            brace_list![Brace::num(0), Brace::uuid(uuid)]
        }
    } else {
        brace_list![Brace::num(0), reference(context, Some(reference_text))?]
    };
    Ok(tail(true, value, transparent))
}

/// `{0,<virtual-key code>,<modifiers>}` with Shift 4, Ctrl 8 and Alt 16
/// summed; `{0,0,0}` for none.
pub fn shortcut(text: &str) -> Result<Brace> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(brace_list![Brace::num(0), Brace::num(0), Brace::num(0)]);
    }
    let unsupported = || anyhow!("unsupported shortcut {text:?}");
    let (modifiers, key) = if let Some(prefix) = text.strip_suffix("Num +") {
        (prefix.strip_suffix('+').unwrap_or(prefix), "Num +")
    } else {
        match text.rsplit_once('+') {
            Some((prefix, key)) => (prefix, key),
            None => ("", text),
        }
    };
    let code: i64 = match key {
        "BackSpace" => 8,
        "Enter" => 13,
        "Esc" => 27,
        "Num *" => 106,
        "Num +" => 107,
        "Num -" => 109,
        "Num ." => 110,
        "Num /" => 111,
        _ => {
            if let Some(digit) = key.strip_prefix("Num ") {
                let digit: i64 = digit.parse().map_err(|_| unsupported())?;
                if digit > 9 {
                    return Err(unsupported());
                }
                96 + digit
            } else if let Some(number) = key.strip_prefix('F').filter(|rest| !rest.is_empty()) {
                let number: i64 = number.parse().map_err(|_| unsupported())?;
                if !(1..=12).contains(&number) {
                    return Err(unsupported());
                }
                111 + number
            } else {
                let mut chars = key.chars();
                let single = chars.next().ok_or_else(unsupported)?;
                if chars.next().is_some()
                    || !(single.is_ascii_uppercase() || single.is_ascii_digit())
                {
                    return Err(unsupported());
                }
                i64::from(u32::from(single))
            }
        }
    };
    let mut mask = 0;
    if !modifiers.is_empty() {
        for modifier in modifiers.split('+') {
            mask |= match modifier {
                "Shift" => 4,
                "Ctrl" => 8,
                "Alt" => 16,
                _ => return Err(unsupported()),
            };
        }
    }
    Ok(brace_list![
        Brace::num(0),
        Brace::num(code),
        Brace::num(mask)
    ])
}
