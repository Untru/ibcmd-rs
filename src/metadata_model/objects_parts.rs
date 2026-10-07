//! The parts every reference object composes: the owner-record helpers
//! (`Obj`), standard attributes, standard tabular sections, tabular
//! sections, attribute wrappers, commands with pictures and shortcuts,
//! characteristics, field and metadata references, enum values.
//!
//! Type descriptions, typed values and the attribute body belong to the
//! simple-objects track (`types.rs`, `attribute.rs`); they are reached only
//! through the functions of `shared` below.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, bail};

use super::{line_number_marker, standard_markers};
use crate::brace_list;
use crate::metadata_model::brace::{Brace, NIL_UUID};
use crate::metadata_model::xml::{Element, MetadataXml};
use crate::metadata_model::{DescriptorContext, ObjectXml, localized, md_base, native_text};

pub(crate) fn num(value: i64) -> Brace {
    Brace::num(value)
}

pub(crate) fn nil() -> Brace {
    Brace::nil_uuid()
}

/// `{<class uuid>,<count>,<item>...}`.
pub(crate) fn collection(class: &str, items: Vec<Brace>) -> Brace {
    let mut list = Vec::with_capacity(items.len() + 2);
    list.push(Brace::uuid(class));
    list.push(num(items.len() as i64));
    list.extend(items);
    Brace::List(list)
}

// Platform type uuids of the typed values the rows hold.
const METADATA_OBJECT_REF: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const FIELD_REF: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";
const CHARACTERISTIC: &str = "fe839d42-d094-40ba-b903-75bccc21ba30";
const STANDARD_ATTRIBUTE: &str = "510405d3-2a0c-4fea-960a-7fee59b32f9b";
const COMMAND_VALUE: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";
const LOCALIZED_STRING: &str = "87024738-fc2a-4436-ada1-df79d395c424";

/// A configuration's compatibility mode, `Version8_3_24` -> `(8, 3, 24)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Compat(pub u32, pub u32, pub u32);

impl Compat {
    /// `Version8_3_24` -> `Compat(8, 3, 24)`.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let rest = text.trim().strip_prefix("Version")?;
        let parts = rest
            .split('_')
            .map(|part| part.parse::<u32>().ok())
            .collect::<Option<Vec<_>>>()?;
        match parts.as_slice() {
            [major, minor, build] => Some(Self(*major, *minor, *build)),
            [major, minor] => Some(Self(*major, *minor, 0)),
            _ => None,
        }
    }
}

/// The compatibility mode of the tree's `Configuration.xml`, read once per
/// state of the file. `DontUse` (or no file) means the platform's own: the
/// release the registry maps the XML dialect to.
pub(crate) fn compatibility(context: &DescriptorContext) -> Compat {
    type Key = (PathBuf, Option<std::time::SystemTime>, u64);
    static CACHE: OnceLock<Mutex<HashMap<Key, Compat>>> = OnceLock::new();
    let path = context.root.join("Configuration.xml");
    let metadata = std::fs::metadata(&path).ok();
    let key: Key = (
        path.clone(),
        metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok()),
        metadata
            .as_ref()
            .map(|metadata| metadata.len())
            .unwrap_or_default(),
    );
    let cache = CACHE.get_or_init(Default::default);
    if let Some(value) = cache.lock().ok().and_then(|map| map.get(&key).copied()) {
        return value;
    }
    let [major, minor, patch] = context.platform().release();
    let default = Compat(major, minor, patch);
    let value = std::fs::read(&path)
        .ok()
        .and_then(|bytes| MetadataXml::parse(&bytes).ok())
        .and_then(|doc| {
            doc.object()
                .ok()
                .and_then(|object| object.path(&["Properties", "CompatibilityMode"]))
                .and_then(|mode| Compat::parse(&mode.text))
        })
        .unwrap_or(default);
    if let Ok(mut map) = cache.lock() {
        map.insert(key, value);
    }
    value
}

/// How a family wraps a tabular section record `{11,...}`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TsWrapper {
    /// `{v,<ts>}`
    Bare(i64),
    /// `{v,<ts>,<Use>}`
    Use(i64),
    /// `{v,<ts>,<LineNumberLength>}`
    Length(i64),
    /// `{v,<ts>,<Use>,<LineNumberLength>}`
    UseAndLength(i64),
}

/// How a family wraps an attribute body `{27,...}`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum AttributeWrapper {
    /// `{0,<body>}` (reports, data processors).
    Bare,
    /// `{v,<body>,<Indexing>,<FullTextSearch>,<DataHistory>}`
    Plain(i64),
    /// `Plain` plus the constant tail `0,{1,<nil>}` of compatibility 8.3.27.
    PlainModern(i64),
    /// `{v,<body>,<Indexing>,<Use>,<FullTextSearch>,<DataHistory>}`
    Hierarchical(i64),
    HierarchicalModern(i64),
    /// A tabular-section attribute: `{8,<body>,<Indexing>,<FullTextSearch>,<DataHistory>}`.
    TabularSection,
    /// A task addressing attribute:
    /// `{v,<body>,<Indexing>,<dimension>,<FullTextSearch>,<DataHistory>}`.
    Addressing(i64),
    /// An accounting flag: `{v,<body>,<DataHistory>}`.
    Flag(i64),
}

/// How a family wraps an owned command record.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CommandWrapper {
    /// `{{0,{0,0,0,<command>}},0}`
    Owner,
    /// `{{0,<command>},0}` (reports, data processors).
    Bare,
}

/// A code table: XML spelling -> stored number.
pub(crate) type Codes = &'static [(&'static str, i64)];

/// The charts' predefined tabular sections: name, marker, attribute markers.
pub(crate) type StandardSections = &'static [(&'static str, i64, Codes)];

/// One slot of an owner record, named by the XML property it carries. A
/// kind's record is a list of slots, so the same table reads a row back.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Slot {
    /// The record version: before 8.3.27, from 8.3.27, from 8.5.1.
    Tag(i64, i64, i64),
    /// Two slots: TypeId and ValueId of the generated type of a category.
    Generated(&'static str),
    /// `<md base>`
    Header,
    /// `{0,<md base>}`
    WrappedHeader,
    /// `true`/`false` -> 1/0.
    Flag(&'static str),
    /// A number as written.
    Number(&'static str),
    /// An enumeration through its code table.
    Code(&'static str, Codes),
    /// A string as written (`CodeMask`).
    Text(&'static str),
    /// `{N,"lang","text",...}`
    Localized(&'static str),
    /// One metadata object (a form, a register, a storage...): uuid or nil.
    Reference(&'static str),
    /// `{0,N,<metadata ref>...}` of an item list.
    References(&'static str),
    /// `{1,{0,N,<field ref>...}}` of a field list.
    Fields(&'static str),
    /// `{<search>,<full-text>,<data get>}` of input by string.
    InputModes,
    /// A type description property.
    TypePattern(&'static str),
    /// The root standard attributes with the family's markers.
    StandardAttributes(Codes),
    /// The charts' predefined tabular sections: name, marker, attribute markers.
    StandardTabularSections(StandardSections),
    Characteristics,
    /// An exchange plan's `<InternalInfo><ThisNode>`.
    ThisNode,
    /// A constant no XML property varies.
    Const(i64),
    /// A nil uuid no XML property fills.
    Nil,
    /// A slot only compatibility 8.3.27 and later stores.
    Modern(&'static Slot),
    /// A slot only compatibility 8.5.1 and later stores.
    Since8_5_1(&'static Slot),
}

/// A record wrapper that changed with compatibility 8.3.27.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Versioned<T> {
    pub old: T,
    pub modern: T,
}

/// What one collection of the root holds.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Coll {
    Templates,
    Forms,
    Commands(CommandWrapper),
    /// Attribute-like children of an XML tag (`Attribute`, `AccountingFlag`...).
    Children(&'static str, Versioned<AttributeWrapper>),
    TabularSections {
        wrapper: Versioned<TsWrapper>,
        attribute_class: &'static str,
        attribute: AttributeWrapper,
    },
    EnumValues,
    /// A collection no XML fills.
    Empty,
}

/// A kind's row: owner record slots, then the collections by class uuid in
/// uuid order.
pub(crate) struct Layout {
    pub slots: &'static [Slot],
    pub collections: &'static [(&'static str, Coll)],
}

/// One object being compiled, with the readers every slot uses.
pub(crate) struct Obj<'a> {
    pub xml: &'a ObjectXml<'a>,
    pub cx: &'a DescriptorContext,
    pub props: &'a Element,
    pub compat: Compat,
    /// `Catalog.Name`
    pub full: String,
}

impl<'a> Obj<'a> {
    pub fn new(xml: &'a ObjectXml<'a>, cx: &'a DescriptorContext) -> Result<Self> {
        Ok(Self {
            xml,
            cx,
            props: xml.properties()?,
            compat: compatibility(cx),
            full: format!("{}.{}", xml.kind, xml.name),
        })
    }

    /// Compatibility 8.3.27 or later: the newer record versions.
    pub fn modern(&self) -> bool {
        self.compat >= Compat(8, 3, 27)
    }

    /// Compatibility 8.5.1 or later.
    pub fn since_8_5_1(&self) -> bool {
        self.compat >= Compat(8, 5, 1)
    }

    pub fn kind(&self) -> &str {
        self.xml.kind
    }

    pub fn text(&self, name: &str) -> &'a str {
        self.props.child_text(name).unwrap_or("")
    }

    pub fn flag(&self, name: &str) -> Result<Brace> {
        flag_of(self.props, name)
    }

    pub fn number(&self, name: &str) -> Result<Brace> {
        number_of(self.props, name)
    }

    pub fn code(&self, name: &str, table: &[(&str, i64)]) -> Result<Brace> {
        code_of(self.props, name, table)
    }

    pub fn loc(&self, name: &str) -> Brace {
        localized(self.props.child(name))
    }

    pub fn header(&self) -> Brace {
        md_base(&self.xml.uuid, self.props)
    }

    /// `{0,<header>}`.
    pub fn wrapped_header(&self) -> Brace {
        brace_list![num(0), self.header()]
    }

    /// TypeId and ValueId of the object's generated type of a category.
    pub fn generated(&self, category: &str) -> Result<[Brace; 2]> {
        generated_of(self.xml.element, category)
            .with_context(|| format!("{} {}", self.xml.kind, self.xml.name))
    }

    /// `<xr:ThisNode>` of an exchange plan's `InternalInfo`.
    pub fn this_node(&self) -> Result<Brace> {
        let uuid = self
            .xml
            .element
            .path(&["InternalInfo", "ThisNode"])
            .map(|node| node.text.trim())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| anyhow!("{} has no InternalInfo/ThisNode", self.full))?;
        Ok(Brace::uuid(uuid))
    }

    /// uuid of a metadata object or child object by full name.
    pub fn resolve(&self, reference: &str) -> Result<String> {
        self.cx
            .index
            .uuid_of(reference.trim())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("unresolved reference {reference}"))
    }

    /// A property naming one metadata object: its uuid, nil when empty.
    pub fn reference(&self, name: &str) -> Result<Brace> {
        let text = self.text(name).trim();
        if text.is_empty() {
            return Ok(nil());
        }
        Ok(Brace::uuid(&self.resolve(text)?))
    }

    /// `{0,N,{"#",<metadata-object ref>,{1,<uuid>}}...}` of an item list
    /// property (`Owners`, `BasedOn`, `RegisterRecords`).
    pub fn references(&self, name: &str) -> Result<Brace> {
        let mut items = vec![num(0), num(0)];
        if let Some(list) = self.props.child(name) {
            for item in list.children_named("Item") {
                let uuid = self.resolve(&item.text)?;
                items.push(brace_list![
                    Brace::str("#"),
                    Brace::uuid(METADATA_OBJECT_REF),
                    brace_list![num(1), Brace::uuid(&uuid)],
                ]);
            }
        }
        items[1] = num(items.len() as i64 - 2);
        Ok(Brace::List(items))
    }

    /// A field of this object or of another one, as characteristics and
    /// field lists name it: `{-N}` for a standard attribute, `{0,<uuid>}`
    /// for an attribute, dimension, resource...
    pub fn field_ref(&self, path: &str) -> Result<Brace> {
        let path = path.trim();
        if let Some((owner, name)) = path.rsplit_once(".StandardAttribute.") {
            let kind = owner.split('.').next().unwrap_or_default();
            let marker = if owner.contains(".TabularSection.") && name == "LineNumber" {
                line_number_marker(kind)
            } else {
                standard_markers(kind)
                    .and_then(|table| lookup(table, name))
                    .ok_or_else(|| anyhow!("no standard attribute marker for {path}"))?
            };
            return Ok(brace_list![num(marker)]);
        }
        Ok(brace_list![num(0), Brace::uuid(&self.resolve(path)?)])
    }

    /// `{1,{0,N,{"#",<field ref>,<field>}...}}` (`InputByString`,
    /// `DataLockFields`).
    pub fn fields(&self, name: &str) -> Result<Brace> {
        let mut items = vec![num(0), num(0)];
        if let Some(list) = self.props.child(name) {
            for field in list.children_named("Field") {
                items.push(brace_list![
                    Brace::str("#"),
                    Brace::uuid(FIELD_REF),
                    self.field_ref(&field.text)?,
                ]);
            }
        }
        items[1] = num(items.len() as i64 - 2);
        Ok(brace_list![num(1), Brace::List(items)])
    }

    /// `{<search mode>,<full-text search>,<data get mode>}` of input by string.
    pub fn input_modes(&self) -> Result<Brace> {
        Ok(brace_list![
            self.code(
                "SearchStringModeOnInputByString",
                &[("Begin", 1), ("AnyPart", 2)]
            )?,
            self.code(
                "FullTextSearchOnInputByString",
                &[("Use", 1), ("DontUse", 2)]
            )?,
            self.code(
                "ChoiceDataGetModeOnInputByString",
                &[("Directly", 0), ("Background", 1)],
            )?,
        ])
    }

    /// The type description of a property (`Type` of a characteristic chart).
    pub fn type_pattern(&self, name: &str) -> Result<Brace> {
        shared::type_pattern(self.props.child(name), self.cx)
    }

    pub fn standard_attributes(&self, markers: &[(&str, i64)]) -> Result<Brace> {
        standard_attributes(self, self.props.child("StandardAttributes"), markers)
    }

    /// `{1,{0,N,<marker>,{3,<synonym>,"comment",<fill>,0,<attrs>,<tooltip>}...}}`
    /// of the charts' predefined tabular sections.
    pub fn standard_tabular_sections(&self, definitions: StandardSections) -> Result<Brace> {
        let Some(list) = self.props.child("StandardTabularSections") else {
            return Ok(brace_list![num(0)]);
        };
        let sections = list
            .children_named("StandardTabularSection")
            .collect::<Vec<_>>();
        if sections.is_empty() {
            return Ok(brace_list![num(0)]);
        }
        let mut items = vec![num(0), num(sections.len() as i64)];
        for section in sections {
            let name = section.attr("name").unwrap_or_default();
            let (_, marker, attribute_markers) = definitions
                .iter()
                .find(|(candidate, _, _)| *candidate == name)
                .ok_or_else(|| anyhow!("unknown standard tabular section {name}"))?;
            let attributes = match section.child("StandardAttributes") {
                Some(attributes) => standard_attribute_body(self, attributes, attribute_markers)?,
                None => brace_list![num(0)],
            };
            items.push(num(*marker));
            items.push(brace_list![
                num(3),
                localized(section.child("Synonym")),
                Brace::str(section.child_text("Comment").unwrap_or_default()),
                code_of(section, "FillChecking", FILL_CHECKING)?,
                num(0),
                attributes,
                localized(section.child("ToolTip")),
            ]);
        }
        Ok(brace_list![num(1), Brace::List(items)])
    }

    /// `{0,{N,{"#",<characteristic>,{4,...}}...}}`.
    pub fn characteristics(&self) -> Result<Brace> {
        let items = self
            .props
            .child("Characteristics")
            .map(|list| list.children_named("Characteristic").collect::<Vec<_>>())
            .unwrap_or_default();
        let mut list = vec![num(items.len() as i64)];
        for item in items {
            let types = item
                .child("CharacteristicTypes")
                .ok_or_else(|| anyhow!("Characteristic has no CharacteristicTypes"))?;
            let values = item
                .child("CharacteristicValues")
                .ok_or_else(|| anyhow!("Characteristic has no CharacteristicValues"))?;
            let source = |element: &Element| -> Result<Brace> {
                let from = element.attr("from").unwrap_or_default();
                // A characteristic can name no source; it stores the nil uuid
                // (the БСП 8.5 extension catalog `_ДемоСегментыПартнеровРасширение`).
                if from.trim().is_empty() {
                    return Ok(brace_list![num(1), nil()]);
                }
                Ok(brace_list![num(1), Brace::uuid(&self.resolve(from)?)])
            };
            let body = brace_list![
                num(4),
                source(types)?,
                source(values)?,
                self.characteristic_field(values.child_text("ObjectField"))?,
                self.characteristic_field(values.child_text("TypeField"))?,
                self.characteristic_field(values.child_text("ValueField"))?,
                self.characteristic_field(types.child_text("TypesFilterField"))?,
                shared::typed_value(types.child("TypesFilterValue"), self.cx)?,
                self.characteristic_field(types.child_text("KeyField"))?,
                self.characteristic_field(types.child_text("DataPathField"))?,
                self.characteristic_field(types.child_text("MultipleValuesUseField"))?,
                self.characteristic_field(values.child_text("MultipleValuesKeyField"))?,
                self.characteristic_field(values.child_text("MultipleValuesOrderField"))?,
            ];
            list.push(brace_list![
                Brace::str("#"),
                Brace::uuid(CHARACTERISTIC),
                body
            ]);
        }
        Ok(brace_list![num(0), Brace::List(list)])
    }

    /// `{1,<field>,0}`, `-1` for none.
    fn characteristic_field(&self, text: Option<&str>) -> Result<Brace> {
        let text = text.map(str::trim).unwrap_or("-1");
        let field = match text {
            "" | "-1" => brace_list![num(-1)],
            "0" => brace_list![num(0)],
            _ if text.starts_with("0:") => {
                let uuid = ibcmd_core::identity::ObjectUuid::parse(&text[2..])
                    .map_err(|error| anyhow!("invalid unresolved characteristic field: {error}"))?;
                brace_list![num(0), Brace::uuid(&uuid.to_string())]
            }
            _ => self.field_ref(text)?,
        };
        Ok(brace_list![num(1), field, num(0)])
    }

    /// `{1,<owner record>,<n>,<collection>...}`.
    pub fn root(&self, owner: Vec<Brace>, collections: Vec<Brace>) -> Result<Brace> {
        let mut items = Vec::with_capacity(collections.len() + 3);
        items.push(num(1));
        items.push(Brace::List(owner));
        items.push(num(collections.len() as i64));
        items.extend(collections);
        Ok(Brace::List(items))
    }

    /// The whole row of a kind's layout.
    pub fn compile(&self, layout: &Layout) -> Result<Brace> {
        let mut owner = Vec::with_capacity(layout.slots.len() + 8);
        for slot in layout.slots {
            self.encode(slot, &mut owner)?;
        }
        let collections = layout
            .collections
            .iter()
            .map(|(class, content)| Ok(collection(class, self.collection_items(content)?)))
            .collect::<Result<Vec<_>>>()?;
        self.root(owner, collections)
    }

    /// Appends the values of one slot.
    fn encode(&self, slot: &Slot, out: &mut Vec<Brace>) -> Result<()> {
        let value = match *slot {
            Slot::Tag(old, modern, since_8_5_1) => num(if self.since_8_5_1() {
                since_8_5_1
            } else if self.modern() {
                modern
            } else {
                old
            }),
            Slot::Generated(category) => {
                let [type_id, value_id] = self.generated(category)?;
                out.push(type_id);
                value_id
            }
            Slot::Header => self.header(),
            Slot::WrappedHeader => self.wrapped_header(),
            Slot::Flag(name) => self.flag(name)?,
            Slot::Number(name) => self.number(name)?,
            Slot::Code(name, table) => self.code(name, table)?,
            Slot::Text(name) => Brace::str(self.text(name)),
            Slot::Localized(name) => self.loc(name),
            Slot::Reference(name) => self.reference(name)?,
            Slot::References(name) => self.references(name)?,
            Slot::Fields(name) => self.fields(name)?,
            Slot::InputModes => self.input_modes()?,
            Slot::TypePattern(name) => self.type_pattern(name)?,
            Slot::StandardAttributes(markers) => self.standard_attributes(markers)?,
            Slot::StandardTabularSections(definitions) => {
                self.standard_tabular_sections(definitions)?
            }
            Slot::Characteristics => self.characteristics()?,
            Slot::ThisNode => self.this_node()?,
            Slot::Const(value) => num(value),
            Slot::Nil => nil(),
            Slot::Modern(inner) => {
                if self.modern() {
                    self.encode(inner, out)?;
                }
                return Ok(());
            }
            Slot::Since8_5_1(inner) => {
                if self.since_8_5_1() {
                    self.encode(inner, out)?;
                }
                return Ok(());
            }
        };
        out.push(value);
        Ok(())
    }

    fn collection_items(&self, content: &Coll) -> Result<Vec<Brace>> {
        let pick = |versioned: &Versioned<_>| {
            if self.modern() {
                versioned.modern
            } else {
                versioned.old
            }
        };
        match *content {
            Coll::Templates => self.templates(),
            Coll::Forms => self.forms(),
            Coll::Commands(wrapper) => self.commands(wrapper),
            Coll::Children(tag, wrapper) => {
                let wrapper = if self.modern() {
                    wrapper.modern
                } else {
                    wrapper.old
                };
                self.children_of(tag, wrapper)
            }
            Coll::TabularSections {
                wrapper,
                attribute_class,
                attribute,
            } => self.tabular_sections(pick(&wrapper), attribute_class, attribute),
            Coll::EnumValues => self.enum_values(),
            Coll::Empty => Ok(Vec::new()),
        }
    }

    /// Direct `ChildObjects` elements with this tag, in document order.
    pub fn children(&self, tag: &str) -> Vec<&'a Element> {
        self.xml
            .child_objects()
            .map(|children| {
                children
                    .children
                    .iter()
                    .filter(|child| child.name == tag)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// uuids of the owned forms, in document order.
    pub fn forms(&self) -> Result<Vec<Brace>> {
        self.owned_names("Form")
    }

    /// uuids of the owned templates, in document order.
    pub fn templates(&self) -> Result<Vec<Brace>> {
        self.owned_names("Template")
    }

    fn owned_names(&self, tag: &str) -> Result<Vec<Brace>> {
        self.children(tag)
            .into_iter()
            .map(|child| {
                let reference = format!("{}.{tag}.{}", self.full, child.text.trim());
                Ok(Brace::uuid(&self.resolve(&reference)?))
            })
            .collect()
    }

    /// Attribute-like children of a tag (`Attribute`, `AddressingAttribute`,
    /// `AccountingFlag`, ...), each `{<wrapper>,0}`.
    pub fn children_of(&self, tag: &str, wrapper: AttributeWrapper) -> Result<Vec<Brace>> {
        self.children(tag)
            .into_iter()
            .map(|child| self.attribute_item(child, wrapper))
            .collect()
    }

    fn attribute_item(&self, element: &Element, wrapper: AttributeWrapper) -> Result<Brace> {
        let properties = child_properties(element)?;
        let body = shared::attribute_body(element, self.cx)?;
        let indexing = || {
            code_of(
                properties,
                "Indexing",
                &[
                    ("DontIndex", 0),
                    ("Index", 1),
                    ("IndexWithAdditionalOrder", 2),
                ],
            )
        };
        let full_text = || code_of(properties, "FullTextSearch", &[("DontUse", 0), ("Use", 1)]);
        let history = || code_of(properties, "DataHistory", &[("DontUse", 0), ("Use", 1)]);
        let usage = || {
            code_of(
                properties,
                "Use",
                &[("ForItem", 0), ("ForFolder", 1), ("ForFolderAndItem", 2)],
            )
        };
        let modern_tail = || [num(0), brace_list![num(1), nil()]];
        let record = match wrapper {
            AttributeWrapper::Bare => brace_list![num(0), body],
            AttributeWrapper::Plain(version) => {
                brace_list![num(version), body, indexing()?, full_text()?, history()?]
            }
            AttributeWrapper::PlainModern(version) => {
                let [zero, tail] = modern_tail();
                brace_list![
                    num(version),
                    body,
                    indexing()?,
                    full_text()?,
                    history()?,
                    zero,
                    tail
                ]
            }
            AttributeWrapper::Hierarchical(version) => brace_list![
                num(version),
                body,
                indexing()?,
                usage()?,
                full_text()?,
                history()?
            ],
            AttributeWrapper::HierarchicalModern(version) => {
                let [zero, tail] = modern_tail();
                brace_list![
                    num(version),
                    body,
                    indexing()?,
                    usage()?,
                    full_text()?,
                    history()?,
                    zero,
                    tail
                ]
            }
            AttributeWrapper::TabularSection => {
                brace_list![num(8), body, indexing()?, full_text()?, history()?]
            }
            AttributeWrapper::Addressing(version) => {
                let dimension = properties
                    .child_text("AddressingDimension")
                    .map(str::trim)
                    .unwrap_or_default();
                let dimension = if dimension.is_empty() {
                    nil()
                } else {
                    Brace::uuid(&self.resolve(dimension)?)
                };
                brace_list![
                    num(version),
                    body,
                    indexing()?,
                    dimension,
                    full_text()?,
                    history()?
                ]
            }
            AttributeWrapper::Flag(version) => brace_list![num(version), body, history()?],
        };
        Ok(brace_list![record, num(0)])
    }

    /// `{<wrapper>,1,{<attribute class>,N,<attribute>...}}` per tabular section.
    pub fn tabular_sections(
        &self,
        wrapper: TsWrapper,
        attribute_class: &str,
        attribute_wrapper: AttributeWrapper,
    ) -> Result<Vec<Brace>> {
        let markers = [("LineNumber", line_number_marker(self.kind()))];
        self.children("TabularSection")
            .into_iter()
            .map(|section| {
                let properties = child_properties(section)?;
                let uuid = element_uuid(section)?;
                let [ts_type, ts_value] = generated_of(section, "TabularSection")?;
                let [row_type, row_value] = generated_of(section, "TabularSectionRow")?;
                let record = brace_list![
                    num(11),
                    ts_type,
                    ts_value,
                    row_type,
                    row_value,
                    brace_list![num(0), md_base(&uuid, properties)],
                    code_of(properties, "FillChecking", FILL_CHECKING)?,
                    standard_attributes(self, properties.child("StandardAttributes"), &markers)?,
                    localized(properties.child("ToolTip")),
                ];
                let usage = || {
                    code_of(
                        properties,
                        "Use",
                        &[("ForItem", 0), ("ForFolder", 1), ("ForFolderAndItem", 2)],
                    )
                };
                let length = || number_of(properties, "LineNumberLength");
                let wrapped = match wrapper {
                    TsWrapper::Bare(version) => brace_list![num(version), record],
                    TsWrapper::Use(version) => brace_list![num(version), record, usage()?],
                    TsWrapper::Length(version) => brace_list![num(version), record, length()?],
                    TsWrapper::UseAndLength(version) => {
                        brace_list![num(version), record, usage()?, length()?]
                    }
                };
                let attributes = section
                    .child("ChildObjects")
                    .map(|children| children.children_named("Attribute").collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|attribute| self.attribute_item(attribute, attribute_wrapper))
                    .collect::<Result<Vec<_>>>()?;
                Ok(brace_list![
                    wrapped,
                    num(1),
                    collection(attribute_class, attributes)
                ])
            })
            .collect()
    }

    /// Owned commands, each `{{0,<command>},0}` or `{{0,{0,0,0,<command>}},0}`.
    pub fn commands(&self, wrapper: CommandWrapper) -> Result<Vec<Brace>> {
        self.children("Command")
            .into_iter()
            .map(|command| {
                let body = self.command(command)?;
                let inner = match wrapper {
                    CommandWrapper::Owner => brace_list![num(0), num(0), num(0), body],
                    CommandWrapper::Bare => body,
                };
                Ok(brace_list![brace_list![num(0), inner], num(0)])
            })
            .collect()
    }

    /// `{1,{2,<uuid>,<command value>},{9,...}}`.
    fn command(&self, element: &Element) -> Result<Brace> {
        let properties = child_properties(element)?;
        let uuid = element_uuid(element)?;
        let group = properties
            .child_text("Group")
            .map(str::trim)
            .unwrap_or_default();
        let group = match builtin_command_group(group) {
            Some(uuid) => uuid.to_string(),
            None => self
                .resolve(group)
                .with_context(|| format!("command group of {}", self.full))?,
        };
        let record = brace_list![
            num(9),
            self.picture(properties.child("Picture"))?,
            code_of(
                properties,
                "Representation",
                &[
                    ("Text", 0),
                    ("Picture", 1),
                    ("PictureAndText", 2),
                    ("Auto", 3)
                ],
            )?,
            localized(properties.child("ToolTip")),
            num(1),
            shortcut(properties.child_text("Shortcut").unwrap_or_default())?,
            num(0),
            brace_list![num(1), Brace::uuid(&group)],
            shared::type_pattern(properties.child("CommandParameterType"), self.cx)?,
            md_base(&uuid, properties),
            flag_of(properties, "ModifiesData")?,
            code_of(
                properties,
                "ParameterUseMode",
                &[("Single", 0), ("Multiple", 1)]
            )?,
            code_of(properties, "OnMainServerUnavalableBehavior", &[("Auto", 0)])?,
        ];
        Ok(brace_list![
            num(1),
            brace_list![num(2), Brace::uuid(&uuid), Brace::uuid(COMMAND_VALUE)],
            record,
        ])
    }

    /// `{4,<present>,<value>,"",<x>,<y>,<load transparent>,0,""}`.
    pub fn picture(&self, element: Option<&Element>) -> Result<Brace> {
        let reference = element
            .and_then(|picture| picture.child_text("Ref"))
            .map(str::trim)
            .unwrap_or_default();
        let pixel = element.and_then(|picture| picture.child("TransparentPixel"));
        let coordinate = |name: &str| -> Brace {
            Brace::atom(pixel.and_then(|pixel| pixel.attr(name)).unwrap_or("-1"))
        };
        if reference.is_empty() {
            return Ok(brace_list![
                num(4),
                num(0),
                brace_list![num(0)],
                Brace::str(""),
                coordinate("x"),
                coordinate("y"),
                num(1),
                num(0),
                Brace::str(""),
            ]);
        }
        let load_transparent = element
            .and_then(|picture| picture.child_text("LoadTransparent"))
            .map(str::trim)
            == Some("true");
        let value = if let Some(name) = reference.strip_prefix("StdPicture.") {
            if let Some(code) = std_picture_code(name) {
                brace_list![num(code)]
            } else if let Some(uuid) = crate::mssql_dump::standard_picture_uuid(reference) {
                brace_list![num(0), Brace::uuid(uuid)]
            } else {
                bail!("no stored value for {reference}");
            }
        } else if reference.starts_with("CommonPicture.") {
            brace_list![num(0), Brace::uuid(&self.resolve(reference)?)]
        } else {
            bail!("unsupported picture reference {reference}");
        };
        Ok(brace_list![
            num(4),
            num(1),
            value,
            Brace::str(""),
            coordinate("x"),
            coordinate("y"),
            Brace::flag(load_transparent),
            num(0),
            Brace::str(""),
        ])
    }

    /// `{{0,<header>},0}`, from compatibility 8.5.1 `{{1,<header>,<colour>},0}`.
    pub fn enum_values(&self) -> Result<Vec<Brace>> {
        self.children("EnumValue")
            .into_iter()
            .map(|value| {
                let properties = child_properties(value)?;
                let uuid = element_uuid(value)?;
                let record = if self.since_8_5_1() {
                    brace_list![
                        num(1),
                        md_base(&uuid, properties),
                        enum_value_color(properties.child_text("Color").unwrap_or("auto"))?,
                    ]
                } else {
                    brace_list![num(0), md_base(&uuid, properties)]
                };
                Ok(brace_list![record, num(0)])
            })
            .collect()
    }
}

pub(crate) const FILL_CHECKING: &[(&str, i64)] = &[("DontCheck", 0), ("ShowError", 1)];

/// The rows keep line breaks inside strings as CRLF; XML hands them over as
/// LF. `localized` and `md_base` already convert; every other string of the
/// tree (comments of standard attributes, masks, filter values...) gets the
/// stored spelling here.
pub(crate) fn restore_crlf(node: &mut Brace) {
    match node {
        Brace::Str(text) if text.contains('\n') => *text = native_text(text),
        Brace::List(items) => items.iter_mut().for_each(restore_crlf),
        _ => {}
    }
}

fn lookup(table: &[(&str, i64)], name: &str) -> Option<i64> {
    table
        .iter()
        .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
}

fn child_properties(element: &Element) -> Result<&Element> {
    element
        .child("Properties")
        .ok_or_else(|| anyhow!("<{}> has no <Properties>", element.name))
}

fn element_uuid(element: &Element) -> Result<String> {
    element
        .attr("uuid")
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| anyhow!("<{}> has no uuid", element.name))
}

fn flag_of(properties: &Element, name: &str) -> Result<Brace> {
    let text = properties
        .child_text(name)
        .ok_or_else(|| anyhow!("no <{name}>"))?;
    match text.trim() {
        "true" => Ok(num(1)),
        "false" => Ok(num(0)),
        other => bail!("<{name}> is {other:?}, not a boolean"),
    }
}

fn number_of(properties: &Element, name: &str) -> Result<Brace> {
    let text = properties
        .child_text(name)
        .ok_or_else(|| anyhow!("no <{name}>"))?
        .trim();
    let value: i64 = text
        .parse()
        .with_context(|| format!("<{name}> is {text:?}, not a number"))?;
    Ok(num(value))
}

fn code_of(properties: &Element, name: &str, table: &[(&str, i64)]) -> Result<Brace> {
    let text = properties
        .child_text(name)
        .ok_or_else(|| anyhow!("no <{name}>"))?
        .trim();
    lookup(table, text)
        .map(num)
        .ok_or_else(|| anyhow!("<{name}> value {text:?} is not mapped"))
}

/// TypeId and ValueId of an element's generated type of a category.
fn generated_of(element: &Element, category: &str) -> Result<[Brace; 2]> {
    let generated = element
        .child("InternalInfo")
        .and_then(|info| {
            info.children_named("GeneratedType")
                .find(|generated| generated.attr("category") == Some(category))
        })
        .ok_or_else(|| anyhow!("no generated type of category {category}"))?;
    let id = |name: &str| -> Result<Brace> {
        generated
            .child_text(name)
            .map(|text| Brace::uuid(text.trim()))
            .ok_or_else(|| anyhow!("generated type {category} has no {name}"))
    };
    Ok([id("TypeId")?, id("ValueId")?])
}

/// `{0}` without a `<StandardAttributes>`, else
/// `{1,{1,N,{<marker>},<class>,<bag>...}}` in the XML's order.
fn standard_attributes(
    obj: &Obj<'_>,
    element: Option<&Element>,
    markers: &[(&str, i64)],
) -> Result<Brace> {
    let Some(element) = element else {
        return Ok(brace_list![num(0)]);
    };
    if element.children_named("StandardAttribute").next().is_none() {
        return Ok(brace_list![num(0)]);
    }
    Ok(brace_list![
        num(1),
        standard_attribute_body(obj, element, markers)?
    ])
}

/// `{1,N,{<marker>},<class>,<bag>...}`.
fn standard_attribute_body(
    obj: &Obj<'_>,
    element: &Element,
    markers: &[(&str, i64)],
) -> Result<Brace> {
    let attributes = element
        .children_named("StandardAttribute")
        .collect::<Vec<_>>();
    let mut items = vec![num(1), num(attributes.len() as i64)];
    for attribute in attributes {
        let name = attribute.attr("name").unwrap_or_default();
        let marker = lookup(markers, name)
            .ok_or_else(|| anyhow!("no marker for standard attribute {name}"))?;
        items.push(brace_list![num(marker)]);
        items.push(Brace::uuid(STANDARD_ATTRIBUTE));
        items.push(standard_attribute_bag(obj, attribute)?);
    }
    Ok(Brace::List(items))
}

/// The property bag of one standard attribute: `{13,24,<key>,<value>...}`
/// (`{14,25,...}` with `TypeReductionMode` from compatibility 8.3.27), keys
/// in uuid order.
fn standard_attribute_bag(obj: &Obj<'_>, attribute: &Element) -> Result<Brace> {
    let typed = |type_uuid: &str, value: Brace| -> Brace {
        brace_list![Brace::str("#"), Brace::uuid(type_uuid), value]
    };
    let nested = |type_uuid: &str, value: i64| -> Brace {
        typed(type_uuid, brace_list![Brace::uuid(type_uuid), num(value)])
    };
    let boolean = |name: &str| -> Result<Brace> {
        let text = attribute.child_text(name).unwrap_or("false").trim();
        Ok(brace_list![Brace::str("B"), Brace::flag(text == "true")])
    };
    let code = |name: &str, table: &[(&str, i64)]| -> Result<i64> {
        let text = attribute
            .child_text(name)
            .ok_or_else(|| anyhow!("standard attribute has no <{name}>"))?
            .trim();
        lookup(table, text).ok_or_else(|| anyhow!("<{name}> value {text:?} is not mapped"))
    };
    let text = |name: &str| -> Brace {
        brace_list![
            Brace::str("S"),
            Brace::str(attribute.child_text(name).unwrap_or_default())
        ]
    };
    let localized_value = |name: &str| typed(LOCALIZED_STRING, localized(attribute.child(name)));
    let choice_form = {
        let reference = attribute
            .child_text("ChoiceForm")
            .map(str::trim)
            .unwrap_or_default();
        let uuid = if reference.is_empty() {
            NIL_UUID.to_string()
        } else {
            obj.resolve(reference)?
        };
        typed(
            "157fa490-4ce9-11d4-9415-008048da11f9",
            brace_list![num(1), Brace::uuid(&uuid)],
        )
    };

    let mut items = vec![
        num(if obj.modern() { 14 } else { 13 }),
        num(if obj.modern() { 25 } else { 24 }),
    ];
    let mut push = |key: &str, value: Brace| {
        items.push(Brace::uuid(key));
        items.push(value);
    };
    push(
        "1183c14f-f814-49c6-9233-a3c26b3f64cf",
        typed(
            "9ad557b1-249e-48dc-824b-3e149ecf10a6",
            shared::link_by_type(attribute.child("LinkByType"), obj.cx)?,
        ),
    );
    push(
        "2723eb98-b4c1-498a-a6f3-70444757902f",
        typed(
            "98ea8e5a-b586-442b-b944-6e3447734aa7",
            num(code("FillChecking", FILL_CHECKING)?),
        ),
    );
    push(
        "2bbba66b-fabf-4863-8ba3-54b3c64c896e",
        boolean("MultiLine")?,
    );
    push(
        "2c8143d5-4248-4c43-8bfb-307c0be2e415",
        boolean("FillFromFillingValue")?,
    );
    push(
        "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9",
        nested(
            "ad3615c5-aae6-4725-89be-91827523abd9",
            code("CreateOnInput", &[("Auto", 0), ("DontUse", 1), ("Use", 2)])?,
        ),
    );
    if obj.modern() {
        push(
            "3b10624f-1e3d-495d-8093-25225efc5313",
            nested(
                "502b7765-f89c-4fd0-924f-0a28d3dc09b7",
                code("TypeReductionMode", &[("TransformValues", 0), ("Deny", 2)])?,
            ),
        );
    }
    push(
        "3eaf5a8b-06d6-47b0-ac7d-a9698247f499",
        shared::typed_value(attribute.child("MaxValue"), obj.cx)?,
    );
    push(
        "4690ff70-e3fa-4914-9127-6a9acc5fc949",
        localized_value("ToolTip"),
    );
    push(
        "4de03908-56f4-4396-a61e-17253afca9ac",
        boolean("ExtendedEdit")?,
    );
    push(
        "580c29e2-8af4-4258-882a-7cf8073e61c8",
        localized_value("Format"),
    );
    push("6c4f7074-e7d4-48eb-b31b-132873666262", choice_form);
    push(
        "6e3a1131-37a3-4da5-8895-572d9d0c9db6",
        nested(
            "ace3fd07-11b2-477e-ab7f-36f0ea37c8dd",
            code("QuickChoice", &[("DontUse", 0), ("Use", 1), ("Auto", 2)])?,
        ),
    );
    push(
        "7ba608f2-e654-42a3-8885-334fe88ca910",
        typed(
            "12ca4003-ac70-450e-b897-37faf86bd313",
            num(code(
                "ChoiceHistoryOnInput",
                &[("Auto", 0), ("DontUse", 1)],
            )?),
        ),
    );
    push(
        "88149a78-9448-4767-867b-0e650d165d2e",
        localized_value("EditFormat"),
    );
    push(
        "90ae4b5d-e0fd-49ef-a008-d67c1e75038c",
        boolean("PasswordMode")?,
    );
    push(
        "9288a8ed-b259-46d0-a8e3-70d87956ff2d",
        nested(
            "d46ea122-3201-4e5e-bed4-e669c6e463c8",
            code("DataHistory", &[("DontUse", 0), ("Use", 1)])?,
        ),
    );
    push(
        "b02800e9-a8d1-42ab-9a12-f673e92be968",
        boolean("MarkNegatives")?,
    );
    push(
        "c65a541f-0b91-4f33-bc88-fbaaa57f9992",
        shared::typed_value(attribute.child("MinValue"), obj.cx)?,
    );
    push(
        "cf4abea3-37b2-11d4-940f-008048da11f9",
        localized_value("Synonym"),
    );
    push("cf4abea4-37b2-11d4-940f-008048da11f9", text("Comment"));
    push(
        "d4232326-022b-421e-b6d3-88e418f74327",
        nested(
            "3b8e6bdd-d648-49d5-af2f-d46d84f87dd5",
            code("FullTextSearch", &[("DontUse", 0), ("Use", 1)])?,
        ),
    );
    push(
        "e3da683b-c54a-457a-a243-b9b4f9bf76dd",
        typed(
            "b76a58b9-2a56-4e46-bb31-8e04ad9f31ae",
            shared::choice_parameter_links(attribute.child("ChoiceParameterLinks"), obj)?,
        ),
    );
    push(
        "e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208",
        shared::typed_value(attribute.child("FillValue"), obj.cx)?,
    );
    push("f49e4ced-4033-4e6c-8755-9fbaaccd6078", text("Mask"));
    push(
        "fcf503b8-1c06-454a-970c-06413e64aee5",
        typed(
            "f2eaae14-91a7-47b9-9d69-097877f41580",
            shared::choice_parameters(attribute.child("ChoiceParameters"), obj.cx)?,
        ),
    );
    Ok(Brace::List(items))
}

/// The platform's command groups, by the names the XML writes.
fn builtin_command_group(name: &str) -> Option<&'static str> {
    Some(match name {
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
        _ => return None,
    })
}

/// The platform pictures a reference stores as a bare negative code.
fn std_picture_code(name: &str) -> Option<i64> {
    Some(match name {
        "InputFieldSelect" => -1,
        "InputFieldClear" => -2,
        "MoveUp" => -3,
        "MoveDown" => -4,
        "InputFieldCalendar" => -5,
        "InputFieldOpen" => -7,
        "MoveLeft" => -8,
        "MoveRight" => -9,
        "CheckAll" => -10,
        "UncheckAll" => -11,
        "Print" => -13,
        "InputFieldChooseType" => -14,
        "ZoomOut" => -15,
        "ZoomIn" => -16,
        "Select" => -100,
        _ => return None,
    })
}

/// `{0,<virtual-key code>,<modifiers>}` of a `<Shortcut>`, Shift 4, Ctrl 8,
/// Alt 16; `{0,0,0}` when empty.
fn shortcut(text: &str) -> Result<Brace> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(brace_list![num(0), num(0), num(0)]);
    }
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
        "Tab" => 9,
        "Enter" => 13,
        "Esc" => 27,
        "Space" => 32,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "Left" => 37,
        "Up" => 38,
        "Right" => 39,
        "Down" => 40,
        "Insert" => 45,
        "Delete" => 46,
        "Num *" => 106,
        "Num +" => 107,
        "Num -" => 109,
        "Num ." => 110,
        "Num /" => 111,
        _ => {
            if let Some(digit) = key.strip_prefix("Num ") {
                96 + digit
                    .parse::<i64>()
                    .ok()
                    .filter(|digit| (0..=9).contains(digit))
                    .ok_or_else(|| anyhow!("unknown shortcut key {key}"))?
            } else if let Some(number) = key.strip_prefix('F').filter(|rest| !rest.is_empty()) {
                111 + number
                    .parse::<i64>()
                    .ok()
                    .filter(|number| (1..=12).contains(number))
                    .ok_or_else(|| anyhow!("unknown shortcut key {key}"))?
            } else {
                let mut chars = key.chars();
                match (chars.next(), chars.next()) {
                    (Some(single), None)
                        if single.is_ascii_uppercase() || single.is_ascii_digit() =>
                    {
                        i64::from(u32::from(single))
                    }
                    _ => bail!("unknown shortcut key {key}"),
                }
            }
        }
    };
    let mut mask = 0;
    if !modifiers.is_empty() {
        for modifier in modifiers.split('+') {
            mask |= match modifier {
                "Shift" => 4,
                "Ctrl" | "Cmd" => 8,
                "Alt" => 16,
                other => bail!("unknown shortcut modifier {other}"),
            };
        }
    }
    Ok(brace_list![num(0), num(code), num(mask)])
}

/// An enum value colour of compatibility 8.5.1: `auto` `{4,4,{0},4}`, a
/// palette colour `{4,4,{<index>},5}`.
fn enum_value_color(text: &str) -> Result<Brace> {
    let text = text.trim();
    if text.is_empty() || text == "auto" {
        return Ok(brace_list![num(4), num(4), brace_list![num(0)], num(4)]);
    }
    let index = match text {
        "pal:FirstBrand" => 0,
        "pal:SecondBrand" => 1,
        "pal:Red" => 2,
        "pal:Orange" => 3,
        "pal:Yellow" => 4,
        "pal:Green" => 5,
        "pal:LightBlue" => 6,
        "pal:Blue" => 7,
        "pal:Gray" => 15,
        other => bail!("unsupported enum value colour {other}"),
    };
    Ok(brace_list![num(4), num(4), brace_list![num(index)], num(5)])
}

/// The seam to the simple-objects track (`types.rs`, `attribute.rs`): type
/// descriptions, typed values, the attribute body and the choice/link
/// properties the standard attributes share with attributes.
pub(crate) mod shared {
    use anyhow::{Result, anyhow};

    use super::Obj;
    use crate::metadata_model::brace::Brace;
    use crate::metadata_model::xml::Element;
    use crate::metadata_model::{DescriptorContext, attribute, types};

    /// `{27,...}` of an attribute-like element.
    pub(crate) fn attribute_body(element: &Element, context: &DescriptorContext) -> Result<Brace> {
        let uuid = element
            .attr("uuid")
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| anyhow!("<{}> has no uuid", element.name))?;
        let properties = element
            .child("Properties")
            .ok_or_else(|| anyhow!("<{}> has no <Properties>", element.name))?;
        attribute::attribute_body(&uuid, properties, context)
    }

    /// `{"Pattern",...}` of a `<Type>`-like element.
    pub(crate) fn type_pattern(
        element: Option<&Element>,
        context: &DescriptorContext,
    ) -> Result<Brace> {
        types::type_pattern(element, context)
    }

    /// A typed value (`FillValue`, `MinValue`, `TypesFilterValue`...).
    pub(crate) fn typed_value(
        element: Option<&Element>,
        context: &DescriptorContext,
    ) -> Result<Brace> {
        types::typed_value(element, context)
    }

    /// `{5006,N,...}` of `<ChoiceParameterLinks>`.
    pub(crate) fn choice_parameter_links(
        element: Option<&Element>,
        obj: &Obj<'_>,
    ) -> Result<Brace> {
        attribute::choice_parameter_links(element, obj.cx)
    }

    /// `{0,N,...}` of `<ChoiceParameters>`.
    pub(crate) fn choice_parameters(
        element: Option<&Element>,
        context: &DescriptorContext,
    ) -> Result<Brace> {
        attribute::choice_parameters(element, context)
    }

    /// `{3,...}` of `<LinkByType>`.
    pub(crate) fn link_by_type(
        element: Option<&Element>,
        context: &DescriptorContext,
    ) -> Result<Brace> {
        attribute::link_by_type(element, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::serialize;

    fn text(node: &Brace) -> String {
        serialize(node).replace("\r\n", "")
    }

    #[test]
    fn shortcuts_are_virtual_key_and_modifier_mask() {
        assert_eq!(text(&shortcut("").unwrap()), "{0,0,0}");
        assert_eq!(text(&shortcut("F3").unwrap()), "{0,114,0}");
        assert_eq!(text(&shortcut("Ctrl+Alt+F").unwrap()), "{0,70,24}");
        assert_eq!(text(&shortcut("Ctrl+Shift+Num +").unwrap()), "{0,107,12}");
        assert!(shortcut("Ctrl+Pause").is_err());
    }

    #[test]
    fn enum_value_colours_are_auto_or_palette() {
        assert_eq!(text(&enum_value_color("auto").unwrap()), "{4,4,{0},4}");
        assert_eq!(text(&enum_value_color("pal:Red").unwrap()), "{4,4,{2},5}");
        assert!(enum_value_color("web:Red").is_err());
    }

    #[test]
    fn compatibility_modes_parse_as_versions() {
        assert_eq!(Compat::parse("Version8_3_24"), Some(Compat(8, 3, 24)));
        assert_eq!(Compat::parse("Version8_5_1"), Some(Compat(8, 5, 1)));
        assert_eq!(Compat::parse("DontUse"), None);
        assert!(Compat(8, 3, 24) < Compat(8, 3, 27));
    }

    #[test]
    fn line_breaks_get_the_stored_crlf_back() {
        let mut tree = brace_list![Brace::str("a\nb\r\nc"), brace_list![Brace::str("x\n")]];
        restore_crlf(&mut tree);
        assert_eq!(
            tree,
            brace_list![Brace::str("a\r\nb\r\nc"), brace_list![Brace::str("x\r\n")]]
        );
    }
}
