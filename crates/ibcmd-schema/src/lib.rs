//! Standalone, versioned schema knowledge derived from public XML behaviour and
//! locally inspected EDT model/export metadata.
//!
//! This crate embeds declarative data only. It neither links to nor starts EDT,
//! Java, OSGi, platform executables, or native libraries.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::sync::OnceLock;

use serde::de::{Error as DeError, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The optional direct `ContextMenu` owned exclusively by a serialized
/// `TextDocumentField`.  The physical adapter supplies the decoded payload so
/// this schema rule owns the wire contract without depending on form parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormTextDocumentContextMenu<T> {
    Absent,
    Present(T),
}

/// Closed failure modes for the `TextDocumentField` direct context-menu slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormTextDocumentContextMenuParseError {
    WrongWrapper,
    WrongDiscriminator,
    InvalidMultiplicity,
    MissingPayload,
    Duplicate,
    ForeignChild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormTextDocumentContextMenuMultiplicity {
    Absent,
    Present,
}

pub fn parse_form_text_document_context_menu_multiplicity(
    value: &str,
) -> Result<FormTextDocumentContextMenuMultiplicity, FormTextDocumentContextMenuParseError> {
    match value.trim() {
        "0" => Ok(FormTextDocumentContextMenuMultiplicity::Absent),
        "1" => Ok(FormTextDocumentContextMenuMultiplicity::Present),
        value if value.parse::<u64>().is_ok_and(|count| count > 1) && !value.starts_with('0') => {
            Err(FormTextDocumentContextMenuParseError::Duplicate)
        }
        _ => Err(FormTextDocumentContextMenuParseError::InvalidMultiplicity),
    }
}

pub fn form_text_document_context_menu_owner_fields(fields: &[&str]) -> bool {
    fields.first().map(|field| field.trim()) == Some("48")
        && fields.get(5).map(|field| field.trim()) == Some("7")
}

/// Return the count/payload slots for direct, schema-owned form children.
///
/// Wrapper 48 exposes these slots only for a `TextDocumentField`
/// (discriminator 7). Wrapper 6 is the existing direct-child layout used by a
/// search addition. Keeping this table in the schema layer prevents the binary
/// packer from applying wrapper-48 slots to unrelated field kinds.
pub fn form_layout_single_child_item_slot_indices(
    wrapper: &str,
    discriminator: Option<&str>,
) -> Option<(usize, usize)> {
    match (wrapper.trim(), discriminator.map(str::trim)) {
        ("48", Some("7")) => Some((41, 42)),
        ("6", _) => Some((15, 16)),
        _ => None,
    }
}

/// Decode the direct `ContextMenu` slots of a `TextDocumentField` (`48`, `7`).
///
/// Slot 41 is a strict 0/1 multiplicity, slot 42 is required only for the
/// present state, and the adapter-provided resolver must accept only a genuine
/// `ContextMenu` child.  This deliberately rejects the similarly-shaped slots
/// of every other wrapper-48 item.
pub fn parse_form_text_document_context_menu<T, F>(
    fields: &[&str],
    mut resolve_context_menu: F,
) -> Result<FormTextDocumentContextMenu<T>, FormTextDocumentContextMenuParseError>
where
    F: FnMut(&str) -> Option<T>,
{
    if fields.first().map(|field| field.trim()) != Some("48") {
        return Err(FormTextDocumentContextMenuParseError::WrongWrapper);
    }
    if fields.get(5).map(|field| field.trim()) != Some("7") {
        return Err(FormTextDocumentContextMenuParseError::WrongDiscriminator);
    }
    match parse_form_text_document_context_menu_multiplicity(
        fields
            .get(41)
            .map(|field| field.trim())
            .ok_or(FormTextDocumentContextMenuParseError::InvalidMultiplicity)?,
    )? {
        FormTextDocumentContextMenuMultiplicity::Absent => Ok(FormTextDocumentContextMenu::Absent),
        FormTextDocumentContextMenuMultiplicity::Present => {
            let payload = fields
                .get(42)
                .ok_or(FormTextDocumentContextMenuParseError::MissingPayload)?;
            resolve_context_menu(payload)
                .map(FormTextDocumentContextMenu::Present)
                .ok_or(FormTextDocumentContextMenuParseError::ForeignChild)
        }
    }
}

/// Parsed, schema-owned representation of a Form `InputField.choiceParameters`
/// raw slot.  The grammar is intentionally closed: values which are not one of
/// the documented boolean, design-time-reference, or fixed-array shapes are
/// rejected rather than partially interpreted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceParameters {
    items: Vec<FormChoiceParameter>,
}

impl FormChoiceParameters {
    pub fn items(&self) -> &[FormChoiceParameter] {
        &self.items
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceParameter {
    name: String,
    presentation: Vec<(String, String)>,
    value: FormChoiceParameterValue,
}

impl FormChoiceParameter {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn presentation(&self) -> &[(String, String)] {
        &self.presentation
    }
    pub fn value(&self) -> &FormChoiceParameterValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterValue {
    Boolean(bool),
    DesignTimeRef(String),
    FixedArray(Vec<FormChoiceParameterArrayItem>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceParameterArrayItem {
    presentation: Vec<(String, String)>,
    value: FormChoiceParameterArrayItemValue,
}

/// Typed value of a FixedArray element.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterArrayItemValue {
    DesignTimeRef(String),
    String(String),
}

/// Canonical EDT Form choice-parameter link. Physical source slots are
/// deliberately excluded from this model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceParameterLink {
    name: String,
    data_path: String,
    value_change: FormChoiceParameterLinkValueChange,
}

impl FormChoiceParameterLink {
    pub fn new(
        name: String,
        data_path: String,
        value_change: FormChoiceParameterLinkValueChange,
    ) -> Self {
        Self {
            name,
            data_path,
            value_change,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data_path(&self) -> &str {
        &self.data_path
    }

    pub fn value_change(&self) -> FormChoiceParameterLinkValueChange {
        self.value_change
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinkValueChange {
    Clear,
    DontChange,
}

impl FormChoiceParameterLinkValueChange {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::DontChange => "DontChange",
        }
    }

    fn from_raw_code(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::Clear),
            "1" => Some(Self::DontChange),
            _ => None,
        }
    }
}

/// Fail-closed result of decoding the mirrored raw choice-parameter-link
/// collections. `Opaque` is selected by the physical adapter and preserves its
/// provenance without exposing physical slots through the canonical link type.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum FormChoiceParameterLinks<O> {
    #[default]
    Absent,
    Empty,
    Typed(Vec<FormChoiceParameterLink>),
    Opaque(O),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinksParseError {
    PrimaryMalformed,
    DuplicateMalformed,
    MirrorMismatch,
    UnresolvedAttribute(String),
}

/// The terminal portion of a physical choice-parameter link.
///
/// `Absent` is the mode-1 form. Mode 2 is either one of the two platform
/// standard markers or a metadata UUID owned by the form attribute. UUIDs are
/// accepted only in canonical lower-case, non-nil form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinkTerminal {
    Absent,
    Standard(FormChoiceParameterLinkStandardTerminal),
    MetadataUuid(String),
}

/// The physical owner/reference carried by a choice-parameter link.
///
/// `TableCurrentData` is intentionally separate from `MetadataUuid`: its UUID
/// identifies the fixed Form item type in the owner wrapper, while its table
/// and column bindings are canonical positive numeric ids.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinkReference {
    FormAttribute {
        attribute_id: String,
        terminal: FormChoiceParameterLinkTerminal,
    },
    TableCurrentData {
        table_id: u64,
        terminal: FormChoiceParameterLinkTableCurrentDataTerminal,
    },
}

/// Terminal carried by a `TableCurrentData` choice-parameter link.
///
/// Binding ids and metadata UUIDs are separate native wire shapes and must not
/// be conflated with a form-attribute `MetadataUuid` terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinkTableCurrentDataTerminal {
    BindingId(u64),
    MetadataUuid(String),
    BindingUuid { binding_id: u64, uuid: String },
}

/// The exact native Form item type used by `TableCurrentData` links.
pub const FORM_CHOICE_PARAMETER_LINK_TABLE_CURRENT_DATA_ITEM_TYPE: &str =
    "02023637-7868-4a5f-8576-835a76e0c9ba";

/// Standard terminal markers carried by mode-2 choice-parameter links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterLinkStandardTerminal {
    Date,
    Owner,
    Ref,
}

impl FormChoiceParameterLinkStandardTerminal {
    const fn data_path_suffix(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Owner => "Owner",
            Self::Ref => "Ref",
        }
    }
}

/// The three EDT-owned members of the Form choice-parameter cluster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormChoiceParameterClusterMember {
    Links,
    Parameters,
    AvailableTypes,
}

impl FormChoiceParameterClusterMember {
    pub const fn xml_local_name(self) -> &'static str {
        match self {
            Self::Links => "ChoiceParameterLinks",
            Self::Parameters => "ChoiceParameters",
            Self::AvailableTypes => "AvailableTypes",
        }
    }
}

/// Explicit state for `AvailableTypes`. The `Typed` payload is intentionally
/// generic because no native wire-format mapping is currently proven.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum FormChoiceParameterAvailableTypes<T, O> {
    #[default]
    Absent,
    Typed(T),
    Opaque(O),
}

/// Schema-owned Form orchestration model. Physical provenance belongs in the
/// caller-selected parameter and opaque payload types, not in the link model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceParameterCluster<L, P, A> {
    links: L,
    parameters: P,
    available_types: A,
}

impl<L, P, A> FormChoiceParameterCluster<L, P, A> {
    pub fn new(links: L, parameters: P, available_types: A) -> Self {
        Self {
            links,
            parameters,
            available_types,
        }
    }

    pub fn links(&self) -> &L {
        &self.links
    }

    pub fn links_mut(&mut self) -> &mut L {
        &mut self.links
    }

    pub fn parameters(&self) -> &P {
        &self.parameters
    }

    pub fn parameters_mut(&mut self) -> &mut P {
        &mut self.parameters
    }

    pub fn available_types(&self) -> &A {
        &self.available_types
    }

    pub fn available_types_mut(&mut self) -> &mut A {
        &mut self.available_types
    }
}

/// Physical envelope profile for a Form ChoiceList raw value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormChoiceListLayoutProfile {
    InputFieldExtendedOptions,
    RadioButtonOptions,
}

/// Stable schema-owned identity for an opaque ChoiceList source diagnostic.
/// The raw payload is intentionally excluded from this value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceListOpaqueDiagnosticIdentity {
    code: &'static str,
    classification: &'static str,
    property: &'static str,
    profile: &'static str,
}

impl FormChoiceListOpaqueDiagnosticIdentity {
    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn classification(&self) -> &'static str {
        self.classification
    }

    pub const fn property(&self) -> &'static str {
        self.property
    }

    pub const fn profile(&self) -> &'static str {
        self.profile
    }
}

/// Bounded opaque ChoiceList diagnostic evidence without raw payload storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceListOpaqueDiagnostic {
    identity: FormChoiceListOpaqueDiagnosticIdentity,
    raw_length: usize,
    raw_sha256: String,
}

impl FormChoiceListOpaqueDiagnostic {
    pub const fn identity(&self) -> &FormChoiceListOpaqueDiagnosticIdentity {
        &self.identity
    }

    pub const fn raw_length(&self) -> usize {
        self.raw_length
    }

    pub fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }
}

impl FormChoiceListLayoutProfile {
    /// Produce deterministic, non-recoverable diagnostic evidence for an
    /// opaque raw ChoiceList value in this physical layout.
    pub fn opaque_diagnostic(self, raw: &str) -> FormChoiceListOpaqueDiagnostic {
        let profile = match self {
            Self::InputFieldExtendedOptions => "input_field_extended_options",
            Self::RadioButtonOptions => "radio_button_options",
        };
        FormChoiceListOpaqueDiagnostic {
            identity: FormChoiceListOpaqueDiagnosticIdentity {
                code: "source_asset.form.choice_list.opaque_asset_not_emitted",
                classification: "opaque_asset_not_emitted",
                property: "ChoiceList",
                profile,
            },
            raw_length: raw.len(),
            raw_sha256: format!("{:x}", Sha256::digest(raw.as_bytes())),
        }
    }
}

/// Schema-owned, fully decoded ChoiceList model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceList {
    layout: FormChoiceListLayoutProfile,
    items: Vec<FormChoiceListItem>,
    empty_sidecar_proof: FormChoiceListEmptySidecarProof,
}

impl FormChoiceList {
    pub fn layout(&self) -> FormChoiceListLayoutProfile {
        self.layout
    }

    pub fn items(&self) -> &[FormChoiceListItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn empty_sidecar_proof(&self) -> &FormChoiceListEmptySidecarProof {
        &self.empty_sidecar_proof
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceListEmptySidecarProof {
    count: usize,
}

impl FormChoiceListEmptySidecarProof {
    pub fn count(&self) -> usize {
        self.count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormChoiceListItem {
    pub presentation_present: bool,
    pub presentation: Vec<(String, String)>,
    pub value: FormChoiceListValue,
}

impl FormChoiceListItem {
    pub fn presentation_present(&self) -> bool {
        self.presentation_present
    }

    pub fn presentation(&self) -> &[(String, String)] {
        &self.presentation
    }

    pub fn value(&self) -> &FormChoiceListValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormChoiceListValue {
    Boolean(bool),
    Decimal(String),
    Nil,
    String(String),
    EmptyRef(String),
    LiteralDesignTimeRef(String),
    DesignTimeRef(String),
}

/// Canonical XML leaf shape for a decoded `FormChoiceListValue`.
///
/// This is deliberately a data-only wire contract: the physical form adapter
/// may escape and serialize it, but it must not decide which QName, nil form,
/// or empty-element form belongs to a source value variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormChoiceListValueWireShape<'a> {
    xml_opening: &'static str,
    xml_closing: &'static str,
    text: Option<&'a str>,
}

impl<'a> FormChoiceListValueWireShape<'a> {
    pub const fn xml_opening(&self) -> &'static str {
        self.xml_opening
    }

    pub const fn xml_closing(&self) -> &'static str {
        self.xml_closing
    }

    pub const fn text(&self) -> Option<&'a str> {
        self.text
    }

    /// Append the canonical element using the caller's XML escaping function.
    /// Escaping is transport-specific; tag names and element shape are not.
    pub fn append_xml_escaped<F>(&self, output: &mut String, escape: F)
    where
        F: FnOnce(&str) -> String,
    {
        output.push_str(self.xml_opening);
        if let Some(text) = self.text {
            output.push_str(&escape(text));
        }
        output.push_str(self.xml_closing);
    }
}

impl FormChoiceListValue {
    /// Return the exhaustive, verified XML wire shape for this source value.
    /// All reference source variants intentionally share one DesignTimeRef
    /// shape: their distinction is parser provenance, not XML syntax.
    pub fn wire_shape(&self) -> FormChoiceListValueWireShape<'_> {
        match self {
            Self::Boolean(value) => FormChoiceListValueWireShape {
                xml_opening: "<Value xsi:type=\"xs:boolean\">",
                xml_closing: "</Value>",
                text: Some(if *value { "true" } else { "false" }),
            },
            Self::Decimal(value) => FormChoiceListValueWireShape {
                xml_opening: "<Value xsi:type=\"xs:decimal\">",
                xml_closing: "</Value>",
                text: Some(value),
            },
            Self::Nil => FormChoiceListValueWireShape {
                xml_opening: "<Value xsi:nil=\"true\"/>",
                xml_closing: "",
                text: None,
            },
            Self::String(value) => FormChoiceListValueWireShape {
                xml_opening: if value.is_empty() {
                    "<Value xsi:type=\"xs:string\"/>"
                } else {
                    "<Value xsi:type=\"xs:string\">"
                },
                xml_closing: if value.is_empty() { "" } else { "</Value>" },
                text: (!value.is_empty()).then_some(value),
            },
            Self::EmptyRef(value)
            | Self::LiteralDesignTimeRef(value)
            | Self::DesignTimeRef(value) => FormChoiceListValueWireShape {
                xml_opening: "<Value xsi:type=\"xr:DesignTimeRef\">",
                xml_closing: "</Value>",
                text: Some(value),
            },
        }
    }
}

const FORM_CHOICE_LIST_ITEM_DISCRIMINATOR: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";
const MAX_FORM_CHOICE_LIST_RAW_BYTES: usize = 64 * 1024;
const MAX_FORM_CHOICE_LIST_ITEMS: usize = 512;
const MAX_FORM_CHOICE_LIST_PRESENTATION_ITEMS: usize = 128;

/// Decode one complete ChoiceList envelope. Domain reference resolution is
/// deliberately supplied by the caller; all physical grammar remains here.
pub fn parse_form_choice_list<FO, FR>(
    raw: &str,
    layout: FormChoiceListLayoutProfile,
    mut resolve_empty_ref_owner: FO,
    mut resolve_reference: FR,
) -> Option<FormChoiceList>
where
    FO: FnMut(&str) -> Option<String>,
    FR: FnMut(&str, &str) -> Option<String>,
{
    if raw.len() > MAX_FORM_CHOICE_LIST_RAW_BYTES {
        return None;
    }
    let fields = braced_fields_bounded(raw, MAX_FORM_CHOICE_LIST_ITEMS * 3 + 2)?;
    if fields.first()?.trim() != "3" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count > MAX_FORM_CHOICE_LIST_ITEMS {
        return None;
    }
    let item_fields_end = count.checked_mul(2)?.checked_add(2)?;
    if fields.len() != item_fields_end.checked_add(count)? {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        if exact_1c_string(fields[2 + index * 2])?.as_str() != "" {
            return None;
        }
        items.push(parse_form_choice_list_item_inner(
            fields[3 + index * 2],
            layout,
            &mut resolve_empty_ref_owner,
            &mut resolve_reference,
        )?);
    }
    for sidecar in &fields[item_fields_end..] {
        parse_form_choice_list_empty_sidecar(sidecar)?;
    }
    Some(FormChoiceList {
        layout,
        items,
        empty_sidecar_proof: FormChoiceListEmptySidecarProof { count },
    })
}

/// Decode one ChoiceList item under an explicit physical layout profile.
pub fn parse_form_choice_list_item<FO, FR>(
    raw: &str,
    layout: FormChoiceListLayoutProfile,
    mut resolve_empty_ref_owner: FO,
    mut resolve_reference: FR,
) -> Option<FormChoiceListItem>
where
    FO: FnMut(&str) -> Option<String>,
    FR: FnMut(&str, &str) -> Option<String>,
{
    if raw.len() > MAX_FORM_CHOICE_LIST_RAW_BYTES {
        return None;
    }
    parse_form_choice_list_item_inner(
        raw,
        layout,
        &mut resolve_empty_ref_owner,
        &mut resolve_reference,
    )
}

fn parse_form_choice_list_item_inner<FO, FR>(
    raw: &str,
    layout: FormChoiceListLayoutProfile,
    resolve_empty_ref_owner: &mut FO,
    resolve_reference: &mut FR,
) -> Option<FormChoiceListItem>
where
    FO: FnMut(&str) -> Option<String>,
    FR: FnMut(&str, &str) -> Option<String>,
{
    let fields = braced_fields_bounded(raw, 3)?;
    if fields.len() != 3
        || exact_1c_string(fields[0])?.as_str() != "#"
        || fields[1].trim() != FORM_CHOICE_LIST_ITEM_DISCRIMINATOR
    {
        return None;
    }
    let payload = braced_fields_bounded(fields[2], 6)?;
    let [zero, mode, raw_value, type_id, value_id, presentation] = payload.as_slice() else {
        return None;
    };
    if zero.trim() != "0" {
        return None;
    }
    let value_fields = braced_fields_bounded(raw_value, 2)?;
    let kind = exact_1c_string(value_fields.first()?)?;
    let nil = Uuid::nil();
    let value = match (kind.as_str(), value_fields.as_slice()) {
        ("N", [_, decimal])
            if mode.trim() == "1"
                && ids_are(type_id, value_id, nil, nil)
                && decimal_is_valid(decimal.trim()) =>
        {
            FormChoiceListValue::Decimal(decimal.trim().to_owned())
        }
        ("S", [_, string]) if mode.trim() == "1" && ids_are(type_id, value_id, nil, nil) => {
            FormChoiceListValue::String(exact_1c_string(string)?)
        }
        ("B", [_, boolean])
            if layout == FormChoiceListLayoutProfile::InputFieldExtendedOptions
                && mode.trim() == "1"
                && ids_are(type_id, value_id, nil, nil) =>
        {
            match boolean.trim() {
                "0" => FormChoiceListValue::Boolean(false),
                "1" => FormChoiceListValue::Boolean(true),
                _ => return None,
            }
        }
        ("U", [_]) => {
            let type_uuid = Uuid::parse_str(type_id.trim()).ok()?;
            let value_uuid = Uuid::parse_str(value_id.trim()).ok()?;
            match (mode.trim(), type_uuid.is_nil(), value_uuid.is_nil()) {
                ("1", true, true) | ("0", true, true)
                    if layout == FormChoiceListLayoutProfile::InputFieldExtendedOptions =>
                {
                    FormChoiceListValue::Nil
                }
                ("1", true, true) => FormChoiceListValue::Nil,
                ("0", false, true) => {
                    if let Some(owner) = resolve_empty_ref_owner(type_id.trim()) {
                        FormChoiceListValue::EmptyRef(format!("{owner}.EmptyRef"))
                    } else if layout == FormChoiceListLayoutProfile::InputFieldExtendedOptions {
                        FormChoiceListValue::LiteralDesignTimeRef(format!(
                            "{type_uuid}.{value_uuid}"
                        ))
                    } else {
                        return None;
                    }
                }
                ("0", false, false) => {
                    if let Some(reference) = resolve_reference(type_id.trim(), value_id.trim()) {
                        FormChoiceListValue::DesignTimeRef(reference)
                    } else {
                        FormChoiceListValue::LiteralDesignTimeRef(format!(
                            "{type_uuid}.{value_uuid}"
                        ))
                    }
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let presentation = parse_form_choice_list_presentation(presentation)?;
    Some(FormChoiceListItem {
        presentation_present: layout == FormChoiceListLayoutProfile::InputFieldExtendedOptions,
        presentation,
        value,
    })
}

fn parse_form_choice_list_presentation(raw: &str) -> Option<Vec<(String, String)>> {
    let fields =
        braced_fields_bounded(raw, MAX_FORM_CHOICE_LIST_PRESENTATION_ITEMS.checked_add(2)?)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count > MAX_FORM_CHOICE_LIST_PRESENTATION_ITEMS || fields.len() != count.checked_add(2)? {
        return None;
    }
    fields[2..]
        .iter()
        .map(|entry| {
            let values = braced_fields_bounded(entry, 2)?;
            match values.as_slice() {
                [language, content] => {
                    Some((exact_1c_string(language)?, exact_1c_string(content)?))
                }
                _ => None,
            }
        })
        .collect()
}

fn parse_form_choice_list_empty_sidecar(raw: &str) -> Option<()> {
    let fields = braced_fields_bounded(raw, 2)?;
    let [outer_flag, descriptor] = fields.as_slice() else {
        return None;
    };
    if outer_flag.trim() != "0" {
        return None;
    }
    let descriptor = braced_fields_bounded(descriptor, 9)?;
    let [
        kind,
        mode,
        picture,
        first_empty,
        first_offset,
        second_offset,
        enabled,
        trailing_flag,
        second_empty,
    ] = descriptor.as_slice()
    else {
        return None;
    };
    let picture = braced_fields_bounded(picture, 1)?;
    (kind.trim() == "4"
        && mode.trim() == "0"
        && matches!(picture.as_slice(), [empty] if empty.trim() == "0")
        && exact_1c_string(first_empty)?.is_empty()
        && first_offset.trim() == "-1"
        && second_offset.trim() == "-1"
        && enabled.trim() == "1"
        && trailing_flag.trim() == "0"
        && exact_1c_string(second_empty)?.is_empty())
    .then_some(())
}

fn ids_are(type_id: &str, value_id: &str, expected_type: Uuid, expected_value: Uuid) -> bool {
    Uuid::parse_str(type_id.trim()).ok() == Some(expected_type)
        && Uuid::parse_str(value_id.trim()).ok() == Some(expected_value)
}

fn decimal_is_valid(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let Some(integer) = parts.next() else {
        return false;
    };
    if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(fraction), None) => {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
        }
        _ => false,
    }
}

#[cfg(test)]
mod form_choice_list_tests {
    use super::*;

    const DISC: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";
    const NIL: &str = "00000000-0000-0000-0000-000000000000";
    const TYPE_ID: &str = "11111111-1111-4111-8111-111111111111";
    const VALUE_ID: &str = "22222222-2222-4222-8222-222222222222";
    const SIDECAR: &str = r#"{0,{4,0,{0},"",-1,-1,1,0,""}}"#;

    fn item(mode: &str, value: &str, type_id: &str, value_id: &str) -> String {
        format!(
            r##"{{"#",{DISC},{{0,{mode},{value},{type_id},{value_id},{{1,1,{{"en","Value"}}}}}}}}"##
        )
    }

    fn envelope(item: &str) -> String {
        format!(r#"{{3,1,"",{item},{SIDECAR}}}"#)
    }

    fn parse(raw: &str, layout: FormChoiceListLayoutProfile) -> Option<FormChoiceList> {
        parse_form_choice_list(
            raw,
            layout,
            |type_id| (type_id == TYPE_ID).then(|| "Enum.Kind".to_owned()),
            |type_id, value_id| {
                (type_id == TYPE_ID && value_id == VALUE_ID)
                    .then(|| "Enum.Kind.EnumValue.Value".to_owned())
            },
        )
    }

    #[test]
    fn both_layouts_decode_values_and_record_empty_sidecar_proof() {
        for (value, expected) in [
            (
                r#"{"N",-12.50}"#,
                FormChoiceListValue::Decimal("-12.50".to_owned()),
            ),
            (
                r#"{"S","text"}"#,
                FormChoiceListValue::String("text".to_owned()),
            ),
        ] {
            let parsed = parse(
                &envelope(&item("1", value, NIL, NIL)),
                FormChoiceListLayoutProfile::RadioButtonOptions,
            )
            .unwrap();
            assert_eq!(parsed.items()[0].value(), &expected);
            assert_eq!(parsed.empty_sidecar_proof().count(), 1);
            assert!(!parsed.items()[0].presentation_present());
        }

        let boolean = parse(
            &envelope(&item("1", r#"{"B",1}"#, NIL, NIL)),
            FormChoiceListLayoutProfile::InputFieldExtendedOptions,
        )
        .unwrap();
        assert_eq!(
            boolean.items()[0].value(),
            &FormChoiceListValue::Boolean(true)
        );
        assert!(boolean.items()[0].presentation_present());

        let empty = parse(
            "{3,0}",
            FormChoiceListLayoutProfile::InputFieldExtendedOptions,
        )
        .unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.empty_sidecar_proof().count(), 0);
    }

    #[test]
    fn nil_empty_ref_and_reference_variants_are_profile_exact() {
        let nil = parse(
            &envelope(&item("1", r#"{"U"}"#, NIL, NIL)),
            FormChoiceListLayoutProfile::RadioButtonOptions,
        )
        .unwrap();
        assert_eq!(nil.items()[0].value(), &FormChoiceListValue::Nil);

        let empty_ref = parse(
            &envelope(&item("0", r#"{"U"}"#, TYPE_ID, NIL)),
            FormChoiceListLayoutProfile::RadioButtonOptions,
        )
        .unwrap();
        assert_eq!(
            empty_ref.items()[0].value(),
            &FormChoiceListValue::EmptyRef("Enum.Kind.EmptyRef".to_owned())
        );

        let reference = parse(
            &envelope(&item("0", r#"{"U"}"#, TYPE_ID, VALUE_ID)),
            FormChoiceListLayoutProfile::RadioButtonOptions,
        )
        .unwrap();
        assert_eq!(
            reference.items()[0].value(),
            &FormChoiceListValue::DesignTimeRef("Enum.Kind.EnumValue.Value".to_owned())
        );

        let literal = parse_form_choice_list(
            &envelope(&item("0", r#"{"U"}"#, TYPE_ID, VALUE_ID)),
            FormChoiceListLayoutProfile::InputFieldExtendedOptions,
            |_| None,
            |_, _| None,
        )
        .unwrap();
        assert_eq!(
            literal.items()[0].value(),
            &FormChoiceListValue::LiteralDesignTimeRef(format!("{TYPE_ID}.{VALUE_ID}"))
        );
    }

    #[test]
    fn choice_list_value_wire_shapes_are_exhaustive_and_reference_variants_match() {
        let cases = [
            (
                FormChoiceListValue::Boolean(false),
                "<Value xsi:type=\"xs:boolean\">",
                Some("false"),
                "</Value>",
            ),
            (
                FormChoiceListValue::Boolean(true),
                "<Value xsi:type=\"xs:boolean\">",
                Some("true"),
                "</Value>",
            ),
            (
                FormChoiceListValue::Decimal("-12.50".to_owned()),
                "<Value xsi:type=\"xs:decimal\">",
                Some("-12.50"),
                "</Value>",
            ),
            (
                FormChoiceListValue::Nil,
                "<Value xsi:nil=\"true\"/>",
                None,
                "",
            ),
            (
                FormChoiceListValue::String(String::new()),
                "<Value xsi:type=\"xs:string\"/>",
                None,
                "",
            ),
            (
                FormChoiceListValue::String("text".to_owned()),
                "<Value xsi:type=\"xs:string\">",
                Some("text"),
                "</Value>",
            ),
        ];
        for (value, opening, text, closing) in cases {
            let shape = value.wire_shape();
            assert_eq!(shape.xml_opening(), opening);
            assert_eq!(shape.text(), text);
            assert_eq!(shape.xml_closing(), closing);
        }

        let reference_shapes = [
            FormChoiceListValue::EmptyRef("Catalog.Status.EmptyRef".to_owned()),
            FormChoiceListValue::LiteralDesignTimeRef("type.value".to_owned()),
            FormChoiceListValue::DesignTimeRef("Catalog.Status.EnumValue.Active".to_owned()),
        ]
        .map(|value| {
            let shape = value.wire_shape();
            (shape.xml_opening(), shape.xml_closing())
        });
        assert_eq!(
            reference_shapes,
            [("<Value xsi:type=\"xr:DesignTimeRef\">", "</Value>"); 3]
        );
    }

    #[test]
    fn malformed_layout_count_uuid_discriminator_owner_and_sidecar_fail_closed() {
        let exact_item = item("0", r#"{"U"}"#, TYPE_ID, VALUE_ID);
        for raw in [
            envelope(&exact_item).replacen("{3,", "{9,", 1),
            envelope(&exact_item).replace(DISC, "33333333-3333-4333-8333-333333333333"),
            envelope(&item("0", r#"{"U"}"#, "not-a-uuid", VALUE_ID)),
            envelope(&item("0", r#"{"U"}"#, TYPE_ID, "not-a-uuid")),
            format!(r#"{{3,1,"",{exact_item}}}"#),
            format!(r#"{{3,1,"",{exact_item},{{0,{{4,0,{{0}},"",-1,-1,0,0,""}}}}}}"#),
            r#"{3,513}"#.to_string(),
            format!(r#"{}suffix"#, envelope(&exact_item)),
        ] {
            assert!(
                parse(&raw, FormChoiceListLayoutProfile::RadioButtonOptions).is_none(),
                "{raw}"
            );
        }

        let empty_ref = envelope(&item("0", r#"{"U"}"#, TYPE_ID, NIL));
        assert!(
            parse_form_choice_list(
                &empty_ref,
                FormChoiceListLayoutProfile::RadioButtonOptions,
                |_| None,
                |_, _| None,
            )
            .is_none()
        );
    }

    #[test]
    fn raw_and_nested_counts_are_bounded_before_allocation() {
        assert!(
            parse(
                &"x".repeat(MAX_FORM_CHOICE_LIST_RAW_BYTES + 1),
                FormChoiceListLayoutProfile::InputFieldExtendedOptions,
            )
            .is_none()
        );
        let oversized_presentation = format!(
            r##"{{"#",{DISC},{{0,1,{{"S","x"}},{NIL},{NIL},{{1,{}}}}}}}"##,
            MAX_FORM_CHOICE_LIST_PRESENTATION_ITEMS + 1
        );
        assert!(
            parse(
                &envelope(&oversized_presentation),
                FormChoiceListLayoutProfile::InputFieldExtendedOptions,
            )
            .is_none()
        );
    }
}

impl FormChoiceParameterArrayItem {
    pub fn presentation(&self) -> &[(String, String)] {
        &self.presentation
    }
    pub fn value_ref(&self) -> &str {
        match &self.value {
            FormChoiceParameterArrayItemValue::DesignTimeRef(value)
            | FormChoiceParameterArrayItemValue::String(value) => value,
        }
    }
    pub fn value(&self) -> &FormChoiceParameterArrayItemValue {
        &self.value
    }
}

const FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";
const FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE: &str = "4500381b-db30-4a10-9db4-990038032acf";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_FORM_CHOICE_PARAMETERS_RAW_BYTES: usize = 64 * 1024;
const MAX_FORM_CHOICE_PARAMETERS_ITEMS: usize = 512;
const MAX_FORM_CHOICE_PARAMETERS_PRESENTATION_ITEMS: usize = 128;
const MAX_FORM_CHOICE_PARAMETERS_FIXED_ARRAY_ITEMS: usize = 512;
const MAX_FORM_CHOICE_PARAMETER_LINKS: usize = 512;

/// Render a verified Form choice-parameters QName from strict Clark notation.
///
/// Only namespaces used by the committed writer policy are accepted. The
/// local name remains deliberately generic, but is restricted to portable XML
/// name characters so policy data cannot introduce markup or a foreign prefix.
pub fn canonical_form_choice_parameters_qname(qname: &str) -> Result<String, SchemaError> {
    let Some(rest) = qname.strip_prefix('{') else {
        return Err(SchemaError::InvalidFormChoiceParametersQName(
            qname.to_owned(),
        ));
    };
    let Some((namespace, local_name)) = rest.split_once('}') else {
        return Err(SchemaError::InvalidFormChoiceParametersQName(
            qname.to_owned(),
        ));
    };
    let mut characters = local_name.chars();
    if !matches!(characters.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        || !characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(SchemaError::InvalidFormChoiceParametersQName(
            qname.to_owned(),
        ));
    }
    let prefix = match namespace {
        "" | "http://v8.1c.ru/8.3/xcf/logform" => "",
        "http://v8.1c.ru/8.2/managed-application/core" => "app:",
        "http://v8.1c.ru/8.1/data/core" => "v8:",
        _ => {
            return Err(SchemaError::InvalidFormChoiceParametersQName(
                qname.to_owned(),
            ));
        }
    };
    Ok(format!("{prefix}{local_name}"))
}

/// Resolve and verify the EDT-owned cluster order from the exact writer policy.
///
/// Known QNames in a mutated order are rejected instead of being silently
/// reordered, keeping the committed evidence fail-closed.
pub fn form_choice_parameter_cluster_order(
    policy: &WriterPolicy,
) -> Result<[FormChoiceParameterClusterMember; 3], SchemaError> {
    let WriterPolicy::FormChoiceParameters {
        owner_qname,
        owner_predecessor_qname,
        owner_successor_qname,
        ..
    } = policy
    else {
        return Err(SchemaError::InvalidFormChoiceParameterClusterPolicy(
            "writer policy kind".to_owned(),
        ));
    };
    let member = |qname: &str| {
        let local_name = canonical_form_choice_parameters_qname(qname)?;
        match local_name.as_str() {
            "ChoiceParameterLinks" => Ok(FormChoiceParameterClusterMember::Links),
            "ChoiceParameters" => Ok(FormChoiceParameterClusterMember::Parameters),
            "AvailableTypes" => Ok(FormChoiceParameterClusterMember::AvailableTypes),
            _ => Err(SchemaError::InvalidFormChoiceParameterClusterPolicy(
                local_name,
            )),
        }
    };
    let order = [
        member(owner_predecessor_qname)?,
        member(owner_qname)?,
        member(owner_successor_qname)?,
    ];
    let expected = [
        FormChoiceParameterClusterMember::Links,
        FormChoiceParameterClusterMember::Parameters,
        FormChoiceParameterClusterMember::AvailableTypes,
    ];
    if order != expected {
        return Err(SchemaError::InvalidFormChoiceParameterClusterPolicy(
            "owner feature order".to_owned(),
        ));
    }
    Ok(order)
}

/// Decode the raw `ChoiceParameters` envelope. `resolve_design_time_ref` is
/// deliberately the only domain hook: it receives the raw type/value IDs and
/// must return a canonical reference. A missing resolution rejects the entire
/// value, preventing an unsupported value from being emitted as a guess.
pub fn parse_form_choice_parameters<F>(
    raw: &str,
    mut resolve_design_time_ref: F,
) -> Option<FormChoiceParameters>
where
    F: FnMut(&str, &str) -> Option<String>,
{
    if raw.len() > MAX_FORM_CHOICE_PARAMETERS_RAW_BYTES {
        return None;
    }
    let fields = braced_fields_bounded(raw, MAX_FORM_CHOICE_PARAMETERS_ITEMS * 2 + 2)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count > MAX_FORM_CHOICE_PARAMETERS_ITEMS {
        return None;
    }
    if fields.len() != count.checked_mul(2)?.checked_add(2)? {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for pair in fields[2..].chunks_exact(2) {
        let name = exact_1c_string(pair[0])?;
        let (presentation, value) =
            parse_form_choice_parameter_value(pair[1], &mut resolve_design_time_ref)?;
        items.push(FormChoiceParameter {
            name,
            presentation,
            value,
        });
    }
    Some(FormChoiceParameters { items })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RawFormChoiceParameterLink {
    name: String,
    reference: FormChoiceParameterLinkReference,
    value_change: FormChoiceParameterLinkValueChange,
}

/// Decode and compare the native mirrored `5006`/`5007` link collections.
///
/// Both collections are parsed independently with exact count, arity, wrapper,
/// terminal, and value-change grammar. Attribute resolution is the only caller
/// hook. Any malformed, foreign, or mismatched input rejects the whole value.
pub fn parse_form_choice_parameter_links<F>(
    primary: &str,
    duplicate: &str,
    mut resolve_attribute: F,
) -> Result<Vec<FormChoiceParameterLink>, FormChoiceParameterLinksParseError>
where
    F: FnMut(&str) -> Option<String>,
{
    let primary = parse_raw_form_choice_parameter_links(primary, "5006", false)
        .ok_or(FormChoiceParameterLinksParseError::PrimaryMalformed)?;
    // This compatibility entrypoint deliberately retains the historical
    // standard-marker-only contract. UUID terminals are available through the
    // typed resolver below, where the caller can enforce owner-scoped lookup.
    if primary.iter().any(|link| {
        matches!(
            link.reference,
            FormChoiceParameterLinkReference::FormAttribute {
                terminal: FormChoiceParameterLinkTerminal::MetadataUuid(_),
                ..
            } | FormChoiceParameterLinkReference::TableCurrentData { .. }
        )
    }) {
        return Err(FormChoiceParameterLinksParseError::PrimaryMalformed);
    }
    let duplicate = parse_raw_form_choice_parameter_links(duplicate, "5007", true)
        .ok_or(FormChoiceParameterLinksParseError::DuplicateMalformed)?;
    if duplicate.iter().any(|link| {
        matches!(
            link.reference,
            FormChoiceParameterLinkReference::FormAttribute {
                terminal: FormChoiceParameterLinkTerminal::MetadataUuid(_),
                ..
            } | FormChoiceParameterLinkReference::TableCurrentData { .. }
        )
    }) {
        return Err(FormChoiceParameterLinksParseError::DuplicateMalformed);
    }
    if primary != duplicate {
        return Err(FormChoiceParameterLinksParseError::MirrorMismatch);
    }
    primary
        .into_iter()
        .map(|link| {
            let data_path = match link.reference {
                FormChoiceParameterLinkReference::FormAttribute {
                    attribute_id,
                    terminal,
                } => {
                    let attribute_name = resolve_attribute(&attribute_id).ok_or(
                        FormChoiceParameterLinksParseError::UnresolvedAttribute(attribute_id),
                    )?;
                    match terminal {
                        FormChoiceParameterLinkTerminal::Absent => attribute_name,
                        FormChoiceParameterLinkTerminal::Standard(terminal) => {
                            format!("{attribute_name}.{}", terminal.data_path_suffix())
                        }
                        FormChoiceParameterLinkTerminal::MetadataUuid(_) => unreachable!(
                            "UUID terminals are rejected by the compatibility entrypoint"
                        ),
                    }
                }
                FormChoiceParameterLinkReference::TableCurrentData { .. } => {
                    unreachable!("table links are rejected by the compatibility entrypoint")
                }
            };
            Ok(FormChoiceParameterLink::new(
                link.name,
                data_path,
                link.value_change,
            ))
        })
        .collect()
}

/// Decode mirrored native link collections and resolve each typed terminal.
///
/// The resolver receives the numeric form-attribute owner id and one of the
/// physical terminal variants. It owns domain resolution (including checking
/// that a UUID belongs to that owner) and returns a canonical data path. Raw
/// 5006/5007 slots are intentionally not exposed through this API.
pub fn parse_form_choice_parameter_links_with_terminal_resolver<F>(
    primary: &str,
    duplicate: &str,
    mut resolve: F,
) -> Result<Vec<FormChoiceParameterLink>, FormChoiceParameterLinksParseError>
where
    F: FnMut(&str, &FormChoiceParameterLinkTerminal) -> Option<String>,
{
    let primary = parse_raw_form_choice_parameter_links(primary, "5006", false)
        .ok_or(FormChoiceParameterLinksParseError::PrimaryMalformed)?;
    if primary.iter().any(|link| {
        matches!(
            link.reference,
            FormChoiceParameterLinkReference::TableCurrentData { .. }
        )
    }) {
        return Err(FormChoiceParameterLinksParseError::PrimaryMalformed);
    }
    let duplicate = parse_raw_form_choice_parameter_links(duplicate, "5007", true)
        .ok_or(FormChoiceParameterLinksParseError::DuplicateMalformed)?;
    if duplicate.iter().any(|link| {
        matches!(
            link.reference,
            FormChoiceParameterLinkReference::TableCurrentData { .. }
        )
    }) {
        return Err(FormChoiceParameterLinksParseError::DuplicateMalformed);
    }
    if primary != duplicate {
        return Err(FormChoiceParameterLinksParseError::MirrorMismatch);
    }
    primary
        .into_iter()
        .map(|link| {
            let FormChoiceParameterLinkReference::FormAttribute {
                attribute_id,
                terminal,
            } = link.reference
            else {
                unreachable!("table links are rejected by the terminal resolver")
            };
            let data_path = resolve(&attribute_id, &terminal).ok_or_else(|| {
                FormChoiceParameterLinksParseError::UnresolvedAttribute(attribute_id.clone())
            })?;
            Ok(FormChoiceParameterLink::new(
                link.name,
                data_path,
                link.value_change,
            ))
        })
        .collect()
}

/// Decode mirrored native link collections and resolve each typed reference.
///
/// Unlike the compatibility entrypoints, this resolver receives the distinct
/// TableCurrentData table/column binding profile as a typed value. Resolution
/// remains domain-owned by the caller; physical 5006/5007 slots stay private.
pub fn parse_form_choice_parameter_links_with_reference_resolver<F>(
    primary: &str,
    duplicate: &str,
    mut resolve: F,
) -> Result<Vec<FormChoiceParameterLink>, FormChoiceParameterLinksParseError>
where
    F: FnMut(&FormChoiceParameterLinkReference) -> Option<String>,
{
    let primary = parse_raw_form_choice_parameter_links(primary, "5006", false)
        .ok_or(FormChoiceParameterLinksParseError::PrimaryMalformed)?;
    let duplicate = parse_raw_form_choice_parameter_links(duplicate, "5007", true)
        .ok_or(FormChoiceParameterLinksParseError::DuplicateMalformed)?;
    if primary != duplicate {
        return Err(FormChoiceParameterLinksParseError::MirrorMismatch);
    }
    primary
        .into_iter()
        .map(|link| {
            let unresolved = match &link.reference {
                FormChoiceParameterLinkReference::FormAttribute { attribute_id, .. } => {
                    attribute_id.clone()
                }
                FormChoiceParameterLinkReference::TableCurrentData { table_id, .. } => {
                    table_id.to_string()
                }
            };
            let data_path = resolve(&link.reference).ok_or(
                FormChoiceParameterLinksParseError::UnresolvedAttribute(unresolved),
            )?;
            Ok(FormChoiceParameterLink::new(
                link.name,
                data_path,
                link.value_change,
            ))
        })
        .collect()
}

fn parse_raw_form_choice_parameter_links(
    raw: &str,
    marker: &str,
    duplicate: bool,
) -> Option<Vec<RawFormChoiceParameterLink>> {
    if raw.len() > MAX_FORM_CHOICE_PARAMETERS_RAW_BYTES {
        return None;
    }
    let fields = braced_fields_bounded(
        raw,
        MAX_FORM_CHOICE_PARAMETER_LINKS
            .checked_mul(if duplicate { 7 } else { 5 })?
            .checked_add(2)?,
    )?;
    if fields.first()?.trim() != marker {
        return None;
    }
    let count_token = fields.get(1)?.trim();
    let count = count_token.parse::<usize>().ok()?;
    if count_token != count.to_string() {
        return None;
    }
    if count > MAX_FORM_CHOICE_PARAMETER_LINKS {
        return None;
    }
    let mut cursor = 2usize;
    let mut links = Vec::with_capacity(count);
    for _ in 0..count {
        let name = exact_1c_string(fields.get(cursor)?)?;
        if name.is_empty() {
            return None;
        }
        cursor += 1;
        let mode = fields.get(cursor)?.trim();
        cursor += 1;
        let owner = braced_fields_bounded(fields.get(cursor)?, 2)?;
        cursor += 1;
        let reference = match (mode, owner.as_slice()) {
            ("1", [attribute_id]) => FormChoiceParameterLinkReference::FormAttribute {
                attribute_id: canonical_positive_id(attribute_id)?,
                terminal: FormChoiceParameterLinkTerminal::Absent,
            },
            ("2", [attribute_id]) => {
                let terminal = braced_fields_bounded(fields.get(cursor)?, 2)?;
                cursor += 1;
                let terminal = match terminal.as_slice() {
                    [terminal] => match terminal.trim() {
                        "-3" => FormChoiceParameterLinkTerminal::Standard(
                            FormChoiceParameterLinkStandardTerminal::Date,
                        ),
                        "-5" => FormChoiceParameterLinkTerminal::Standard(
                            FormChoiceParameterLinkStandardTerminal::Owner,
                        ),
                        "-8" => FormChoiceParameterLinkTerminal::Standard(
                            FormChoiceParameterLinkStandardTerminal::Ref,
                        ),
                        _ => return None,
                    },
                    [kind, uuid] if kind.trim() == "0" => {
                        let uuid_text = uuid.trim();
                        let uuid = Uuid::parse_str(uuid_text).ok()?;
                        if uuid.is_nil() || uuid.to_string() != uuid_text {
                            return None;
                        }
                        FormChoiceParameterLinkTerminal::MetadataUuid(uuid.to_string())
                    }
                    _ => return None,
                };
                FormChoiceParameterLinkReference::FormAttribute {
                    attribute_id: canonical_positive_id(attribute_id)?,
                    terminal,
                }
            }
            ("2", [table_id, item_type])
                if item_type.trim() == FORM_CHOICE_PARAMETER_LINK_TABLE_CURRENT_DATA_ITEM_TYPE =>
            {
                let terminal = braced_fields_bounded(fields.get(cursor)?, 2)?;
                let terminal = match terminal.as_slice() {
                    [binding_id] => FormChoiceParameterLinkTableCurrentDataTerminal::BindingId(
                        canonical_positive_id(binding_id)?.parse().ok()?,
                    ),
                    [kind, uuid] if kind.trim() == "0" => {
                        let uuid_text = uuid.trim();
                        let uuid = Uuid::parse_str(uuid_text).ok()?;
                        if uuid.is_nil() || uuid.to_string() != uuid_text {
                            return None;
                        }
                        FormChoiceParameterLinkTableCurrentDataTerminal::MetadataUuid(
                            uuid.to_string(),
                        )
                    }
                    [binding_id, uuid] => {
                        let binding_id = canonical_positive_id(binding_id)?.parse().ok()?;
                        let uuid_text = uuid.trim();
                        let uuid = Uuid::parse_str(uuid_text).ok()?;
                        if uuid.is_nil() || uuid.to_string() != uuid_text {
                            return None;
                        }
                        FormChoiceParameterLinkTableCurrentDataTerminal::BindingUuid {
                            binding_id,
                            uuid: uuid.to_string(),
                        }
                    }
                    _ => return None,
                };
                cursor += 1;
                FormChoiceParameterLinkReference::TableCurrentData {
                    table_id: canonical_positive_id(table_id)?.parse().ok()?,
                    terminal,
                }
            }
            _ => return None,
        };
        let value_change =
            FormChoiceParameterLinkValueChange::from_raw_code(fields.get(cursor)?.trim())?;
        cursor += 1;
        if duplicate {
            if exact_1c_string(fields.get(cursor)?)?.is_empty()
                && exact_1c_string(fields.get(cursor + 1)?)?.is_empty()
            {
                cursor += 2;
            } else {
                return None;
            }
        }
        links.push(RawFormChoiceParameterLink {
            name,
            reference,
            value_change,
        });
    }
    (cursor == fields.len()).then_some(links)
}

fn canonical_positive_id(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let value = raw.parse::<u64>().ok()?;
    (value != 0 && raw == value.to_string()).then(|| raw.to_owned())
}

fn parse_form_choice_parameter_value<F>(
    raw: &str,
    resolve: &mut F,
) -> Option<(Vec<(String, String)>, FormChoiceParameterValue)>
where
    F: FnMut(&str, &str) -> Option<String>,
{
    let fields = braced_fields_bounded(raw, 3)?;
    if fields.len() != 3
        || exact_1c_string(fields[0]).as_deref() != Some("#")
        || fields[1].trim() != FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR
    {
        return None;
    }
    let payload = braced_fields_bounded(fields[2], 6)?;
    if payload.len() != 6 || payload[0].trim() != "0" || !matches!(payload[1].trim(), "0" | "1") {
        return None;
    }
    let presentation = parse_choice_parameter_presentation(payload[5])?;
    let typed = braced_fields_bounded(payload[2], 3)?;
    let value = match typed.as_slice() {
        [kind, value]
            if kind.trim() == r#""B""# && payload[1].trim() == "1" && nil_ids(&payload) =>
        {
            match value.trim() {
                "0" => FormChoiceParameterValue::Boolean(false),
                "1" => FormChoiceParameterValue::Boolean(true),
                _ => return None,
            }
        }
        [kind] if kind.trim() == r#""U""# && payload[1].trim() == "0" && non_nil_ids(&payload) => {
            FormChoiceParameterValue::DesignTimeRef(resolve(payload[3].trim(), payload[4].trim())?)
        }
        [kind, array_type, values]
            if exact_1c_string(kind).as_deref() == Some("#")
                && array_type
                    .trim()
                    .eq_ignore_ascii_case(FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE)
                && payload[1].trim() == "1"
                && nil_ids(&payload) =>
        {
            FormChoiceParameterValue::FixedArray(parse_choice_parameter_array(values, resolve)?)
        }
        _ => return None,
    };
    Some((presentation, value))
}

fn parse_choice_parameter_array<F>(
    raw: &str,
    resolve: &mut F,
) -> Option<Vec<FormChoiceParameterArrayItem>>
where
    F: FnMut(&str, &str) -> Option<String>,
{
    let fields = braced_fields_bounded(raw, MAX_FORM_CHOICE_PARAMETERS_FIXED_ARRAY_ITEMS + 1)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count > MAX_FORM_CHOICE_PARAMETERS_FIXED_ARRAY_ITEMS {
        return None;
    }
    if fields.len() != count.checked_add(1)? {
        return None;
    }
    fields[1..]
        .iter()
        .map(|item| {
            let fields = braced_fields_bounded(item, 3)?;
            if fields.len() != 3
                || exact_1c_string(fields[0]).as_deref() != Some("#")
                || fields[1].trim() != FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR
            {
                return None;
            }
            let payload = braced_fields_bounded(fields[2], 6)?;
            let [zero, mode, typed, type_id, value_id, presentation] = payload.as_slice() else {
                return None;
            };
            if zero.trim() != "0" {
                return None;
            }
            let typed = braced_fields_bounded(typed, 2)?;
            let value = match (mode.trim(), typed.as_slice()) {
                ("0", [kind]) if kind.trim() == r#""U""# && non_nil_pair(type_id, value_id) => {
                    FormChoiceParameterArrayItemValue::DesignTimeRef(resolve(
                        type_id.trim(),
                        value_id.trim(),
                    )?)
                }
                ("1", [kind, value])
                    if kind.trim() == r#""S""# && exact_nil_pair(type_id, value_id) =>
                {
                    FormChoiceParameterArrayItemValue::String(exact_1c_string(value)?)
                }
                _ => return None,
            };
            Some(FormChoiceParameterArrayItem {
                presentation: parse_choice_parameter_presentation(presentation)?,
                value,
            })
        })
        .collect()
}

fn nil_ids(payload: &[&str]) -> bool {
    payload
        .get(3)
        .is_some_and(|id| id.trim().eq_ignore_ascii_case(NIL_UUID))
        && payload
            .get(4)
            .is_some_and(|id| id.trim().eq_ignore_ascii_case(NIL_UUID))
}

fn non_nil_ids(payload: &[&str]) -> bool {
    matches!((payload.get(3), payload.get(4)), (Some(type_id), Some(value_id)) if non_nil_pair(type_id, value_id))
}

fn non_nil_pair(type_id: &str, value_id: &str) -> bool {
    Uuid::parse_str(type_id.trim()).is_ok_and(|id| !id.is_nil())
        && Uuid::parse_str(value_id.trim()).is_ok_and(|id| !id.is_nil())
}

fn exact_nil_pair(type_id: &str, value_id: &str) -> bool {
    type_id.trim() == NIL_UUID && value_id.trim() == NIL_UUID
}

fn parse_choice_parameter_presentation(raw: &str) -> Option<Vec<(String, String)>> {
    let fields = braced_fields_bounded(raw, MAX_FORM_CHOICE_PARAMETERS_PRESENTATION_ITEMS + 2)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count > MAX_FORM_CHOICE_PARAMETERS_PRESENTATION_ITEMS {
        return None;
    }
    if fields.len() != count.checked_add(2)? {
        return None;
    }
    fields[2..]
        .iter()
        .map(|item| {
            let pair = braced_fields_bounded(item, 2)?;
            match pair.as_slice() {
                [lang, content] => Some((exact_1c_string(lang)?, exact_1c_string(content)?)),
                _ => None,
            }
        })
        .collect()
}

fn exact_1c_string(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let mut chars = raw.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut value = String::new();
    while let Some((index, ch)) = chars.next() {
        if ch == '"' {
            if matches!(chars.clone().next(), Some((_, '"'))) {
                value.push('"');
                chars.next();
                continue;
            }
            return (index + 1 == raw.len()).then_some(value);
        }
        value.push(ch);
    }
    None
}

fn braced_fields_bounded(raw: &str, max_fields: usize) -> Option<Vec<&str>> {
    let raw = raw.trim();
    if scan_braced(raw, 0)? != raw.len() {
        return None;
    }
    let inner = &raw[1..raw.len() - 1];
    let mut result = Vec::with_capacity(max_fields.min(16));
    let mut start = 0;
    let mut depth = 0usize;
    let mut quoted = false;
    let mut chars = inner.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if quoted {
            if ch == '"' {
                if matches!(chars.peek(), Some((_, '"'))) {
                    chars.next();
                } else {
                    quoted = false;
                }
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '{' => depth = depth.checked_add(1)?,
            '}' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                if result.len() >= max_fields {
                    return None;
                }
                result.push(inner[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return None;
    }
    if result.len() >= max_fields {
        return None;
    }
    result.push(inner[start..].trim());
    Some(result)
}

fn scan_braced(raw: &str, start: usize) -> Option<usize> {
    if raw.get(start..)?.chars().next()? != '{' {
        return None;
    }
    let mut depth = 0usize;
    let mut quoted = false;
    let mut chars = raw[start..].char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if quoted {
            if ch == '"' {
                if matches!(chars.peek(), Some((_, '"'))) {
                    chars.next();
                } else {
                    quoted = false;
                }
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '{' => depth = depth.checked_add(1)?,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod form_choice_parameters_tests {
    use super::*;

    const OWNER: &str = "11111111-1111-4111-8111-111111111111";
    const VALUE: &str = "22222222-2222-4222-8222-222222222222";
    const NIL: &str = "00000000-0000-0000-0000-000000000000";

    fn presentation() -> String {
        "{1,1,{\"en\",\"value\"}}".to_owned()
    }
    fn reference() -> String {
        format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,0,{{\"U\"}},{OWNER},{VALUE},{}}}}}",
            presentation()
        )
    }
    fn envelope(value: &str) -> String {
        format!("{{0,1,\"parameter\",{value}}}")
    }
    fn resolver(type_id: &str, value_id: &str) -> Option<String> {
        (type_id == OWNER && value_id == VALUE).then_some("Enum.Status.EnumValue.Active".to_owned())
    }

    #[test]
    fn form_choice_parameters_decodes_boolean_reference_and_fixed_array() {
        let boolean = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{{\"B\",1}},{NIL},{NIL},{}}}}}",
            presentation()
        );
        let parsed = parse_form_choice_parameters(&envelope(&boolean), resolver).unwrap();
        assert!(matches!(
            parsed.items()[0].value(),
            FormChoiceParameterValue::Boolean(true)
        ));
        let parsed = parse_form_choice_parameters(&envelope(&reference()), resolver).unwrap();
        assert!(
            matches!(parsed.items()[0].value(), FormChoiceParameterValue::DesignTimeRef(value) if value == "Enum.Status.EnumValue.Active")
        );
        let array = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE},{{1,{}}}}}",
            reference()
        );
        let fixed = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{array},{NIL},{NIL},{}}}}}",
            presentation()
        );
        assert!(
            matches!(parse_form_choice_parameters(&envelope(&fixed), resolver).unwrap().items()[0].value(), FormChoiceParameterValue::FixedArray(values) if values.len() == 1)
        );
    }

    #[test]
    fn form_choice_parameters_fixed_array_preserves_mixed_reference_and_string_items() {
        let string_item = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{{\"S\",\"Printer\"}},{NIL},{NIL},{}}}}}",
            presentation()
        );
        let array = format!("{{2,{},{}}}", reference(), string_item);
        let fixed_array = format!("{{\"#\",{FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE},{array}}}");
        let fixed = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{fixed_array},{NIL},{NIL},{}}}}}",
            presentation()
        );
        let parsed = parse_form_choice_parameters(&envelope(&fixed), resolver).unwrap();
        let FormChoiceParameterValue::FixedArray(values) = parsed.items()[0].value() else {
            panic!("expected fixed array");
        };
        assert_eq!(values.len(), 2);
        assert!(matches!(
            values[0].value(),
            FormChoiceParameterArrayItemValue::DesignTimeRef(value)
                if value == "Enum.Status.EnumValue.Active"
        ));
        assert!(matches!(
            values[1].value(),
            FormChoiceParameterArrayItemValue::String(value) if value == "Printer"
        ));

        for malformed in [
            string_item.replace("{\"S\",\"Printer\"}", "{\"S\"}"),
            string_item.replace(NIL, "00000000-0000-0000-0000-000000000001"),
            string_item.replace(NIL, "00000000-0000-0000-0000-00000000000A"),
        ] {
            let array = format!("{{1,{malformed}}}");
            let fixed_array = format!("{{\"#\",{FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE},{array}}}");
            let fixed = format!(
                "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{fixed_array},{NIL},{NIL},{}}}}}",
                presentation()
            );
            assert!(parse_form_choice_parameters(&envelope(&fixed), resolver).is_none());
        }
    }

    #[test]
    fn form_choice_parameters_fail_closed_on_malformed_or_unsupported_values() {
        let bad_count = "{0,2,\"one\",{}}";
        assert!(parse_form_choice_parameters(bad_count, resolver).is_none());
        assert!(parse_form_choice_parameters("{0,18446744073709551615}", resolver).is_none());
        let bad_boolean = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{{\"B\",2}},{NIL},{NIL},{}}}}}",
            presentation()
        );
        assert!(parse_form_choice_parameters(&envelope(&bad_boolean), resolver).is_none());
        let invalid_type = reference().replace(OWNER, "not-a-type-id");
        assert!(parse_form_choice_parameters(&envelope(&invalid_type), resolver).is_none());
        let recursive_array = [
            "{\"#\",",
            FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE,
            ",{1,{\"#\",",
            FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE,
            ",{0}}}}",
        ]
        .concat();
        let fixed = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{recursive_array},{NIL},{NIL},{}}}}}",
            presentation()
        );
        assert!(parse_form_choice_parameters(&envelope(&fixed), resolver).is_none());
        let unsupported = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{{\"S\",\"opaque\"}},{NIL},{NIL},{}}}}}",
            presentation()
        );
        assert!(parse_form_choice_parameters(&envelope(&unsupported), resolver).is_none());
    }

    #[test]
    fn form_choice_parameters_validate_reference_ids_before_the_resolver() {
        let permissive = |_: &str, _: &str| Some("forged".to_owned());
        assert!(
            parse_form_choice_parameters(
                &envelope(&reference().replace(OWNER, "not-a-uuid")),
                permissive,
            )
            .is_none()
        );
        assert!(
            parse_form_choice_parameters(
                &envelope(&reference().replace(VALUE, "also-not-a-uuid")),
                permissive,
            )
            .is_none()
        );
        assert!(
            parse_form_choice_parameters(&envelope(&reference().replace(OWNER, NIL)), permissive,)
                .is_none()
        );
        assert!(
            parse_form_choice_parameters(&envelope(&reference().replace(VALUE, NIL)), permissive,)
                .is_none()
        );
    }

    #[test]
    fn form_choice_parameters_bound_raw_input_and_each_collection() {
        let permissive = |_: &str, _: &str| Some("resolved".to_owned());
        assert!(
            parse_form_choice_parameters(
                &format!("{{{}}}", "x".repeat(MAX_FORM_CHOICE_PARAMETERS_RAW_BYTES)),
                permissive,
            )
            .is_none()
        );
        assert!(
            parse_form_choice_parameters(
                &format!("{{0,{}}}", MAX_FORM_CHOICE_PARAMETERS_ITEMS + 1),
                permissive,
            )
            .is_none()
        );
        let oversized_presentation = format!(
            "{{1,{}}}",
            MAX_FORM_CHOICE_PARAMETERS_PRESENTATION_ITEMS + 1
        );
        let bad_presentation = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,0,{{\"U\"}},{OWNER},{VALUE},{oversized_presentation}}}}}"
        );
        assert!(parse_form_choice_parameters(&envelope(&bad_presentation), permissive).is_none());
        let oversized_array = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_FIXED_ARRAY_TYPE},{{{}}}}}",
            MAX_FORM_CHOICE_PARAMETERS_FIXED_ARRAY_ITEMS + 1
        );
        let bad_array = format!(
            "{{\"#\",{FORM_CHOICE_PARAMETER_ITEM_DISCRIMINATOR},{{0,1,{oversized_array},{NIL},{NIL},{}}}}}",
            presentation()
        );
        assert!(parse_form_choice_parameters(&envelope(&bad_array), permissive).is_none());
        assert!(
            parse_form_choice_parameters(
                &format!(
                    "{{{}}}",
                    ",".repeat(MAX_FORM_CHOICE_PARAMETERS_ITEMS * 2 + 2)
                ),
                permissive,
            )
            .is_none()
        );
    }

    #[test]
    fn form_choice_parameters_qname_maps_exact_namespaces() {
        assert_eq!(
            canonical_form_choice_parameters_qname("{}ChoiceParameters").unwrap(),
            "ChoiceParameters"
        );
        assert_eq!(
            canonical_form_choice_parameters_qname(
                "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters"
            )
            .unwrap(),
            "ChoiceParameters"
        );
        assert_eq!(
            canonical_form_choice_parameters_qname(
                "{http://v8.1c.ru/8.2/managed-application/core}item"
            )
            .unwrap(),
            "app:item"
        );
        assert_eq!(
            canonical_form_choice_parameters_qname("{http://v8.1c.ru/8.1/data/core}_Value-1")
                .unwrap(),
            "v8:_Value-1"
        );
    }

    #[test]
    fn form_choice_parameters_qname_fails_closed_on_near_misses() {
        for qname in [
            "",
            "ChoiceParameters",
            " {}ChoiceParameters",
            "{}ChoiceParameters ",
            "{http://v8.1c.ru/8.3/xcf/logform/}ChoiceParameters",
            "{HTTP://v8.1c.ru/8.3/xcf/logform}ChoiceParameters",
            "{http://v8.1c.ru/8.3/xcf/logform}1ChoiceParameters",
            "{http://v8.1c.ru/8.3/xcf/logform}-ChoiceParameters",
            "{http://v8.1c.ru/8.3/xcf/logform}:ChoiceParameters",
            "{http://v8.1c.ru/8.3/xcf/logform}Choice/Parameters",
            "{http://v8.1c.ru/8.3/xcf/logform}Choice Parameters",
            "{http://v8.1c.ru/8.3/xcf/logform}Choice{Parameters}",
            "{http://v8.1c.ru/8.3/xcf/logform}Имя",
            "{http://example.invalid/form}ChoiceParameters",
        ] {
            assert!(matches!(
                canonical_form_choice_parameters_qname(qname),
                Err(SchemaError::InvalidFormChoiceParametersQName(value)) if value == qname
            ));
        }
    }

    #[test]
    fn form_choice_parameter_cluster_order_is_schema_owned_and_exact() {
        let policy = exact_form_choice_parameters_policy();
        assert_eq!(
            form_choice_parameter_cluster_order(&policy).unwrap(),
            [
                FormChoiceParameterClusterMember::Links,
                FormChoiceParameterClusterMember::Parameters,
                FormChoiceParameterClusterMember::AvailableTypes,
            ]
        );
        assert_eq!(
            FormChoiceParameterClusterMember::Links.xml_local_name(),
            "ChoiceParameterLinks"
        );
        assert_eq!(
            FormChoiceParameterClusterMember::Parameters.xml_local_name(),
            "ChoiceParameters"
        );
        assert_eq!(
            FormChoiceParameterClusterMember::AvailableTypes.xml_local_name(),
            "AvailableTypes"
        );
        assert_eq!(
            FormChoiceParameterLinkValueChange::Clear.xml_value(),
            "Clear"
        );
        assert_eq!(
            FormChoiceParameterLinkValueChange::DontChange.xml_value(),
            "DontChange"
        );
    }

    #[test]
    fn form_choice_parameter_cluster_order_rejects_mutations() {
        let mut mutated = exact_form_choice_parameters_policy();
        let WriterPolicy::FormChoiceParameters {
            owner_qname,
            owner_predecessor_qname,
            ..
        } = &mut mutated
        else {
            unreachable!()
        };
        std::mem::swap(owner_qname, owner_predecessor_qname);
        assert!(matches!(
            form_choice_parameter_cluster_order(&mutated),
            Err(SchemaError::InvalidFormChoiceParameterClusterPolicy(reason))
                if reason == "owner feature order"
        ));
    }

    #[test]
    fn form_choice_parameter_links_parse_exact_mirrors_and_value_changes() {
        let primary = r#"{5006,3,"Filter.Organization",1,{27},0,"Filter.Partner",2,{9},{-8},1,"Date",2,{1},{-3},1}"#;
        let duplicate = r#"{5007,3,"Filter.Organization",1,{27},0,"","","Filter.Partner",2,{9},{-8},1,"","","Date",2,{1},{-3},1,"",""}"#;
        let links = parse_form_choice_parameter_links(primary, duplicate, |id| match id {
            "1" => Some("Object".to_owned()),
            "27" => Some("Organization".to_owned()),
            "9" => Some("Partner".to_owned()),
            _ => None,
        })
        .unwrap();
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].name(), "Filter.Organization");
        assert_eq!(links[0].data_path(), "Organization");
        assert_eq!(
            links[0].value_change(),
            FormChoiceParameterLinkValueChange::Clear
        );
        assert_eq!(links[1].data_path(), "Partner.Ref");
        assert_eq!(
            links[1].value_change(),
            FormChoiceParameterLinkValueChange::DontChange
        );
        assert_eq!(links[2].name(), "Date");
        assert_eq!(links[2].data_path(), "Object.Date");
        assert_eq!(
            links[2].value_change(),
            FormChoiceParameterLinkValueChange::DontChange
        );
        assert_eq!(
            parse_form_choice_parameter_links("{5006,0}", "{5007,0}", |_| None).unwrap(),
            Vec::<FormChoiceParameterLink>::new()
        );
    }

    #[test]
    fn form_choice_parameter_links_parse_uuid_terminals_with_typed_resolver() {
        let primary = r#"{5006,2,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0,"Filter.Currency",2,{1},{0,22222222-2222-4222-8222-222222222222},1}"#;
        let duplicate = r#"{5007,2,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0,"","","Filter.Currency",2,{1},{0,22222222-2222-4222-8222-222222222222},1,"",""}"#;
        let links = parse_form_choice_parameter_links_with_terminal_resolver(
            primary,
            duplicate,
            |owner_id, terminal| match (owner_id, terminal) {
                ("1", FormChoiceParameterLinkTerminal::MetadataUuid(uuid))
                    if uuid == "11111111-1111-4111-8111-111111111111" =>
                {
                    Some("Owner".to_owned())
                }
                ("1", FormChoiceParameterLinkTerminal::MetadataUuid(uuid))
                    if uuid == "22222222-2222-4222-8222-222222222222" =>
                {
                    Some("Currency".to_owned())
                }
                _ => None,
            },
        )
        .unwrap();
        assert_eq!(
            links,
            vec![
                FormChoiceParameterLink::new(
                    "Filter.Owner".to_owned(),
                    "Owner".to_owned(),
                    FormChoiceParameterLinkValueChange::Clear,
                ),
                FormChoiceParameterLink::new(
                    "Filter.Currency".to_owned(),
                    "Currency".to_owned(),
                    FormChoiceParameterLinkValueChange::DontChange,
                ),
            ]
        );

        let one_primary =
            r#"{5006,1,"Filter.Owner",2,{1},{0,33333333-3333-4333-8333-333333333333},0}"#;
        let one_duplicate =
            r#"{5007,1,"Filter.Owner",2,{1},{0,33333333-3333-4333-8333-333333333333},0,"",""}"#;
        let links = parse_form_choice_parameter_links_with_terminal_resolver(
            one_primary,
            one_duplicate,
            |owner_id, terminal| {
                (owner_id == "1"
                    && matches!(
                        terminal,
                        FormChoiceParameterLinkTerminal::MetadataUuid(uuid)
                            if uuid == "33333333-3333-4333-8333-333333333333"
                    ))
                .then_some("Owner".to_owned())
            },
        )
        .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data_path(), "Owner");

        // The legacy public API remains intentionally standard-marker-only.
        assert_eq!(
            parse_form_choice_parameter_links(one_primary, one_duplicate, |_| Some("Owner".into())),
            Err(FormChoiceParameterLinksParseError::PrimaryMalformed)
        );
    }

    #[test]
    fn form_choice_parameter_links_parse_table_current_data_with_typed_reference_resolver() {
        for (primary, duplicate, table_id, binding_id, name, data_path) in [
            (
                r#"{5006,1,"Отбор.Партнер",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#,
                r#"{5007,1,"Отбор.Партнер",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0,"",""}"#,
                1050,
                21,
                "Отбор.Партнер",
                "Items.ТаблицаДопРеквизитов.CurrentData.Партнер",
            ),
            (
                r#"{5006,1,"Отбор.СтранаПроисхождения",2,{785,02023637-7868-4a5f-8576-835a76e0c9ba},{12},0}"#,
                r#"{5007,1,"Отбор.СтранаПроисхождения",2,{785,02023637-7868-4a5f-8576-835a76e0c9ba},{12},0,"",""}"#,
                785,
                12,
                "Отбор.СтранаПроисхождения",
                "Items.ТаблицаСумм.CurrentData.СтранаПроисхождения",
            ),
        ] {
            let links = parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                duplicate,
                |reference| match reference {
                    FormChoiceParameterLinkReference::TableCurrentData {
                        table_id: actual_table_id,
                        terminal:
                            FormChoiceParameterLinkTableCurrentDataTerminal::BindingId(
                                actual_binding_id,
                            ),
                    } if *actual_table_id == table_id && *actual_binding_id == binding_id => {
                        Some(data_path.to_owned())
                    }
                    _ => None,
                },
            )
            .unwrap();
            assert_eq!(
                links,
                vec![FormChoiceParameterLink::new(
                    name.to_owned(),
                    data_path.to_owned(),
                    FormChoiceParameterLinkValueChange::Clear,
                )]
            );
        }

        let live_primary = r#"{5006,2,"Отбор.ОрганизацияПолучатель",1,{3},0,"Отбор.Организация",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c},0}"#;
        let live_duplicate = r#"{5007,2,"Отбор.ОрганизацияПолучатель",1,{3},0,"","","Отбор.Организация",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c},0,"",""}"#;
        let links = parse_form_choice_parameter_links_with_reference_resolver(
            live_primary,
            live_duplicate,
            |reference| match reference {
                FormChoiceParameterLinkReference::FormAttribute {
                    attribute_id,
                    terminal: FormChoiceParameterLinkTerminal::Absent,
                } if attribute_id == "3" => Some("resolved.direct".to_owned()),
                FormChoiceParameterLinkReference::TableCurrentData {
                    table_id: 81,
                    terminal: FormChoiceParameterLinkTableCurrentDataTerminal::MetadataUuid(uuid),
                } if uuid == "461bb43b-8803-4f48-811f-6beef397ee4c" => {
                    Some("resolved.table_metadata_uuid".to_owned())
                }
                _ => None,
            },
        )
        .unwrap();
        assert_eq!(
            links,
            vec![
                FormChoiceParameterLink::new(
                    "Отбор.ОрганизацияПолучатель".to_owned(),
                    "resolved.direct".to_owned(),
                    FormChoiceParameterLinkValueChange::Clear,
                ),
                FormChoiceParameterLink::new(
                    "Отбор.Организация".to_owned(),
                    "resolved.table_metadata_uuid".to_owned(),
                    FormChoiceParameterLinkValueChange::Clear,
                ),
            ]
        );

        // Existing entrypoints retain their established form-attribute-only
        // contract; TableCurrentData is reachable only through its typed API.
        let primary =
            r#"{5006,1,"Отбор.Партнер",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#;
        let duplicate = r#"{5007,1,"Отбор.Партнер",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0,"",""}"#;
        assert_eq!(
            parse_form_choice_parameter_links(primary, duplicate, |_| Some("x".to_owned())),
            Err(FormChoiceParameterLinksParseError::PrimaryMalformed)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_terminal_resolver(primary, duplicate, |_, _| {
                Some("x".to_owned())
            },),
            Err(FormChoiceParameterLinksParseError::PrimaryMalformed)
        );
        assert_eq!(
            parse_form_choice_parameter_links(live_primary, live_duplicate, |_| {
                Some("x".to_owned())
            }),
            Err(FormChoiceParameterLinksParseError::PrimaryMalformed)
        );
    }

    #[test]
    fn form_choice_parameter_links_table_current_data_fail_closed() {
        let primary =
            r#"{5006,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#;
        let duplicate = r#"{5007,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0,"",""}"#;
        let resolve = |_: &FormChoiceParameterLinkReference| Some("Partner".to_owned());
        for malformed in [
            r#"{5006,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9bb},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{1050,00000000-0000-0000-0000-000000000000},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba,extra},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{01050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{0,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{-1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0}"#,
            r#"{5006,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{021},0}"#,
            r#"{5006,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{0},0}"#,
            r#"{5006,1,"Filter.Partner",1,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},0}"#,
        ] {
            assert_eq!(
                parse_form_choice_parameter_links_with_reference_resolver(
                    malformed, duplicate, resolve,
                ),
                Err(FormChoiceParameterLinksParseError::PrimaryMalformed),
                "{malformed}"
            );
        }
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{22},0,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::MirrorMismatch)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Filter.Partner",2,{1050,02023637-7868-4a5f-8576-835a76e0c9ba},{21},0,"x",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::DuplicateMalformed)
        );
    }

    #[test]
    fn form_choice_parameter_links_table_current_data_metadata_uuid_terminal_fail_closed() {
        let primary = r#"{5006,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c},0}"#;
        let duplicate = r#"{5007,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c},0,"",""}"#;
        let resolve = |_: &FormChoiceParameterLinkReference| Some("resolved".to_owned());
        for malformed in [
            r#"{5006,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{00,461bb43b-8803-4f48-811f-6beef397ee4c},0}"#,
            r#"{5006,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,00000000-0000-0000-0000-000000000000},0}"#,
            r#"{5006,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4C},0}"#,
            r#"{5006,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c,extra},0}"#,
        ] {
            assert_eq!(
                parse_form_choice_parameter_links_with_reference_resolver(
                    malformed, duplicate, resolve,
                ),
                Err(FormChoiceParameterLinksParseError::PrimaryMalformed),
                "{malformed}"
            );
        }
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4d},0,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::MirrorMismatch)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Filter.Organization",2,{81,02023637-7868-4a5f-8576-835a76e0c9ba},{0,461bb43b-8803-4f48-811f-6beef397ee4c},0,"x",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::DuplicateMalformed)
        );
    }

    #[test]
    fn form_choice_parameter_links_table_current_data_binding_uuid_is_typed_and_fail_closed() {
        let primary = r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b6687},0}"#;
        let duplicate = r#"{5007,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b6687},0,"",""}"#;
        let links = parse_form_choice_parameter_links_with_reference_resolver(
            primary,
            duplicate,
            |reference| match reference {
                FormChoiceParameterLinkReference::TableCurrentData {
                    table_id: 94,
                    terminal:
                        FormChoiceParameterLinkTableCurrentDataTerminal::BindingUuid {
                            binding_id: 18,
                            uuid,
                        },
                } if uuid == "5bdad865-f2c5-434b-8041-ba4aad3b6687" => {
                    Some("resolved.binding_uuid".to_owned())
                }
                _ => None,
            },
        )
        .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data_path(), "resolved.binding_uuid");
        assert_eq!(
            parse_form_choice_parameter_links(primary, duplicate, |_| Some("x".to_owned())),
            Err(FormChoiceParameterLinksParseError::PrimaryMalformed)
        );

        let resolve = |_: &FormChoiceParameterLinkReference| Some("resolved".to_owned());
        for malformed in [
            r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{018,5bdad865-f2c5-434b-8041-ba4aad3b6687},0}"#,
            r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{-18,5bdad865-f2c5-434b-8041-ba4aad3b6687},0}"#,
            r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,00000000-0000-0000-0000-000000000000},0}"#,
            r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b668A},0}"#,
            r#"{5006,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b6687,extra},0}"#,
        ] {
            assert_eq!(
                parse_form_choice_parameter_links_with_reference_resolver(
                    malformed, duplicate, resolve,
                ),
                Err(FormChoiceParameterLinksParseError::PrimaryMalformed),
                "{malformed}"
            );
        }
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b6688},0,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::MirrorMismatch)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_reference_resolver(
                primary,
                r#"{5007,1,"Отбор.Организация",2,{94,02023637-7868-4a5f-8576-835a76e0c9ba},{18,5bdad865-f2c5-434b-8041-ba4aad3b6687},0,"x",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::DuplicateMalformed)
        );
    }

    #[test]
    fn form_choice_parameter_links_uuid_terminals_fail_closed() {
        let valid_primary =
            r#"{5006,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0}"#;
        let valid_duplicate =
            r#"{5007,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0,"",""}"#;
        let resolve = |_: &str, _: &FormChoiceParameterLinkTerminal| Some("Owner".to_owned());
        for malformed in [
            r#"{5006,1,"Filter.Owner",2,{1},{1,11111111-1111-4111-8111-111111111111},0}"#,
            r#"{5006,1,"Filter.Owner",2,{1},{0,00000000-0000-0000-0000-000000000000},0}"#,
            r#"{5006,1,"Filter.Owner",2,{1},{0,not-a-uuid},0}"#,
            r#"{5006,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-11111111111A},0}"#,
            r#"{5006,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111,extra},0}"#,
        ] {
            assert_eq!(
                parse_form_choice_parameter_links_with_terminal_resolver(
                    malformed,
                    valid_duplicate,
                    resolve,
                ),
                Err(FormChoiceParameterLinksParseError::PrimaryMalformed),
                "{malformed}"
            );
        }
        assert_eq!(
            parse_form_choice_parameter_links_with_terminal_resolver(
                valid_primary,
                r#"{5007,1,"Filter.Owner",2,{1},{0,22222222-2222-4222-8222-222222222222},0,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::MirrorMismatch)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_terminal_resolver(
                valid_primary,
                r#"{5007,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0,"x",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::DuplicateMalformed)
        );
        assert_eq!(
            parse_form_choice_parameter_links_with_terminal_resolver(
                r#"{5006,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0}"#,
                r#"{5007,1,"Filter.Owner",2,{1},{0,11111111-1111-4111-8111-111111111111},0,"",""}"#,
                |_, _| None,
            ),
            Err(FormChoiceParameterLinksParseError::UnresolvedAttribute(
                "1".to_owned()
            ))
        );
    }

    #[test]
    fn form_choice_parameter_links_reject_malformed_foreign_and_mismatched_mirrors() {
        let primary = r#"{5006,2,"Filter.Organization",1,{27},0,"Filter.Partner",1,{9},1}"#;
        let duplicate =
            r#"{5007,2,"Filter.Organization",1,{27},0,"","","Filter.Partner",1,{9},1,"",""}"#;
        let resolve = |id: &str| match id {
            "27" => Some("Organization".to_owned()),
            "9" => Some("Partner".to_owned()),
            _ => None,
        };
        for malformed in [
            r#"{5006,02,"Filter.Organization",1,{27},0,"Filter.Partner",1,{9},1}"#,
            r#"{5006,1,"Filter.Organization",1,{27},0,"Filter.Partner",1,{9},1}"#,
            r#"{5006,2,"Filter.Organization",1,{27},2,"Filter.Partner",1,{9},1}"#,
            r#"{5006,2,"Filter.Organization",2,{27},{-7},0,"Filter.Partner",1,{9},1}"#,
            r#"{5006,2,"Filter.Organization",1,{27,28},0,"Filter.Partner",1,{9},1}"#,
            r#"{5006,2,"Filter.Organization",1,{27},0,"Filter.Partner",1,{9},1}garbage"#,
        ] {
            assert_eq!(
                parse_form_choice_parameter_links(malformed, duplicate, resolve),
                Err(FormChoiceParameterLinksParseError::PrimaryMalformed),
                "{malformed}"
            );
        }
        assert_eq!(
            parse_form_choice_parameter_links(
                primary,
                r#"{5007,2,"Filter.Organization",1,{27},0,"","x","Filter.Partner",1,{9},1,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::DuplicateMalformed)
        );
        assert_eq!(
            parse_form_choice_parameter_links(
                primary,
                r#"{5007,2,"Filter.Organization",1,{27},0,"","","Filter.Other",1,{9},1,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::MirrorMismatch)
        );
        assert_eq!(
            parse_form_choice_parameter_links(
                r#"{5006,1,"Filter.Foreign",1,{99},0}"#,
                r#"{5007,1,"Filter.Foreign",1,{99},0,"",""}"#,
                resolve,
            ),
            Err(FormChoiceParameterLinksParseError::UnresolvedAttribute(
                "99".to_owned()
            ))
        );
    }
}

/// Embedded EDT-derived model inventory.
pub const BUNDLED_MODEL_INVENTORY_JSON: &str =
    include_str!("../data/edt-2025.2.3-model-inventory.json");

/// Embedded EDT EPackage classifier and feature identifiers.
pub const BUNDLED_PACKAGE_FEATURES_JSON: &str =
    include_str!("../data/edt-2025.2.3-package-features.json");

/// Embedded EDT Xcore-derived feature semantics for every packaged model resource.
pub const BUNDLED_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-feature-semantics.json");

/// Runtime projection containing only the model fact required by the verified
/// Form `ListSettings` tail policy.
///
/// Keeping this projection separate prevents the complete EDT research corpus
/// from becoming product-binary payload. Schema tests prove that the projection
/// is structurally identical to the corresponding parsed feature.
const BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-dcs-list-settings-feature-semantics.json");

/// Embedded exhaustive canonical-model implementation coverage.
pub const BUNDLED_CANONICAL_COVERAGE_JSON: &str =
    include_str!("../data/edt-2025.2.3-canonical-coverage.json");

/// Embedded, provider-derived metadata and produced-type feature order.
pub const BUNDLED_METADATA_ORDER_JSON: &str =
    include_str!("../data/edt-2025.2.3-metadata-order.json");

/// Embedded, verified writer behaviour rules.
pub const BUNDLED_WRITER_RULES_JSON: &str = include_str!("../data/edt-2025.2.3-writer-rules.json");

/// Compact, portable proof binding the production ChoiceList policy to one
/// exact research artifact without embedding its platform descriptors.
const BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON: &str =
    include_str!("../data/edt-2025.2.3-form-choice-list-string-writer-proof.json");

/// Embedded, exact EDT writer evidence for empty string values in a Form
/// `ChoiceList`. The complete artifact is research/test-only so its platform
/// descriptors cannot become product-binary payload.
#[cfg(test)]
const BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON: &str =
    include_str!("../data/edt-2025.2.3-form-choice-list-string-writer-evidence.json");

/// Embedded, exact EDT writer evidence for the bounded DCS settings tail.
pub const BUNDLED_DCS_WRITER_EVIDENCE_JSON: &str =
    include_str!("../data/edt-2025.2.3-dcs-writer-evidence.json");
/// Embedded platform-authenticated policy for the bounded DCS root selection.
pub const BUNDLED_DCS_SELECTION_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-selection-evidence.json");
/// Embedded platform-authenticated policy for the shared DCS order cohort.
pub const BUNDLED_DCS_ORDER_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-order-evidence.json");
/// Embedded platform-authenticated policy for the shared DCS filter cohort.
pub const BUNDLED_DCS_FILTER_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-filter-evidence.json");
/// Embedded platform-authenticated policy for the bounded shared DCS
/// conditional-appearance cohort.
pub const BUNDLED_DCS_CONDITIONAL_APPEARANCE_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-conditional-appearance-evidence.json");
/// Embedded platform-authenticated wrapper policy for Form-wide
/// `Attributes/ConditionalAppearance`.
pub const BUNDLED_DCS_FORM_ATTRIBUTES_CONDITIONAL_APPEARANCE_EVIDENCE_JSON: &str = include_str!(
    "../data/platform-8.3.27-xml-2.20-dcs-form-attributes-conditional-appearance-evidence.json"
);
/// Embedded platform-authenticated policy for the exact standalone settings
/// subtrees that remain source-owned by the XML codec.
pub const BUNDLED_DCS_SETTINGS_SOURCE_OWNED_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-settings-source-owned-evidence.json");
/// Embedded platform-authenticated policy for the bounded
/// `SchemaFile + Settings[*] + SchemaFile` DCS template envelope.
pub const BUNDLED_DCS_SCHEMA_TEMPLATE_ENVELOPE_EVIDENCE_JSON: &str =
    include_str!("../data/platform-8.3.27-xml-2.20-dcs-schema-template-envelope-evidence.json");

/// Embedded, exact EDT and live native-export evidence for the bounded
/// `InputFieldExtInfo.choiceParameters` writer.
pub const BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON: &str =
    include_str!("../data/edt-2025.2.3-form-choice-parameters-writer-evidence.json");
const BUNDLED_FORM_CHOICE_PARAMETERS_LIVE_FIXTURE_JSON: &str =
    include_str!("../../../tests/fixtures/form_choice_parameters_slot27_live.json");

/// Canonical XML value of the Task number auto-prefix policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskNumberAutoPrefix {
    BusinessProcessNumber,
}

impl TaskNumberAutoPrefix {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::BusinessProcessNumber => "BusinessProcessNumber",
        }
    }
}

/// Decode the physical Task owner slot into its canonical XML policy.
///
/// The `0` mapping is authenticated by an isolated two-round native export on
/// 1C:Enterprise 8.3.27.2214 with XML source version 2.20. Unknown physical
/// tokens deliberately fail closed.
pub fn parse_task_number_auto_prefix_slot(value: &str) -> Option<TaskNumberAutoPrefix> {
    match value.trim() {
        "0" => Some(TaskNumberAutoPrefix::BusinessProcessNumber),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskNumberAllowedLength {
    Variable,
}

impl TaskNumberAllowedLength {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::Variable => "Variable",
        }
    }
}

pub fn parse_task_number_allowed_length_slot(value: &str) -> Option<TaskNumberAllowedLength> {
    match value.trim() {
        "1" => Some(TaskNumberAllowedLength::Variable),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskChoiceHistoryOnInput {
    Auto,
}

impl TaskChoiceHistoryOnInput {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
        }
    }
}

pub fn parse_task_choice_history_on_input_slot(value: &str) -> Option<TaskChoiceHistoryOnInput> {
    match value.trim() {
        "1" => Some(TaskChoiceHistoryOnInput::Auto),
        _ => None,
    }
}

pub fn parse_task_include_help_in_contents_slot(value: &str) -> Option<bool> {
    match value.trim() {
        "1" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskDataLockControlMode {
    Automatic,
}

impl TaskDataLockControlMode {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
        }
    }
}

pub fn parse_task_data_lock_control_mode_slot(value: &str) -> Option<TaskDataLockControlMode> {
    match value.trim() {
        "1" => Some(TaskDataLockControlMode::Automatic),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskFullTextSearch {
    Use,
}

impl TaskFullTextSearch {
    pub const fn xml_value(self) -> &'static str {
        match self {
            Self::Use => "Use",
        }
    }
}

pub fn parse_task_full_text_search_slot(value: &str) -> Option<TaskFullTextSearch> {
    match value.trim() {
        "0" => Some(TaskFullTextSearch::Use),
        _ => None,
    }
}

/// Metadata object families whose generated reference types use the exact
/// `cfg:<Kind>Ref.<Name>` QName shape.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GeneratedMetadataReferenceOwnerKind {
    Catalog,
    Document,
    Enum,
    ExchangePlan,
    ChartOfAccounts,
    ChartOfCharacteristicTypes,
    ChartOfCalculationTypes,
    BusinessProcess,
    Task,
}

impl GeneratedMetadataReferenceOwnerKind {
    pub const fn reference_token(self) -> &'static str {
        match self {
            Self::Catalog => "Catalog",
            Self::Document => "Document",
            Self::Enum => "Enum",
            Self::ExchangePlan => "ExchangePlan",
            Self::ChartOfAccounts => "ChartOfAccounts",
            Self::ChartOfCharacteristicTypes => "ChartOfCharacteristicTypes",
            Self::ChartOfCalculationTypes => "ChartOfCalculationTypes",
            Self::BusinessProcess => "BusinessProcess",
            Self::Task => "Task",
        }
    }

    fn parse(reference_token: &str) -> Option<Self> {
        match reference_token {
            "Catalog" => Some(Self::Catalog),
            "Document" => Some(Self::Document),
            "Enum" => Some(Self::Enum),
            "ExchangePlan" => Some(Self::ExchangePlan),
            "ChartOfAccounts" => Some(Self::ChartOfAccounts),
            "ChartOfCharacteristicTypes" => Some(Self::ChartOfCharacteristicTypes),
            "ChartOfCalculationTypes" => Some(Self::ChartOfCalculationTypes),
            "BusinessProcess" => Some(Self::BusinessProcess),
            "Task" => Some(Self::Task),
            _ => None,
        }
    }
}

/// A parsed generated metadata reference owner.
///
/// Parsing is exact and fail-closed: only a supported metadata owner kind and
/// one non-empty, non-dotted name are accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedMetadataReferenceOwner<'a> {
    kind: GeneratedMetadataReferenceOwnerKind,
    name: &'a str,
}

impl<'a> GeneratedMetadataReferenceOwner<'a> {
    pub const fn kind(self) -> GeneratedMetadataReferenceOwnerKind {
        self.kind
    }

    pub const fn name(self) -> &'a str {
        self.name
    }

    /// Canonical design-time owner reference used by existing XML serializers.
    pub fn owner_reference(self) -> String {
        format!("{}.{}", self.kind.reference_token(), self.name)
    }
}

/// Parse an exact generated reference type QName into its typed metadata owner.
pub fn parse_generated_metadata_reference_owner(
    type_reference: &str,
) -> Option<GeneratedMetadataReferenceOwner<'_>> {
    let generated_type = type_reference.strip_prefix("cfg:")?;
    let (kind, name) = generated_type.split_once("Ref.")?;
    let kind = GeneratedMetadataReferenceOwnerKind::parse(kind)?;
    if name.is_empty() || name.trim() != name || name.contains('.') {
        return None;
    }
    Some(GeneratedMetadataReferenceOwner { kind, name })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedMetadataOwnerRole {
    Ref,
    Object,
    Record,
    RecordManager,
    RecordSet,
    RecordKey,
}

/// Metadata families which have generated owner type QNames.
///
/// `Ref` remains deliberately narrower: it is accepted only for the existing
/// reference-owner families, while the remaining roles are accepted for the
/// exact platform families below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedMetadataOwnerFamily {
    AccountingRegister,
    AccumulationRegister,
    BusinessProcess,
    CalculationRegister,
    Catalog,
    ChartOfAccounts,
    ChartOfCalculationTypes,
    ChartOfCharacteristicTypes,
    DataProcessor,
    Document,
    Enum,
    ExchangePlan,
    InformationRegister,
    Recalculation,
    Report,
    Sequence,
    Task,
}

impl GeneratedMetadataOwnerFamily {
    pub const fn token(self) -> &'static str {
        match self {
            Self::AccountingRegister => "AccountingRegister",
            Self::AccumulationRegister => "AccumulationRegister",
            Self::BusinessProcess => "BusinessProcess",
            Self::CalculationRegister => "CalculationRegister",
            Self::Catalog => "Catalog",
            Self::ChartOfAccounts => "ChartOfAccounts",
            Self::ChartOfCalculationTypes => "ChartOfCalculationTypes",
            Self::ChartOfCharacteristicTypes => "ChartOfCharacteristicTypes",
            Self::DataProcessor => "DataProcessor",
            Self::Document => "Document",
            Self::Enum => "Enum",
            Self::ExchangePlan => "ExchangePlan",
            Self::InformationRegister => "InformationRegister",
            Self::Recalculation => "Recalculation",
            Self::Report => "Report",
            Self::Sequence => "Sequence",
            Self::Task => "Task",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [
            Self::AccountingRegister,
            Self::AccumulationRegister,
            Self::BusinessProcess,
            Self::CalculationRegister,
            Self::Catalog,
            Self::ChartOfAccounts,
            Self::ChartOfCalculationTypes,
            Self::ChartOfCharacteristicTypes,
            Self::DataProcessor,
            Self::Document,
            Self::Enum,
            Self::ExchangePlan,
            Self::InformationRegister,
            Self::Recalculation,
            Self::Report,
            Self::Sequence,
            Self::Task,
        ]
        .into_iter()
        .find(|family| family.token() == value)
    }

    /// Whether this family emits the given non-reference generated owner role.
    /// This is intentionally a compatibility matrix, not a family×role
    /// product: accepting a plausible-looking but unproduced QName would make
    /// downstream form owner resolution over-broad.
    const fn allows_role(self, role: GeneratedMetadataOwnerRole) -> bool {
        matches!(
            (self, role),
            (
                Self::AccountingRegister,
                GeneratedMetadataOwnerRole::Object
                    | GeneratedMetadataOwnerRole::Record
                    | GeneratedMetadataOwnerRole::RecordSet
                    | GeneratedMetadataOwnerRole::RecordKey
            ) | (
                Self::AccumulationRegister,
                GeneratedMetadataOwnerRole::Object
                    | GeneratedMetadataOwnerRole::RecordSet
                    | GeneratedMetadataOwnerRole::RecordKey
            ) | (Self::BusinessProcess, GeneratedMetadataOwnerRole::Object)
                | (
                    Self::CalculationRegister,
                    GeneratedMetadataOwnerRole::Object
                        | GeneratedMetadataOwnerRole::Record
                        | GeneratedMetadataOwnerRole::RecordSet
                        | GeneratedMetadataOwnerRole::RecordKey
                )
                | (Self::Catalog, GeneratedMetadataOwnerRole::Object)
                | (Self::ChartOfAccounts, GeneratedMetadataOwnerRole::Object)
                | (
                    Self::ChartOfCalculationTypes,
                    GeneratedMetadataOwnerRole::Object
                )
                | (
                    Self::ChartOfCharacteristicTypes,
                    GeneratedMetadataOwnerRole::Object
                )
                | (Self::DataProcessor, GeneratedMetadataOwnerRole::Object)
                | (Self::Document, GeneratedMetadataOwnerRole::Object)
                | (
                    Self::InformationRegister,
                    GeneratedMetadataOwnerRole::Record
                        | GeneratedMetadataOwnerRole::RecordManager
                        | GeneratedMetadataOwnerRole::RecordSet
                        | GeneratedMetadataOwnerRole::RecordKey
                )
                | (
                    Self::Recalculation,
                    GeneratedMetadataOwnerRole::Record | GeneratedMetadataOwnerRole::RecordSet
                )
                | (
                    Self::Sequence,
                    GeneratedMetadataOwnerRole::Record | GeneratedMetadataOwnerRole::RecordSet
                )
                | (Self::ExchangePlan, GeneratedMetadataOwnerRole::Object)
                | (Self::Report, GeneratedMetadataOwnerRole::Object)
                | (Self::Task, GeneratedMetadataOwnerRole::Object)
        )
    }
}

impl GeneratedMetadataOwnerRole {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Ref => "Ref",
            Self::Object => "Object",
            Self::Record => "Record",
            Self::RecordManager => "RecordManager",
            Self::RecordSet => "RecordSet",
            Self::RecordKey => "RecordKey",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Ref,
            Self::Object,
            Self::Record,
            Self::RecordManager,
            Self::RecordSet,
            Self::RecordKey,
        ]
        .into_iter()
        .find(|role| role.token() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedMetadataOwner<'a> {
    family: GeneratedMetadataOwnerFamily,
    role: GeneratedMetadataOwnerRole,
    name: &'a str,
}
impl<'a> GeneratedMetadataOwner<'a> {
    pub const fn family(self) -> GeneratedMetadataOwnerFamily {
        self.family
    }
    pub const fn role(self) -> GeneratedMetadataOwnerRole {
        self.role
    }
    pub const fn name(self) -> &'a str {
        self.name
    }
    pub fn owner_reference(self) -> String {
        format!("{}.{}", self.family.token(), self.name)
    }
}
pub fn parse_generated_metadata_owner(value: &str) -> Option<GeneratedMetadataOwner<'_>> {
    let value = value.strip_prefix("cfg:")?;
    let (generated, name) = value.split_once('.')?;
    let role = [
        GeneratedMetadataOwnerRole::RecordManager,
        GeneratedMetadataOwnerRole::RecordSet,
        GeneratedMetadataOwnerRole::RecordKey,
        GeneratedMetadataOwnerRole::Object,
        GeneratedMetadataOwnerRole::Record,
        GeneratedMetadataOwnerRole::Ref,
    ]
    .into_iter()
    .find(|role| generated.ends_with(role.token()))?;
    let family_token = generated.strip_suffix(role.token())?;
    // Reference QNames have an intentionally smaller supported family set.
    if role == GeneratedMetadataOwnerRole::Ref
        && GeneratedMetadataReferenceOwnerKind::parse(family_token).is_none()
    {
        return None;
    }
    let family = GeneratedMetadataOwnerFamily::parse(family_token)?;
    if role != GeneratedMetadataOwnerRole::Ref && !family.allows_role(role) {
        return None;
    }
    (!name.is_empty() && name.trim() == name && !name.contains('.'))
        .then_some(GeneratedMetadataOwner { family, role, name })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataDataPathRole {
    Attribute,
    Dimension,
    Resource,
    TabularSection,
    TabularAttribute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataDataPath<'a> {
    family: GeneratedMetadataOwnerFamily,
    owner_name: &'a str,
    role: MetadataDataPathRole,
    table_name: Option<&'a str>,
    member_name: &'a str,
}

impl<'a> MetadataDataPath<'a> {
    pub fn owner_reference(self) -> String {
        format!("{}.{}", self.family.token(), self.owner_name)
    }
    pub const fn role(self) -> MetadataDataPathRole {
        self.role
    }
    pub const fn table_name(self) -> Option<&'a str> {
        self.table_name
    }
    pub const fn member_name(self) -> &'a str {
        self.member_name
    }
}

/// Parse one exact metadata data path used by form bindings.
pub fn parse_metadata_data_path(value: &str) -> Option<MetadataDataPath<'_>> {
    let parts = value.split('.').collect::<Vec<_>>();
    let valid_name = |name: &&str| !name.is_empty() && name.trim() == *name;
    let (family, owner_name) = match parts.as_slice() {
        [family, owner, ..] if valid_name(family) && valid_name(owner) => {
            (GeneratedMetadataOwnerFamily::parse(family)?, *owner)
        }
        _ => return None,
    };
    let (role, table_name, member_name) = match &parts[2..] {
        ["Attribute", name] | ["Dimension", name] | ["Resource", name] if valid_name(name) => {
            let role = match parts[2] {
                "Attribute" => MetadataDataPathRole::Attribute,
                "Dimension" => MetadataDataPathRole::Dimension,
                _ => MetadataDataPathRole::Resource,
            };
            (role, None, *name)
        }
        ["TabularSection", table] if valid_name(table) => {
            (MetadataDataPathRole::TabularSection, Some(*table), *table)
        }
        ["TabularSection", table, "Attribute", name] if valid_name(table) && valid_name(name) => {
            (MetadataDataPathRole::TabularAttribute, Some(*table), *name)
        }
        _ => return None,
    };
    Some(MetadataDataPath {
        family,
        owner_name,
        role,
        table_name,
        member_name,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusSource {
    pub product: String,
    pub release: String,
    pub derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub bundles: usize,
    pub model_types: usize,
    pub importers: usize,
    pub exporters: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInventory {
    pub symbolic_name: String,
    pub version: Option<String>,
    pub model_types: Vec<String>,
    pub importers: Vec<String>,
    pub exporters: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInventory {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: InventorySummary,
    pub bundles: Vec<BundleInventory>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFeatureSummary {
    pub packages: usize,
    pub classifiers: usize,
    pub features: usize,
    pub operations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFeatureCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: PackageFeatureSummary,
    pub packages: Vec<ModelPackage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackage {
    pub bundle: String,
    pub package_class: String,
    pub name: Option<String>,
    pub namespace_uri: Option<String>,
    pub namespace_prefix: Option<String>,
    pub classifiers: Vec<ModelClassifier>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelClassifier {
    pub token: String,
    pub id: i32,
    pub feature_count: Option<i32>,
    pub operation_count: Option<i32>,
    pub features: Vec<ModelMember>,
    pub operations: Vec<ModelMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMember {
    pub token: String,
    pub id: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterRuleCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub rules: Vec<WriterRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterRule {
    pub id: String,
    pub source_class: String,
    pub model_type: String,
    pub feature: String,
    pub operations: Vec<String>,
    pub conditions: Vec<String>,
    pub delegate: Option<String>,
    #[serde(default)]
    pub policy: Option<WriterPolicy>,
    pub evidence: RuleEvidence,
}

const MAX_DCS_WRITER_EVIDENCE_JSON_BYTES: usize = 32 * 1024;
const MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES: usize = 4 * 1024;
const MAX_DCS_WRITER_EVIDENCE_FACTS: usize = 16;
const MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS: usize = 8;
const MAX_DCS_WRITER_EVIDENCE_SOURCES: usize = 8;
const DCS_SETTINGS_MODEL_NAMESPACE: &str = "http://g5.1c.ru/v8/dt/data-composition-system/settings";
const DCS_SETTINGS_CLASSIFIER: &str = "DataCompositionSettings";
const DCS_STANDALONE_QNAME_EVIDENCE_SOURCES: &[&str] = &[
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.resource.DcssResource#doSave:139-157",
];
const DCS_FORM_QNAME_EVIDENCE_SOURCES: &[&str] = &[
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.form.export.xml/com._1c.g5.v8.dt.form.export.xml.writer.ListSettingsWriter#write:92-103",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.form.export.xml/com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider#fillSpecifiedPackageNsUri:0-10",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.form.export.xml/com._1c.g5.v8.dt.internal.form.export.xml.FormFeatureNameProvider#fillSpecifiedFeatureNames",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.export.xml/com._1c.g5.v8.dt.export.xml.BaseQNameProvider#getElementQName:0-57",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.export.xml/com._1c.g5.v8.dt.export.xml.BaseQNameProvider#needToCapitalizeFirstLetterOfFeatureName:0-1",
];
const DCS_NO_TYPE_ID_EVIDENCE_SOURCES: &[&str] = &[
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.resource.DcssResource#doSave:139-157",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.form.export.xml/com._1c.g5.v8.dt.form.export.xml.writer.ListSettingsWriter#write:92-103",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.util.DcsV8Serializer#writeSettings(ExportContextXmlStreamWriter,DataCompositionSettings,QName,IDtProject):0-52",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.util.DcsV8Serializer#writeSettings(ExportContextXmlStreamWriter,DataCompositionSettings,QName,Version,Map):0-392",
];
const DCS_OPAQUE_NEGATIVE_EVIDENCE_SOURCES: &[&str] = &[
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.util.DcsV8Serializer#readSettings:24-483",
    "edt-derived://2025.2.3+30/com._1c.g5.v8.dt.dcs/com._1c.g5.v8.dt.dcs.util.DcsV8Serializer#readSettings:445-455",
];

/// Verified field identity for the only schema-driven Form `ListSettings` tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsListSettingsTailField {
    ItemsViewMode,
    ItemsUserSettingId,
}

/// Exact verified policy for the two final Form `ListSettings` children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsListSettingsTailPolicy {
    namespace_uri: String,
    tail_order: [DcsListSettingsTailField; 2],
    items_view_mode_qname: String,
    items_view_mode_default: String,
    items_user_setting_id_qname: String,
    items_user_setting_id_default: String,
}

/// Exact physical wrappers and type-qualification rule for bounded DCS output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSettingsSerializationPolicy {
    standalone_document_qname: String,
    form_list_settings_qname: String,
}

/// Exact, bounded standalone settings subtrees which may be retained from an
/// authenticated source document but may not be synthesized or mutated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSettingsSourceOwnedPolicy {
    namespace_uri: String,
    core_namespace_uri: String,
    xml_schema_namespace_uri: String,
    xsi_namespace_uri: String,
    data_parameters_qname: String,
    data_parameter_item_qname: String,
    settings_parameter_value_type_qname: String,
    parameter_qname: String,
    parameter_name: String,
    value_qname: String,
    value_type_qname: String,
    value: String,
    order_qname: String,
    structure_item_qname: String,
    structure_item_group_type_qname: String,
    structure_order_qname: String,
    structure_order_item_qname: String,
    order_item_auto_type_qname: String,
    structure_selection_qname: String,
    structure_selection_item_qname: String,
    selected_item_auto_type_qname: String,
    max_data_parameter_items: usize,
    max_structure_items: usize,
}

/// Semantic document roles in the bounded native DCS schema-template body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsSchemaTemplateEnvelopeDocumentRole {
    PrimarySchemaFile,
    Settings,
    TerminalSchemaFile,
}

/// Exact, platform-authenticated framing and positional-binding policy for a
/// DCS schema-template body. The schema shell remains source-owned; this type
/// owns only the evidenced envelope and delegation to standalone Settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSchemaTemplateEnvelopePolicy {
    schema_namespace_uri: String,
    settings_namespace_uri: String,
    source_root_qname: String,
    source_settings_variant_qname: String,
    source_inline_settings_qname: String,
    native_schema_file_qname: String,
    native_schema_qname: String,
    native_settings_qname: String,
    header_marker: u32,
    settings_count_offset_bytes: usize,
    stored_lengths_offset_bytes: usize,
    stored_length_width_bytes: usize,
    minimum_attested_settings_variants: usize,
    maximum_attested_settings_variants: usize,
}

impl DcsSchemaTemplateEnvelopePolicy {
    pub fn schema_namespace_uri(&self) -> &str {
        &self.schema_namespace_uri
    }

    pub fn settings_namespace_uri(&self) -> &str {
        &self.settings_namespace_uri
    }

    pub fn source_root_qname(&self) -> &str {
        &self.source_root_qname
    }

    pub fn source_settings_variant_qname(&self) -> &str {
        &self.source_settings_variant_qname
    }

    pub fn source_inline_settings_qname(&self) -> &str {
        &self.source_inline_settings_qname
    }

    pub fn native_schema_file_qname(&self) -> &str {
        &self.native_schema_file_qname
    }

    pub fn native_schema_qname(&self) -> &str {
        &self.native_schema_qname
    }

    pub fn native_settings_qname(&self) -> &str {
        &self.native_settings_qname
    }

    pub const fn header_marker(&self) -> u32 {
        self.header_marker
    }

    pub const fn settings_count_offset_bytes(&self) -> usize {
        self.settings_count_offset_bytes
    }

    pub const fn stored_lengths_offset_bytes(&self) -> usize {
        self.stored_lengths_offset_bytes
    }

    pub const fn stored_length_width_bytes(&self) -> usize {
        self.stored_length_width_bytes
    }

    pub const fn min_attested_settings_variants(&self) -> usize {
        self.minimum_attested_settings_variants
    }

    pub const fn max_attested_settings_variants(&self) -> usize {
        self.maximum_attested_settings_variants
    }

    pub const fn settings_count_is_little_endian_u32(&self) -> bool {
        true
    }

    pub const fn stored_lengths_are_little_endian_u64(&self) -> bool {
        true
    }

    pub const fn stored_lengths_cover_primary_and_each_settings(&self) -> bool {
        true
    }

    pub const fn terminal_schema_file_consumes_remaining_bytes(&self) -> bool {
        true
    }

    pub const fn documents_require_utf8_bom(&self) -> bool {
        true
    }

    pub const fn settings_bind_positionally(&self) -> bool {
        true
    }

    pub const fn source_variants_must_be_direct_root_children(&self) -> bool {
        true
    }

    pub const fn terminal_schema_file_is_empty(&self) -> bool {
        true
    }

    pub const fn supports_attested_settings_variant_count(&self, count: usize) -> bool {
        count >= self.minimum_attested_settings_variants
            && count <= self.maximum_attested_settings_variants
    }

    /// Returns the role for a physical document index, where every Settings
    /// document repeats between the primary and terminal SchemaFile.
    pub const fn document_role(
        &self,
        settings_count: usize,
        document_index: usize,
    ) -> Option<DcsSchemaTemplateEnvelopeDocumentRole> {
        if !self.supports_attested_settings_variant_count(settings_count) {
            return None;
        }
        if document_index == 0 {
            Some(DcsSchemaTemplateEnvelopeDocumentRole::PrimarySchemaFile)
        } else if document_index <= settings_count {
            Some(DcsSchemaTemplateEnvelopeDocumentRole::Settings)
        } else if document_index == settings_count + 1 {
            Some(DcsSchemaTemplateEnvelopeDocumentRole::TerminalSchemaFile)
        } else {
            None
        }
    }

    /// The header stores one length for the primary SchemaFile and one for
    /// every Settings document; it deliberately omits the terminal length.
    pub const fn stored_length_count(&self, settings_count: usize) -> Option<usize> {
        if self.supports_attested_settings_variant_count(settings_count) {
            settings_count.checked_add(1)
        } else {
            None
        }
    }

    pub const fn header_size_bytes(&self, settings_count: usize) -> Option<usize> {
        let length_count = match self.stored_length_count(settings_count) {
            Some(count) => count,
            None => return None,
        };
        match length_count.checked_mul(self.stored_length_width_bytes) {
            Some(length_bytes) => self.stored_lengths_offset_bytes.checked_add(length_bytes),
            None => None,
        }
    }
}

impl DcsSettingsSourceOwnedPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
    pub fn core_namespace_uri(&self) -> &str {
        &self.core_namespace_uri
    }
    pub fn xml_schema_namespace_uri(&self) -> &str {
        &self.xml_schema_namespace_uri
    }
    pub fn xsi_namespace_uri(&self) -> &str {
        &self.xsi_namespace_uri
    }
    pub fn data_parameters_qname(&self) -> &str {
        &self.data_parameters_qname
    }
    pub fn data_parameter_item_qname(&self) -> &str {
        &self.data_parameter_item_qname
    }
    pub fn settings_parameter_value_type_qname(&self) -> &str {
        &self.settings_parameter_value_type_qname
    }
    pub fn parameter_qname(&self) -> &str {
        &self.parameter_qname
    }
    pub fn parameter_name(&self) -> &str {
        &self.parameter_name
    }
    pub fn value_qname(&self) -> &str {
        &self.value_qname
    }
    pub fn value_type_qname(&self) -> &str {
        &self.value_type_qname
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn order_qname(&self) -> &str {
        &self.order_qname
    }
    pub fn structure_item_qname(&self) -> &str {
        &self.structure_item_qname
    }
    pub fn structure_item_group_type_qname(&self) -> &str {
        &self.structure_item_group_type_qname
    }
    pub fn structure_order_qname(&self) -> &str {
        &self.structure_order_qname
    }
    pub fn structure_order_item_qname(&self) -> &str {
        &self.structure_order_item_qname
    }
    pub fn order_item_auto_type_qname(&self) -> &str {
        &self.order_item_auto_type_qname
    }
    pub fn structure_selection_qname(&self) -> &str {
        &self.structure_selection_qname
    }
    pub fn structure_selection_item_qname(&self) -> &str {
        &self.structure_selection_item_qname
    }
    pub fn selected_item_auto_type_qname(&self) -> &str {
        &self.selected_item_auto_type_qname
    }
    pub const fn max_data_parameter_items(&self) -> usize {
        self.max_data_parameter_items
    }
    pub const fn max_structure_items(&self) -> usize {
        self.max_structure_items
    }
    pub const fn is_standalone_only(&self) -> bool {
        true
    }
    pub const fn data_parameters_follows_selection_and_precedes_order(&self) -> bool {
        true
    }
    pub const fn structure_item_is_terminal(&self) -> bool {
        true
    }
    pub const fn unknown_children_are_unsupported(&self) -> bool {
        true
    }
    pub const fn generic_opaque_emission_is_forbidden(&self) -> bool {
        true
    }
}

/// Exact QName and placement policy authenticated by the immutable 8.3.27
/// XML 2.20 DCS micro-CF. Patch build is provenance, not a dialect selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSelectionPolicy {
    namespace_uri: String,
    selection_qname: String,
    item_qname: String,
    field_qname: String,
    field_type_qname: String,
    auto_type_qname: String,
}

/// Exact order QName, item sequence, and context policy authenticated by
/// standalone DCS and Form round-trips on the common 8.3.27/XML 2.20 contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsOrderPolicy {
    namespace_uri: String,
    order_qname: String,
    storage_order_qname: String,
    item_qname: String,
    use_qname: String,
    field_qname: String,
    order_type_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    field_type_qname: String,
    supported_order_types: Vec<String>,
    max_emitted_items: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    storage_record_type_uuid: String,
}

/// Exact filter QName, type, ordering, default, and storage policy proven by
/// clean-room standalone and Form round-trips on 8.3.27/XML 2.20.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsFilterPolicy {
    namespace_uri: String,
    core_namespace_uri: String,
    xml_schema_namespace_uri: String,
    filter_qname: String,
    storage_filter_qname: String,
    item_qname: String,
    use_qname: String,
    left_qname: String,
    comparison_type_qname: String,
    right_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    comparison_item_type_qname: String,
    left_field_type_qname: String,
    right_string_type_qname: String,
    supported_comparison_types: Vec<String>,
    max_emitted_items: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    comparison_storage_record_type_uuid: String,
}

impl DcsFilterPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
    pub fn core_namespace_uri(&self) -> &str {
        &self.core_namespace_uri
    }
    pub fn xml_schema_namespace_uri(&self) -> &str {
        &self.xml_schema_namespace_uri
    }
    pub fn filter_qname(&self) -> &str {
        &self.filter_qname
    }
    pub fn storage_filter_qname(&self) -> &str {
        &self.storage_filter_qname
    }
    pub fn item_qname(&self) -> &str {
        &self.item_qname
    }
    pub fn use_qname(&self) -> &str {
        &self.use_qname
    }
    pub fn left_qname(&self) -> &str {
        &self.left_qname
    }
    pub fn comparison_type_qname(&self) -> &str {
        &self.comparison_type_qname
    }
    pub fn right_qname(&self) -> &str {
        &self.right_qname
    }
    pub fn view_mode_qname(&self) -> &str {
        &self.view_mode_qname
    }
    pub fn user_setting_id_qname(&self) -> &str {
        &self.user_setting_id_qname
    }
    pub fn comparison_item_type_qname(&self) -> &str {
        &self.comparison_item_type_qname
    }
    pub fn left_field_type_qname(&self) -> &str {
        &self.left_field_type_qname
    }
    pub fn right_string_type_qname(&self) -> &str {
        &self.right_string_type_qname
    }
    pub fn supported_comparison_types(&self) -> &[String] {
        &self.supported_comparison_types
    }
    pub const fn supported_use_values(&self) -> &'static [Option<bool>] {
        &[None]
    }
    pub const fn max_emitted_items(&self) -> usize {
        self.max_emitted_items
    }
    pub fn supported_view_modes(&self) -> &[String] {
        &self.supported_view_modes
    }
    pub fn metadata_only_user_setting_id(&self) -> &str {
        &self.metadata_only_user_setting_id
    }
    pub fn comparison_storage_record_type_uuid(&self) -> &str {
        &self.comparison_storage_record_type_uuid
    }
    pub const fn follows_selection_and_precedes_order_and_structure_items(&self) -> bool {
        true
    }
    pub const fn propertyless_empty_filter_is_unsupported(&self) -> bool {
        true
    }
    pub const fn metadata_only_filter_requires_view_mode_and_user_setting_id(&self) -> bool {
        true
    }
    pub const fn metadata_only_storage_property_is_absent(&self) -> bool {
        true
    }
}

/// Exact conditional-appearance QName, type, default, placement, and storage
/// policy proven by one clean-room standalone/Form two-round cohort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsConditionalAppearancePolicy {
    namespace_uri: String,
    core_namespace_uri: String,
    ui_namespace_uri: String,
    web_color_namespace_uri: String,
    xml_schema_namespace_uri: String,
    conditional_appearance_qname: String,
    storage_conditional_appearance_qname: String,
    item_qname: String,
    selection_qname: String,
    field_qname: String,
    filter_qname: String,
    appearance_qname: String,
    core_item_qname: String,
    parameter_qname: String,
    value_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    parameter_value_type_qname: String,
    color_type_qname: String,
    max_emitted_items: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    storage_property_name: String,
    storage_record_type_uuid: String,
}

impl DcsConditionalAppearancePolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
    pub fn core_namespace_uri(&self) -> &str {
        &self.core_namespace_uri
    }
    pub fn ui_namespace_uri(&self) -> &str {
        &self.ui_namespace_uri
    }
    pub fn web_color_namespace_uri(&self) -> &str {
        &self.web_color_namespace_uri
    }
    pub fn xml_schema_namespace_uri(&self) -> &str {
        &self.xml_schema_namespace_uri
    }
    pub fn conditional_appearance_qname(&self) -> &str {
        &self.conditional_appearance_qname
    }
    pub fn storage_conditional_appearance_qname(&self) -> &str {
        &self.storage_conditional_appearance_qname
    }
    pub fn item_qname(&self) -> &str {
        &self.item_qname
    }
    pub fn selection_qname(&self) -> &str {
        &self.selection_qname
    }
    pub fn field_qname(&self) -> &str {
        &self.field_qname
    }
    pub fn filter_qname(&self) -> &str {
        &self.filter_qname
    }
    pub fn appearance_qname(&self) -> &str {
        &self.appearance_qname
    }
    pub fn core_item_qname(&self) -> &str {
        &self.core_item_qname
    }
    pub fn parameter_qname(&self) -> &str {
        &self.parameter_qname
    }
    pub fn value_qname(&self) -> &str {
        &self.value_qname
    }
    pub fn view_mode_qname(&self) -> &str {
        &self.view_mode_qname
    }
    pub fn user_setting_id_qname(&self) -> &str {
        &self.user_setting_id_qname
    }
    pub fn parameter_value_type_qname(&self) -> &str {
        &self.parameter_value_type_qname
    }
    pub fn color_type_qname(&self) -> &str {
        &self.color_type_qname
    }
    pub const fn max_emitted_items(&self) -> usize {
        self.max_emitted_items
    }
    pub fn supported_view_modes(&self) -> &[String] {
        &self.supported_view_modes
    }
    pub fn metadata_only_user_setting_id(&self) -> &str {
        &self.metadata_only_user_setting_id
    }
    pub fn storage_property_name(&self) -> &str {
        &self.storage_property_name
    }
    pub fn storage_record_type_uuid(&self) -> &str {
        &self.storage_record_type_uuid
    }
    pub const fn follows_filter_and_order_and_precedes_structure_items(&self) -> bool {
        true
    }
    pub const fn metadata_only_storage_property_is_absent(&self) -> bool {
        true
    }
    pub const fn empty_nested_filter_is_unsupported(&self) -> bool {
        true
    }
}

/// Exact physical envelope used by Form-wide
/// `Attributes/ConditionalAppearance`. The conditional-appearance body itself
/// remains owned by [`DcsConditionalAppearancePolicy`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsFormAttributesConditionalAppearancePolicy {
    form_namespace_uri: String,
    settings_namespace_uri: String,
    core_namespace_uri: String,
    style_namespace_uri: String,
    system_font_namespace_uri: String,
    core_data_namespace_uri: String,
    ui_namespace_uri: String,
    web_color_namespace_uri: String,
    windows_color_namespace_uri: String,
    xml_schema_namespace_uri: String,
    xsi_namespace_uri: String,
    wrapper_qname: String,
    storage_root_qname: String,
    storage_child_qname: String,
    storage_container_marker: String,
    storage_absent_container_marker: String,
    storage_inactive_marker: [String; 2],
    storage_active_marker: [String; 2],
    storage_field_order: [String; 2],
    storage_selection_type_indexes: [u32; 2],
    storage_filter_type_indexes: [u32; 1],
    max_emitted_items: usize,
}

impl DcsFormAttributesConditionalAppearancePolicy {
    pub fn form_namespace_uri(&self) -> &str {
        &self.form_namespace_uri
    }
    pub fn settings_namespace_uri(&self) -> &str {
        &self.settings_namespace_uri
    }
    pub fn core_namespace_uri(&self) -> &str {
        &self.core_namespace_uri
    }
    pub fn style_namespace_uri(&self) -> &str {
        &self.style_namespace_uri
    }
    pub fn system_font_namespace_uri(&self) -> &str {
        &self.system_font_namespace_uri
    }
    pub fn core_data_namespace_uri(&self) -> &str {
        &self.core_data_namespace_uri
    }
    pub fn ui_namespace_uri(&self) -> &str {
        &self.ui_namespace_uri
    }
    pub fn web_color_namespace_uri(&self) -> &str {
        &self.web_color_namespace_uri
    }
    pub fn windows_color_namespace_uri(&self) -> &str {
        &self.windows_color_namespace_uri
    }
    pub fn xml_schema_namespace_uri(&self) -> &str {
        &self.xml_schema_namespace_uri
    }
    pub fn xsi_namespace_uri(&self) -> &str {
        &self.xsi_namespace_uri
    }
    pub fn wrapper_qname(&self) -> &str {
        &self.wrapper_qname
    }
    pub fn storage_root_qname(&self) -> &str {
        &self.storage_root_qname
    }
    pub fn storage_child_qname(&self) -> &str {
        &self.storage_child_qname
    }
    pub fn storage_container_marker(&self) -> &str {
        &self.storage_container_marker
    }
    pub fn storage_absent_container_marker(&self) -> &str {
        &self.storage_absent_container_marker
    }
    pub fn storage_inactive_marker(&self) -> &[String; 2] {
        &self.storage_inactive_marker
    }
    pub fn storage_active_marker(&self) -> &[String; 2] {
        &self.storage_active_marker
    }
    pub fn storage_field_order(&self) -> &[String; 2] {
        &self.storage_field_order
    }
    pub const fn storage_selection_type_indexes(&self) -> &[u32; 2] {
        &self.storage_selection_type_indexes
    }
    pub const fn storage_filter_type_indexes(&self) -> &[u32; 1] {
        &self.storage_filter_type_indexes
    }
    pub const fn max_emitted_items(&self) -> usize {
        self.max_emitted_items
    }
    pub const fn follows_all_attributes(&self) -> bool {
        true
    }
    pub const fn uses_unkeyed_direct_base64_tail(&self) -> bool {
        true
    }
    pub const fn has_storage_record_type_uuid(&self) -> bool {
        false
    }
    pub const fn container_metadata_is_forbidden(&self) -> bool {
        true
    }
}

impl DcsOrderPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
    pub fn order_qname(&self) -> &str {
        &self.order_qname
    }
    pub fn storage_order_qname(&self) -> &str {
        &self.storage_order_qname
    }
    pub fn item_qname(&self) -> &str {
        &self.item_qname
    }
    pub fn use_qname(&self) -> &str {
        &self.use_qname
    }
    pub fn field_qname(&self) -> &str {
        &self.field_qname
    }
    pub fn order_type_qname(&self) -> &str {
        &self.order_type_qname
    }
    pub fn view_mode_qname(&self) -> &str {
        &self.view_mode_qname
    }
    pub fn user_setting_id_qname(&self) -> &str {
        &self.user_setting_id_qname
    }
    pub fn field_type_qname(&self) -> &str {
        &self.field_type_qname
    }
    pub const fn follows_selection_and_precedes_structure_items(&self) -> bool {
        true
    }
    pub const fn propertyless_empty_order_is_unsupported(&self) -> bool {
        true
    }
    pub const fn metadata_only_order_requires_view_mode_and_user_setting_id(&self) -> bool {
        true
    }
    pub const fn root_auto_is_unsupported(&self) -> bool {
        true
    }
    pub fn supported_order_types(&self) -> &[String] {
        &self.supported_order_types
    }
    pub const fn supported_use_values(&self) -> &'static [Option<bool>] {
        &[None, Some(false)]
    }
    pub const fn max_emitted_items(&self) -> usize {
        self.max_emitted_items
    }
    pub fn supported_view_modes(&self) -> &[String] {
        &self.supported_view_modes
    }
    pub fn metadata_only_user_setting_id(&self) -> &str {
        &self.metadata_only_user_setting_id
    }
    pub fn storage_record_type_uuid(&self) -> &str {
        &self.storage_record_type_uuid
    }
}

impl DcsSelectionPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
    pub fn selection_qname(&self) -> &str {
        &self.selection_qname
    }
    pub fn item_qname(&self) -> &str {
        &self.item_qname
    }
    pub fn field_qname(&self) -> &str {
        &self.field_qname
    }
    pub fn field_type_qname(&self) -> &str {
        &self.field_type_qname
    }
    pub fn auto_type_qname(&self) -> &str {
        &self.auto_type_qname
    }
    /// The evidenced root selection precedes `order` and structure `item`s.
    pub const fn precedes_order_and_structure_items(&self) -> bool {
        true
    }
    /// An explicitly empty selection was not present in the platform corpus.
    pub const fn empty_selection_is_unsupported(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSelectionEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsSelectionEvidenceSource,
    policy: DcsSelectionEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSelectionEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    platform_version: String,
    source_version: String,
    fixture_id: String,
    raw_body_sha256: String,
    native_xml_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSelectionEvidencePolicy {
    namespace: String,
    selection_qname: String,
    item_qname: String,
    field_qname: String,
    field_type_qname: String,
    auto_type_qname: String,
    settings_placement: String,
    empty_selection_emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsSourceOwnedEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsSettingsSourceOwnedContractSource,
    sources: DcsSettingsSourceOwnedEvidenceSources,
    policy: DcsSettingsSourceOwnedEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsSourceOwnedContractSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsSourceOwnedEvidenceSources {
    baseline: DcsSettingsSourceOwnedPositiveSource,
    data_parameters: DcsSettingsDataParametersPositiveSource,
    unknown_child_negative: DcsSettingsUnknownChildNegativeSource,
    platform_line: String,
    source_version: String,
    ibcmd_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsSourceOwnedPositiveSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    native_xml_sha256: String,
    packed_body_sha256: String,
    unpacked_body_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsDataParametersPositiveSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    native_xml_sha256: String,
    packed_body_sha256: String,
    unpacked_body_sha256: String,
    fragment_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsUnknownChildNegativeSource {
    product: String,
    release: String,
    derivation: String,
    evidence_key: String,
    reader_outcome: String,
    production_outcome: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSettingsSourceOwnedEvidencePolicy {
    namespace: String,
    core_namespace: String,
    xml_schema_namespace: String,
    xsi_namespace: String,
    data_parameters_qname: String,
    data_parameter_item_qname: String,
    settings_parameter_value_type_qname: String,
    parameter_qname: String,
    parameter_name: String,
    value_qname: String,
    value_type_qname: String,
    value: String,
    order_qname: String,
    structure_item_qname: String,
    structure_item_group_type_qname: String,
    structure_order_qname: String,
    structure_order_item_qname: String,
    order_item_auto_type_qname: String,
    structure_selection_qname: String,
    structure_selection_item_qname: String,
    selected_item_auto_type_qname: String,
    root_child_order: Vec<String>,
    data_parameter_item_child_order: Vec<String>,
    structure_item_child_order: Vec<String>,
    structure_order_child_order: Vec<String>,
    structure_selection_child_order: Vec<String>,
    data_parameters_placement: String,
    structure_item_placement: String,
    scope: String,
    max_data_parameter_items: usize,
    max_structure_items: usize,
    unknown_children: String,
    generic_opaque_emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSchemaTemplateEnvelopeEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsSchemaTemplateEnvelopeEvidenceSource,
    fixture: DcsSchemaTemplateEnvelopeEvidenceFixture,
    policy: DcsSchemaTemplateEnvelopeEvidencePolicy,
    two_variant_shape: DcsSchemaTemplateEnvelopeTwoVariantShape,
    proven_claims: Vec<String>,
    non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSchemaTemplateEnvelopeEvidenceSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSchemaTemplateEnvelopeEvidenceFixture {
    fixture_id: String,
    platform_line: String,
    platform_version: String,
    source_version: String,
    ibcmd_sha256: String,
    extractor_identity: String,
    extractor_sha256: String,
    seed_sha256: String,
    round1_cf_sha256: String,
    round2_cf_sha256: String,
    round1_template_sha256: String,
    round2_template_sha256: String,
    round1_packed_sha256: String,
    round2_packed_sha256: String,
    round1_unpacked_sha256: String,
    round2_unpacked_sha256: String,
    configuration_encoded_sha256: String,
    configuration_decoded_sha256: String,
    raw_packed_encoded_sha256: String,
    raw_packed_decoded_sha256: String,
    raw_unpacked_encoded_sha256: String,
    raw_unpacked_decoded_sha256: String,
    native_xml_encoded_sha256: String,
    native_xml_decoded_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSchemaTemplateEnvelopeEvidencePolicy {
    schema_namespace: String,
    settings_namespace: String,
    source_root_qname: String,
    source_settings_variant_qname: String,
    source_inline_settings_qname: String,
    native_schema_file_qname: String,
    native_schema_qname: String,
    native_settings_qname: String,
    header_marker: u32,
    settings_count_offset_bytes: usize,
    settings_count_encoding: String,
    stored_lengths_offset_bytes: usize,
    stored_length_width_bytes: usize,
    stored_length_encoding: String,
    stored_length_roles: Vec<String>,
    document_roles: Vec<String>,
    minimum_attested_settings_variants: usize,
    maximum_attested_settings_variants: usize,
    stored_lengths_cover: String,
    terminal_document_framing: String,
    document_encoding: String,
    settings_binding: String,
    source_variant_placement: String,
    terminal_schema_file_shape: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsSchemaTemplateEnvelopeTwoVariantShape {
    settings_document_count: usize,
    header_bytes: usize,
    stored_document_lengths: Vec<u64>,
    trailing_document_length: u64,
    document_roles: Vec<String>,
    document_sha256: Vec<String>,
    variant_names: Vec<String>,
    source_variant_order_matches_external_settings_order: bool,
    terminal_schema_file_is_empty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsOrderEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsOrderContractSource,
    sources: DcsOrderEvidenceSources,
    policy: DcsOrderEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsOrderContractSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsOrderEvidenceSources {
    standalone: DcsOrderEvidenceSource,
    form: DcsFormOrderEvidenceSource,
    form_metadata_only: DcsMetadataOnlyOrderEvidenceSource,
    unica_desc: DcsUnicaOrderEvidenceSource,
    source_version: String,
    platform_line: String,
    ibcmd_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsOrderEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    raw_body_sha256: String,
    native_xml_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormOrderEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    raw_body_sha256: String,
    native_xml_sha256: String,
    storage_order_sha256: String,
    embedded_order_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsMetadataOnlyOrderEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    fragment_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsUnicaOrderEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    repository_revision: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsOrderEvidencePolicy {
    namespace: String,
    order_qname: String,
    storage_order_qname: String,
    item_qname: String,
    use_qname: String,
    field_qname: String,
    order_type_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    field_type_qname: String,
    settings_placement: String,
    item_child_order: Vec<String>,
    order_child_order: Vec<String>,
    supported_order_types: Vec<String>,
    supported_use_values: Vec<String>,
    max_emitted_items: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    storage_record_type_uuid: String,
    propertyless_empty_order_emission: String,
    metadata_only_order_emission: String,
    root_auto_emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsFilterContractSource,
    sources: DcsFilterEvidenceSources,
    policy: DcsFilterEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterContractSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterEvidenceSources {
    comparison: DcsFilterComparisonEvidenceSource,
    metadata_only: DcsFilterMetadataOnlyEvidenceSource,
    unica_cross_evidence: DcsFilterUnicaEvidenceSource,
    source_version: String,
    platform_line: String,
    ibcmd_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterComparisonEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_raw_body_sha256: String,
    form_native_xml_sha256: String,
    form_storage_filter_sha256: String,
    form_embedded_filter_sha256: String,
    standalone_raw_body_sha256: String,
    standalone_native_xml_sha256: String,
    standalone_filter_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterMetadataOnlyEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_raw_body_sha256: String,
    form_native_xml_sha256: String,
    form_embedded_filter_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterUnicaEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    repository_revision: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFilterEvidencePolicy {
    namespace: String,
    core_namespace: String,
    xml_schema_namespace: String,
    filter_qname: String,
    storage_filter_qname: String,
    item_qname: String,
    use_qname: String,
    left_qname: String,
    comparison_type_qname: String,
    right_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    comparison_item_type_qname: String,
    left_field_type_qname: String,
    right_string_type_qname: String,
    settings_placement: String,
    item_child_order: Vec<String>,
    filter_child_order: Vec<String>,
    supported_comparison_types: Vec<String>,
    supported_use_values: Vec<String>,
    supported_right_types: Vec<String>,
    max_emitted_items: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    comparison_storage_record_type_uuid: String,
    metadata_only_embedded_emission: String,
    metadata_only_storage_representation: String,
    propertyless_empty_filter_emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceEvidenceCorpus {
    schema_version: u32,
    contract: String,
    source: DcsConditionalAppearanceContractSource,
    sources: DcsConditionalAppearanceEvidenceSources,
    policy: DcsConditionalAppearanceEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceContractSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceEvidenceSources {
    comparison: DcsConditionalAppearanceComparisonSource,
    metadata_only: DcsConditionalAppearanceMetadataOnlySource,
    unica_cross_evidence: DcsFilterUnicaEvidenceSource,
    source_version: String,
    platform_line: String,
    ibcmd_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceComparisonSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_raw_body_sha256: String,
    form_native_xml_sha256: String,
    form_storage_sha256: String,
    form_embedded_sha256: String,
    standalone_raw_body_sha256: String,
    standalone_native_xml_sha256: String,
    standalone_fragment_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceMetadataOnlySource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_embedded_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsConditionalAppearanceEvidencePolicy {
    namespace: String,
    core_namespace: String,
    ui_namespace: String,
    web_color_namespace: String,
    xml_schema_namespace: String,
    conditional_appearance_qname: String,
    storage_conditional_appearance_qname: String,
    item_qname: String,
    selection_qname: String,
    field_qname: String,
    filter_qname: String,
    appearance_qname: String,
    core_item_qname: String,
    parameter_qname: String,
    value_qname: String,
    view_mode_qname: String,
    user_setting_id_qname: String,
    parameter_value_type_qname: String,
    color_type_qname: String,
    settings_placement: String,
    item_child_order: Vec<String>,
    selection_child_order: Vec<String>,
    appearance_child_order: Vec<String>,
    container_child_order: Vec<String>,
    supported_parameters: Vec<String>,
    supported_values: Vec<String>,
    max_emitted_items: usize,
    max_selected_fields: usize,
    max_appearance_values: usize,
    supported_view_modes: Vec<String>,
    metadata_only_user_setting_id: String,
    storage_property_name: String,
    storage_record_type_uuid: String,
    metadata_only_embedded_emission: String,
    metadata_only_storage_representation: String,
    empty_nested_filter_emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormAttributesConditionalAppearanceEvidenceCorpus {
    schema_version: u32,
    contract: String,
    body_contract: String,
    source: DcsConditionalAppearanceContractSource,
    sources: DcsFormAttributesConditionalAppearanceEvidenceSources,
    policy: DcsFormAttributesConditionalAppearanceEvidencePolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormAttributesConditionalAppearanceEvidenceSources {
    comparison: DcsFormAttributesConditionalAppearanceComparisonSource,
    absent: DcsFormAttributesConditionalAppearanceAbsentSource,
    source_version: String,
    platform_line: String,
    ibcmd_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormAttributesConditionalAppearanceComparisonSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_raw_body_sha256: String,
    form_native_xml_sha256: String,
    wrapper_sha256: String,
    storage_settings_sha256: String,
    database_locale: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormAttributesConditionalAppearanceAbsentSource {
    product: String,
    release: String,
    derivation: String,
    fixture_id: String,
    form_raw_body_sha256: String,
    form_native_xml_sha256: String,
    round_trips: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormAttributesConditionalAppearanceEvidencePolicy {
    form_namespace: String,
    settings_namespace: String,
    core_namespace: String,
    style_namespace: String,
    system_font_namespace: String,
    core_data_namespace: String,
    ui_namespace: String,
    web_color_namespace: String,
    windows_color_namespace: String,
    xml_schema_namespace: String,
    xsi_namespace: String,
    wrapper_qname: String,
    storage_root_qname: String,
    storage_child_qname: String,
    wrapper_placement: String,
    storage_envelope: String,
    storage_container_marker: String,
    storage_absent_container_marker: String,
    storage_inactive_marker: [String; 2],
    storage_active_marker: [String; 2],
    storage_field_order: [String; 2],
    storage_selection_type_indexes: [u32; 2],
    storage_filter_type_indexes: [u32; 1],
    storage_record_type_uuid: Option<String>,
    container_metadata: String,
    absence_representation: String,
    max_emitted_items: usize,
}

impl DcsSelectionEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 16 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "selection evidence exceeds 16384 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-settings-selection-v1",
            ),
            (
                "platform line",
                self.source.platform_version.starts_with("8.3.27."),
            ),
            (
                "source product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "source release",
                self.source.release == self.source.platform_version,
            ),
            (
                "source derivation",
                self.source.derivation
                    == "two fresh isolated file-infobase round-trips with pinned ibcmd; immutable raw body and selected native XML retained; Unica supplied cross-evidence only",
            ),
            ("source version", self.source.source_version == "2.20"),
            (
                "fixture",
                self.source.fixture_id == "8.3.27.2214-xml-2.20-dcs-selection-auto",
            ),
            ("round trips", self.source.round_trips >= 2),
            ("namespace", self.policy.namespace == NS),
            (
                "selection QName",
                self.policy.selection_qname == format!("{{{NS}}}selection"),
            ),
            (
                "item QName",
                self.policy.item_qname == format!("{{{NS}}}item"),
            ),
            (
                "field QName",
                self.policy.field_qname == format!("{{{NS}}}field"),
            ),
            (
                "field type QName",
                self.policy.field_type_qname == format!("{{{NS}}}SelectedItemField"),
            ),
            (
                "auto type QName",
                self.policy.auto_type_qname == format!("{{{NS}}}SelectedItemAuto"),
            ),
            (
                "settings placement",
                self.policy.settings_placement == "before-order-and-structure-items",
            ),
            (
                "empty selection emission",
                self.policy.empty_selection_emission == "unsupported",
            ),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS selection {field} drifted"
            )));
        }
        for (field, digest) in [
            ("raw body SHA-256", self.source.raw_body_sha256.as_str()),
            ("native XML SHA-256", self.source.native_xml_sha256.as_str()),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                    "DCS selection {field} is invalid"
                )));
            }
        }
        Ok(())
    }

    fn into_policy(self) -> DcsSelectionPolicy {
        DcsSelectionPolicy {
            namespace_uri: self.policy.namespace,
            selection_qname: self.policy.selection_qname,
            item_qname: self.policy.item_qname,
            field_qname: self.policy.field_qname,
            field_type_qname: self.policy.field_type_qname,
            auto_type_qname: self.policy.auto_type_qname,
        }
    }
}

impl DcsSettingsSourceOwnedEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 24 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "settings source-owned evidence exceeds 24576 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        const CORE_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
        const XS_NS: &str = "http://www.w3.org/2001/XMLSchema";
        const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
        const IBCMD_SHA: &str = "11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b";
        let baseline = &self.sources.baseline;
        let data_parameters = &self.sources.data_parameters;
        let unknown = &self.sources.unknown_child_negative;
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-settings-source-owned-v1",
            ),
            (
                "contract source product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "contract source release",
                self.source.release == "8.3.27 / XML 2.20",
            ),
            (
                "contract source derivation",
                self.source.derivation
                    == "bounded source-owned policy synthesized from two exact standalone DCS cohorts; patch build is retained as provenance and does not select a separate XML dialect without a structural counterexample",
            ),
            ("platform line", self.sources.platform_line == "8.3.27"),
            ("source version", self.sources.source_version == "2.20"),
            ("ibcmd SHA-256", self.sources.ibcmd_sha256 == IBCMD_SHA),
            (
                "baseline product",
                baseline.product == "1C:Enterprise Platform",
            ),
            ("baseline release", baseline.release == "8.3.27.2214"),
            (
                "baseline derivation",
                baseline.derivation
                    == "two isolated file-infobase round-trips retained as the immutable dcs-core fixture",
            ),
            (
                "baseline fixture",
                baseline.fixture_id == "8.3.27.2214-xml-2.20-dcs-core",
            ),
            (
                "baseline native XML SHA-256",
                baseline.native_xml_sha256
                    == "4e29eb37d821ff47a65c0240751427369c19cf9ec57c7eaa366e4b9efc35818f",
            ),
            (
                "baseline packed body SHA-256",
                baseline.packed_body_sha256
                    == "6c9d6e02140f1410bf92650d5fe717b07cd66ec45db1567ab4b2ae7dc0782a8b",
            ),
            (
                "baseline unpacked body SHA-256",
                baseline.unpacked_body_sha256
                    == "39790f6f4ff59a5487396eb435a12e4c1a74418c2b3750286dadac8cd40f4510",
            ),
            ("baseline round trips", baseline.round_trips == 2),
            (
                "dataParameters product",
                data_parameters.product == "1C:Enterprise Platform",
            ),
            (
                "dataParameters release",
                data_parameters.release == "8.3.27.2214",
            ),
            (
                "dataParameters derivation",
                data_parameters.derivation
                    == "two fresh isolated file-infobase round-trips retained as the immutable dcs-data-parameters-source-owned fixture",
            ),
            (
                "dataParameters fixture",
                data_parameters.fixture_id
                    == "8.3.27.2214-xml-2.20-dcs-data-parameters-source-owned",
            ),
            (
                "dataParameters native XML SHA-256",
                data_parameters.native_xml_sha256
                    == "db505e5863f238570714fd2f281303fdf6b0a940040535a66e29f080828d6f7c",
            ),
            (
                "dataParameters packed body SHA-256",
                data_parameters.packed_body_sha256
                    == "c1f1ba3b778c6a6054730ee389ed244f9c985d9e32c098caf61de18b8a4b4155",
            ),
            (
                "dataParameters unpacked body SHA-256",
                data_parameters.unpacked_body_sha256
                    == "ff4bdb55f20904504c781bdd6ec7d1e2a3203fe53c497819c7c58ae6815e3713",
            ),
            (
                "dataParameters fragment SHA-256",
                data_parameters.fragment_sha256
                    == "a8318c0031c780148c9b3496f4b6981d3857e2b83b3f4d965c71b895b36831c9",
            ),
            (
                "dataParameters round trips",
                data_parameters.round_trips == 2,
            ),
            ("negative product", unknown.product == "1C:EDT"),
            ("negative release", unknown.release == "2025.2.3+30"),
            (
                "negative derivation",
                unknown.derivation
                    == "exact readSettings bytecode review already retained by the bundled DCS writer evidence",
            ),
            (
                "negative evidence key",
                unknown.evidence_key == "dcs.DataCompositionSettings.opaque-extension.placement",
            ),
            (
                "negative reader outcome",
                unknown.reader_outcome == "throwWrongElement",
            ),
            (
                "negative production outcome",
                unknown.production_outcome == "unsupported-no-lossless-placement",
            ),
            ("namespace", self.policy.namespace == NS),
            ("core namespace", self.policy.core_namespace == CORE_NS),
            (
                "XML Schema namespace",
                self.policy.xml_schema_namespace == XS_NS,
            ),
            ("xsi namespace", self.policy.xsi_namespace == XSI_NS),
            (
                "dataParameters QName",
                self.policy.data_parameters_qname == format!("{{{NS}}}dataParameters"),
            ),
            (
                "data parameter item QName",
                self.policy.data_parameter_item_qname == format!("{{{CORE_NS}}}item"),
            ),
            (
                "SettingsParameterValue type QName",
                self.policy.settings_parameter_value_type_qname
                    == format!("{{{NS}}}SettingsParameterValue"),
            ),
            (
                "parameter QName",
                self.policy.parameter_qname == format!("{{{CORE_NS}}}parameter"),
            ),
            ("parameter name", self.policy.parameter_name == "Caption"),
            (
                "value QName",
                self.policy.value_qname == format!("{{{CORE_NS}}}value"),
            ),
            (
                "value type QName",
                self.policy.value_type_qname == format!("{{{XS_NS}}}string"),
            ),
            ("value", self.policy.value == "Opaque probe"),
            (
                "order QName",
                self.policy.order_qname == format!("{{{NS}}}order"),
            ),
            (
                "structure item QName",
                self.policy.structure_item_qname == format!("{{{NS}}}item"),
            ),
            (
                "StructureItemGroup type QName",
                self.policy.structure_item_group_type_qname
                    == format!("{{{NS}}}StructureItemGroup"),
            ),
            (
                "structure order QName",
                self.policy.structure_order_qname == format!("{{{NS}}}order"),
            ),
            (
                "structure order item QName",
                self.policy.structure_order_item_qname == format!("{{{NS}}}item"),
            ),
            (
                "OrderItemAuto type QName",
                self.policy.order_item_auto_type_qname == format!("{{{NS}}}OrderItemAuto"),
            ),
            (
                "structure selection QName",
                self.policy.structure_selection_qname == format!("{{{NS}}}selection"),
            ),
            (
                "structure selection item QName",
                self.policy.structure_selection_item_qname == format!("{{{NS}}}item"),
            ),
            (
                "SelectedItemAuto type QName",
                self.policy.selected_item_auto_type_qname == format!("{{{NS}}}SelectedItemAuto"),
            ),
            (
                "root child order",
                self.policy.root_child_order
                    == [
                        "selection",
                        "dataParameters",
                        "order",
                        "item(StructureItemGroup)",
                    ],
            ),
            (
                "data parameter item child order",
                self.policy.data_parameter_item_child_order == ["parameter", "value"],
            ),
            (
                "structure item child order",
                self.policy.structure_item_child_order == ["order", "selection"],
            ),
            (
                "structure order child order",
                self.policy.structure_order_child_order == ["item(OrderItemAuto)"],
            ),
            (
                "structure selection child order",
                self.policy.structure_selection_child_order == ["item(SelectedItemAuto)"],
            ),
            (
                "dataParameters placement",
                self.policy.data_parameters_placement == "after-selection-before-order",
            ),
            (
                "structure item placement",
                self.policy.structure_item_placement == "terminal",
            ),
            ("scope", self.policy.scope == "standalone-only"),
            (
                "maximum data parameter items",
                self.policy.max_data_parameter_items == 1,
            ),
            (
                "maximum structure items",
                self.policy.max_structure_items == 1,
            ),
            (
                "unknown children",
                self.policy.unknown_children == "unsupported-no-lossless-placement",
            ),
            (
                "generic opaque emission",
                self.policy.generic_opaque_emission == "forbidden",
            ),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS settings source-owned {field} drifted"
            )));
        }
        Ok(())
    }

    fn into_policy(self) -> DcsSettingsSourceOwnedPolicy {
        DcsSettingsSourceOwnedPolicy {
            namespace_uri: self.policy.namespace,
            core_namespace_uri: self.policy.core_namespace,
            xml_schema_namespace_uri: self.policy.xml_schema_namespace,
            xsi_namespace_uri: self.policy.xsi_namespace,
            data_parameters_qname: self.policy.data_parameters_qname,
            data_parameter_item_qname: self.policy.data_parameter_item_qname,
            settings_parameter_value_type_qname: self.policy.settings_parameter_value_type_qname,
            parameter_qname: self.policy.parameter_qname,
            parameter_name: self.policy.parameter_name,
            value_qname: self.policy.value_qname,
            value_type_qname: self.policy.value_type_qname,
            value: self.policy.value,
            order_qname: self.policy.order_qname,
            structure_item_qname: self.policy.structure_item_qname,
            structure_item_group_type_qname: self.policy.structure_item_group_type_qname,
            structure_order_qname: self.policy.structure_order_qname,
            structure_order_item_qname: self.policy.structure_order_item_qname,
            order_item_auto_type_qname: self.policy.order_item_auto_type_qname,
            structure_selection_qname: self.policy.structure_selection_qname,
            structure_selection_item_qname: self.policy.structure_selection_item_qname,
            selected_item_auto_type_qname: self.policy.selected_item_auto_type_qname,
            max_data_parameter_items: self.policy.max_data_parameter_items,
            max_structure_items: self.policy.max_structure_items,
        }
    }
}

impl DcsSchemaTemplateEnvelopeEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 16 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "DCS schema-template envelope evidence exceeds 16384 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        let expected_source = DcsSchemaTemplateEnvelopeEvidenceSource {
            product: "1C:Enterprise Platform".to_owned(),
            release: "8.3.27 / XML 2.20".to_owned(),
            derivation: "bounded schema-template envelope policy synthesized from the immutable one-variant dcs-core baseline and exact two-variant platform evidence on 8.3.27.2214; no other patch build is inferred".to_owned(),
        };
        let expected_fixture = DcsSchemaTemplateEnvelopeEvidenceFixture {
            fixture_id: "8.3.27.2214-xml-2.20-dcs-multi-variant-envelope".to_owned(),
            platform_line: "8.3.27".to_owned(),
            platform_version: "8.3.27.2214".to_owned(),
            source_version: "2.20".to_owned(),
            ibcmd_sha256: "11c77778927faef858fa4ab544ed627b9b6824a623ee7e5d6e6d5a0cf732d02b"
                .to_owned(),
            extractor_identity: "ibcmd-rs cf extract retained lab build".to_owned(),
            extractor_sha256: "5ec1be63f1a850c2673ee8ce7b896fb98934f363d86fff4e9d581b57cfcab722"
                .to_owned(),
            seed_sha256: "d32bf50308d0af77243ac4a5b2b9d090ef9c32ddf5575e9583b679dbb785e881"
                .to_owned(),
            round1_cf_sha256: "de78c33dc31f49112f26bc9e880a079f3a8a122275ddea7e00f64a348c0807e6"
                .to_owned(),
            round2_cf_sha256: "212401d593ee02dba8a634c93debb23b75dfed2b6ce1073b7b2d9d2d285d78d4"
                .to_owned(),
            round1_template_sha256:
                "f7d9fcf20107c3ea228686138962b073c69945083e32dd29bf943be508cea680".to_owned(),
            round2_template_sha256:
                "f7d9fcf20107c3ea228686138962b073c69945083e32dd29bf943be508cea680".to_owned(),
            round1_packed_sha256:
                "145a48abb9bb6896d5a0ad0f76fc354fe42f6c6edb37639721d56c6aed4b408c".to_owned(),
            round2_packed_sha256:
                "145a48abb9bb6896d5a0ad0f76fc354fe42f6c6edb37639721d56c6aed4b408c".to_owned(),
            round1_unpacked_sha256:
                "17b58ff54fb5982b7a5db842758ed108072c58238eb231183822dddb265c7a87".to_owned(),
            round2_unpacked_sha256:
                "17b58ff54fb5982b7a5db842758ed108072c58238eb231183822dddb265c7a87".to_owned(),
            configuration_encoded_sha256:
                "bc5a9f7823b061fdf285de591dec24696016c751afa0f88751967fb4604040e0".to_owned(),
            configuration_decoded_sha256:
                "212401d593ee02dba8a634c93debb23b75dfed2b6ce1073b7b2d9d2d285d78d4".to_owned(),
            raw_packed_encoded_sha256:
                "e618568d9979a1b8bd380488dd378029a6f157b691f023971d87208befa0eaff".to_owned(),
            raw_packed_decoded_sha256:
                "145a48abb9bb6896d5a0ad0f76fc354fe42f6c6edb37639721d56c6aed4b408c".to_owned(),
            raw_unpacked_encoded_sha256:
                "3ca3e131a41d7cac6cf3e4d0d60acc8acc4e5e1922cd394e44eef0a4687f0914".to_owned(),
            raw_unpacked_decoded_sha256:
                "17b58ff54fb5982b7a5db842758ed108072c58238eb231183822dddb265c7a87".to_owned(),
            native_xml_encoded_sha256:
                "94a3fe8358475179213c176c49f3615a16d195166174927961b21334cdb62020".to_owned(),
            native_xml_decoded_sha256:
                "f7d9fcf20107c3ea228686138962b073c69945083e32dd29bf943be508cea680".to_owned(),
            round_trips: 2,
        };
        const SCHEMA_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
        const SETTINGS_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        let expected_policy = DcsSchemaTemplateEnvelopeEvidencePolicy {
            schema_namespace: SCHEMA_NS.to_owned(),
            settings_namespace: SETTINGS_NS.to_owned(),
            source_root_qname: format!("{{{SCHEMA_NS}}}DataCompositionSchema"),
            source_settings_variant_qname: format!("{{{SCHEMA_NS}}}settingsVariant"),
            source_inline_settings_qname: format!("{{{SETTINGS_NS}}}settings"),
            native_schema_file_qname: "{}SchemaFile".to_owned(),
            native_schema_qname: format!("{{{SCHEMA_NS}}}dataCompositionSchema"),
            native_settings_qname: format!("{{{SETTINGS_NS}}}Settings"),
            header_marker: 0,
            settings_count_offset_bytes: 4,
            settings_count_encoding: "little-endian-u32".to_owned(),
            stored_lengths_offset_bytes: 8,
            stored_length_width_bytes: 8,
            stored_length_encoding: "little-endian-u64".to_owned(),
            stored_length_roles: vec!["PrimarySchemaFile".to_owned(), "Settings[*]".to_owned()],
            document_roles: vec![
                "PrimarySchemaFile".to_owned(),
                "Settings[*]".to_owned(),
                "TerminalSchemaFile".to_owned(),
            ],
            minimum_attested_settings_variants: 1,
            maximum_attested_settings_variants: 2,
            stored_lengths_cover: "primary-schema-file-and-each-settings-document".to_owned(),
            terminal_document_framing: "remaining-bytes".to_owned(),
            document_encoding: "utf-8-with-bom".to_owned(),
            settings_binding: "direct-settings-variant-order".to_owned(),
            source_variant_placement: "direct-root-child".to_owned(),
            terminal_schema_file_shape: "empty-data-composition-schema".to_owned(),
        };
        let expected_two_variant_shape = DcsSchemaTemplateEnvelopeTwoVariantShape {
            settings_document_count: 2,
            header_bytes: 32,
            stored_document_lengths: vec![3467, 1142, 826],
            trailing_document_length: 263,
            document_roles: vec![
                "PrimarySchemaFile".to_owned(),
                "Settings[0]".to_owned(),
                "Settings[1]".to_owned(),
                "TerminalSchemaFile".to_owned(),
            ],
            document_sha256: vec![
                "d66470fbd49885773af38530faf2bc584136ca5ccfd4830c4520464fc40e06fa".to_owned(),
                "191d0d0ea288a68188789d4df1ea79f0655768f63025f961b9ab1be746038fa4".to_owned(),
                "5447b605113dad84ded4b1ad6159428e5d769daea542db87d181041c5c04cae1".to_owned(),
                "2eb60bcedf55e8ae8b6c16f862b3ed3f5dd0d2cd2306cbe02c79d69e44e65f81".to_owned(),
            ],
            variant_names: vec!["Main".to_owned(), "Secondary Secondary".to_owned()],
            source_variant_order_matches_external_settings_order: true,
            terminal_schema_file_is_empty: true,
        };
        let expected_proven_claims = [
            "The second little-endian u32 header field is the external Settings document count: it changed from one to two when a second direct settingsVariant was added.",
            "For the attested one- and two-variant shapes, the header stores settings_count + 1 little-endian u64 lengths for PrimarySchemaFile and every external Settings document; TerminalSchemaFile consumes the remaining bytes.",
            "Two external standalone Settings documents bind positionally to two direct root settingsVariant nodes and materialize as direct inline settings children in source order.",
            "The native selected tree and packed and unpacked template body are byte-identical across two fresh ru_RU platform rounds.",
            "The terminal empty SchemaFile is unchanged from dcs-core while the primary SchemaFile expands only for the second metadata shell.",
        ];
        let expected_non_claims = [
            "No absent Settings document, duplicate variant name, empty name or presentation, more than two variants, defaultSettings, nested variant, or alternate positional binding is inferred.",
            "No AreaTemplate, additional nonterminal SchemaFile, appearance side-table, appIndex, current-configuration reference, or reverse AreaTemplate writer is inferred.",
            "No full typed DataCompositionSchema semantic model is inferred; the schema shell remains source-owned and only the evidenced envelope and Settings delegation are owned.",
            "No arbitrary unknown schema child, cross-profile replay, EDT source roundtrip, or whole-profile 8.3.27 alias is inferred.",
            "Unica generated only the hypothesis seed; pinned 8.3.27.2214 output is the authority.",
        ];

        let checks = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-schema-template-envelope-v1",
            ),
            ("contract source", self.source == expected_source),
            (
                "fixture provenance or artifact hashes",
                self.fixture == expected_fixture,
            ),
            (
                "framing and positional policy",
                self.policy == expected_policy,
            ),
            (
                "two-variant physical shape",
                self.two_variant_shape == expected_two_variant_shape,
            ),
            (
                "proven claims",
                self.proven_claims == expected_proven_claims,
            ),
            ("non-claims", self.non_claims == expected_non_claims),
        ];
        if let Some((field, _)) = checks.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS schema-template envelope {field} drifted"
            )));
        }

        let stored_length_count = self
            .two_variant_shape
            .settings_document_count
            .checked_add(1);
        let computed_header_bytes = stored_length_count
            .and_then(|count| count.checked_mul(self.policy.stored_length_width_bytes))
            .and_then(|bytes| self.policy.stored_lengths_offset_bytes.checked_add(bytes));
        if stored_length_count != Some(self.two_variant_shape.stored_document_lengths.len())
            || computed_header_bytes != Some(self.two_variant_shape.header_bytes)
            || self.two_variant_shape.document_roles.len()
                != self.two_variant_shape.settings_document_count + 2
            || self.two_variant_shape.document_sha256.len()
                != self.two_variant_shape.document_roles.len()
        {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "DCS schema-template envelope internal count/role relation drifted".to_owned(),
            ));
        }
        Ok(())
    }

    fn into_policy(self) -> DcsSchemaTemplateEnvelopePolicy {
        DcsSchemaTemplateEnvelopePolicy {
            schema_namespace_uri: self.policy.schema_namespace,
            settings_namespace_uri: self.policy.settings_namespace,
            source_root_qname: self.policy.source_root_qname,
            source_settings_variant_qname: self.policy.source_settings_variant_qname,
            source_inline_settings_qname: self.policy.source_inline_settings_qname,
            native_schema_file_qname: self.policy.native_schema_file_qname,
            native_schema_qname: self.policy.native_schema_qname,
            native_settings_qname: self.policy.native_settings_qname,
            header_marker: self.policy.header_marker,
            settings_count_offset_bytes: self.policy.settings_count_offset_bytes,
            stored_lengths_offset_bytes: self.policy.stored_lengths_offset_bytes,
            stored_length_width_bytes: self.policy.stored_length_width_bytes,
            minimum_attested_settings_variants: self.policy.minimum_attested_settings_variants,
            maximum_attested_settings_variants: self.policy.maximum_attested_settings_variants,
        }
    }
}

impl DcsOrderEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 32 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "order evidence exceeds 32768 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        let standalone = &self.sources.standalone;
        let form = &self.sources.form;
        let metadata_only = &self.sources.form_metadata_only;
        let unica_desc = &self.sources.unica_desc;
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-settings-order-v1",
            ),
            (
                "contract source product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "contract source release",
                self.source.release == "8.3.27 / XML 2.20",
            ),
            (
                "contract source derivation",
                self.source.derivation
                    == "component contract synthesized from exact standalone and Form experiments; patch builds are retained as provenance and do not select separate XML dialects without a structural counterexample",
            ),
            ("platform line", self.sources.platform_line == "8.3.27"),
            ("source version", self.sources.source_version == "2.20"),
            (
                "standalone product",
                standalone.product == "1C:Enterprise Platform",
            ),
            ("form product", form.product == "1C:Enterprise Platform"),
            (
                "metadata-only product",
                metadata_only.product == "1C:Enterprise Platform",
            ),
            (
                "Unica Desc product",
                unica_desc.product == "1C:Enterprise Platform via the public Unica corpus",
            ),
            (
                "standalone release",
                standalone.release.starts_with("8.3.27."),
            ),
            ("form release", form.release == standalone.release),
            (
                "metadata-only release",
                metadata_only.release == standalone.release,
            ),
            ("Unica Desc release", unica_desc.release == "8.3.27.2074"),
            (
                "standalone derivation",
                standalone.derivation
                    == "two fresh isolated file-infobase round-trips with pinned ibcmd; immutable raw body and selected native XML retained; Unica supplied the initial seed only",
            ),
            (
                "form derivation",
                form.derivation
                    == "retained platform-saved CF followed by one fresh isolated file-infobase load/apply/save/export round-trip; exact raw Form row and native XML were byte-stable",
            ),
            (
                "metadata-only derivation",
                metadata_only.derivation
                    == "byte-identical lexical fragment observed in retained and fresh native Form exports; private owner locator is intentionally not published and this source authenticates only the exact metadata-only fragment",
            ),
            (
                "Unica Desc derivation",
                unica_desc.derivation
                    == "public Unica rich dcs.compile checkpoint with explicit Desc root and group order accepted through two semantic platform round-trips; prefix and whitespace bytes were not used as evidence",
            ),
            (
                "Unica Desc revision",
                unica_desc.repository_revision == "a527d40962d047c6922c903b37510b30f697da42",
            ),
            (
                "standalone fixture",
                standalone.fixture_id == "8.3.27.2214-xml-2.20-dcs-core",
            ),
            (
                "form fixture",
                form.fixture_id == "8.3.27.2214-xml-2.20-dcs-order",
            ),
            ("standalone rounds", standalone.round_trips >= 2),
            ("form rounds", form.round_trips >= 2),
            ("metadata-only rounds", metadata_only.round_trips >= 2),
            ("Unica Desc rounds", unica_desc.round_trips >= 2),
            ("namespace", self.policy.namespace == NS),
            (
                "order QName",
                self.policy.order_qname == format!("{{{NS}}}order"),
            ),
            (
                "storage Order QName",
                self.policy.storage_order_qname == format!("{{{NS}}}Order"),
            ),
            (
                "item QName",
                self.policy.item_qname == format!("{{{NS}}}item"),
            ),
            ("use QName", self.policy.use_qname == format!("{{{NS}}}use")),
            (
                "field QName",
                self.policy.field_qname == format!("{{{NS}}}field"),
            ),
            (
                "order type QName",
                self.policy.order_type_qname == format!("{{{NS}}}orderType"),
            ),
            (
                "view mode QName",
                self.policy.view_mode_qname == format!("{{{NS}}}viewMode"),
            ),
            (
                "user setting ID QName",
                self.policy.user_setting_id_qname == format!("{{{NS}}}userSettingID"),
            ),
            (
                "field type QName",
                self.policy.field_type_qname == format!("{{{NS}}}OrderItemField"),
            ),
            (
                "settings placement",
                self.policy.settings_placement == "after-selection-before-structure-items",
            ),
            (
                "item child order",
                self.policy.item_child_order == ["use?", "field", "orderType"],
            ),
            (
                "order child order",
                self.policy.order_child_order == ["items", "viewMode?", "userSettingID?"],
            ),
            (
                "supported order types",
                self.policy.supported_order_types == ["Asc", "Desc"],
            ),
            (
                "supported use values",
                self.policy.supported_use_values == ["omitted", "false"],
            ),
            ("maximum emitted items", self.policy.max_emitted_items == 1),
            (
                "supported view modes",
                self.policy.supported_view_modes == ["Normal"],
            ),
            (
                "metadata-only user setting ID",
                self.policy.metadata_only_user_setting_id == "88619765-ccb3-46c6-ac52-38e9c992ebd4",
            ),
            (
                "storage UUID",
                self.policy.storage_record_type_uuid == "11743ff3-2db3-4cfc-9404-90ed8209437f"
                    && Uuid::parse_str(&self.policy.storage_record_type_uuid)
                        .is_ok_and(|value| !value.is_nil()),
            ),
            (
                "propertyless empty order emission",
                self.policy.propertyless_empty_order_emission == "unsupported",
            ),
            (
                "metadata-only order emission",
                self.policy.metadata_only_order_emission == "requires-viewMode-and-userSettingID",
            ),
            (
                "root Auto emission",
                self.policy.root_auto_emission == "unsupported",
            ),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS order {field} drifted"
            )));
        }
        for (field, digest) in [
            ("ibcmd SHA-256", self.sources.ibcmd_sha256.as_str()),
            (
                "standalone raw body SHA-256",
                standalone.raw_body_sha256.as_str(),
            ),
            (
                "standalone native XML SHA-256",
                standalone.native_xml_sha256.as_str(),
            ),
            ("Form raw body SHA-256", form.raw_body_sha256.as_str()),
            ("Form native XML SHA-256", form.native_xml_sha256.as_str()),
            (
                "Form storage Order SHA-256",
                form.storage_order_sha256.as_str(),
            ),
            (
                "Form embedded order SHA-256",
                form.embedded_order_sha256.as_str(),
            ),
            (
                "Form metadata-only Order SHA-256",
                metadata_only.fragment_sha256.as_str(),
            ),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                    "DCS order {field} is invalid"
                )));
            }
        }
        Ok(())
    }

    fn into_policy(self) -> DcsOrderPolicy {
        DcsOrderPolicy {
            namespace_uri: self.policy.namespace,
            order_qname: self.policy.order_qname,
            storage_order_qname: self.policy.storage_order_qname,
            item_qname: self.policy.item_qname,
            use_qname: self.policy.use_qname,
            field_qname: self.policy.field_qname,
            order_type_qname: self.policy.order_type_qname,
            view_mode_qname: self.policy.view_mode_qname,
            user_setting_id_qname: self.policy.user_setting_id_qname,
            field_type_qname: self.policy.field_type_qname,
            supported_order_types: self.policy.supported_order_types,
            max_emitted_items: self.policy.max_emitted_items,
            supported_view_modes: self.policy.supported_view_modes,
            metadata_only_user_setting_id: self.policy.metadata_only_user_setting_id,
            storage_record_type_uuid: self.policy.storage_record_type_uuid,
        }
    }
}

impl DcsFilterEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 32 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "filter evidence exceeds 32768 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        const CORE_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
        const XS_NS: &str = "http://www.w3.org/2001/XMLSchema";
        let comparison = &self.sources.comparison;
        let metadata_only = &self.sources.metadata_only;
        let unica = &self.sources.unica_cross_evidence;
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-settings-filter-v1",
            ),
            (
                "contract source product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "contract source release",
                self.source.release == "8.3.27 / XML 2.20",
            ),
            (
                "contract source derivation",
                !self.source.derivation.trim().is_empty(),
            ),
            ("platform line", self.sources.platform_line == "8.3.27"),
            ("source version", self.sources.source_version == "2.20"),
            (
                "comparison product",
                comparison.product == "1C:Enterprise Platform",
            ),
            (
                "metadata-only product",
                metadata_only.product == "1C:Enterprise Platform",
            ),
            (
                "Unica product",
                unica.product == "1C:Enterprise Platform via the public Unica corpus",
            ),
            ("comparison release", comparison.release == "8.3.27.2214"),
            (
                "metadata-only release",
                metadata_only.release == comparison.release,
            ),
            ("Unica release", unica.release == "8.3.27.2074"),
            (
                "comparison derivation",
                !comparison.derivation.trim().is_empty(),
            ),
            (
                "metadata-only derivation",
                !metadata_only.derivation.trim().is_empty(),
            ),
            ("Unica derivation", !unica.derivation.trim().is_empty()),
            (
                "fixture",
                comparison.fixture_id == "8.3.27.2214-xml-2.20-dcs-filter"
                    && metadata_only.fixture_id == comparison.fixture_id,
            ),
            ("comparison rounds", comparison.round_trips >= 2),
            ("metadata-only rounds", metadata_only.round_trips >= 2),
            ("Unica rounds", unica.round_trips >= 2),
            (
                "Unica revision",
                unica.repository_revision == "a527d40962d047c6922c903b37510b30f697da42",
            ),
            ("namespace", self.policy.namespace == NS),
            ("core namespace", self.policy.core_namespace == CORE_NS),
            (
                "XML Schema namespace",
                self.policy.xml_schema_namespace == XS_NS,
            ),
            (
                "filter QName",
                self.policy.filter_qname == format!("{{{NS}}}filter"),
            ),
            (
                "storage Filter QName",
                self.policy.storage_filter_qname == format!("{{{NS}}}Filter"),
            ),
            (
                "item QName",
                self.policy.item_qname == format!("{{{NS}}}item"),
            ),
            ("use QName", self.policy.use_qname == format!("{{{NS}}}use")),
            (
                "left QName",
                self.policy.left_qname == format!("{{{NS}}}left"),
            ),
            (
                "comparison type QName",
                self.policy.comparison_type_qname == format!("{{{NS}}}comparisonType"),
            ),
            (
                "right QName",
                self.policy.right_qname == format!("{{{NS}}}right"),
            ),
            (
                "view mode QName",
                self.policy.view_mode_qname == format!("{{{NS}}}viewMode"),
            ),
            (
                "user setting ID QName",
                self.policy.user_setting_id_qname == format!("{{{NS}}}userSettingID"),
            ),
            (
                "comparison item type QName",
                self.policy.comparison_item_type_qname == format!("{{{NS}}}FilterItemComparison"),
            ),
            (
                "left field type QName",
                self.policy.left_field_type_qname == format!("{{{CORE_NS}}}Field"),
            ),
            (
                "right string type QName",
                self.policy.right_string_type_qname == format!("{{{XS_NS}}}string"),
            ),
            (
                "settings placement",
                self.policy.settings_placement
                    == "after-selection-before-order-and-structure-items",
            ),
            (
                "item child order",
                self.policy.item_child_order == ["use?", "left", "comparisonType", "right"],
            ),
            (
                "filter child order",
                self.policy.filter_child_order == ["items", "viewMode?", "userSettingID?"],
            ),
            (
                "supported comparisons",
                self.policy.supported_comparison_types == ["Equal"],
            ),
            (
                "supported use values",
                self.policy.supported_use_values == ["omitted"],
            ),
            (
                "supported right types",
                self.policy.supported_right_types == ["string"],
            ),
            ("maximum emitted items", self.policy.max_emitted_items == 1),
            (
                "supported view modes",
                self.policy.supported_view_modes == ["Normal"],
            ),
            (
                "metadata-only user setting ID",
                self.policy.metadata_only_user_setting_id == "dfcece9d-5077-440b-b6b3-45a5cb4538eb",
            ),
            (
                "comparison storage record type UUID",
                Uuid::parse_str(&self.policy.comparison_storage_record_type_uuid)
                    .is_ok_and(|value| !value.is_nil()),
            ),
            (
                "metadata-only embedded emission",
                self.policy.metadata_only_embedded_emission
                    == "requires-viewMode-and-userSettingID",
            ),
            (
                "metadata-only storage representation",
                self.policy.metadata_only_storage_representation
                    == "Filter-property-absent-when-AutoSaveUserSettings-true",
            ),
            (
                "propertyless empty filter emission",
                self.policy.propertyless_empty_filter_emission == "unsupported",
            ),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS filter {field} drifted"
            )));
        }
        for (field, digest) in [
            ("ibcmd SHA-256", self.sources.ibcmd_sha256.as_str()),
            (
                "comparison Form raw body SHA-256",
                comparison.form_raw_body_sha256.as_str(),
            ),
            (
                "comparison Form native XML SHA-256",
                comparison.form_native_xml_sha256.as_str(),
            ),
            (
                "comparison Form storage Filter SHA-256",
                comparison.form_storage_filter_sha256.as_str(),
            ),
            (
                "comparison Form embedded filter SHA-256",
                comparison.form_embedded_filter_sha256.as_str(),
            ),
            (
                "standalone raw body SHA-256",
                comparison.standalone_raw_body_sha256.as_str(),
            ),
            (
                "standalone native XML SHA-256",
                comparison.standalone_native_xml_sha256.as_str(),
            ),
            (
                "standalone filter SHA-256",
                comparison.standalone_filter_sha256.as_str(),
            ),
            (
                "metadata-only Form raw body SHA-256",
                metadata_only.form_raw_body_sha256.as_str(),
            ),
            (
                "metadata-only Form native XML SHA-256",
                metadata_only.form_native_xml_sha256.as_str(),
            ),
            (
                "metadata-only Form embedded filter SHA-256",
                metadata_only.form_embedded_filter_sha256.as_str(),
            ),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                    "DCS filter {field} is invalid"
                )));
            }
        }
        Ok(())
    }

    fn into_policy(self) -> DcsFilterPolicy {
        DcsFilterPolicy {
            namespace_uri: self.policy.namespace,
            core_namespace_uri: self.policy.core_namespace,
            xml_schema_namespace_uri: self.policy.xml_schema_namespace,
            filter_qname: self.policy.filter_qname,
            storage_filter_qname: self.policy.storage_filter_qname,
            item_qname: self.policy.item_qname,
            use_qname: self.policy.use_qname,
            left_qname: self.policy.left_qname,
            comparison_type_qname: self.policy.comparison_type_qname,
            right_qname: self.policy.right_qname,
            view_mode_qname: self.policy.view_mode_qname,
            user_setting_id_qname: self.policy.user_setting_id_qname,
            comparison_item_type_qname: self.policy.comparison_item_type_qname,
            left_field_type_qname: self.policy.left_field_type_qname,
            right_string_type_qname: self.policy.right_string_type_qname,
            supported_comparison_types: self.policy.supported_comparison_types,
            max_emitted_items: self.policy.max_emitted_items,
            supported_view_modes: self.policy.supported_view_modes,
            metadata_only_user_setting_id: self.policy.metadata_only_user_setting_id,
            comparison_storage_record_type_uuid: self.policy.comparison_storage_record_type_uuid,
        }
    }
}

impl DcsConditionalAppearanceEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 32 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "conditional-appearance evidence exceeds 32768 UTF-8 bytes".to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        const CORE_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
        const UI_NS: &str = "http://v8.1c.ru/8.1/data/ui";
        const WEB_NS: &str = "http://v8.1c.ru/8.1/data/ui/colors/web";
        const XS_NS: &str = "http://www.w3.org/2001/XMLSchema";
        let comparison = &self.sources.comparison;
        let metadata = &self.sources.metadata_only;
        let unica = &self.sources.unica_cross_evidence;
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-settings-conditional-appearance-v1",
            ),
            (
                "contract source product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "contract source release",
                self.source.release == "8.3.27 / XML 2.20",
            ),
            (
                "contract source derivation",
                !self.source.derivation.trim().is_empty(),
            ),
            ("platform line", self.sources.platform_line == "8.3.27"),
            ("source version", self.sources.source_version == "2.20"),
            (
                "comparison product",
                comparison.product == "1C:Enterprise Platform",
            ),
            (
                "metadata product",
                metadata.product == "1C:Enterprise Platform",
            ),
            ("comparison release", comparison.release == "8.3.27.2214"),
            ("metadata release", metadata.release == comparison.release),
            (
                "fixture",
                comparison.fixture_id == "8.3.27.2214-xml-2.20-dcs-conditional-appearance",
            ),
            (
                "metadata fixture",
                metadata.fixture_id == "8.3.27.2214-xml-2.20-dcs-filter",
            ),
            (
                "comparison derivation",
                !comparison.derivation.trim().is_empty(),
            ),
            (
                "metadata derivation",
                !metadata.derivation.trim().is_empty(),
            ),
            ("comparison rounds", comparison.round_trips >= 2),
            ("metadata rounds", metadata.round_trips >= 2),
            ("Unica rounds", unica.round_trips >= 2),
            ("Unica release", unica.release == "8.3.27.2074"),
            (
                "Unica revision",
                unica.repository_revision == "a527d40962d047c6922c903b37510b30f697da42",
            ),
            ("namespace", self.policy.namespace == NS),
            ("core namespace", self.policy.core_namespace == CORE_NS),
            ("UI namespace", self.policy.ui_namespace == UI_NS),
            ("web namespace", self.policy.web_color_namespace == WEB_NS),
            (
                "XML Schema namespace",
                self.policy.xml_schema_namespace == XS_NS,
            ),
            (
                "conditional appearance QName",
                self.policy.conditional_appearance_qname
                    == format!("{{{NS}}}conditionalAppearance"),
            ),
            (
                "storage QName",
                self.policy.storage_conditional_appearance_qname
                    == format!("{{{NS}}}ConditionalAppearance"),
            ),
            (
                "item QName",
                self.policy.item_qname == format!("{{{NS}}}item"),
            ),
            (
                "selection QName",
                self.policy.selection_qname == format!("{{{NS}}}selection"),
            ),
            (
                "field QName",
                self.policy.field_qname == format!("{{{NS}}}field"),
            ),
            (
                "filter QName",
                self.policy.filter_qname == format!("{{{NS}}}filter"),
            ),
            (
                "appearance QName",
                self.policy.appearance_qname == format!("{{{NS}}}appearance"),
            ),
            (
                "core item QName",
                self.policy.core_item_qname == format!("{{{CORE_NS}}}item"),
            ),
            (
                "parameter QName",
                self.policy.parameter_qname == format!("{{{CORE_NS}}}parameter"),
            ),
            (
                "value QName",
                self.policy.value_qname == format!("{{{CORE_NS}}}value"),
            ),
            (
                "view mode QName",
                self.policy.view_mode_qname == format!("{{{NS}}}viewMode"),
            ),
            (
                "user setting QName",
                self.policy.user_setting_id_qname == format!("{{{NS}}}userSettingID"),
            ),
            (
                "parameter value type",
                self.policy.parameter_value_type_qname == format!("{{{NS}}}SettingsParameterValue"),
            ),
            (
                "color type",
                self.policy.color_type_qname == format!("{{{UI_NS}}}Color"),
            ),
            (
                "settings placement",
                self.policy.settings_placement == "after-filter-and-order-before-structure-items",
            ),
            (
                "item order",
                self.policy.item_child_order == ["selection", "filter", "appearance"],
            ),
            (
                "selection order",
                self.policy.selection_child_order == ["item(field)"],
            ),
            (
                "appearance order",
                self.policy.appearance_child_order == ["coreItem(parameter,value)"],
            ),
            (
                "container order",
                self.policy.container_child_order == ["items", "viewMode?", "userSettingID?"],
            ),
            (
                "supported parameter",
                self.policy.supported_parameters == ["TextColor"],
            ),
            (
                "supported value",
                self.policy.supported_values == ["WebRed"],
            ),
            ("maximum items", self.policy.max_emitted_items == 1),
            (
                "maximum selected fields",
                self.policy.max_selected_fields == 1,
            ),
            ("maximum values", self.policy.max_appearance_values == 1),
            (
                "supported view modes",
                self.policy.supported_view_modes == ["Normal"],
            ),
            (
                "metadata ID",
                self.policy.metadata_only_user_setting_id == "b75fecce-942b-4aed-abc9-e6a02e460fb3",
            ),
            (
                "storage property",
                self.policy.storage_property_name == "Appearance",
            ),
            (
                "storage UUID",
                Uuid::parse_str(&self.policy.storage_record_type_uuid)
                    .is_ok_and(|value| !value.is_nil()),
            ),
            (
                "metadata emission",
                self.policy.metadata_only_embedded_emission
                    == "requires-viewMode-and-userSettingID",
            ),
            (
                "metadata storage",
                self.policy.metadata_only_storage_representation
                    == "Appearance-property-absent-when-AutoSaveUserSettings-true",
            ),
            (
                "empty nested filter",
                self.policy.empty_nested_filter_emission == "unsupported",
            ),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS conditional appearance {field} drifted"
            )));
        }
        for (field, digest) in [
            ("ibcmd", self.sources.ibcmd_sha256.as_str()),
            ("Form raw body", comparison.form_raw_body_sha256.as_str()),
            (
                "Form native XML",
                comparison.form_native_xml_sha256.as_str(),
            ),
            ("Form storage", comparison.form_storage_sha256.as_str()),
            ("Form embedded", comparison.form_embedded_sha256.as_str()),
            (
                "standalone raw body",
                comparison.standalone_raw_body_sha256.as_str(),
            ),
            (
                "standalone native XML",
                comparison.standalone_native_xml_sha256.as_str(),
            ),
            (
                "standalone fragment",
                comparison.standalone_fragment_sha256.as_str(),
            ),
            ("metadata embedded", metadata.form_embedded_sha256.as_str()),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                    "DCS conditional appearance {field} SHA-256 is invalid"
                )));
            }
        }
        Ok(())
    }

    fn into_policy(self) -> DcsConditionalAppearancePolicy {
        DcsConditionalAppearancePolicy {
            namespace_uri: self.policy.namespace,
            core_namespace_uri: self.policy.core_namespace,
            ui_namespace_uri: self.policy.ui_namespace,
            web_color_namespace_uri: self.policy.web_color_namespace,
            xml_schema_namespace_uri: self.policy.xml_schema_namespace,
            conditional_appearance_qname: self.policy.conditional_appearance_qname,
            storage_conditional_appearance_qname: self.policy.storage_conditional_appearance_qname,
            item_qname: self.policy.item_qname,
            selection_qname: self.policy.selection_qname,
            field_qname: self.policy.field_qname,
            filter_qname: self.policy.filter_qname,
            appearance_qname: self.policy.appearance_qname,
            core_item_qname: self.policy.core_item_qname,
            parameter_qname: self.policy.parameter_qname,
            value_qname: self.policy.value_qname,
            view_mode_qname: self.policy.view_mode_qname,
            user_setting_id_qname: self.policy.user_setting_id_qname,
            parameter_value_type_qname: self.policy.parameter_value_type_qname,
            color_type_qname: self.policy.color_type_qname,
            max_emitted_items: self.policy.max_emitted_items,
            supported_view_modes: self.policy.supported_view_modes,
            metadata_only_user_setting_id: self.policy.metadata_only_user_setting_id,
            storage_property_name: self.policy.storage_property_name,
            storage_record_type_uuid: self.policy.storage_record_type_uuid,
        }
    }
}

impl DcsFormAttributesConditionalAppearanceEvidenceCorpus {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > 16 * 1024 {
            return Err(SchemaError::InvalidDcsWriterEvidence(
                "Form Attributes conditional-appearance evidence exceeds 16384 UTF-8 bytes"
                    .to_owned(),
            ));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        const FORM_NS: &str = "http://v8.1c.ru/8.3/xcf/logform";
        const SETTINGS_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
        let comparison = &self.sources.comparison;
        let absent = &self.sources.absent;
        let expected = [
            ("schema version", self.schema_version == 1),
            (
                "contract",
                self.contract == "8.3.27-xml-2.20-dcs-form-attributes-conditional-appearance-v1",
            ),
            (
                "body contract",
                self.body_contract == "8.3.27-xml-2.20-dcs-settings-conditional-appearance-v1",
            ),
            (
                "contract product",
                self.source.product == "1C:Enterprise Platform",
            ),
            (
                "contract release",
                self.source.release == "8.3.27 / XML 2.20",
            ),
            (
                "contract derivation",
                !self.source.derivation.trim().is_empty(),
            ),
            ("platform line", self.sources.platform_line == "8.3.27"),
            ("source version", self.sources.source_version == "2.20"),
            (
                "comparison product",
                comparison.product == "1C:Enterprise Platform",
            ),
            ("comparison release", comparison.release == "8.3.27.2214"),
            (
                "comparison fixture",
                comparison.fixture_id
                    == "8.3.27.2214-xml-2.20-dcs-form-attributes-conditional-appearance",
            ),
            (
                "comparison derivation",
                !comparison.derivation.trim().is_empty(),
            ),
            ("comparison locale", comparison.database_locale == "ru_RU"),
            ("comparison rounds", comparison.round_trips >= 2),
            ("absent product", absent.product == comparison.product),
            ("absent release", absent.release == comparison.release),
            (
                "absent fixture",
                absent.fixture_id == "8.3.27.2214-xml-2.20-dcs-conditional-appearance",
            ),
            ("absent derivation", !absent.derivation.trim().is_empty()),
            ("absent rounds", absent.round_trips >= 2),
            ("Form namespace", self.policy.form_namespace == FORM_NS),
            (
                "settings namespace",
                self.policy.settings_namespace == SETTINGS_NS,
            ),
            (
                "core namespace",
                self.policy.core_namespace == "http://v8.1c.ru/8.1/data-composition-system/core",
            ),
            (
                "style namespace",
                self.policy.style_namespace == "http://v8.1c.ru/8.1/data/ui/style",
            ),
            (
                "system font namespace",
                self.policy.system_font_namespace == "http://v8.1c.ru/8.1/data/ui/fonts/system",
            ),
            (
                "core data namespace",
                self.policy.core_data_namespace == "http://v8.1c.ru/8.1/data/core",
            ),
            (
                "UI namespace",
                self.policy.ui_namespace == "http://v8.1c.ru/8.1/data/ui",
            ),
            (
                "web color namespace",
                self.policy.web_color_namespace == "http://v8.1c.ru/8.1/data/ui/colors/web",
            ),
            (
                "Windows color namespace",
                self.policy.windows_color_namespace == "http://v8.1c.ru/8.1/data/ui/colors/windows",
            ),
            (
                "XML Schema namespace",
                self.policy.xml_schema_namespace == "http://www.w3.org/2001/XMLSchema",
            ),
            (
                "xsi namespace",
                self.policy.xsi_namespace == "http://www.w3.org/2001/XMLSchema-instance",
            ),
            (
                "wrapper QName",
                self.policy.wrapper_qname == format!("{{{FORM_NS}}}ConditionalAppearance"),
            ),
            (
                "storage root QName",
                self.policy.storage_root_qname == format!("{{{SETTINGS_NS}}}Settings"),
            ),
            (
                "storage child QName",
                self.policy.storage_child_qname
                    == format!("{{{SETTINGS_NS}}}conditionalAppearance"),
            ),
            (
                "wrapper placement",
                self.policy.wrapper_placement == "after-last-Attribute-before-Attributes-end",
            ),
            (
                "storage envelope",
                self.policy.storage_envelope
                    == "unkeyed-direct-base64-tail-with-active-descriptor-after-declared-attributes",
            ),
            (
                "storage container marker",
                self.policy.storage_container_marker == "4",
            ),
            (
                "storage absent container marker",
                self.policy.storage_absent_container_marker == "0",
            ),
            (
                "inactive marker",
                self.policy.storage_inactive_marker == ["0", "0"],
            ),
            (
                "active marker",
                self.policy.storage_active_marker == ["0", "1"],
            ),
            (
                "field order",
                self.policy.storage_field_order == ["selection", "filter"],
            ),
            (
                "selection type indexes",
                self.policy.storage_selection_type_indexes == [26, 9],
            ),
            (
                "filter type indexes",
                self.policy.storage_filter_type_indexes == [26],
            ),
            (
                "storage UUID",
                self.policy.storage_record_type_uuid.is_none(),
            ),
            (
                "container metadata",
                self.policy.container_metadata == "forbidden",
            ),
            (
                "absence representation",
                self.policy.absence_representation == "wrapper-absent-empty-settings-tail-present",
            ),
            ("maximum items", self.policy.max_emitted_items == 1),
        ];
        if let Some((field, _)) = expected.into_iter().find(|(_, valid)| !valid) {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "DCS Form Attributes conditional appearance {field} drifted"
            )));
        }
        for (field, digest) in [
            ("ibcmd", self.sources.ibcmd_sha256.as_str()),
            ("Form raw body", comparison.form_raw_body_sha256.as_str()),
            (
                "Form native XML",
                comparison.form_native_xml_sha256.as_str(),
            ),
            ("wrapper", comparison.wrapper_sha256.as_str()),
            (
                "storage Settings",
                comparison.storage_settings_sha256.as_str(),
            ),
            ("absent Form raw body", absent.form_raw_body_sha256.as_str()),
            ("absent Form XML", absent.form_native_xml_sha256.as_str()),
        ] {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                    "DCS Form Attributes conditional appearance {field} SHA-256 is invalid"
                )));
            }
        }
        Ok(())
    }

    fn into_policy(self) -> DcsFormAttributesConditionalAppearancePolicy {
        DcsFormAttributesConditionalAppearancePolicy {
            form_namespace_uri: self.policy.form_namespace,
            settings_namespace_uri: self.policy.settings_namespace,
            core_namespace_uri: self.policy.core_namespace,
            style_namespace_uri: self.policy.style_namespace,
            system_font_namespace_uri: self.policy.system_font_namespace,
            core_data_namespace_uri: self.policy.core_data_namespace,
            ui_namespace_uri: self.policy.ui_namespace,
            web_color_namespace_uri: self.policy.web_color_namespace,
            windows_color_namespace_uri: self.policy.windows_color_namespace,
            xml_schema_namespace_uri: self.policy.xml_schema_namespace,
            xsi_namespace_uri: self.policy.xsi_namespace,
            wrapper_qname: self.policy.wrapper_qname,
            storage_root_qname: self.policy.storage_root_qname,
            storage_child_qname: self.policy.storage_child_qname,
            storage_container_marker: self.policy.storage_container_marker,
            storage_absent_container_marker: self.policy.storage_absent_container_marker,
            storage_inactive_marker: self.policy.storage_inactive_marker,
            storage_active_marker: self.policy.storage_active_marker,
            storage_field_order: self.policy.storage_field_order,
            storage_selection_type_indexes: self.policy.storage_selection_type_indexes,
            storage_filter_type_indexes: self.policy.storage_filter_type_indexes,
            max_emitted_items: self.policy.max_emitted_items,
        }
    }
}

impl DcsSettingsSerializationPolicy {
    pub fn standalone_document_qname(&self) -> &str {
        &self.standalone_document_qname
    }

    pub fn form_list_settings_qname(&self) -> &str {
        &self.form_list_settings_qname
    }

    /// EDT's exact standalone/Form caller chains and both settings-writer
    /// bodies emit no TypeId or `xsi:type` for the settings wrapper.
    pub const fn type_id_is_absent(&self) -> bool {
        true
    }
}

impl DcsListSettingsTailPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }

    pub const fn tail_order(&self) -> &[DcsListSettingsTailField; 2] {
        &self.tail_order
    }

    pub fn items_view_mode_qname(&self) -> &str {
        &self.items_view_mode_qname
    }

    pub fn items_view_mode_default(&self) -> &str {
        &self.items_view_mode_default
    }

    pub fn items_user_setting_id_qname(&self) -> &str {
        &self.items_user_setting_id_qname
    }

    pub fn items_user_setting_id_default(&self) -> &str {
        &self.items_user_setting_id_default
    }
}

/// Strict, bounded representation of the committed EDT DCS writer evidence.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DcsWriterEvidenceCorpus {
    schema_version: u32,
    source: DcsWriterEvidenceSource,
    verified_facts: Vec<DcsWriterEvidenceFact>,
    missing_keys: Vec<DcsWriterEvidenceMissingKey>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    input_contract: String,
    invocation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceFact {
    key: String,
    value: DcsWriterEvidenceValue,
    evidence: DcsWriterEvidenceProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(untagged)]
enum DcsWriterEvidenceValue {
    Text(String),
    TailOrder(Vec<String>),
    StandaloneQName(DcsStandaloneQNameEvidence),
    FormWrapperQName(DcsFormWrapperQNameEvidence),
    NoTypeId(DcsNoTypeIdEvidence),
    EnumNotDefault(DcsEnumNotDefaultEvidence),
    StringNotDefault(DcsStringNotDefaultEvidence),
    DefaultValue(DcsDefaultValueEvidence),
    FormDelegate(DcsFormDelegateEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsStandaloneQNameEvidence {
    qname: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormWrapperQNameEvidence {
    qname: String,
    qname_source: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsNoTypeIdEvidence {
    emission: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsEnumNotDefaultEvidence {
    qname: String,
    default_model_constant: String,
    writer: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsStringNotDefaultEvidence {
    qname: String,
    default_string: String,
    writer: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsDefaultValueEvidence {
    predicate: String,
    operations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormDelegateEvidence {
    delegate: String,
    qname_source: String,
    null_branch: DcsNullBranchEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsNullBranchEvidence {
    from_offset: u32,
    target_offset: u32,
    target_opcode: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceProof {
    kind: String,
    status: EvidenceStatus,
    sources: Vec<String>,
    note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceMissingKey {
    key: String,
    status: String,
    reason: String,
    evidence: DcsWriterEvidenceProof,
}

/// A structured subset of verified writer behaviour.  Free-form operations remain useful
/// provenance, but production writers must consume this typed policy instead of parsing prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WriterPolicy {
    FormChoiceList {
        #[serde(rename = "itemOrder")]
        item_order: Vec<FormChoiceListItemPart>,
        #[serde(rename = "emptyCollection")]
        empty_collection: FormChoiceListEmptyCollection,
        #[serde(rename = "emptyStringValue")]
        empty_string_value: FormChoiceListEmptyStringValue,
    },
    FormListSettings {
        #[serde(rename = "nullValue")]
        null_value: FormListSettingsNullValue,
        delegate: String,
    },
    FormChoiceParameters {
        #[serde(rename = "ownerQName")]
        owner_qname: String,
        #[serde(rename = "ownerPredecessorQName")]
        owner_predecessor_qname: String,
        #[serde(rename = "ownerSuccessorQName")]
        owner_successor_qname: String,
        #[serde(rename = "emptyCollection")]
        empty_collection: FormChoiceParametersEmptyCollection,
        item: Box<FormChoiceParameterItemPolicy>,
        #[serde(rename = "fixedArray")]
        fixed_array: Box<FormChoiceParameterFixedArrayPolicy>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceListItemPart {
    Presentation,
    CheckState,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceListEmptyCollection {
    WriteWrapperWhenWriteDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceListEmptyStringValue {
    SelfClosing,
}

const MAX_FORM_CHOICE_LIST_STRING_WRITER_PROOF_BYTES: usize = 1024;
const FORM_CHOICE_LIST_STRING_WRITER_FULL_EVIDENCE_SHA256: &str =
    "394b38699352b707682bdfe267537bef318b8535eeed7d112fd9a07a3079e042";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringWriterProof {
    schema_version: u32,
    source: FormChoiceListStringWriterProofSource,
    rule: FormChoiceListStringWriterProofRule,
    emission: FormChoiceListEmptyStringValue,
    full_evidence_sha256: String,
    provenance_ids: [String; 2],
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringWriterProofSource {
    product: String,
    release: String,
    derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringWriterProofRule {
    id: String,
    model_type: String,
    feature: String,
}

#[cfg(test)]
const MAX_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_BYTES: usize = 16 * 1024;

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceListStringWriterEvidence {
    schema_version: u32,
    source: FormChoiceListStringEvidenceSource,
    verified_facts: Vec<FormChoiceListStringEvidenceFact>,
    missing_keys: Vec<String>,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceSource {
    product: String,
    release: String,
    root_identity: FormChoiceListStringEvidenceRootIdentity,
    validated_bundles: Vec<FormChoiceListStringEvidenceBundle>,
    derivation: String,
    input_contract: String,
    invocation: String,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceRootIdentity {
    leaf: String,
    product_version: String,
    build_id: String,
    product: String,
    application: String,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceBundle {
    symbolic_name: String,
    version: String,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceFact {
    key: String,
    value: FormChoiceListStringEvidenceValue,
    evidence: FormChoiceListStringEvidenceProof,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceValue {
    model_value_type: String,
    empty_predicate: String,
    element: String,
    xsi_type: String,
    emission: FormChoiceListEmptyStringValue,
    delegate_chain: Vec<String>,
    branch: FormChoiceListStringEvidenceBranch,
    method_envelopes: Vec<FormChoiceListStringEvidenceMethodEnvelope>,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceBranch {
    string_type_offset: u32,
    empty_predicate_offset: u32,
    non_empty_target_offset: u32,
    empty_element_offset: u32,
    xsi_type_attribute_offset: u32,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceMethodEnvelope {
    method: String,
    descriptor: String,
    instruction_count: usize,
    first_offset: u32,
    last_offset: u32,
    branch_graph: Vec<String>,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceListStringEvidenceProof {
    kind: String,
    status: String,
    sources: Vec<String>,
    note: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormListSettingsNullValue {
    Omit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceParametersEmptyCollection {
    OmitWhenWriteDefaultFalse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceParameterValuePart {
    Presentation,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParameterItemPolicy {
    #[serde(rename = "itemQName")]
    pub item_qname: String,
    #[serde(rename = "nameAttributeQName")]
    pub name_attribute_qname: String,
    #[serde(rename = "valueQName")]
    pub value_qname: String,
    pub value_xsi_type: String,
    pub value_order: Vec<FormChoiceParameterValuePart>,
    #[serde(rename = "presentationQName")]
    pub presentation_qname: String,
    #[serde(rename = "scalarValueQName")]
    pub scalar_value_qname: String,
    pub boolean_xsi_type: String,
    pub design_time_ref_xsi_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParameterFixedArrayPolicy {
    pub xsi_type: String,
    #[serde(rename = "itemQName")]
    pub item_qname: String,
    pub item_xsi_type: String,
    pub item_order: Vec<FormChoiceParameterValuePart>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParametersWriterEvidence {
    schema_version: u32,
    source: FormChoiceParametersEvidenceSource,
    scope: FormChoiceParametersEvidenceScope,
    verified_facts: FormChoiceParametersVerifiedFacts,
    missing_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceSource {
    product: String,
    release: String,
    root_identity: FormChoiceParametersEvidenceRootIdentity,
    validated_bundles: Vec<FormChoiceParametersEvidenceBundle>,
    derivation: String,
    invocation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceRootIdentity {
    leaf: String,
    product_version: String,
    build_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceBundle {
    symbolic_name: String,
    version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceScope {
    disposition: String,
    production_emission: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersVerifiedFacts {
    model: FormChoiceParametersModelFact,
    owner_order: FormChoiceParametersOwnerOrderFact,
    writer: FormChoiceParametersWriterFact,
    live_slot27: FormChoiceParametersLiveFact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersModelFact {
    model_type: String,
    feature: String,
    lower_bound: u32,
    upper_bound: i32,
    #[serde(rename = "ownerQName")]
    owner_qname: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersOwnerOrderFact {
    #[serde(rename = "predecessorQName")]
    predecessor_qname: String,
    #[serde(rename = "featureQName")]
    feature_qname: String,
    #[serde(rename = "successorQName")]
    successor_qname: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersWriterFact {
    delegate: String,
    empty_collection: FormChoiceParametersEmptyCollection,
    item: FormChoiceParameterItemPolicy,
    fixed_array: FormChoiceParameterFixedArrayPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersLiveFact {
    fixture: String,
    fixture_sha256: String,
    raw_row: String,
    raw_source: String,
    raw_source_sha256: String,
    raw_slot: usize,
    native_source: String,
    native_source_sha256: String,
    item_names_in_order: Vec<String>,
    value_kinds_in_order: Vec<String>,
}

/// Exact identity used by a writer-rule consumer.  The release is deliberately part of the
/// key: silently reusing evidence obtained from a different EDT release is forbidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriterRuleKey<'a> {
    pub source_release: &'a str,
    pub model_type: &'a str,
    pub feature: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriterRuleLookupError {
    SourceReleaseMismatch {
        requested: String,
        available: String,
    },
    Missing {
        model_type: String,
        feature: String,
    },
    Ambiguous {
        model_type: String,
        feature: String,
    },
    Unverified {
        id: String,
        status: String,
    },
    MissingTypedPolicy {
        id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEvidence {
    pub kind: String,
    pub status: String,
    pub note: String,
}

/// A stable semantic identity for an Xcore feature.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticKey {
    pub namespace_uri: String,
    pub classifier: String,
    pub feature: String,
}

/// Whether a corpus statement has been confirmed by evidence or is still incomplete.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStatus {
    Pending,
    Verified,
}

/// Provenance and confirmation state for one group of feature facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureEvidence {
    pub status: EvidenceStatus,
    pub kind: String,
    #[serde(default)]
    pub sources: Vec<String>,
    pub note: Option<String>,
}

/// The Xcore declaration kind of a feature.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureKind {
    Attribute,
    Reference,
    Containment,
}

/// Xcore feature modifiers preserved by the semantics corpus.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum XcoreFeatureQualifier {
    Container,
    Derived,
    Transient,
    Unsettable,
    Unique,
}

/// The kind of an Xcore classifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureClassifierKind {
    Class,
    Interface,
    Enum,
    Datatype,
}

/// A value whose availability has independent evidence.
///
/// `Known { value: None }` records a verified absence, whereas `Pending` records that the
/// importer has not yet established whether the value exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum EvidenceValue<T> {
    Pending,
    Known { value: Option<T> },
}

/// XML writer behaviour associated with a feature.
///
/// Any field may be unknown while its evidence is pending. All fields are required once the
/// behaviour is verified.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlFeatureBehavior {
    #[serde(rename = "qname")]
    pub qname: Option<String>,
    pub order: Option<u32>,
    pub emit_default: Option<bool>,
    pub version_gate: EvidenceValue<String>,
    pub delegate: EvidenceValue<String>,
    pub evidence: FeatureEvidence,
}

/// Semantics for one Xcore feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemantics {
    pub name: String,
    pub kind: FeatureKind,
    pub model_type: String,
    pub lower_bound: u32,
    /// `None` means an unbounded upper bound.
    pub upper_bound: Option<u32>,
    /// The value explicitly declared by the model, rather than an inferred language default.
    pub default_value: Option<String>,
    pub qualifiers: Vec<XcoreFeatureQualifier>,
    pub model_evidence: FeatureEvidence,
    pub xml: XmlFeatureBehavior,
}

/// A classifier and its Xcore feature declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsClassifier {
    pub name: String,
    pub kind: FeatureClassifierKind,
    pub features: Vec<FeatureSemantics>,
}

/// One Xcore resource and the package it declares.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsPackage {
    pub bundle: String,
    pub resource: String,
    pub package_name: String,
    pub namespace_uri: String,
    pub classifiers: Vec<FeatureSemanticsClassifier>,
}

/// Counts that allow a corpus to detect truncation without reading EDT.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsSummary {
    pub packages: usize,
    pub classifiers: usize,
    pub features: usize,
}

/// A standalone, versioned feature-semantics corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: FeatureSemanticsSummary,
    pub packages: Vec<FeatureSemanticsPackage>,
}

/// How one EDT feature is preserved by the canonical model.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    Typed,
    OpaqueLossless,
    Unsupported,
    PlatformOnly,
}

/// Canonical implementation family used for deterministic coverage reporting.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanonicalCoverageFamily {
    Metadata,
    Forms,
    Dcs,
    Mxl,
    Common,
    Other,
}

/// One explicit EDT feature to canonical-model mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageEntry {
    pub key: FeatureSemanticKey,
    pub family: CanonicalCoverageFamily,
    pub status: CoverageStatus,
    pub canonical_type: Option<String>,
    pub canonical_field: Option<String>,
    pub opaque_placement: Option<String>,
    pub diagnostic_code: Option<String>,
    pub evidence: FeatureEvidence,
}

/// Derived coverage totals for completeness and reporting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageSummary {
    pub entries: usize,
    pub typed: usize,
    pub opaque_lossless: usize,
    pub unsupported: usize,
    pub platform_only: usize,
}

/// Status totals for one canonical implementation family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageFamilyAggregate {
    pub family: CanonicalCoverageFamily,
    pub entries: usize,
    pub typed: usize,
    pub opaque_lossless: usize,
    pub unsupported: usize,
    pub platform_only: usize,
}

/// Reusable migration work grouped without object, feature, UUID, or file names.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalMigrationBacklogEntry {
    pub rule: String,
    pub family: CanonicalCoverageFamily,
    pub package: String,
    pub classifier_kind: FeatureClassifierKind,
    pub feature_kind: FeatureKind,
    pub features: usize,
}

/// Complete coverage mapping for one EDT-derived feature corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: CanonicalCoverageSummary,
    pub family_aggregates: Vec<CanonicalCoverageFamilyAggregate>,
    pub migration_backlog: Vec<CanonicalMigrationBacklogEntry>,
    pub entries: Vec<CanonicalCoverageEntry>,
}

const MAX_CANONICAL_COVERAGE_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_CANONICAL_COVERAGE_STRING_BYTES: usize = 4 * 1024;
const MAX_CANONICAL_COVERAGE_ENTRIES: usize = 5_000;
const MAX_CANONICAL_COVERAGE_FAMILY_AGGREGATES: usize = 6;
const MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES: usize = 256;
const MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES: usize = 16;

struct BoundedText<const MAX: usize>;

impl<'de, const MAX: usize> Deserialize<'de> for BoundedText<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TextVisitor<const MAX: usize>;

        impl<const MAX: usize> Visitor<'_> for TextVisitor<MAX> {
            type Value = BoundedText<MAX>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write!(formatter, "a string of at most {MAX} UTF-8 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                if value.len() > MAX {
                    return Err(E::custom(format!(
                        "canonical coverage string exceeds {MAX} UTF-8 bytes"
                    )));
                }
                Ok(BoundedText)
            }

            fn visit_borrowed_str<E>(self, value: &'_ str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_str(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_string(TextVisitor::<MAX>)
    }
}

struct BoundedVec<T, const MAX: usize>(PhantomData<T>);

impl<T, const MAX: usize> Default for BoundedVec<T, MAX> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'de, T, const MAX: usize> Deserialize<'de> for BoundedVec<T, MAX>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct VecVisitor<T, const MAX: usize>(PhantomData<T>);

        impl<'de, T, const MAX: usize> Visitor<'de> for VecVisitor<T, MAX>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAX>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write!(formatter, "an array of at most {MAX} elements")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if sequence.size_hint().is_some_and(|size| size > MAX) {
                    return Err(A::Error::custom(format!(
                        "canonical coverage array exceeds {MAX} elements"
                    )));
                }
                let mut count = 0usize;
                while sequence.next_element::<T>()?.is_some() {
                    count += 1;
                    if count > MAX {
                        return Err(A::Error::custom(format!(
                            "canonical coverage array exceeds {MAX} elements"
                        )));
                    }
                }
                Ok(BoundedVec(PhantomData))
            }
        }

        deserializer.deserialize_seq(VecVisitor::<T, MAX>(PhantomData))
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalCoveragePreflight {
    schema_version: u32,
    source: CoverageSourcePreflight,
    summary: CoverageSummaryPreflight,
    family_aggregates:
        BoundedVec<CoverageFamilyAggregatePreflight, MAX_CANONICAL_COVERAGE_FAMILY_AGGREGATES>,
    migration_backlog: BoundedVec<CoverageBacklogPreflight, MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES>,
    entries: BoundedVec<CoverageEntryPreflight, MAX_CANONICAL_COVERAGE_ENTRIES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageSourcePreflight {
    product: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    release: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    derivation: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageSummaryPreflight {
    entries: usize,
    typed: usize,
    opaque_lossless: usize,
    unsupported: usize,
    platform_only: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageFamilyAggregatePreflight {
    family: CanonicalCoverageFamily,
    entries: usize,
    typed: usize,
    opaque_lossless: usize,
    unsupported: usize,
    platform_only: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageBacklogPreflight {
    rule: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    family: CanonicalCoverageFamily,
    package: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    classifier_kind: FeatureClassifierKind,
    feature_kind: FeatureKind,
    features: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageEntryPreflight {
    key: CoverageKeyPreflight,
    family: CanonicalCoverageFamily,
    status: CoverageStatus,
    canonical_type: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    canonical_field: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    opaque_placement: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    diagnostic_code: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    evidence: CoverageEvidencePreflight,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageKeyPreflight {
    namespace_uri: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    classifier: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    feature: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageEvidencePreflight {
    status: EvidenceStatus,
    kind: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    #[serde(default)]
    sources: BoundedVec<
        BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
        MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES,
    >,
    note: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
}

fn preflight_canonical_coverage_json(json: &str) -> Result<(), SchemaError> {
    enforce_canonical_coverage_json_size(json)?;
    serde_json::from_str::<CanonicalCoveragePreflight>(json)
        .map(|_| ())
        .map_err(|error| SchemaError::InvalidJson(error.to_string()))
}

fn enforce_canonical_coverage_json_size(json: &str) -> Result<(), SchemaError> {
    if json.len() > MAX_CANONICAL_COVERAGE_JSON_BYTES {
        return Err(SchemaError::InvalidJson(format!(
            "canonical coverage JSON exceeds {MAX_CANONICAL_COVERAGE_JSON_BYTES} UTF-8 bytes"
        )));
    }
    Ok(())
}

const CANONICAL_COVERAGE_FAMILIES: [CanonicalCoverageFamily; 6] = [
    CanonicalCoverageFamily::Metadata,
    CanonicalCoverageFamily::Forms,
    CanonicalCoverageFamily::Dcs,
    CanonicalCoverageFamily::Mxl,
    CanonicalCoverageFamily::Common,
    CanonicalCoverageFamily::Other,
];

fn canonical_coverage_family(
    package: &str,
    classifier_kind: FeatureClassifierKind,
) -> Option<CanonicalCoverageFamily> {
    use CanonicalCoverageFamily::{Dcs, Forms, Other};
    use FeatureClassifierKind::{Class, Interface};

    let routed = match package {
        "com._1c.g5.v8.dt.form.layout.model.calculation.context"
        | "com._1c.g5.v8.dt.form.layout.model.description"
        | "com._1c.g5.v8.dt.form.layout.model.generation.context"
        | "com._1c.g5.v8.dt.form.layout.model.transformation.context"
        | "com._1c.g5.v8.dt.form.mapping.model"
        | "com._1c.g5.v8.dt.form.model"
            if matches!(classifier_kind, Class | Interface) =>
        {
            Forms
        }
        "com._1c.g5.v8.dt.dcs.expressions.model"
        | "com._1c.g5.v8.dt.dcs.model.appearancetemplate"
        | "com._1c.g5.v8.dt.dcs.model.areaTemplate"
        | "com._1c.g5.v8.dt.dcs.model.common"
        | "com._1c.g5.v8.dt.dcs.model.core"
        | "com._1c.g5.v8.dt.dcs.model.dbcopies"
        | "com._1c.g5.v8.dt.dcs.model.schema"
        | "com._1c.g5.v8.dt.dcs.model.settings"
        | "com._1c.g5.v8.dt.ql.dcs.model"
            if classifier_kind == Class =>
        {
            Dcs
        }
        "com._1c.g5.v8.dt.debug.model.core" if classifier_kind == Class => Other,
        "com._1c.g5.v8.dt.mcore"
        | "com._1c.g5.v8.dt.scc.model"
        | "com._1c.g5.v8.dt.supply.settings.model"
            if matches!(classifier_kind, Class | Interface) =>
        {
            Other
        }
        "com._1c.g5.v8.dt.aggregates.model"
        | "com._1c.g5.v8.dt.bp.scheme.model"
        | "com._1c.g5.v8.dt.bsl.model"
        | "com._1c.g5.v8.dt.cai.model"
        | "com._1c.g5.v8.dt.chart.model"
        | "com._1c.g5.v8.dt.chart.model.timescale"
        | "com._1c.g5.v8.dt.cmi.model"
        | "com._1c.g5.v8.dt.cmi.model.deriveddata"
        | "com._1c.g5.v8.dt.compare.model"
        | "com._1c.g5.v8.dt.debug.model.area"
        | "com._1c.g5.v8.dt.debug.model.attach"
        | "com._1c.g5.v8.dt.debug.model.base.data"
        | "com._1c.g5.v8.dt.debug.model.breakpoints"
        | "com._1c.g5.v8.dt.debug.model.bsl.exceptions"
        | "com._1c.g5.v8.dt.debug.model.calculations"
        | "com._1c.g5.v8.dt.debug.model.dbgui.commands"
        | "com._1c.g5.v8.dt.debug.model.foreground.data"
        | "com._1c.g5.v8.dt.debug.model.measure"
        | "com._1c.g5.v8.dt.debug.model.rdbg.request.response"
        | "com._1c.g5.v8.dt.debug.model.rte.filter"
        | "com._1c.g5.v8.dt.debug.model.rte.info"
        | "com._1c.g5.v8.dt.debug.model.virtual"
        | "com._1c.g5.v8.dt.dendrogram.model"
        | "com._1c.g5.v8.dt.ganttchart.model"
        | "com._1c.g5.v8.dt.geographicalschema.model"
        | "com._1c.g5.v8.dt.hpwa.model"
        | "com._1c.g5.v8.dt.lcore.model"
        | "com._1c.g5.v8.dt.planner.model"
        | "com._1c.g5.v8.dt.platform.model"
        | "com._1c.g5.v8.dt.platform.services.model"
        | "com._1c.g5.v8.dt.ql.model"
        | "com._1c.g5.v8.dt.right.ql.model"
        | "com._1c.g5.v8.dt.right.templates.model"
        | "com._1c.g5.v8.dt.rights.model"
        | "com._1c.g5.v8.dt.schedule.model"
        | "com._1c.g5.v8.dt.style.model"
        | "com._1c.g5.v8.dt.v8help.model"
        | "com._1c.g5.v8.dt.ws.wsdefinitions.model"
        | "com._1c.g5.v8.dt.xdto.model"
        | "com._1c.g5.v8.dt.xdto.type.model"
            if classifier_kind == Class =>
        {
            Other
        }
        _ => return None,
    };
    Some(routed)
}

fn recompute_family_aggregates(
    entries: &[CanonicalCoverageEntry],
) -> Vec<CanonicalCoverageFamilyAggregate> {
    let mut counts = BTreeMap::<CanonicalCoverageFamily, CanonicalCoverageFamilyAggregate>::new();
    for family in CANONICAL_COVERAGE_FAMILIES {
        counts.insert(
            family,
            CanonicalCoverageFamilyAggregate {
                family,
                entries: 0,
                typed: 0,
                opaque_lossless: 0,
                unsupported: 0,
                platform_only: 0,
            },
        );
    }
    for entry in entries {
        let aggregate = counts
            .get_mut(&entry.family)
            .expect("all canonical coverage families are initialized");
        aggregate.entries += 1;
        match entry.status {
            CoverageStatus::Typed => aggregate.typed += 1,
            CoverageStatus::OpaqueLossless => aggregate.opaque_lossless += 1,
            CoverageStatus::Unsupported => aggregate.unsupported += 1,
            CoverageStatus::PlatformOnly => aggregate.platform_only += 1,
        }
    }
    counts.into_values().collect()
}

fn recompute_migration_backlog(
    coverage: &CanonicalCoverageCorpus,
    features: &FeatureSemanticsCorpus,
) -> Result<Vec<CanonicalMigrationBacklogEntry>, SchemaError> {
    let coverage_by_key = coverage
        .entries
        .iter()
        .map(|entry| (entry.key.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut groups = BTreeMap::<
        (
            CanonicalCoverageFamily,
            String,
            FeatureClassifierKind,
            FeatureKind,
        ),
        usize,
    >::new();

    for package in &features.packages {
        for classifier in &package.classifiers {
            if classifier.features.is_empty() {
                continue;
            }
            let family = canonical_coverage_family(&package.package_name, classifier.kind)
                .ok_or_else(|| SchemaError::UnknownCoverageRoute {
                    package: package.package_name.clone(),
                    classifier_kind: classifier.kind,
                })?;
            for feature in &classifier.features {
                let key = FeatureSemanticKey {
                    namespace_uri: package.namespace_uri.clone(),
                    classifier: classifier.name.clone(),
                    feature: feature.name.clone(),
                };
                let entry = coverage_by_key
                    .get(&key)
                    .expect("exact full join is checked before backlog recomputation");
                if entry.family != family {
                    return Err(SchemaError::InvalidCoverageEntry {
                        key,
                        reason: "coverage family does not match canonical package/classifier route",
                    });
                }
                if entry.status == CoverageStatus::Unsupported
                    && entry.diagnostic_code.as_deref() == Some("schema.unmapped")
                {
                    *groups
                        .entry((
                            family,
                            package.package_name.clone(),
                            classifier.kind,
                            feature.kind,
                        ))
                        .or_default() += 1;
                }
            }
        }
    }

    Ok(groups
        .into_iter()
        .map(
            |((family, package, classifier_kind, feature_kind), features)| {
                CanonicalMigrationBacklogEntry {
                    rule: "unsupported/schema.unmapped".to_owned(),
                    family,
                    package,
                    classifier_kind,
                    feature_kind,
                    features,
                }
            },
        )
        .collect())
}

/// EDT writer-provider section whose feature order was observed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderSection {
    InternalInfo,
    Properties,
    ChildObjects,
    ProducedTypes,
}

/// Version condition attached to a provider order record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderVersionPredicate {
    Always,
    #[serde(rename = "greaterThan(V8_3_14)")]
    GreaterThanV8_3_14,
    #[serde(rename = "notGreaterThan(V8_3_14)")]
    NotGreaterThanV8_3_14,
}

/// Explicit provider fallback; it is not an XML default or emission rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MetadataOrderFallback {
    #[serde(rename = "eClass.getEAllReferences() when ORDER_MAP has no key")]
    AllReferencesWhenUnmapped,
    #[serde(
        rename = "ListBuilder(eClass, defaultPropertyFilter).build() when propertiesOrderMap has no key"
    )]
    DefaultPropertyFilterWhenUnmapped,
    #[serde(
        rename = "eClass.getEStructuralFeature(\"producedTypes\") when present, otherwise empty list"
    )]
    ProducedTypesWhenPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderOperationKind {
    Cursor,
    Next,
    Emit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderOperation {
    pub operation: MetadataOrderOperationKind,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderRecord {
    pub provider: String,
    pub classifier: String,
    pub section: MetadataOrderSection,
    pub ordered_features: Vec<String>,
    #[serde(default)]
    pub order_operations: Vec<MetadataOrderOperation>,
    pub version_predicate: MetadataOrderVersionPredicate,
    pub fallback: MetadataOrderFallback,
    pub evidence: FeatureEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderSummary {
    pub bundle: String,
    pub verified_records: usize,
    pub rejected_records: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: MetadataOrderSummary,
    pub records: Vec<MetadataOrderRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaError {
    InvalidJson(String),
    UnsupportedSchemaVersion(u32),
    EmptyField(&'static str),
    DuplicateValue {
        field: &'static str,
        value: String,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    NonPortablePath(String),
    InvalidCardinality {
        lower: u32,
        upper: u32,
    },
    IncompleteVerifiedXmlBehavior {
        key: FeatureSemanticKey,
        field: &'static str,
    },
    InvalidCoverageEntry {
        key: FeatureSemanticKey,
        reason: &'static str,
    },
    CoverageMismatch {
        kind: &'static str,
        key: FeatureSemanticKey,
    },
    UnknownCoverageRoute {
        package: String,
        classifier_kind: FeatureClassifierKind,
    },
    CoverageDerivedDataMismatch(&'static str),
    InvalidDcsWriterEvidence(String),
    InvalidFormChoiceListStringWriterEvidence(String),
    InvalidFormChoiceParametersWriterEvidence(String),
    InvalidFormChoiceParametersQName(String),
    InvalidFormChoiceParameterClusterPolicy(String),
}

impl Display for SchemaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid schema JSON: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported schema version {version}")
            }
            Self::EmptyField(field) => write!(formatter, "{field} is empty"),
            Self::DuplicateValue { field, value } => {
                write!(formatter, "duplicate {field} `{value}`")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} summary mismatch: expected {expected}, actual {actual}"
            ),
            Self::NonPortablePath(value) => {
                write!(formatter, "corpus contains a non-portable path `{value}`")
            }
            Self::InvalidCardinality { lower, upper } => {
                write!(formatter, "invalid cardinality {lower}..{upper}")
            }
            Self::IncompleteVerifiedXmlBehavior { key, field } => write!(
                formatter,
                "verified XML behaviour for {} / {} / {} is missing {field}",
                key.namespace_uri, key.classifier, key.feature
            ),
            Self::InvalidCoverageEntry { key, reason } => write!(
                formatter,
                "invalid canonical coverage for {} / {} / {}: {reason}",
                key.namespace_uri, key.classifier, key.feature
            ),
            Self::CoverageMismatch { kind, key } => write!(
                formatter,
                "{kind} canonical coverage key {} / {} / {}",
                key.namespace_uri, key.classifier, key.feature
            ),
            Self::UnknownCoverageRoute {
                package,
                classifier_kind,
            } => write!(
                formatter,
                "canonical coverage has no route for package `{package}` / classifier kind `{classifier_kind:?}`"
            ),
            Self::CoverageDerivedDataMismatch(field) => {
                write!(
                    formatter,
                    "canonical coverage {field} does not match recomputation"
                )
            }
            Self::InvalidDcsWriterEvidence(reason) => {
                write!(formatter, "invalid DCS writer evidence: {reason}")
            }
            Self::InvalidFormChoiceListStringWriterEvidence(reason) => {
                write!(
                    formatter,
                    "invalid Form choice-list string writer evidence: {reason}"
                )
            }
            Self::InvalidFormChoiceParametersWriterEvidence(reason) => {
                write!(
                    formatter,
                    "invalid Form choice-parameters writer evidence: {reason}"
                )
            }
            Self::InvalidFormChoiceParametersQName(value) => {
                write!(formatter, "invalid Form choice-parameters QName '{value}'")
            }
            Self::InvalidFormChoiceParameterClusterPolicy(reason) => {
                write!(
                    formatter,
                    "invalid Form choice-parameter cluster policy: {reason}"
                )
            }
        }
    }
}

impl Error for SchemaError {}

impl Display for WriterRuleLookupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceReleaseMismatch {
                requested,
                available,
            } => write!(
                formatter,
                "writer rule source release mismatch: requested `{requested}`, available `{available}`"
            ),
            Self::Missing {
                model_type,
                feature,
            } => write!(
                formatter,
                "writer rule is missing for `{model_type}` / `{feature}`"
            ),
            Self::Ambiguous {
                model_type,
                feature,
            } => write!(
                formatter,
                "writer rule is ambiguous for `{model_type}` / `{feature}`"
            ),
            Self::Unverified { id, status } => {
                write!(
                    formatter,
                    "writer rule `{id}` has unverified status `{status}`"
                )
            }
            Self::MissingTypedPolicy { id } => {
                write!(formatter, "writer rule `{id}` has no typed policy")
            }
        }
    }
}

impl Error for WriterRuleLookupError {}

impl ModelInventory {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let inventory: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut bundle_names = BTreeSet::new();
        let mut model_types = 0usize;
        let mut importers = 0usize;
        let mut exporters = 0usize;
        for bundle in &self.bundles {
            validate_text("bundle symbolic name", &bundle.symbolic_name)?;
            if !bundle_names.insert(bundle.symbolic_name.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "bundle symbolic name",
                    value: bundle.symbolic_name.clone(),
                });
            }
            validate_unique_names("model type", &bundle.model_types)?;
            validate_unique_names("importer", &bundle.importers)?;
            validate_unique_names("exporter", &bundle.exporters)?;
            model_types += bundle.model_types.len();
            importers += bundle.importers.len();
            exporters += bundle.exporters.len();
        }
        validate_count("bundles", self.summary.bundles, self.bundles.len())?;
        validate_count("modelTypes", self.summary.model_types, model_types)?;
        validate_count("importers", self.summary.importers, importers)?;
        validate_count("exporters", self.summary.exporters, exporters)
    }

    pub fn bundle(&self, symbolic_name: &str) -> Option<&BundleInventory> {
        self.bundles
            .iter()
            .find(|bundle| bundle.symbolic_name == symbolic_name)
    }
}

impl PackageFeatureCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut package_names = BTreeSet::new();
        let mut classifier_count = 0usize;
        let mut feature_count = 0usize;
        let mut operation_count = 0usize;
        for package in &self.packages {
            validate_text("model package bundle", &package.bundle)?;
            validate_text("model package class", &package.package_class)?;
            if !package_names.insert(package.package_class.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "model package class",
                    value: package.package_class.clone(),
                });
            }
            let mut classifier_tokens = BTreeSet::new();
            for classifier in &package.classifiers {
                validate_text("model classifier token", &classifier.token)?;
                if !classifier_tokens.insert(classifier.token.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "model classifier token",
                        value: classifier.token.clone(),
                    });
                }
                validate_members("model feature", &classifier.features)?;
                validate_members("model operation", &classifier.operations)?;
                classifier_count += 1;
                feature_count += classifier.features.len();
                operation_count += classifier.operations.len();
            }
        }
        validate_count("packages", self.summary.packages, self.packages.len())?;
        validate_count("classifiers", self.summary.classifiers, classifier_count)?;
        validate_count("features", self.summary.features, feature_count)?;
        validate_count("operations", self.summary.operations, operation_count)
    }

    pub fn package(&self, package_class: &str) -> Option<&ModelPackage> {
        self.packages
            .iter()
            .find(|package| package.package_class == package_class)
    }
}

impl DcsWriterEvidenceCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > MAX_DCS_WRITER_EVIDENCE_JSON_BYTES {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "JSON exceeds {MAX_DCS_WRITER_EVIDENCE_JSON_BYTES} UTF-8 bytes"
            )));
        }
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.schema_version != 1 {
            return Err(SchemaError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.source.product != "1C:EDT" || self.source.release != "2025.2.3+30" {
            return Err(invalid_dcs_writer_evidence(
                "source product or release does not match the verified evidence",
            ));
        }
        if self.source.derivation
            != "mixed provenance: deterministic extractor for writer-tail and delegation facts; manual exact javap bytecode review for wrapper QNames, no-TypeId call chains, and readSettings unknown-child rejection; no JAR, bytecode, source, Xcore, or machine path retained"
            || self.source.input_contract
                != "the deterministic base extractor requires exact-release dcs and form-export bundle JARs; supplemental manual review used exact-release dcs, form-export, and export-xml bundle classes named in each fact"
            || self.source.invocation
                != "base: pwsh tools/report-edt-dcs-writer-evidence.ps1 -InputInventory <external-version-matched-inventory.json> -EdtRelease <release> -OutputReport <portable-report.json>; supplemental: javap -v -p -c -constants on the exact classes and methods listed in fact provenance"
        {
            return Err(invalid_dcs_writer_evidence(
                "source derivation, input contract, or invocation does not match mixed provenance",
            ));
        }
        for (field, value) in [
            ("source product", self.source.product.as_str()),
            ("source release", self.source.release.as_str()),
            ("source derivation", self.source.derivation.as_str()),
            ("source input contract", self.source.input_contract.as_str()),
            ("source invocation", self.source.invocation.as_str()),
        ] {
            validate_dcs_writer_evidence_text(field, value)?;
        }
        if self.verified_facts.len() > MAX_DCS_WRITER_EVIDENCE_FACTS {
            return Err(invalid_dcs_writer_evidence(format!(
                "verified facts exceed {MAX_DCS_WRITER_EVIDENCE_FACTS}"
            )));
        }
        if self.missing_keys.len() > MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS {
            return Err(invalid_dcs_writer_evidence(format!(
                "missing keys exceed {MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS}"
            )));
        }

        let expected_fact_keys = BTreeSet::from([
            "dcs.settings.document.qname",
            "form.DynamicListExtInfo.listSettings.qname",
            "dcs.DataCompositionSettings.type-id",
            "dcs.DataCompositionSettings.namespace",
            "dcs.DataCompositionSettings.verified-tail-order",
            "dcs.DataCompositionSettings.itemsViewMode",
            "dcs.DataCompositionSettings.itemsUserSettingID",
            "dcs.DataCompositionSettings.default-value",
            "form.DynamicListExtInfo.listSettings.delegate",
        ]);
        let mut fact_keys = BTreeSet::new();
        let manually_reviewed_fact_keys = BTreeSet::from([
            "dcs.settings.document.qname",
            "form.DynamicListExtInfo.listSettings.qname",
            "dcs.DataCompositionSettings.type-id",
        ]);
        for fact in &self.verified_facts {
            validate_dcs_writer_evidence_text("verified fact key", &fact.key)?;
            if !fact_keys.insert(fact.key.as_str()) {
                return Err(invalid_dcs_writer_evidence(format!(
                    "duplicate verified fact `{}`",
                    fact.key
                )));
            }
            let manual = manually_reviewed_fact_keys.contains(fact.key.as_str());
            let expected_kind = if manual {
                "manually-reviewed-javap-bytecode-exact"
            } else {
                "javap-v-exact-method-control-flow-constant-pool"
            };
            if fact.evidence.status != EvidenceStatus::Verified
                || fact.evidence.kind != expected_kind
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` is not backed by its exact provenance kind",
                    fact.key
                )));
            }
            if fact.evidence.sources.is_empty()
                || fact.evidence.sources.len() > MAX_DCS_WRITER_EVIDENCE_SOURCES
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` has an invalid evidence source count",
                    fact.key
                )));
            }
            validate_dcs_writer_evidence_text("evidence kind", &fact.evidence.kind)?;
            validate_dcs_writer_evidence_text("evidence note", &fact.evidence.note)?;
            for source in &fact.evidence.sources {
                validate_dcs_writer_evidence_text("evidence source", source)?;
            }
            let has_script_source = fact
                .evidence
                .sources
                .iter()
                .any(|source| source == "tools/report-edt-dcs-writer-evidence.ps1");
            let all_edt_derived = fact
                .evidence
                .sources
                .iter()
                .all(|source| source.starts_with("edt-derived://2025.2.3+30/"));
            if (manual && (!all_edt_derived || has_script_source))
                || (!manual && !has_script_source)
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` provenance sources do not match its derivation",
                    fact.key
                )));
            }
            let exact_manual_sources = match fact.key.as_str() {
                "dcs.settings.document.qname" => Some(DCS_STANDALONE_QNAME_EVIDENCE_SOURCES),
                "form.DynamicListExtInfo.listSettings.qname" => {
                    Some(DCS_FORM_QNAME_EVIDENCE_SOURCES)
                }
                "dcs.DataCompositionSettings.type-id" => Some(DCS_NO_TYPE_ID_EVIDENCE_SOURCES),
                _ => None,
            };
            if let Some(expected) = exact_manual_sources
                && !fact
                    .evidence
                    .sources
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied())
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` exact bytecode coordinates drifted",
                    fact.key
                )));
            }
        }
        if fact_keys != expected_fact_keys {
            return Err(invalid_dcs_writer_evidence(
                "verified fact keys differ from the exact supported evidence set",
            ));
        }

        let expected_missing_keys =
            BTreeSet::from(["dcs.DataCompositionSettings.opaque-extension.placement"]);
        let mut missing_keys = BTreeSet::new();
        for missing in &self.missing_keys {
            for (field, value) in [
                ("missing key", missing.key.as_str()),
                ("missing key status", missing.status.as_str()),
                ("missing key reason", missing.reason.as_str()),
            ] {
                validate_dcs_writer_evidence_text(field, value)?;
            }
            if missing.status != "unsupported-no-lossless-placement"
                || !missing_keys.insert(missing.key.as_str())
            {
                return Err(invalid_dcs_writer_evidence(
                    "missing evidence keys are duplicate or have an unexpected status",
                ));
            }
            if missing.evidence.status != EvidenceStatus::Verified
                || missing.evidence.kind != "manually-reviewed-javap-bytecode-exact"
                || missing.evidence.sources.is_empty()
                || missing.evidence.sources.len() > MAX_DCS_WRITER_EVIDENCE_SOURCES
                || !missing
                    .evidence
                    .sources
                    .iter()
                    .all(|source| source.starts_with("edt-derived://2025.2.3+30/"))
                || !missing
                    .evidence
                    .sources
                    .iter()
                    .map(String::as_str)
                    .eq(DCS_OPAQUE_NEGATIVE_EVIDENCE_SOURCES.iter().copied())
            {
                return Err(invalid_dcs_writer_evidence(
                    "unsupported opaque placement lacks exact negative bytecode evidence",
                ));
            }
            validate_dcs_writer_evidence_text("missing-key evidence kind", &missing.evidence.kind)?;
            validate_dcs_writer_evidence_text("missing-key evidence note", &missing.evidence.note)?;
            for source in &missing.evidence.sources {
                validate_dcs_writer_evidence_text("missing-key evidence source", source)?;
            }
        }
        if missing_keys != expected_missing_keys {
            return Err(invalid_dcs_writer_evidence(
                "missing evidence keys differ from the exact unsupported no-lossless-placement fact",
            ));
        }

        self.verified_form_list_settings_tail_evidence()?;
        self.verified_settings_envelope_evidence().map(|_| ())
    }

    pub fn form_list_settings_tail_policy(
        &self,
        feature_semantics: &FeatureSemanticsCorpus,
    ) -> Result<DcsListSettingsTailPolicy, SchemaError> {
        feature_semantics.validate()?;
        if feature_semantics.source.release != self.source.release {
            return Err(invalid_dcs_writer_evidence(
                "writer evidence and feature semantics releases differ",
            ));
        }
        let view_feature = feature_semantics
            .feature(&FeatureSemanticKey {
                namespace_uri: DCS_SETTINGS_MODEL_NAMESPACE.to_owned(),
                classifier: DCS_SETTINGS_CLASSIFIER.to_owned(),
                feature: "itemsViewMode".to_owned(),
            })
            .ok_or_else(|| {
                invalid_dcs_writer_evidence(
                    "verified itemsViewMode feature semantics are unavailable",
                )
            })?;
        if view_feature.model_evidence.status != EvidenceStatus::Verified {
            return Err(invalid_dcs_writer_evidence(
                "itemsViewMode model default is not verified",
            ));
        }

        let (namespace, view, user_id) = self.verified_form_list_settings_tail_evidence()?;
        let model_default = view_feature.default_value.as_deref().ok_or_else(|| {
            invalid_dcs_writer_evidence("verified itemsViewMode model default is absent")
        })?;
        if (view.default_model_constant.as_str(), model_default) != ("QUICK_ACCESS", "QuickAccess")
        {
            return Err(invalid_dcs_writer_evidence(format!(
                "itemsViewMode exact default join requires writer `QUICK_ACCESS` and model `QuickAccess`, got writer `{}` and model `{model_default}`",
                view.default_model_constant
            )));
        }

        Ok(DcsListSettingsTailPolicy {
            namespace_uri: namespace.to_owned(),
            tail_order: [
                DcsListSettingsTailField::ItemsViewMode,
                DcsListSettingsTailField::ItemsUserSettingId,
            ],
            items_view_mode_qname: view.qname.clone(),
            items_view_mode_default: model_default.to_owned(),
            items_user_setting_id_qname: user_id.qname.clone(),
            items_user_setting_id_default: user_id.default_string.clone(),
        })
    }

    pub fn settings_serialization_policy(
        &self,
        feature_semantics: &FeatureSemanticsCorpus,
    ) -> Result<DcsSettingsSerializationPolicy, SchemaError> {
        // Keep the typed-tail/model-default join as part of the same evidence
        // boundary; an envelope is not usable if either projection drifts.
        self.form_list_settings_tail_policy(feature_semantics)?;
        self.verified_settings_envelope_evidence()
    }

    fn verified_settings_envelope_evidence(
        &self,
    ) -> Result<DcsSettingsSerializationPolicy, SchemaError> {
        let standalone_document_qname = match self.fact_value("dcs.settings.document.qname")? {
            DcsWriterEvidenceValue::StandaloneQName(value)
                if value.qname
                    == "{http://v8.1c.ru/8.1/data-composition-system/settings}Settings" =>
            {
                value.qname.clone()
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "standalone Settings QName drifted",
                ));
            }
        };
        let form_list_settings_qname = match self
            .fact_value("form.DynamicListExtInfo.listSettings.qname")?
        {
            DcsWriterEvidenceValue::FormWrapperQName(value)
                if value.qname == "{http://v8.1c.ru/8.3/xcf/logform}ListSettings"
                    && value.qname_source
                        == "ListSettingsWriter -> FormFeatureNameProvider/BaseQNameProvider fallback" =>
            {
                value.qname.clone()
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "Form ListSettings wrapper QName drifted",
                ));
            }
        };
        match self.fact_value("dcs.DataCompositionSettings.type-id")? {
            DcsWriterEvidenceValue::NoTypeId(value) if value.emission == "absent" => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "DataCompositionSettings TypeId absence drifted",
                ));
            }
        }
        Ok(DcsSettingsSerializationPolicy {
            standalone_document_qname,
            form_list_settings_qname,
        })
    }

    fn verified_form_list_settings_tail_evidence(
        &self,
    ) -> Result<
        (
            &str,
            &DcsEnumNotDefaultEvidence,
            &DcsStringNotDefaultEvidence,
        ),
        SchemaError,
    > {
        let namespace = match self.fact_value("dcs.DataCompositionSettings.namespace")? {
            DcsWriterEvidenceValue::Text(value)
                if value == "http://v8.1c.ru/8.1/data-composition-system/settings" =>
            {
                value.as_str()
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "DataCompositionSettings namespace drifted",
                ));
            }
        };
        match self.fact_value("dcs.DataCompositionSettings.verified-tail-order")? {
            DcsWriterEvidenceValue::TailOrder(order)
                if order == &["itemsViewMode", "itemsUserSettingID"] => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "verified settings tail order drifted",
                ));
            }
        }
        let view = match self.fact_value("dcs.DataCompositionSettings.itemsViewMode")? {
            DcsWriterEvidenceValue::EnumNotDefault(value)
                if value.qname
                    == "{http://v8.1c.ru/8.1/data-composition-system/settings}itemsViewMode"
                    && value.default_model_constant == "QUICK_ACCESS"
                    && value.writer == "V8XmlSerializer.writeEnumNotDefault" =>
            {
                value
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "itemsViewMode writer policy drifted",
                ));
            }
        };
        let user_id = match self.fact_value("dcs.DataCompositionSettings.itemsUserSettingID")? {
            DcsWriterEvidenceValue::StringNotDefault(value)
                if value.qname
                    == "{http://v8.1c.ru/8.1/data-composition-system/settings}itemsUserSettingID"
                    && value.default_string.is_empty()
                    && value.writer == "V8XmlSerializer.writeStringNotDefault" =>
            {
                value
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "itemsUserSettingID writer policy drifted",
                ));
            }
        };
        match self.fact_value("dcs.DataCompositionSettings.default-value")? {
            DcsWriterEvidenceValue::DefaultValue(value)
                if value.predicate == "DcsDefaultValueUtil.isDefaultValue"
                    && value.operations
                        == [
                            "V8XmlSerializer.writeEmptyElement",
                            "DcsV8Serializer.writeSettingsNamespace",
                        ] => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "settings default-value policy drifted",
                ));
            }
        }
        match self.fact_value("form.DynamicListExtInfo.listSettings.delegate")? {
            DcsWriterEvidenceValue::FormDelegate(value)
                if value.delegate == "DcsV8Serializer.writeSettings"
                    && value.qname_source == "IQNameProvider.getElementQName"
                    && value.null_branch.from_offset == 48
                    && value.null_branch.target_offset == 106
                    && value.null_branch.target_opcode == "return" => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "Form ListSettings delegate or null omission policy drifted",
                ));
            }
        }
        Ok((namespace, view, user_id))
    }

    fn fact_value(&self, key: &str) -> Result<&DcsWriterEvidenceValue, SchemaError> {
        self.verified_facts
            .iter()
            .find(|fact| fact.key == key)
            .map(|fact| &fact.value)
            .ok_or_else(|| invalid_dcs_writer_evidence(format!("missing verified fact `{key}`")))
    }
}

fn invalid_dcs_writer_evidence(reason: impl Into<String>) -> SchemaError {
    SchemaError::InvalidDcsWriterEvidence(reason.into())
}

fn validate_dcs_writer_evidence_text(field: &'static str, value: &str) -> Result<(), SchemaError> {
    if value.is_empty() {
        return Err(invalid_dcs_writer_evidence(format!("{field} is empty")));
    }
    if value.len() > MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES {
        return Err(invalid_dcs_writer_evidence(format!(
            "{field} exceeds {MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES} UTF-8 bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_dcs_writer_evidence(format!(
            "{field} contains a control character"
        )));
    }
    Ok(())
}

fn exact_form_choice_parameters_policy() -> WriterPolicy {
    WriterPolicy::FormChoiceParameters {
        owner_qname: "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters".to_owned(),
        owner_predecessor_qname: "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameterLinks".to_owned(),
        owner_successor_qname: "{http://v8.1c.ru/8.3/xcf/logform}AvailableTypes".to_owned(),
        empty_collection: FormChoiceParametersEmptyCollection::OmitWhenWriteDefaultFalse,
        item: Box::new(FormChoiceParameterItemPolicy {
            item_qname: "{http://v8.1c.ru/8.2/managed-application/core}item".to_owned(),
            name_attribute_qname: "{}name".to_owned(),
            value_qname: "{http://v8.1c.ru/8.2/managed-application/core}value".to_owned(),
            value_xsi_type: "FormChoiceListDesTimeValue".to_owned(),
            value_order: vec![
                FormChoiceParameterValuePart::Presentation,
                FormChoiceParameterValuePart::Value,
            ],
            presentation_qname: "{http://v8.1c.ru/8.3/xcf/logform}Presentation".to_owned(),
            scalar_value_qname: "{http://v8.1c.ru/8.3/xcf/logform}Value".to_owned(),
            boolean_xsi_type: "xs:boolean".to_owned(),
            design_time_ref_xsi_type: "xr:DesignTimeRef".to_owned(),
        }),
        fixed_array: Box::new(FormChoiceParameterFixedArrayPolicy {
            xsi_type: "v8:FixedArray".to_owned(),
            item_qname: "{http://v8.1c.ru/8.1/data/core}Value".to_owned(),
            item_xsi_type: "FormChoiceListDesTimeValue".to_owned(),
            item_order: vec![
                FormChoiceParameterValuePart::Presentation,
                FormChoiceParameterValuePart::Value,
            ],
        }),
    }
}

impl FormChoiceListStringWriterProof {
    fn parse(json: &str) -> Result<Self, SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned())
        };
        if json.len() > MAX_FORM_CHOICE_LIST_STRING_WRITER_PROOF_BYTES {
            return Err(invalid("compact proof exceeds the bounded JSON size"));
        }
        let proof: Self = serde_json::from_str(json)
            .map_err(|error| invalid(&format!("invalid compact proof JSON: {error}")))?;
        proof.validate()?;
        Ok(proof)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned())
        };
        if self.schema_version != 1
            || self.source.product != "1C:EDT"
            || self.source.release != "2025.2.3+30"
            || self.source.derivation != "compact-full-artifact-digest-bound-semantic-projection-v1"
            || self.rule.id != "form.choice-list.design-time-value"
            || self.rule.model_type != "FormChoiceList"
            || self.rule.feature != "values"
            || self.emission != FormChoiceListEmptyStringValue::SelfClosing
            || self.full_evidence_sha256 != FORM_CHOICE_LIST_STRING_WRITER_FULL_EVIDENCE_SHA256
            || self.provenance_ids
                != [
                    "choice-list-empty-string/full-artifact-v1",
                    "choice-list-empty-string/semantic-projection-v1",
                ]
        {
            return Err(invalid(
                "compact proof differs from the exact supported projection",
            ));
        }
        Ok(())
    }
}

fn bind_form_choice_list_string_writer_proof(
    json: &str,
    corpus: &WriterRuleCorpus,
) -> Result<(), SchemaError> {
    let invalid =
        |reason: &str| SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned());
    let proof = FormChoiceListStringWriterProof::parse(json)?;
    if corpus.source.release != proof.source.release {
        return Err(invalid("writer corpus and compact proof releases differ"));
    }
    let mut matching = corpus.rules.iter().filter(|rule| {
        rule.model_type == proof.rule.model_type && rule.feature == proof.rule.feature
    });
    let rule = matching
        .next()
        .ok_or_else(|| invalid("compact-proof writer rule is absent"))?;
    if matching.next().is_some() {
        return Err(invalid("compact-proof writer rule is ambiguous"));
    }
    let Some(WriterPolicy::FormChoiceList {
        empty_string_value, ..
    }) = rule.policy.as_ref()
    else {
        return Err(invalid(
            "compact-proof matching typed writer policy is absent",
        ));
    };
    if rule.id != proof.rule.id
        || rule.evidence.status != "verified"
        || *empty_string_value != proof.emission
    {
        return Err(invalid(
            "writer rule and compact empty-string proof are not cross-bound",
        ));
    }
    Ok(())
}

#[cfg(test)]
impl FormChoiceListStringWriterEvidence {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned())
        };
        if json.len() > MAX_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_BYTES {
            return Err(invalid("evidence exceeds the bounded JSON size"));
        }
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| invalid(&format!("invalid JSON: {error}")))?;
        let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
        if digest != FORM_CHOICE_LIST_STRING_WRITER_FULL_EVIDENCE_SHA256 {
            return Err(invalid("exact evidence artifact SHA-256 differs"));
        }
        evidence.validate()?;
        Ok(evidence)
    }

    fn validate(&self) -> Result<(), SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned())
        };
        if self.schema_version != 1
            || self.source.product != "1C:EDT"
            || self.source.release != "2025.2.3+30"
            || self.source.root_identity.leaf != "1c-edt-2025.2.3+30-x86_64"
            || self.source.root_identity.product_version != "2025.2.3"
            || self.source.root_identity.build_id != "2025.2.3.30"
            || self.source.root_identity.product != "com._1c.g5.v8.dt.product.application.rcp"
            || self.source.root_identity.application != "org.eclipse.ui.ide.workbench"
        {
            return Err(invalid("exact release identity differs"));
        }
        let expected_bundles = [
            ("com._1c.g5.v8.dt.form.export.xml", "10.1.0.v202602241426"),
            ("com._1c.g5.v8.dt.export.xml", "13.0.100.v202602241426"),
        ];
        let actual_bundles = self
            .source
            .validated_bundles
            .iter()
            .map(|bundle| (bundle.symbolic_name.as_str(), bundle.version.as_str()))
            .collect::<Vec<_>>();
        if actual_bundles != expected_bundles
            || self.source.derivation.trim().is_empty()
            || self.source.input_contract.trim().is_empty()
            || self.source.invocation
                != "pwsh tools/report-edt-form-choice-list-string-writer-evidence.ps1 -EdtRoot <installed-exact-release-edt-root> -EdtRelease <release> -OutputReport <portable-report.json>"
            || !self.missing_keys.is_empty()
            || self.verified_facts.len() != 1
        {
            return Err(invalid("exact evidence envelope differs"));
        }
        let fact = &self.verified_facts[0];
        if fact.key != "form.FormChoiceListDesTimeValue.value.empty-string"
            || fact.value.model_value_type != "mcore:StringValue"
            || fact.value.empty_predicate != "Strings.isNullOrEmpty"
            || fact.value.element != "feature QName"
            || fact.value.xsi_type != "xs:string"
            || fact.value.emission != FormChoiceListEmptyStringValue::SelfClosing
            || fact.evidence.kind != "javap-v-exact-method-control-flow-constant-pool"
            || fact.evidence.status != "verified"
            || fact.evidence.note.trim().is_empty()
        {
            return Err(invalid("verified empty-string fact differs"));
        }
        Ok(())
    }

    fn emission(&self) -> FormChoiceListEmptyStringValue {
        self.verified_facts[0].value.emission
    }
}

#[cfg(test)]
pub fn bundled_form_choice_list_string_writer_evidence()
-> Result<FormChoiceListStringWriterEvidence, SchemaError> {
    FormChoiceListStringWriterEvidence::parse(BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON)
}

#[cfg(test)]
pub fn bind_form_choice_list_string_writer_evidence(
    json: &str,
    corpus: &WriterRuleCorpus,
) -> Result<(), SchemaError> {
    let invalid =
        |reason: &str| SchemaError::InvalidFormChoiceListStringWriterEvidence(reason.to_owned());
    let evidence = FormChoiceListStringWriterEvidence::parse(json)?;
    if corpus.source.release != evidence.source.release {
        return Err(invalid("writer corpus and evidence releases differ"));
    }
    let mut matching = corpus
        .rules
        .iter()
        .filter(|rule| rule.model_type == "FormChoiceList" && rule.feature == "values");
    let rule = matching
        .next()
        .ok_or_else(|| invalid("matching writer rule is absent"))?;
    if matching.next().is_some() {
        return Err(invalid("matching writer rule is ambiguous"));
    }
    let Some(WriterPolicy::FormChoiceList {
        empty_string_value, ..
    }) = rule.policy.as_ref()
    else {
        return Err(invalid("matching typed writer policy is absent"));
    };
    if rule.id != "form.choice-list.design-time-value"
        || rule.source_class
            != "com._1c.g5.v8.dt.form.export.xml.writer.FormChoiceListDesTimeValueWriter"
        || rule.delegate.as_deref()
            != Some("com._1c.g5.v8.dt.form.export.xml.writer.FormSmartFeatureWriter")
        || rule.evidence.kind != "javap-v-exact-method-control-flow-constant-pool"
        || rule.evidence.status != "verified"
        || *empty_string_value != evidence.emission()
    {
        return Err(invalid(
            "writer rule and exact empty-string evidence are not cross-bound",
        ));
    }
    Ok(())
}

impl FormChoiceParametersWriterEvidence {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceParametersWriterEvidence(reason.to_owned())
        };
        if self.schema_version != 1
            || self.source.product != "1C:EDT"
            || self.source.release != "2025.2.3+30"
            || self.source.root_identity.leaf != "1c-edt-2025.2.3+30-x86_64"
            || self.source.root_identity.product_version != "2025.2.3"
            || self.source.root_identity.build_id != "2025.2.3.30"
        {
            return Err(invalid("exact release identity differs"));
        }
        let expected_bundles = [
            ("com._1c.g5.v8.dt.export.xml", "13.0.100.v202602241426"),
            ("com._1c.g5.v8.dt.form.export.xml", "10.1.0.v202602241426"),
            ("com._1c.g5.v8.dt.form.model", "14.0.0.v202602241426"),
            ("com._1c.g5.v8.dt.mcore", "8.6.0.v202602241426"),
        ];
        let actual_bundles = self
            .source
            .validated_bundles
            .iter()
            .map(|bundle| (bundle.symbolic_name.as_str(), bundle.version.as_str()))
            .collect::<Vec<_>>();
        if actual_bundles != expected_bundles {
            return Err(invalid("validated bundle set differs"));
        }
        if self.source.derivation.trim().is_empty()
            || self.source.invocation
                != "tools/report-edt-form-choice-parameters-writer-evidence.ps1"
            || self.scope.disposition != "production-emission-evidence"
            || !self.scope.production_emission
            || !self.missing_keys.is_empty()
        {
            return Err(invalid("production evidence envelope differs"));
        }
        let fixture_sha256 = format!(
            "{:x}",
            Sha256::digest(BUNDLED_FORM_CHOICE_PARAMETERS_LIVE_FIXTURE_JSON.as_bytes())
        );
        if self.verified_facts.live_slot27.fixture_sha256 != fixture_sha256 {
            return Err(invalid(
                "committed live fixture bytes do not match the bound SHA-256",
            ));
        }
        let facts = &self.verified_facts;
        if facts.model.model_type != "InputFieldExtInfo"
            || facts.model.feature != "choiceParameters"
            || facts.model.lower_bound != 0
            || facts.model.upper_bound != -1
            || facts.owner_order.feature_qname != facts.model.owner_qname
            || facts.writer.delegate != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
            || facts.live_slot27.fixture != "tests/fixtures/form_choice_parameters_slot27_live.json"
            || facts.live_slot27.fixture_sha256
                != "05e4ef14ae7e3de0b2cc7d1b46e042be6ec70df629c57355036c5c7e58148bf7"
            || facts.live_slot27.raw_row != "34accda9-6211-4bc3-be8d-e42a24260653.0"
            || facts.live_slot27.raw_source
                != "candidate_dump/Config_inflated/34accda9-6211-4bc3-be8d-e42a24260653.0__part0.txt"
            || facts.live_slot27.raw_source_sha256
                != "77a99cffaa0b5c81ccccafa3a5fa01dec56342b49d1cce2e56f97f28b62785b1"
            || facts.live_slot27.raw_slot != 27
            || facts.live_slot27.native_source
                != "DataProcessors/УправлениеПродажамиНаOzon/Forms/НастройкиИнтеграции/Ext/Form.xml"
            || facts.live_slot27.native_source_sha256
                != "30cf0689522d6b74408da77426a178df282361f36d3787c0cfaf456c85cb8b03"
            || facts.live_slot27.item_names_in_order
                != [
                    "Отбор.Статус",
                    "Отбор.ХозяйственнаяОперация",
                    "Отбор.ПометкаУдаления",
                ]
            || facts.live_slot27.value_kinds_in_order != ["U", "FixedArray", "B"]
        {
            return Err(invalid(
                "verified model, writer, or live slot-27 facts differ",
            ));
        }
        let expected = exact_form_choice_parameters_policy();
        let WriterPolicy::FormChoiceParameters {
            owner_qname,
            owner_predecessor_qname,
            owner_successor_qname,
            empty_collection,
            item,
            fixed_array,
        } = expected
        else {
            unreachable!()
        };
        if facts.model.owner_qname != owner_qname
            || facts.owner_order.predecessor_qname != owner_predecessor_qname
            || facts.owner_order.successor_qname != owner_successor_qname
            || facts.writer.empty_collection != empty_collection
            || &facts.writer.item != item.as_ref()
            || &facts.writer.fixed_array != fixed_array.as_ref()
        {
            return Err(invalid(
                "verified QName, hierarchy, order, or fixed-array facts differ",
            ));
        }
        Ok(())
    }

    fn policy(&self) -> WriterPolicy {
        WriterPolicy::FormChoiceParameters {
            owner_qname: self.verified_facts.model.owner_qname.clone(),
            owner_predecessor_qname: self.verified_facts.owner_order.predecessor_qname.clone(),
            owner_successor_qname: self.verified_facts.owner_order.successor_qname.clone(),
            empty_collection: self.verified_facts.writer.empty_collection,
            item: Box::new(self.verified_facts.writer.item.clone()),
            fixed_array: Box::new(self.verified_facts.writer.fixed_array.clone()),
        }
    }
}

pub fn bind_form_choice_parameters_writer_evidence(
    json: &str,
    corpus: &WriterRuleCorpus,
) -> Result<(), SchemaError> {
    let evidence = FormChoiceParametersWriterEvidence::parse(json)?;
    let rule = corpus
        .rules
        .iter()
        .find(|rule| rule.model_type == "InputFieldExtInfo" && rule.feature == "choiceParameters")
        .ok_or_else(|| {
            SchemaError::InvalidFormChoiceParametersWriterEvidence(
                "matching writer rule is absent".to_owned(),
            )
        })?;
    if rule.id != "form.input-field-ext-info.choice-parameters"
        || rule.source_class != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
        || rule.delegate.as_deref()
            != Some("com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter")
        || rule.evidence.status != "verified"
        || rule.policy.as_ref() != Some(&evidence.policy())
    {
        return Err(SchemaError::InvalidFormChoiceParametersWriterEvidence(
            "writer rule and exact evidence are not cross-bound".to_owned(),
        ));
    }
    Ok(())
}

impl WriterRuleCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut identifiers = BTreeSet::new();
        let mut exact_keys = BTreeSet::new();
        for rule in &self.rules {
            for (field, value) in [
                ("writer rule id", rule.id.as_str()),
                ("source class", rule.source_class.as_str()),
                ("model type", rule.model_type.as_str()),
                ("feature", rule.feature.as_str()),
                ("evidence kind", rule.evidence.kind.as_str()),
                ("evidence status", rule.evidence.status.as_str()),
            ] {
                validate_text(field, value)?;
            }
            if !identifiers.insert(rule.id.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "writer rule id",
                    value: rule.id.clone(),
                });
            }
            if !exact_keys.insert((rule.model_type.as_str(), rule.feature.as_str())) {
                return Err(SchemaError::DuplicateValue {
                    field: "writer rule model type/feature",
                    value: format!("{} / {}", rule.model_type, rule.feature),
                });
            }
            if rule.operations.is_empty() {
                return Err(SchemaError::EmptyField("writer rule operations"));
            }
            validate_unique_names("writer operation", &rule.operations)?;
            if let Some(policy) = &rule.policy {
                match policy {
                    WriterPolicy::FormChoiceList { item_order, .. } => {
                        if item_order.as_slice()
                            != [
                                FormChoiceListItemPart::Presentation,
                                FormChoiceListItemPart::CheckState,
                                FormChoiceListItemPart::Value,
                            ]
                        {
                            return Err(SchemaError::EmptyField(
                                "form choice-list verified item order",
                            ));
                        }
                    }
                    WriterPolicy::FormListSettings { delegate, .. } => {
                        validate_text("form list-settings delegate", delegate)?;
                        if rule.delegate.as_deref() != Some(delegate.as_str()) {
                            return Err(SchemaError::EmptyField(
                                "form list-settings matching delegate",
                            ));
                        }
                    }
                    policy @ WriterPolicy::FormChoiceParameters { .. } => {
                        if policy != &exact_form_choice_parameters_policy()
                            || rule.id != "form.input-field-ext-info.choice-parameters"
                            || rule.model_type != "InputFieldExtInfo"
                            || rule.feature != "choiceParameters"
                            || rule.source_class
                                != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
                            || rule.delegate.as_deref()
                                != Some("com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter")
                        {
                            return Err(SchemaError::InvalidFormChoiceParametersWriterEvidence(
                                "dedicated writer policy identity or exact facts differ".to_owned(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn rule(&self, id: &str) -> Option<&WriterRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    pub fn rules_for_class<'a>(
        &'a self,
        source_class: &'a str,
    ) -> impl Iterator<Item = &'a WriterRule> + 'a {
        self.rules
            .iter()
            .filter(move |rule| rule.source_class == source_class)
    }

    /// Returns one verified, structured rule for an exact corpus release/model/feature key.
    ///
    /// Every incomplete state is an error.  Callers must not fall back to a rule from another
    /// release or interpret the human-readable `operations`/`conditions` fields.
    pub fn exact_rule(&self, key: WriterRuleKey<'_>) -> Result<&WriterRule, WriterRuleLookupError> {
        if key.source_release != self.source.release {
            return Err(WriterRuleLookupError::SourceReleaseMismatch {
                requested: key.source_release.to_owned(),
                available: self.source.release.clone(),
            });
        }
        let mut matches = self
            .rules
            .iter()
            .filter(|rule| rule.model_type == key.model_type && rule.feature == key.feature);
        let Some(rule) = matches.next() else {
            return Err(WriterRuleLookupError::Missing {
                model_type: key.model_type.to_owned(),
                feature: key.feature.to_owned(),
            });
        };
        if matches.next().is_some() {
            return Err(WriterRuleLookupError::Ambiguous {
                model_type: key.model_type.to_owned(),
                feature: key.feature.to_owned(),
            });
        }
        if rule.evidence.status != "verified" {
            return Err(WriterRuleLookupError::Unverified {
                id: rule.id.clone(),
                status: rule.evidence.status.clone(),
            });
        }
        if rule.policy.is_none() {
            return Err(WriterRuleLookupError::MissingTypedPolicy {
                id: rule.id.clone(),
            });
        }
        Ok(rule)
    }
}

impl FeatureSemanticsCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut keys = BTreeSet::new();
        let mut package_names = BTreeSet::new();
        let mut classifier_count = 0usize;
        let mut feature_count = 0usize;
        for package in &self.packages {
            validate_text("feature semantics bundle", &package.bundle)?;
            validate_portable_pathlike("feature semantics resource", &package.resource)?;
            validate_text("feature semantics package", &package.package_name)?;
            validate_uri("feature semantics namespace URI", &package.namespace_uri)?;
            if !package_names.insert(package.namespace_uri.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "feature semantics namespace URI",
                    value: package.namespace_uri.clone(),
                });
            }
            let mut classifiers = BTreeSet::new();
            for classifier in &package.classifiers {
                validate_text("feature semantics classifier", &classifier.name)?;
                if !classifiers.insert(classifier.name.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "feature semantics classifier",
                        value: classifier.name.clone(),
                    });
                }
                classifier_count += 1;
                for semantics in &classifier.features {
                    let key = FeatureSemanticKey {
                        namespace_uri: package.namespace_uri.clone(),
                        classifier: classifier.name.clone(),
                        feature: semantics.name.clone(),
                    };
                    validate_feature_semantic_key(&key)?;
                    if !keys.insert(key.clone()) {
                        return Err(SchemaError::DuplicateValue {
                            field: "feature semantic key",
                            value: format!(
                                "{} / {} / {}",
                                key.namespace_uri, key.classifier, key.feature
                            ),
                        });
                    }
                    validate_text("feature model type", &semantics.model_type)?;
                    let mut qualifiers = BTreeSet::new();
                    for qualifier in &semantics.qualifiers {
                        if !qualifiers.insert(*qualifier) {
                            return Err(SchemaError::DuplicateValue {
                                field: "feature qualifier",
                                value: format!("{qualifier:?}").to_ascii_lowercase(),
                            });
                        }
                    }
                    if let Some(upper) = semantics.upper_bound
                        && semantics.lower_bound > upper
                    {
                        return Err(SchemaError::InvalidCardinality {
                            lower: semantics.lower_bound,
                            upper,
                        });
                    }
                    validate_feature_evidence("feature model evidence", &semantics.model_evidence)?;
                    validate_xml_feature_behavior(
                        &key,
                        &semantics.xml,
                        semantics.xml.evidence.status,
                    )?;
                    feature_count += 1;
                }
            }
        }
        validate_count("packages", self.summary.packages, self.packages.len())?;
        validate_count("classifiers", self.summary.classifiers, classifier_count)?;
        validate_count("features", self.summary.features, feature_count)
    }

    pub fn feature(&self, key: &FeatureSemanticKey) -> Option<&FeatureSemantics> {
        self.packages
            .iter()
            .find(|package| package.namespace_uri == key.namespace_uri)
            .and_then(|package| {
                package
                    .classifiers
                    .iter()
                    .find(|classifier| classifier.name == key.classifier)
            })
            .and_then(|classifier| {
                classifier
                    .features
                    .iter()
                    .find(|semantics| semantics.name == key.feature)
            })
    }
}

impl CanonicalCoverageCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        preflight_canonical_coverage_json(json)?;
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut keys = BTreeSet::new();
        let mut typed = 0usize;
        let mut opaque_lossless = 0usize;
        let mut unsupported = 0usize;
        let mut platform_only = 0usize;

        for entry in &self.entries {
            validate_feature_semantic_key(&entry.key)?;
            validate_feature_evidence("canonical coverage evidence", &entry.evidence)?;
            if entry.evidence.status != EvidenceStatus::Verified {
                return Err(SchemaError::InvalidCoverageEntry {
                    key: entry.key.clone(),
                    reason: "coverage mapping requires verified evidence",
                });
            }
            if !keys.insert(entry.key.clone()) {
                return Err(SchemaError::DuplicateValue {
                    field: "canonical coverage key",
                    value: format!(
                        "{} / {} / {}",
                        entry.key.namespace_uri, entry.key.classifier, entry.key.feature
                    ),
                });
            }
            for (field, value) in [
                ("canonical coverage type", entry.canonical_type.as_deref()),
                ("canonical coverage field", entry.canonical_field.as_deref()),
                (
                    "canonical opaque placement",
                    entry.opaque_placement.as_deref(),
                ),
                (
                    "canonical diagnostic code",
                    entry.diagnostic_code.as_deref(),
                ),
            ] {
                if let Some(value) = value {
                    validate_text(field, value)?;
                }
            }
            match entry.status {
                CoverageStatus::Typed => {
                    if entry.canonical_type.is_none() || entry.canonical_field.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "typed mapping requires canonical type and field",
                        });
                    }
                    if entry.opaque_placement.is_some() || entry.diagnostic_code.is_some() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "typed mapping contains irrelevant status fields",
                        });
                    }
                    typed += 1;
                }
                CoverageStatus::OpaqueLossless => {
                    if entry.opaque_placement.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "opaque-lossless mapping requires placement",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.diagnostic_code.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "opaque-lossless mapping contains irrelevant status fields",
                        });
                    }
                    opaque_lossless += 1;
                }
                CoverageStatus::Unsupported => {
                    if entry.diagnostic_code.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "unsupported mapping requires diagnostic code",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.opaque_placement.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "unsupported mapping contains irrelevant status fields",
                        });
                    }
                    unsupported += 1;
                }
                CoverageStatus::PlatformOnly => {
                    if entry.evidence.note.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "platform-only mapping requires an evidence note",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.opaque_placement.is_some()
                        || entry.diagnostic_code.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "platform-only mapping contains irrelevant status fields",
                        });
                    }
                    platform_only += 1;
                }
            }
        }

        validate_count("coverage entries", self.summary.entries, self.entries.len())?;
        validate_count("typed coverage", self.summary.typed, typed)?;
        validate_count(
            "opaque-lossless coverage",
            self.summary.opaque_lossless,
            opaque_lossless,
        )?;
        validate_count(
            "unsupported coverage",
            self.summary.unsupported,
            unsupported,
        )?;
        validate_count(
            "platform-only coverage",
            self.summary.platform_only,
            platform_only,
        )?;
        if self.family_aggregates != recompute_family_aggregates(&self.entries) {
            return Err(SchemaError::CoverageDerivedDataMismatch(
                "family aggregates",
            ));
        }

        let mut previous_backlog_key = None;
        for item in &self.migration_backlog {
            validate_text("canonical migration rule", &item.rule)?;
            validate_text("canonical migration package", &item.package)?;
            if item.features == 0 {
                return Err(SchemaError::CoverageDerivedDataMismatch(
                    "migration backlog",
                ));
            }
            let key = (
                item.family,
                item.package.as_str(),
                item.classifier_kind,
                item.feature_kind,
            );
            if previous_backlog_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err(SchemaError::CoverageDerivedDataMismatch(
                    "migration backlog order",
                ));
            }
            previous_backlog_key = Some(key);
        }
        Ok(())
    }

    /// Proves that coverage and feature corpora form an exact full join.
    pub fn validate_against(&self, features: &FeatureSemanticsCorpus) -> Result<(), SchemaError> {
        self.validate()?;
        features.validate()?;

        let feature_keys = features
            .packages
            .iter()
            .flat_map(|package| {
                package.classifiers.iter().flat_map(move |classifier| {
                    classifier
                        .features
                        .iter()
                        .map(move |feature| FeatureSemanticKey {
                            namespace_uri: package.namespace_uri.clone(),
                            classifier: classifier.name.clone(),
                            feature: feature.name.clone(),
                        })
                })
            })
            .collect::<BTreeSet<_>>();
        let coverage_keys = self
            .entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<BTreeSet<_>>();

        if let Some(key) = feature_keys.difference(&coverage_keys).next() {
            return Err(SchemaError::CoverageMismatch {
                kind: "unmapped",
                key: key.clone(),
            });
        }
        if let Some(key) = coverage_keys.difference(&feature_keys).next() {
            return Err(SchemaError::CoverageMismatch {
                kind: "stale",
                key: key.clone(),
            });
        }
        if self.migration_backlog != recompute_migration_backlog(self, features)? {
            return Err(SchemaError::CoverageDerivedDataMismatch(
                "migration backlog",
            ));
        }
        Ok(())
    }
}

impl MetadataOrderCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        validate_text("metadata order bundle", &self.summary.bundle)?;
        let mut keys = BTreeSet::new();

        for record in &self.records {
            validate_text("metadata order provider", &record.provider)?;
            validate_text("metadata order classifier", &record.classifier)?;
            if record.ordered_features.is_empty() {
                return Err(SchemaError::EmptyField("metadata order ordered features"));
            }
            let mut features = BTreeSet::new();
            for feature in &record.ordered_features {
                validate_text("metadata order feature", feature)?;
                if !features.insert(feature.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "metadata order feature",
                        value: feature.clone(),
                    });
                }
            }
            for operation in &record.order_operations {
                validate_text("metadata order operation feature", &operation.feature)?;
            }
            let operation_features = record
                .order_operations
                .iter()
                .map(|operation| operation.feature.as_str())
                .collect::<Vec<_>>();
            let ordered_features = record
                .ordered_features
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            match record.section {
                MetadataOrderSection::Properties => {
                    if operation_features != ordered_features
                        || record.order_operations.first().is_none_or(|operation| {
                            operation.operation != MetadataOrderOperationKind::Cursor
                        })
                        || record.order_operations.iter().any(|operation| {
                            operation.operation == MetadataOrderOperationKind::Emit
                        })
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid properties order operations for {}",
                            record.classifier
                        )));
                    }
                    if record.fallback != MetadataOrderFallback::DefaultPropertyFilterWhenUnmapped {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid properties fallback for {}",
                            record.classifier
                        )));
                    }
                }
                MetadataOrderSection::InternalInfo | MetadataOrderSection::ChildObjects => {
                    if operation_features != ordered_features
                        || record.order_operations.iter().any(|operation| {
                            operation.operation != MetadataOrderOperationKind::Emit
                        })
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid {:?} order operations for {}",
                            record.section, record.classifier
                        )));
                    }
                    if record.fallback != MetadataOrderFallback::ProducedTypesWhenPresent {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid {:?} fallback for {}",
                            record.section, record.classifier
                        )));
                    }
                }
                MetadataOrderSection::ProducedTypes => {
                    if !record.order_operations.is_empty()
                        || record.fallback != MetadataOrderFallback::AllReferencesWhenUnmapped
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid produced-types operations or fallback for {}",
                            record.classifier
                        )));
                    }
                }
            }
            validate_feature_evidence("metadata order evidence", &record.evidence)?;
            if record.evidence.status != EvidenceStatus::Verified {
                return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                    key: FeatureSemanticKey {
                        namespace_uri: record.provider.clone(),
                        classifier: record.classifier.clone(),
                        feature: format!("{:?}", record.section),
                    },
                    field: "metadata order evidence",
                });
            }
            if !keys.insert((
                record.provider.as_str(),
                record.classifier.as_str(),
                record.section,
                record.version_predicate,
            )) {
                return Err(SchemaError::DuplicateValue {
                    field: "metadata order provider/classifier/section/version",
                    value: format!(
                        "{} / {} / {:?} / {:?}",
                        record.provider,
                        record.classifier,
                        record.section,
                        record.version_predicate
                    ),
                });
            }
        }

        validate_count(
            "metadata verified records",
            self.summary.verified_records,
            self.records.len(),
        )
    }

    pub fn order(
        &self,
        provider: &str,
        classifier: &str,
        section: MetadataOrderSection,
        version_predicate: MetadataOrderVersionPredicate,
    ) -> Option<&MetadataOrderRecord> {
        self.records.iter().find(|record| {
            record.provider == provider
                && record.classifier == classifier
                && record.section == section
                && record.version_predicate == version_predicate
        })
    }
}

pub fn bundled_model_inventory() -> Result<ModelInventory, SchemaError> {
    ModelInventory::parse(BUNDLED_MODEL_INVENTORY_JSON)
}

pub fn bundled_package_features() -> Result<PackageFeatureCorpus, SchemaError> {
    PackageFeatureCorpus::parse(BUNDLED_PACKAGE_FEATURES_JSON)
}

pub fn bundled_feature_semantics() -> Result<FeatureSemanticsCorpus, SchemaError> {
    FeatureSemanticsCorpus::parse(BUNDLED_FEATURE_SEMANTICS_JSON)
}

fn bundled_dcs_list_settings_feature_semantics() -> Result<FeatureSemanticsCorpus, SchemaError> {
    FeatureSemanticsCorpus::parse(BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON)
}

pub fn bundled_canonical_coverage() -> Result<CanonicalCoverageCorpus, SchemaError> {
    let coverage = CanonicalCoverageCorpus::parse(BUNDLED_CANONICAL_COVERAGE_JSON)?;
    coverage.validate_against(&bundled_feature_semantics()?)?;
    Ok(coverage)
}

pub fn bundled_metadata_order() -> Result<MetadataOrderCorpus, SchemaError> {
    MetadataOrderCorpus::parse(BUNDLED_METADATA_ORDER_JSON)
}

pub fn bundled_writer_rules() -> Result<WriterRuleCorpus, SchemaError> {
    let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON)?;
    bind_form_choice_list_string_writer_proof(
        BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON,
        &corpus,
    )?;
    bind_form_choice_parameters_writer_evidence(
        BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
        &corpus,
    )?;
    Ok(corpus)
}

pub fn bundled_form_choice_parameters_writer_evidence()
-> Result<FormChoiceParametersWriterEvidence, SchemaError> {
    FormChoiceParametersWriterEvidence::parse(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON)
}

pub fn bundled_dcs_writer_evidence() -> Result<DcsWriterEvidenceCorpus, SchemaError> {
    DcsWriterEvidenceCorpus::parse(BUNDLED_DCS_WRITER_EVIDENCE_JSON)
}

/// Returns the immutable platform-authenticated root selection policy.
pub fn bundled_dcs_selection_policy() -> Result<DcsSelectionPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsSelectionPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsSelectionEvidenceCorpus::parse(BUNDLED_DCS_SELECTION_EVIDENCE_JSON)
                .map(DcsSelectionEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the immutable platform-authenticated policy for the exact bounded
/// standalone settings subtrees that remain source-owned.
pub fn bundled_dcs_settings_source_owned_policy()
-> Result<DcsSettingsSourceOwnedPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsSettingsSourceOwnedPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsSettingsSourceOwnedEvidenceCorpus::parse(
                BUNDLED_DCS_SETTINGS_SOURCE_OWNED_EVIDENCE_JSON,
            )
            .map(DcsSettingsSourceOwnedEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the immutable platform-authenticated DCS schema-template envelope
/// policy, bounded to the one- and two-variant cohorts actually attested.
pub fn bundled_dcs_schema_template_envelope_policy()
-> Result<DcsSchemaTemplateEnvelopePolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsSchemaTemplateEnvelopePolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsSchemaTemplateEnvelopeEvidenceCorpus::parse(
                BUNDLED_DCS_SCHEMA_TEMPLATE_ENVELOPE_EVIDENCE_JSON,
            )
            .map(DcsSchemaTemplateEnvelopeEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the immutable platform-authenticated standalone/Form order policy.
pub fn bundled_dcs_order_policy() -> Result<DcsOrderPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsOrderPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsOrderEvidenceCorpus::parse(BUNDLED_DCS_ORDER_EVIDENCE_JSON)
                .map(DcsOrderEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the immutable platform-authenticated standalone/Form filter policy.
pub fn bundled_dcs_filter_policy() -> Result<DcsFilterPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsFilterPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsFilterEvidenceCorpus::parse(BUNDLED_DCS_FILTER_EVIDENCE_JSON)
                .map(DcsFilterEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the immutable platform-authenticated standalone/Form
/// conditional-appearance policy.
pub fn bundled_dcs_conditional_appearance_policy()
-> Result<DcsConditionalAppearancePolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsConditionalAppearancePolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsConditionalAppearanceEvidenceCorpus::parse(
                BUNDLED_DCS_CONDITIONAL_APPEARANCE_EVIDENCE_JSON,
            )
            .map(DcsConditionalAppearanceEvidenceCorpus::into_policy)
        })
        .clone()
}

/// Returns the platform-authenticated physical wrapper policy for Form-wide
/// `Attributes/ConditionalAppearance`.
pub fn bundled_dcs_form_attributes_conditional_appearance_policy()
-> Result<DcsFormAttributesConditionalAppearancePolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsFormAttributesConditionalAppearancePolicy, SchemaError>> =
        OnceLock::new();
    POLICY
        .get_or_init(|| {
            DcsFormAttributesConditionalAppearanceEvidenceCorpus::parse(
                BUNDLED_DCS_FORM_ATTRIBUTES_CONDITIONAL_APPEARANCE_EVIDENCE_JSON,
            )
            .map(DcsFormAttributesConditionalAppearanceEvidenceCorpus::into_policy)
        })
        .clone()
}

pub fn bundled_dcs_list_settings_tail_policy() -> Result<DcsListSettingsTailPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsListSettingsTailPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            let evidence = bundled_dcs_writer_evidence()?;
            let feature_semantics = bundled_dcs_list_settings_feature_semantics()?;
            evidence.form_list_settings_tail_policy(&feature_semantics)
        })
        .clone()
}

pub fn bundled_dcs_settings_serialization_policy()
-> Result<DcsSettingsSerializationPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsSettingsSerializationPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            let evidence = bundled_dcs_writer_evidence()?;
            let semantics = bundled_dcs_list_settings_feature_semantics()?;
            evidence.settings_serialization_policy(&semantics)
        })
        .clone()
}

fn validate_source(schema_version: u32, source: &CorpusSource) -> Result<(), SchemaError> {
    if schema_version != 1 {
        return Err(SchemaError::UnsupportedSchemaVersion(schema_version));
    }
    for (field, value) in [
        ("source product", source.product.as_str()),
        ("source release", source.release.as_str()),
        ("source derivation", source.derivation.as_str()),
    ] {
        validate_text(field, value)?;
    }
    Ok(())
}

fn validate_feature_semantic_key(key: &FeatureSemanticKey) -> Result<(), SchemaError> {
    validate_uri("feature semantic namespace URI", &key.namespace_uri)?;
    validate_text("feature semantic classifier", &key.classifier)?;
    validate_text("feature semantic feature", &key.feature)
}

fn validate_feature_evidence(
    field: &'static str,
    evidence: &FeatureEvidence,
) -> Result<(), SchemaError> {
    validate_text("feature evidence kind", &evidence.kind)?;
    if evidence.status == EvidenceStatus::Verified && evidence.sources.is_empty() {
        return Err(SchemaError::EmptyField("verified feature evidence sources"));
    }
    let mut sources = BTreeSet::new();
    for source in &evidence.sources {
        validate_portable_pathlike("feature evidence source", source)?;
        if !sources.insert(source.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field: "feature evidence source",
                value: source.clone(),
            });
        }
    }
    if let Some(note) = &evidence.note {
        validate_text(field, note)?;
    }
    Ok(())
}

fn validate_xml_feature_behavior(
    key: &FeatureSemanticKey,
    behavior: &XmlFeatureBehavior,
    status: EvidenceStatus,
) -> Result<(), SchemaError> {
    validate_feature_evidence("feature XML evidence", &behavior.evidence)?;
    if let Some(qname) = &behavior.qname {
        validate_text("XML QName", qname)?;
    }
    validate_evidence_value("XML version gate", &behavior.version_gate)?;
    validate_evidence_value("XML delegate", &behavior.delegate)?;
    if status == EvidenceStatus::Verified {
        if behavior.qname.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "QName",
            });
        }
        if behavior.order.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "order",
            });
        }
        if behavior.emit_default.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "default emission",
            });
        }
        if matches!(behavior.version_gate, EvidenceValue::Pending) {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "version gate",
            });
        }
        if matches!(behavior.delegate, EvidenceValue::Pending) {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "delegate",
            });
        }
    }
    Ok(())
}

fn validate_evidence_value(
    field: &'static str,
    evidence: &EvidenceValue<String>,
) -> Result<(), SchemaError> {
    if let EvidenceValue::Known { value: Some(value) } = evidence {
        validate_text(field, value)?;
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), SchemaError> {
    if value.trim().is_empty() {
        return Err(SchemaError::EmptyField(field));
    }
    let lower = value.to_ascii_lowercase();
    let drive_rooted = value.as_bytes().get(1) == Some(&b':')
        && matches!(value.as_bytes().get(2), Some(b'/') | Some(b'\\'));
    if drive_rooted
        || lower.starts_with("file:")
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || value.starts_with('/')
        || lower.contains("program files")
        || lower.contains("users\\")
    {
        return Err(SchemaError::NonPortablePath(value.to_owned()));
    }
    Ok(())
}

fn validate_portable_pathlike(field: &'static str, value: &str) -> Result<(), SchemaError> {
    validate_text(field, value)
}

fn validate_uri(field: &'static str, value: &str) -> Result<(), SchemaError> {
    validate_text(field, value)?;
    let Some((scheme, _)) = value.split_once(':') else {
        return Err(SchemaError::InvalidJson(format!("{field} is not a URI")));
    };
    if scheme.is_empty()
        || !scheme.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphabetic()
                || (index > 0
                    && (character.is_ascii_digit() || matches!(character, '+' | '-' | '.')))
        })
    {
        return Err(SchemaError::InvalidJson(format!("{field} is not a URI")));
    }
    Ok(())
}

fn validate_unique_names(field: &'static str, values: &[String]) -> Result<(), SchemaError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field,
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_members(field: &'static str, values: &[ModelMember]) -> Result<(), SchemaError> {
    let mut tokens = BTreeSet::new();
    for value in values {
        validate_text(field, &value.token)?;
        if !tokens.insert(value.token.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field,
                value: value.token.clone(),
            });
        }
    }
    Ok(())
}

fn validate_count(field: &'static str, expected: usize, actual: usize) -> Result<(), SchemaError> {
    if expected != actual {
        return Err(SchemaError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_document_context_menu_slots_are_owner_scoped_and_fail_closed() {
        let mut fields = vec!["0"; 43];
        fields[0] = "48";
        fields[5] = "7";
        fields[41] = "0";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Ok(FormTextDocumentContextMenu::Absent)
        );

        fields[41] = "1";
        fields[42] = "payload";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |payload| {
                (payload == "payload").then_some("menu")
            }),
            Ok(FormTextDocumentContextMenu::Present("menu"))
        );
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| None::<()>),
            Err(FormTextDocumentContextMenuParseError::ForeignChild)
        );

        fields[41] = "2";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::Duplicate)
        );
        fields[41] = "3";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::Duplicate)
        );
        fields[41] = "02";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::InvalidMultiplicity)
        );
        fields[41] = "1";
        fields.truncate(42);
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::MissingPayload)
        );

        fields.resize(43, "0");
        fields[0] = "48";
        fields[5] = "2";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::WrongDiscriminator)
        );
        fields[0] = "37";
        assert_eq!(
            parse_form_text_document_context_menu(&fields, |_| Some("menu")),
            Err(FormTextDocumentContextMenuParseError::WrongWrapper)
        );
    }

    #[test]
    fn generated_metadata_reference_owner_parses_all_supported_kinds_exactly() {
        let cases = [
            ("Catalog", GeneratedMetadataReferenceOwnerKind::Catalog),
            ("Document", GeneratedMetadataReferenceOwnerKind::Document),
            ("Enum", GeneratedMetadataReferenceOwnerKind::Enum),
            (
                "ExchangePlan",
                GeneratedMetadataReferenceOwnerKind::ExchangePlan,
            ),
            (
                "ChartOfAccounts",
                GeneratedMetadataReferenceOwnerKind::ChartOfAccounts,
            ),
            (
                "ChartOfCharacteristicTypes",
                GeneratedMetadataReferenceOwnerKind::ChartOfCharacteristicTypes,
            ),
            (
                "ChartOfCalculationTypes",
                GeneratedMetadataReferenceOwnerKind::ChartOfCalculationTypes,
            ),
            (
                "BusinessProcess",
                GeneratedMetadataReferenceOwnerKind::BusinessProcess,
            ),
            ("Task", GeneratedMetadataReferenceOwnerKind::Task),
        ];

        for (token, expected_kind) in cases {
            let type_reference = format!("cfg:{token}Ref.Владелец");
            let owner = parse_generated_metadata_reference_owner(&type_reference).unwrap();
            assert_eq!(owner.kind(), expected_kind);
            assert_eq!(owner.name(), "Владелец");
            assert_eq!(owner.owner_reference(), format!("{token}.Владелец"));
        }
    }

    #[test]
    fn task_number_auto_prefix_slot_uses_platform_proven_mapping() {
        assert_eq!(
            parse_task_number_auto_prefix_slot("0"),
            Some(TaskNumberAutoPrefix::BusinessProcessNumber)
        );
        assert_eq!(
            parse_task_number_auto_prefix_slot(" 0 ")
                .unwrap()
                .xml_value(),
            "BusinessProcessNumber"
        );

        for unknown in ["", "1", "-1", "BusinessProcessNumber"] {
            assert_eq!(parse_task_number_auto_prefix_slot(unknown), None);
        }
    }

    #[test]
    fn task_scalar_slots_use_platform_proven_xml_mappings() {
        assert_eq!(
            parse_task_number_allowed_length_slot("1")
                .unwrap()
                .xml_value(),
            "Variable"
        );
        assert_eq!(
            parse_task_choice_history_on_input_slot("1")
                .unwrap()
                .xml_value(),
            "Auto"
        );
        assert_eq!(parse_task_include_help_in_contents_slot("1"), Some(false));
        assert_eq!(
            parse_task_data_lock_control_mode_slot("1")
                .unwrap()
                .xml_value(),
            "Automatic"
        );
        assert_eq!(
            parse_task_full_text_search_slot("0").unwrap().xml_value(),
            "Use"
        );

        assert!(parse_task_number_allowed_length_slot("0").is_none());
        assert!(parse_task_choice_history_on_input_slot("0").is_none());
        assert!(parse_task_include_help_in_contents_slot("0").is_none());
        assert!(parse_task_data_lock_control_mode_slot("0").is_none());
        assert!(parse_task_full_text_search_slot("1").is_none());
    }

    #[test]
    fn generated_metadata_reference_owner_rejects_unknown_or_inexact_shapes() {
        for hostile in [
            "",
            "CatalogRef.Owner",
            "cfg:catalogRef.Owner",
            "cfg:UnknownRef.Owner",
            "cfg:Ref.Owner",
            "cfg:Enum.Owner",
            "cfg:EnumRef.",
            "cfg:EnumRef.Owner.Child",
            "cfg:EnumRef.OwnerRef.Child",
            "cfg:EnumRefOwner",
            "cfg:EnumRef.Owner ",
            " cfg:EnumRef.Owner",
        ] {
            assert!(
                parse_generated_metadata_reference_owner(hostile).is_none(),
                "{hostile:?}"
            );
        }

        let catalog_named_enum =
            parse_generated_metadata_reference_owner("cfg:CatalogRef.Enum").unwrap();
        assert_eq!(
            catalog_named_enum.kind(),
            GeneratedMetadataReferenceOwnerKind::Catalog
        );
        assert_eq!(catalog_named_enum.name(), "Enum");
        assert_eq!(catalog_named_enum.owner_reference(), "Catalog.Enum");
    }

    #[test]
    fn generated_metadata_owner_and_data_paths_are_exact_and_role_aware() {
        for (reference, family, role, owner) in [
            (
                "cfg:CatalogRef.Products",
                GeneratedMetadataOwnerFamily::Catalog,
                GeneratedMetadataOwnerRole::Ref,
                "Catalog.Products",
            ),
            (
                "cfg:DocumentObject.Invoice",
                GeneratedMetadataOwnerFamily::Document,
                GeneratedMetadataOwnerRole::Object,
                "Document.Invoice",
            ),
            (
                "cfg:InformationRegisterRecord.Prices",
                GeneratedMetadataOwnerFamily::InformationRegister,
                GeneratedMetadataOwnerRole::Record,
                "InformationRegister.Prices",
            ),
            (
                "cfg:InformationRegisterRecordManager.Prices",
                GeneratedMetadataOwnerFamily::InformationRegister,
                GeneratedMetadataOwnerRole::RecordManager,
                "InformationRegister.Prices",
            ),
            (
                "cfg:InformationRegisterRecordSet.Prices",
                GeneratedMetadataOwnerFamily::InformationRegister,
                GeneratedMetadataOwnerRole::RecordSet,
                "InformationRegister.Prices",
            ),
            (
                "cfg:AccountingRegisterRecordKey.Ledger",
                GeneratedMetadataOwnerFamily::AccountingRegister,
                GeneratedMetadataOwnerRole::RecordKey,
                "AccountingRegister.Ledger",
            ),
            (
                "cfg:AccumulationRegisterObject.Totals",
                GeneratedMetadataOwnerFamily::AccumulationRegister,
                GeneratedMetadataOwnerRole::Object,
                "AccumulationRegister.Totals",
            ),
            (
                "cfg:AccumulationRegisterRecordKey.Totals",
                GeneratedMetadataOwnerFamily::AccumulationRegister,
                GeneratedMetadataOwnerRole::RecordKey,
                "AccumulationRegister.Totals",
            ),
            (
                "cfg:CalculationRegisterRecord.Payroll",
                GeneratedMetadataOwnerFamily::CalculationRegister,
                GeneratedMetadataOwnerRole::Record,
                "CalculationRegister.Payroll",
            ),
            (
                "cfg:CalculationRegisterRecordSet.Payroll",
                GeneratedMetadataOwnerFamily::CalculationRegister,
                GeneratedMetadataOwnerRole::RecordSet,
                "CalculationRegister.Payroll",
            ),
            (
                "cfg:InformationRegisterRecordKey.Prices",
                GeneratedMetadataOwnerFamily::InformationRegister,
                GeneratedMetadataOwnerRole::RecordKey,
                "InformationRegister.Prices",
            ),
            (
                "cfg:ExchangePlanObject.Sync",
                GeneratedMetadataOwnerFamily::ExchangePlan,
                GeneratedMetadataOwnerRole::Object,
                "ExchangePlan.Sync",
            ),
            (
                "cfg:ReportObject.PurchaseBook",
                GeneratedMetadataOwnerFamily::Report,
                GeneratedMetadataOwnerRole::Object,
                "Report.PurchaseBook",
            ),
        ] {
            let parsed = parse_generated_metadata_owner(reference).unwrap();
            assert_eq!(parsed.family(), family);
            assert_eq!(parsed.role(), role);
            assert_eq!(parsed.owner_reference(), owner);
        }
        for hostile in [
            "cfg:UnknownRef.Owner",
            "cfg:DocumentObject.",
            "cfg:DocumentObject.Invoice.Child",
            "cfg:EnumObject.Status",
            "cfg:ReportRecord.Sales",
            "cfg:DataProcessorRecordKey.Loader",
            "cfg:DocumentUnknown.Invoice",
        ] {
            assert!(
                parse_generated_metadata_owner(hostile).is_none(),
                "{hostile:?}"
            );
        }

        let nested =
            parse_metadata_data_path("Document.Invoice.TabularSection.Lines.Attribute.Amount")
                .unwrap();
        assert_eq!(nested.role(), MetadataDataPathRole::TabularAttribute);
        assert_eq!(nested.owner_reference(), "Document.Invoice");
        assert_eq!(nested.table_name(), Some("Lines"));
        assert_eq!(nested.member_name(), "Amount");
        for hostile in [
            "Document.Invoice.Dimension.Region.More",
            "Document.Invoice.TabularSection.Lines.Dimension.Region",
            "Unknown.Invoice.Attribute.Value",
            "Document.Invoice.Attribute.",
        ] {
            assert!(parse_metadata_data_path(hostile).is_none(), "{hostile:?}");
        }
    }

    #[test]
    fn opaque_choice_list_diagnostic_is_profile_exact_and_nonrecoverable() {
        let raw = "{9,2}";
        let input = FormChoiceListLayoutProfile::InputFieldExtendedOptions.opaque_diagnostic(raw);
        let radio = FormChoiceListLayoutProfile::RadioButtonOptions.opaque_diagnostic(raw);
        for diagnostic in [&input, &radio] {
            assert_eq!(
                diagnostic.identity().code(),
                "source_asset.form.choice_list.opaque_asset_not_emitted"
            );
            assert_eq!(
                diagnostic.identity().classification(),
                "opaque_asset_not_emitted"
            );
            assert_eq!(diagnostic.identity().property(), "ChoiceList");
            assert_eq!(diagnostic.raw_length(), 5);
            assert_eq!(
                diagnostic.raw_sha256(),
                "fc5a90866629ef9f896b3530012f5ff3aae4d21ca25a51c44347e937c75e6926"
            );
            assert!(!format!("{diagnostic:?}").contains(raw));
        }
        assert_eq!(input.identity().profile(), "input_field_extended_options");
        assert_eq!(radio.identity().profile(), "radio_button_options");
        assert_eq!(input.raw_length(), radio.raw_length());
        assert_eq!(input.raw_sha256(), radio.raw_sha256());
        assert_eq!(
            input,
            FormChoiceListLayoutProfile::InputFieldExtendedOptions.opaque_diagnostic(raw)
        );
        let mutated =
            FormChoiceListLayoutProfile::InputFieldExtendedOptions.opaque_diagnostic("{9,3}");
        assert_eq!(mutated.raw_length(), input.raw_length());
        assert_ne!(mutated.raw_sha256(), input.raw_sha256());
    }

    #[test]
    fn bundled_inventory_is_complete_and_portable() {
        let inventory = bundled_model_inventory().unwrap();
        assert_eq!(inventory.source.release, "2025.2.3+30");
        assert_eq!(inventory.summary.bundles, 76);
        assert_eq!(inventory.summary.model_types, 3_902);
        assert_eq!(inventory.summary.importers, 229);
        assert_eq!(inventory.summary.exporters, 265);
        assert!(
            inventory
                .bundle("com._1c.g5.v8.dt.form.export.xml")
                .unwrap()
                .exporters
                .iter()
                .any(|name| name.ends_with("FormChoiceListDesTimeValueWriter"))
        );
    }

    #[test]
    fn bundled_writer_rules_are_verified_and_queryable() {
        let corpus = bundled_writer_rules().unwrap();
        assert_eq!(corpus.rules.len(), 4);
        let choice = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "values",
            })
            .unwrap();
        assert_eq!(choice.evidence.status, "verified");
        assert_eq!(
            choice.policy,
            Some(WriterPolicy::FormChoiceList {
                item_order: vec![
                    FormChoiceListItemPart::Presentation,
                    FormChoiceListItemPart::CheckState,
                    FormChoiceListItemPart::Value,
                ],
                empty_collection: FormChoiceListEmptyCollection::WriteWrapperWhenWriteDefault,
                empty_string_value: FormChoiceListEmptyStringValue::SelfClosing,
            })
        );

        let settings = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "DynamicListExtInfo",
                feature: "listSettings",
            })
            .unwrap();
        assert_eq!(
            settings.policy,
            Some(WriterPolicy::FormListSettings {
                null_value: FormListSettingsNullValue::Omit,
                delegate: "DcsV8Serializer.writeSettings".to_owned(),
            })
        );

        let choice_parameters = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "InputFieldExtInfo",
                feature: "choiceParameters",
            })
            .unwrap();
        assert_eq!(
            choice_parameters.policy,
            Some(exact_form_choice_parameters_policy())
        );
        let bundled_json: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).unwrap();
        let bundled_policy = bundled_json["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|rule| rule["feature"] == "choiceParameters")
            .unwrap()["policy"]
            .clone();
        assert_eq!(
            serde_json::to_value(choice_parameters.policy.as_ref().unwrap()).unwrap(),
            bundled_policy,
            "typed indirection must not change the writer-policy JSON contract"
        );
    }

    #[test]
    fn form_choice_parameters_policy_is_strictly_cross_bound_to_production_evidence() {
        let evidence =
            bundled_form_choice_parameters_writer_evidence().expect("strict production evidence");
        assert!(evidence.scope.production_emission);
        assert!(evidence.missing_keys.is_empty());
        let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        bind_form_choice_parameters_writer_evidence(
            BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
            &corpus,
        )
        .unwrap();

        let raw: serde_json::Value =
            serde_json::from_str(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON).unwrap();
        let mut unknown = raw.clone();
        unknown["verifiedFacts"]["writer"]["item"]["unexpected"] = serde_json::json!(true);
        assert!(
            FormChoiceParametersWriterEvidence::parse(&serde_json::to_string(&unknown).unwrap())
                .is_err()
        );
        let mut missing = raw.clone();
        missing["missingKeys"] = serde_json::json!(["form.choiceParameters.qname"]);
        assert!(
            FormChoiceParametersWriterEvidence::parse(&serde_json::to_string(&missing).unwrap())
                .is_err()
        );
        let mut wrong_successor = raw;
        wrong_successor["verifiedFacts"]["ownerOrder"]["successorQName"] =
            serde_json::json!("{http://v8.1c.ru/8.3/xcf/logform}Wrong");
        assert!(
            FormChoiceParametersWriterEvidence::parse(
                &serde_json::to_string(&wrong_successor).unwrap()
            )
            .is_err()
        );
        for pointer in [
            "/verifiedFacts/liveSlot27/fixtureSha256",
            "/verifiedFacts/liveSlot27/rawSourceSha256",
            "/verifiedFacts/liveSlot27/nativeSourceSha256",
        ] {
            let mut wrong_hash: serde_json::Value =
                serde_json::from_str(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON).unwrap();
            *wrong_hash.pointer_mut(pointer).unwrap() = serde_json::json!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            );
            assert!(
                FormChoiceParametersWriterEvidence::parse(
                    &serde_json::to_string(&wrong_hash).unwrap()
                )
                .is_err(),
                "evidence mutation {pointer} must fail closed"
            );
        }

        let mut wrong_policy = corpus;
        let rule = wrong_policy
            .rules
            .iter_mut()
            .find(|rule| rule.feature == "choiceParameters")
            .unwrap();
        let Some(WriterPolicy::FormChoiceParameters {
            owner_successor_qname,
            ..
        }) = rule.policy.as_mut()
        else {
            panic!("dedicated choice-parameters policy");
        };
        *owner_successor_qname = "{http://v8.1c.ru/8.3/xcf/logform}Wrong".to_owned();
        assert!(
            bind_form_choice_parameters_writer_evidence(
                BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
                &wrong_policy,
            )
            .is_err()
        );

        let mut missing_policy = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        missing_policy
            .rules
            .iter_mut()
            .find(|rule| rule.feature == "choiceParameters")
            .unwrap()
            .policy = None;
        assert!(
            bind_form_choice_parameters_writer_evidence(
                BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
                &missing_policy,
            )
            .is_err()
        );

        let mut unknown_policy_field: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).unwrap();
        let policy = unknown_policy_field["rules"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|rule| rule["feature"] == "choiceParameters")
            .unwrap()
            .get_mut("policy")
            .unwrap();
        policy["unexpected"] = serde_json::json!(true);
        assert!(
            WriterRuleCorpus::parse(&serde_json::to_string(&unknown_policy_field).unwrap())
                .is_err()
        );

        let original_rules: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).unwrap();
        let rule_index = original_rules["rules"]
            .as_array()
            .unwrap()
            .iter()
            .position(|rule| rule["feature"] == "choiceParameters")
            .unwrap();
        let policy_mutations = [
            (
                "/ownerQName",
                serde_json::json!("{urn:wrong}ChoiceParameters"),
            ),
            (
                "/ownerPredecessorQName",
                serde_json::json!("{urn:wrong}ChoiceParameterLinks"),
            ),
            (
                "/ownerSuccessorQName",
                serde_json::json!("{urn:wrong}AvailableTypes"),
            ),
            (
                "/emptyCollection",
                serde_json::json!("write-wrapper-when-write-default"),
            ),
            ("/item/itemQName", serde_json::json!("{urn:wrong}item")),
            (
                "/item/nameAttributeQName",
                serde_json::json!("{urn:wrong}name"),
            ),
            ("/item/valueQName", serde_json::json!("{urn:wrong}value")),
            ("/item/valueXsiType", serde_json::json!("Wrong")),
            (
                "/item/valueOrder",
                serde_json::json!(["value", "presentation"]),
            ),
            (
                "/item/presentationQName",
                serde_json::json!("{urn:wrong}Presentation"),
            ),
            (
                "/item/scalarValueQName",
                serde_json::json!("{urn:wrong}Value"),
            ),
            ("/item/booleanXsiType", serde_json::json!("xs:string")),
            ("/item/designTimeRefXsiType", serde_json::json!("xs:string")),
            ("/fixedArray/xsiType", serde_json::json!("v8:Array")),
            (
                "/fixedArray/itemQName",
                serde_json::json!("{urn:wrong}Value"),
            ),
            ("/fixedArray/itemXsiType", serde_json::json!("Wrong")),
            (
                "/fixedArray/itemOrder",
                serde_json::json!(["value", "presentation"]),
            ),
        ];
        for (relative_pointer, replacement) in policy_mutations {
            let mut mutated = original_rules.clone();
            let pointer = format!("/rules/{rule_index}/policy{relative_pointer}");
            *mutated
                .pointer_mut(&pointer)
                .unwrap_or_else(|| panic!("policy mutation pointer {pointer}")) = replacement;
            assert!(
                WriterRuleCorpus::parse(&serde_json::to_string(&mutated).unwrap()).is_err(),
                "policy mutation {relative_pointer} must fail closed"
            );
        }
    }

    #[test]
    fn form_choice_list_empty_string_policy_matches_exact_research_evidence() {
        let compact = FormChoiceListStringWriterProof::parse(
            BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON,
        )
        .expect("strict compact Form choice-list string proof");
        let report = bundled_form_choice_list_string_writer_evidence()
            .expect("strict Form choice-list string evidence");
        assert_eq!(
            compact.full_evidence_sha256,
            format!(
                "{:x}",
                Sha256::digest(BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON.as_bytes())
            )
        );
        assert_eq!(
            compact.full_evidence_sha256,
            FORM_CHOICE_LIST_STRING_WRITER_FULL_EVIDENCE_SHA256
        );
        assert_eq!(compact.source.product, report.source.product);
        assert_eq!(compact.source.release, report.source.release);
        assert_eq!(compact.rule.id, "form.choice-list.design-time-value");
        assert_eq!(compact.rule.model_type, "FormChoiceList");
        assert_eq!(compact.rule.feature, "values");
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.source.product, "1C:EDT");
        assert_eq!(report.source.release, "2025.2.3+30");
        assert_eq!(
            report.source.root_identity.leaf,
            "1c-edt-2025.2.3+30-x86_64"
        );
        assert_eq!(report.source.root_identity.product_version, "2025.2.3");
        assert_eq!(report.source.root_identity.build_id, "2025.2.3.30");
        assert_eq!(
            report.source.root_identity.product,
            "com._1c.g5.v8.dt.product.application.rcp"
        );
        assert_eq!(
            report.source.root_identity.application,
            "org.eclipse.ui.ide.workbench"
        );
        assert_eq!(report.source.validated_bundles.len(), 2);
        assert_eq!(
            (
                report.source.validated_bundles[0].symbolic_name.as_str(),
                report.source.validated_bundles[0].version.as_str(),
            ),
            ("com._1c.g5.v8.dt.form.export.xml", "10.1.0.v202602241426",)
        );
        assert_eq!(
            (
                report.source.validated_bundles[1].symbolic_name.as_str(),
                report.source.validated_bundles[1].version.as_str(),
            ),
            ("com._1c.g5.v8.dt.export.xml", "13.0.100.v202602241426",)
        );
        assert!(!report.source.derivation.trim().is_empty());
        assert!(!report.source.input_contract.trim().is_empty());
        assert!(!report.source.invocation.trim().is_empty());
        assert!(report.missing_keys.is_empty());
        assert_eq!(report.verified_facts.len(), 1);

        let fact = &report.verified_facts[0];
        assert_eq!(
            fact.key,
            "form.FormChoiceListDesTimeValue.value.empty-string"
        );
        assert_eq!(fact.value.model_value_type, "mcore:StringValue");
        assert_eq!(fact.value.empty_predicate, "Strings.isNullOrEmpty");
        assert_eq!(fact.value.element, "feature QName");
        assert_eq!(fact.value.xsi_type, "xs:string");
        assert_eq!(
            fact.value.delegate_chain,
            [
                "FormChoiceListDesTimeValueWriter.write",
                "FormSmartFeatureWriter.write",
                "FormValueWriter.writeValue",
                "ValueWriter.writeValue",
                "ExportXmlStreamWriter.writeEmptyElement",
                "XMLStreamWriter.writeEmptyElement",
            ]
        );
        assert_eq!(fact.value.branch.string_type_offset, 144);
        assert_eq!(fact.value.branch.empty_predicate_offset, 163);
        assert_eq!(fact.value.branch.non_empty_target_offset, 187);
        assert_eq!(fact.value.branch.empty_element_offset, 171);
        assert_eq!(fact.value.branch.xsi_type_attribute_offset, 181);
        assert_eq!(fact.value.method_envelopes.len(), 6);
        let feature_descriptor = "(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;)V";
        let value_descriptor = "(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Ljava/lang/Object;Ljavax/xml/namespace/QName;ZLorg/eclipse/emf/ecore/EStructuralFeature;Lcom/_1c/g5/v8/dt/export/xml/IExportContext;)V";
        let expected_envelopes = [
            (
                "FormChoiceListDesTimeValueWriter.write",
                feature_descriptor,
                108,
                253,
                8,
            ),
            (
                "FormSmartFeatureWriter.write",
                feature_descriptor,
                90,
                209,
                11,
            ),
            (
                "FormSmartFeatureWriter.fillSpecialClassifierWriters",
                "()Lcom/google/common/collect/ImmutableMap;",
                165,
                360,
                0,
            ),
            ("FormValueWriter.writeValue", value_descriptor, 125, 314, 14),
            ("ValueWriter.writeValue", value_descriptor, 567, 1345, 64),
            (
                "ExportXmlStreamWriter.writeEmptyElement",
                "(Ljavax/xml/namespace/QName;)V",
                21,
                42,
                1,
            ),
        ];
        for (envelope, (method, descriptor, count, last_offset, branch_count)) in
            fact.value.method_envelopes.iter().zip(expected_envelopes)
        {
            assert_eq!(envelope.method, method);
            assert_eq!(envelope.descriptor, descriptor);
            assert_eq!(envelope.instruction_count, count);
            assert_eq!(envelope.first_offset, 0);
            assert_eq!(envelope.last_offset, last_offset);
            assert_eq!(envelope.branch_graph.len(), branch_count);
        }
        assert_eq!(
            fact.evidence.kind,
            "javap-v-exact-method-control-flow-constant-pool"
        );
        assert_eq!(fact.evidence.status, "verified");
        assert_eq!(fact.evidence.sources.len(), 7);
        assert!(!fact.evidence.note.trim().is_empty());
        assert!(fact.evidence.sources.iter().all(|source| {
            source == "tools/report-edt-form-choice-list-string-writer-evidence.ps1"
                || source.starts_with("edt-derived://2025.2.3+30/")
        }));
        assert_eq!(compact.emission, fact.value.emission);

        let corpus = bundled_writer_rules().unwrap();
        let policy = corpus
            .exact_rule(WriterRuleKey {
                source_release: &report.source.release,
                model_type: "FormChoiceList",
                feature: "values",
            })
            .unwrap()
            .policy
            .as_ref()
            .expect("verified choice-list writer policy");
        let WriterPolicy::FormChoiceList {
            empty_string_value, ..
        } = policy
        else {
            panic!("unexpected choice-list writer policy kind");
        };
        assert_eq!(*empty_string_value, fact.value.emission);

        for marker in [
            b"ibcmd.exe".as_slice(),
            b"1cv8.exe",
            b"1cv8c.exe",
            b"\\1cv8\\",
            b"/1cv8/",
            b".jar",
            b"org.eclipse",
            b"JNI_CreateJavaVM",
            b"JNIEnv",
            b"JavaVM",
            b"OSGi",
        ] {
            assert!(
                !BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON
                    .as_bytes()
                    .windows(marker.len())
                    .any(|window| window == marker),
                "compact Form choice-list proof contains forbidden payload marker `{}`",
                String::from_utf8_lossy(marker)
            );
        }

        let raw: serde_json::Value =
            serde_json::from_str(BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON).unwrap();
        let mut extra_field = raw.clone();
        extra_field["unexpected"] = serde_json::json!(true);
        assert!(
            FormChoiceListStringWriterEvidence::parse(
                &serde_json::to_string(&extra_field).unwrap()
            )
            .is_err()
        );

        let mut missing_emission = raw.clone();
        missing_emission["verifiedFacts"][0]["value"]
            .as_object_mut()
            .unwrap()
            .remove("emission");
        assert!(
            FormChoiceListStringWriterEvidence::parse(
                &serde_json::to_string(&missing_emission).unwrap()
            )
            .is_err()
        );

        let mut other_emission = raw.clone();
        other_emission["verifiedFacts"][0]["value"]["emission"] = serde_json::json!("paired");
        assert!(
            FormChoiceListStringWriterEvidence::parse(
                &serde_json::to_string(&other_emission).unwrap()
            )
            .is_err()
        );

        let mut extra_fact = raw;
        let duplicate = extra_fact["verifiedFacts"][0].clone();
        extra_fact["verifiedFacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(
            FormChoiceListStringWriterEvidence::parse(&serde_json::to_string(&extra_fact).unwrap())
                .is_err()
        );
    }

    #[test]
    fn form_choice_list_empty_string_compact_proof_is_production_bound_and_fails_closed() {
        let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        bind_form_choice_list_string_writer_proof(
            BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON,
            &corpus,
        )
        .unwrap();

        let original: serde_json::Value =
            serde_json::from_str(BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON).unwrap();
        for (pointer, replacement) in [
            ("/schemaVersion", serde_json::json!(2)),
            ("/source/product", serde_json::json!("wrong")),
            ("/source/release", serde_json::json!("2025.2.3+31")),
            ("/source/derivation", serde_json::json!("wrong")),
            ("/rule/id", serde_json::json!("form.choice-list.wrong")),
            ("/rule/modelType", serde_json::json!("Wrong")),
            ("/rule/feature", serde_json::json!("wrong")),
            ("/emission", serde_json::json!("paired")),
            ("/fullEvidenceSha256", serde_json::json!("00")),
            (
                "/provenanceIds/0",
                serde_json::json!("choice-list-empty-string/wrong"),
            ),
        ] {
            let mut corrupted = original.clone();
            *corrupted.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                FormChoiceListStringWriterProof::parse(&serde_json::to_string(&corrupted).unwrap())
                    .is_err(),
                "compact proof mutation {pointer} must fail closed"
            );
        }
        let mut extra = original;
        extra["unexpected"] = serde_json::json!(true);
        assert!(
            FormChoiceListStringWriterProof::parse(&serde_json::to_string(&extra).unwrap())
                .is_err()
        );

        let corruptions: [fn(&mut WriterRule); 3] = [
            |rule: &mut WriterRule| rule.id = "form.choice-list.wrong".to_owned(),
            |rule: &mut WriterRule| rule.evidence.status = "pending".to_owned(),
            |rule: &mut WriterRule| rule.policy = None,
        ];
        for corrupt_rule in corruptions {
            let mut corrupted_corpus = corpus.clone();
            let rule = corrupted_corpus
                .rules
                .iter_mut()
                .find(|rule| rule.model_type == "FormChoiceList" && rule.feature == "values")
                .unwrap();
            corrupt_rule(rule);
            assert!(matches!(
                bind_form_choice_list_string_writer_proof(
                    BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_PROOF_JSON,
                    &corrupted_corpus,
                ),
                Err(SchemaError::InvalidFormChoiceListStringWriterEvidence(_))
            ));
        }
    }

    #[test]
    fn form_choice_list_empty_string_full_evidence_is_research_bound_and_fails_closed() {
        let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        bind_form_choice_list_string_writer_evidence(
            BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON,
            &corpus,
        )
        .unwrap();

        let mut corrupted: serde_json::Value =
            serde_json::from_str(BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON).unwrap();
        corrupted["verifiedFacts"][0]["value"]["xsiType"] = serde_json::json!("xs:anyType");
        assert!(matches!(
            FormChoiceListStringWriterEvidence::parse(&serde_json::to_string(&corrupted).unwrap()),
            Err(SchemaError::InvalidFormChoiceListStringWriterEvidence(_))
        ));
        assert!(matches!(
            FormChoiceListStringWriterEvidence::parse(
                &" ".repeat(MAX_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_BYTES + 1)
            ),
            Err(SchemaError::InvalidFormChoiceListStringWriterEvidence(_))
        ));

        let corruptions: [fn(&mut WriterRule); 5] = [
            |rule: &mut WriterRule| rule.id = "form.choice-list.wrong".to_owned(),
            |rule: &mut WriterRule| rule.source_class = "wrong.Source".to_owned(),
            |rule: &mut WriterRule| rule.delegate = Some("wrong.Delegate".to_owned()),
            |rule: &mut WriterRule| rule.evidence.status = "pending".to_owned(),
            |rule: &mut WriterRule| rule.policy = None,
        ];
        for corrupt_rule in corruptions {
            let mut corrupted_corpus = corpus.clone();
            let rule = corrupted_corpus
                .rules
                .iter_mut()
                .find(|rule| rule.model_type == "FormChoiceList" && rule.feature == "values")
                .unwrap();
            corrupt_rule(rule);
            assert!(matches!(
                bind_form_choice_list_string_writer_evidence(
                    BUNDLED_FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON,
                    &corrupted_corpus,
                ),
                Err(SchemaError::InvalidFormChoiceListStringWriterEvidence(_))
            ));
        }
    }

    #[test]
    fn exact_writer_rule_lookup_fails_closed() {
        let corpus = bundled_writer_rules().unwrap();
        assert!(matches!(
            corpus.exact_rule(WriterRuleKey {
                source_release: "2026.1",
                model_type: "FormChoiceList",
                feature: "values",
            }),
            Err(WriterRuleLookupError::SourceReleaseMismatch { .. })
        ));
        assert!(matches!(
            corpus.exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "unknown",
            }),
            Err(WriterRuleLookupError::Missing { .. })
        ));

        let mut pending = corpus.clone();
        pending
            .rules
            .iter_mut()
            .find(|rule| rule.id == "form.choice-list.design-time-value")
            .expect("fixture rule")
            .evidence
            .status = "pending".to_owned();
        assert!(matches!(
            pending.exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "values",
            }),
            Err(WriterRuleLookupError::Unverified { .. })
        ));

        let raw: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).expect("bundled writer rules JSON");
        let choice_index = raw["rules"]
            .as_array()
            .and_then(|rules| {
                rules
                    .iter()
                    .position(|rule| rule["id"] == "form.choice-list.design-time-value")
            })
            .expect("choice-list writer rule");

        let mut missing_empty_string = raw.clone();
        missing_empty_string["rules"][choice_index]["policy"]
            .as_object_mut()
            .expect("choice-list writer policy")
            .remove("emptyStringValue");
        assert!(
            WriterRuleCorpus::parse(
                &serde_json::to_string(&missing_empty_string).expect("mutated JSON")
            )
            .is_err()
        );

        let mut unsupported_empty_string = raw;
        unsupported_empty_string["rules"][choice_index]["policy"]["emptyStringValue"] =
            serde_json::json!("paired");
        assert!(
            WriterRuleCorpus::parse(
                &serde_json::to_string(&unsupported_empty_string).expect("mutated JSON")
            )
            .is_err()
        );
    }

    #[test]
    fn bundled_package_features_include_real_form_model_fields() {
        let corpus = bundled_package_features().unwrap();
        assert!(corpus.summary.packages > 50);
        assert!(corpus.summary.classifiers > 1_000);
        assert!(corpus.summary.features > 5_000);
        let package = corpus
            .package("com._1c.g5.v8.dt.form.model.FormPackage")
            .unwrap();
        let form = package
            .classifiers
            .iter()
            .find(|classifier| classifier.token == "FORM")
            .unwrap();
        assert_eq!(form.feature_count, Some(65));
        assert!(
            form.features
                .iter()
                .any(|feature| feature.token == "SHOW_TITLE851" && feature.id == 47)
        );
    }

    #[test]
    fn bundled_feature_semantics_cover_all_resources_and_representative_families() {
        let corpus = bundled_feature_semantics().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.packages, 63);
        assert_eq!(corpus.summary.classifiers, 1_820);
        assert_eq!(corpus.summary.features, 4_966);

        let key = |classifier: &str, feature: &str| FeatureSemanticKey {
            namespace_uri: "http://g5.1c.ru/v8/dt/form".to_owned(),
            classifier: classifier.to_owned(),
            feature: feature.to_owned(),
        };

        let attributes = corpus.feature(&key("Form", "attributes")).unwrap();
        assert_eq!(attributes.kind, FeatureKind::Containment);
        assert_eq!(attributes.model_type, "FormAttribute");
        assert_eq!((attributes.lower_bound, attributes.upper_bound), (0, None));

        let base_form = corpus.feature(&key("Form", "baseForm")).unwrap();
        assert_eq!(base_form.kind, FeatureKind::Reference);
        assert_eq!((base_form.lower_bound, base_form.upper_bound), (0, Some(1)));
        assert_eq!(base_form.qualifiers, vec![XcoreFeatureQualifier::Transient]);

        let segments = corpus
            .feature(&key("AbstractDataPath", "segments"))
            .unwrap();
        assert_eq!(segments.kind, FeatureKind::Attribute);
        assert_eq!(segments.model_type, "String");
        assert_eq!((segments.lower_bound, segments.upper_bound), (1, None));

        let image_scale = corpus
            .feature(&key("ImageFieldExtInfo", "imageScale"))
            .unwrap();
        assert_eq!(image_scale.default_value.as_deref(), Some("100"));
        assert_eq!(image_scale.xml.evidence.status, EvidenceStatus::Pending);

        let dcs_enabled = corpus
            .feature(&FeatureSemanticKey {
                namespace_uri: "http://g5.1c.ru/v8/dt/data-composition-system/settings".to_owned(),
                classifier: "AvailableFieldUseRestriction".to_owned(),
                feature: "enabled".to_owned(),
            })
            .unwrap();
        assert_eq!(dcs_enabled.kind, FeatureKind::Attribute);
        assert_eq!(dcs_enabled.model_type, "boolean");

        let mcore_gap = corpus
            .feature(&FeatureSemanticKey {
                namespace_uri: "http://g5.1c.ru/v8/dt/mcore".to_owned(),
                classifier: "AbstractLine".to_owned(),
                feature: "gap".to_owned(),
            })
            .unwrap();
        assert_eq!(mcore_gap.model_type, "boolean");

        assert!(corpus.packages.iter().any(|package| {
            package.namespace_uri == "http://g5.1c.ru/v8/dt/binary"
                && package
                    .classifiers
                    .iter()
                    .any(|classifier| classifier.name == "BinaryData")
        }));
    }

    #[test]
    fn bundled_canonical_coverage_is_an_exact_full_join() {
        let corpus = bundled_canonical_coverage().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.entries, 4_966);
        assert_eq!(corpus.summary.typed, 28);
        assert_eq!(corpus.summary.opaque_lossless, 0);
        assert_eq!(corpus.summary.unsupported, 4_938);
        assert_eq!(corpus.summary.platform_only, 0);

        let family_count = |family: &str| {
            corpus
                .entries
                .iter()
                .filter(|entry| {
                    serde_json::to_value(entry.family).unwrap() == serde_json::json!(family)
                })
                .count()
        };
        assert_eq!(family_count("metadata"), 0);
        assert_eq!(family_count("forms"), 2_314);
        assert_eq!(family_count("dcs"), 511);
        assert_eq!(family_count("mxl"), 0);
        assert_eq!(family_count("common"), 0);
        assert_eq!(family_count("other"), 2_141);
        assert_eq!(
            corpus
                .family_aggregates
                .iter()
                .map(|aggregate| aggregate.entries)
                .sum::<usize>(),
            4_966
        );
        assert_eq!(
            corpus
                .migration_backlog
                .iter()
                .map(|item| item.features)
                .sum::<usize>(),
            4_938
        );
        assert!(
            corpus
                .migration_backlog
                .iter()
                .all(|item| item.rule == "unsupported/schema.unmapped")
        );
        assert_eq!(
            corpus
                .entries
                .iter()
                .filter(|entry| entry.status == CoverageStatus::Typed)
                .map(|entry| (
                    entry.key.classifier.as_str(),
                    entry.key.feature.as_str(),
                    entry.canonical_field.as_deref().unwrap()
                ))
                .collect::<Vec<_>>(),
            [
                ("DataCompositionConditionalAppearance", "items", "items"),
                (
                    "DataCompositionConditionalAppearance",
                    "userSettingID",
                    "user_setting_id"
                ),
                (
                    "DataCompositionConditionalAppearance",
                    "viewMode",
                    "view_mode"
                ),
                (
                    "DataCompositionConditionalAppearanceItem",
                    "appearance",
                    "appearance"
                ),
                (
                    "DataCompositionConditionalAppearanceItem",
                    "filter",
                    "filter"
                ),
                (
                    "DataCompositionConditionalAppearanceItem",
                    "selection",
                    "selected_field"
                ),
                ("DataCompositionFilter", "items", "items"),
                ("DataCompositionFilter", "userSettingID", "user_setting_id"),
                ("DataCompositionFilter", "viewMode", "view_mode"),
                (
                    "DataCompositionFilterItem",
                    "comparisonType",
                    "comparison_type"
                ),
                ("DataCompositionFilterItem", "left", "field"),
                ("DataCompositionFilterItem", "right", "right"),
                ("DataCompositionOrder", "items", "items"),
                ("DataCompositionOrder", "userSettingID", "user_setting_id"),
                ("DataCompositionOrder", "viewMode", "view_mode"),
                ("DataCompositionOrderItem", "field", "field"),
                ("DataCompositionOrderItem", "orderType", "order_type"),
                ("DataCompositionOrderItem", "use", "use_value"),
                ("DataCompositionSelectedField", "field", "field"),
                ("DataCompositionSelectedFields", "items", "items"),
                (
                    "DataCompositionSettings",
                    "conditionalAppearance",
                    "conditional_appearance"
                ),
                ("DataCompositionSettings", "filter", "filter"),
                (
                    "DataCompositionSettings",
                    "itemsUserSettingID",
                    "items_user_setting_id"
                ),
                (
                    "DataCompositionSettings",
                    "itemsViewMode",
                    "items_view_mode"
                ),
                ("DataCompositionSettings", "order", "order"),
                ("DataCompositionSettings", "selection", "selection"),
                ("DynamicListExtInfo", "listSettings", "settings"),
                ("Form", "conditionalAppearance", "conditional_appearance"),
            ]
        );
        let list_settings = corpus
            .entries
            .iter()
            .find(|entry| {
                entry.key.namespace_uri == "http://g5.1c.ru/v8/dt/form"
                    && entry.key.classifier == "DynamicListExtInfo"
                    && entry.key.feature == "listSettings"
            })
            .unwrap();
        assert_eq!(list_settings.status, CoverageStatus::Typed);
        assert_eq!(
            list_settings.canonical_type.as_deref(),
            Some("DcsSettingsEnvelope")
        );
        assert_eq!(list_settings.canonical_field.as_deref(), Some("settings"));
        assert_eq!(
            list_settings.evidence.sources,
            [
                "crates/ibcmd-core/src/dcs.rs",
                "crates/ibcmd-schema/data/edt-2025.2.3-dcs-writer-evidence.json",
                "crates/ibcmd-schema/data/edt-2025.2.3-writer-rules.json",
                "model/Form.xcore",
            ]
        );
        assert!(corpus.entries.iter().all(|entry| {
            entry.status != CoverageStatus::Unsupported
                || entry.diagnostic_code.as_deref() == Some("schema.unmapped")
        }));
    }

    #[test]
    fn bundled_metadata_order_is_verified_and_queryable() {
        let corpus = bundled_metadata_order().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.verified_records, 60);
        assert_eq!(corpus.summary.rejected_records, 0);
        let catalog = corpus
            .order(
                "ProducedTypesOrderProvider",
                "CATALOG_TYPES",
                MetadataOrderSection::ProducedTypes,
                MetadataOrderVersionPredicate::Always,
            )
            .unwrap();
        assert_eq!(
            catalog.fallback,
            MetadataOrderFallback::AllReferencesWhenUnmapped
        );
        assert_eq!(
            catalog.ordered_features,
            [
                "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
                "BASIC_DB_OBJECT_TYPES__REF_TYPE",
                "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
                "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
                "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
            ]
        );
        assert!(
            corpus
                .records
                .iter()
                .all(|record| record.evidence.status == EvidenceStatus::Verified)
        );

        let configuration = corpus
            .order(
                "MetadataObjectFeatureOrderProvider",
                "CONFIGURATION",
                MetadataOrderSection::Properties,
                MetadataOrderVersionPredicate::GreaterThanV8_3_14,
            )
            .unwrap();
        assert_eq!(
            configuration.fallback,
            MetadataOrderFallback::DefaultPropertyFilterWhenUnmapped
        );
        assert_eq!(
            configuration.order_operations[0].operation,
            MetadataOrderOperationKind::Cursor
        );
        assert_eq!(
            configuration.order_operations[0].feature,
            "MD_OBJECT__COMMENT"
        );
        assert!(
            corpus
                .order(
                    "MetadataObjectFeatureOrderProvider",
                    "CONFIGURATION",
                    MetadataOrderSection::InternalInfo,
                    MetadataOrderVersionPredicate::Always,
                )
                .is_some()
        );
        assert!(
            corpus
                .order(
                    "MetadataObjectFeatureOrderProvider",
                    "DOCUMENT",
                    MetadataOrderSection::Properties,
                    MetadataOrderVersionPredicate::Always,
                )
                .is_some()
        );
    }

    #[test]
    fn metadata_order_rejects_duplicate_classifier_section_version() {
        let mut corpus = bundled_metadata_order().unwrap();
        corpus.records.push(corpus.records[0].clone());
        corpus.summary.verified_records += 1;
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::DuplicateValue {
                field: "metadata order provider/classifier/section/version",
                ..
            })
        ));
    }

    #[test]
    fn validation_rejects_machine_specific_paths() {
        let mut inventory = bundled_model_inventory().unwrap();
        inventory.bundles[0].model_types[0] = r"C:\Program Files\1C\secret".to_owned();
        assert!(matches!(
            inventory.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
    }

    fn feature_semantics_fixture() -> FeatureSemanticsCorpus {
        FeatureSemanticsCorpus {
            schema_version: 1,
            source: CorpusSource {
                product: "1C:EDT".to_owned(),
                release: "2025.2.3+30".to_owned(),
                derivation: "local Xcore inventory".to_owned(),
            },
            summary: FeatureSemanticsSummary {
                packages: 1,
                classifiers: 1,
                features: 1,
            },
            packages: vec![FeatureSemanticsPackage {
                bundle: "com._1c.g5.v8.dt.form.model".to_owned(),
                resource: "model/form.xcore".to_owned(),
                package_name: "form".to_owned(),
                namespace_uri: "http://v8.1c.ru/8.3/xcf/logform".to_owned(),
                classifiers: vec![FeatureSemanticsClassifier {
                    name: "Form".to_owned(),
                    kind: FeatureClassifierKind::Class,
                    features: vec![FeatureSemantics {
                        name: "baseForm".to_owned(),
                        kind: FeatureKind::Reference,
                        model_type: "Form".to_owned(),
                        lower_bound: 0,
                        upper_bound: Some(1),
                        default_value: None,
                        qualifiers: vec![XcoreFeatureQualifier::Transient],
                        model_evidence: FeatureEvidence {
                            status: EvidenceStatus::Verified,
                            kind: "xcore".to_owned(),
                            sources: vec!["model/form.xcore".to_owned()],
                            note: None,
                        },
                        xml: XmlFeatureBehavior {
                            qname: Some("form:baseForm".to_owned()),
                            order: Some(12),
                            emit_default: Some(false),
                            version_gate: EvidenceValue::Known {
                                value: Some("8.3".to_owned()),
                            },
                            delegate: EvidenceValue::Known {
                                value: Some("FormWriter".to_owned()),
                            },
                            evidence: FeatureEvidence {
                                status: EvidenceStatus::Verified,
                                kind: "writer-inspection".to_owned(),
                                sources: vec!["FormWriter".to_owned()],
                                note: None,
                            },
                        },
                    }],
                }],
            }],
        }
    }

    #[test]
    fn feature_semantics_reject_duplicate_semantic_keys() {
        let mut corpus = feature_semantics_fixture();
        let duplicate = corpus.packages[0].classifiers[0].features[0].clone();
        corpus.packages[0].classifiers[0].features.push(duplicate);
        corpus.summary.features = 2;
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::DuplicateValue {
                field: "feature semantic key",
                ..
            })
        ));
    }

    #[test]
    fn feature_semantics_reject_invalid_cardinality() {
        let mut corpus = feature_semantics_fixture();
        let feature = &mut corpus.packages[0].classifiers[0].features[0];
        feature.lower_bound = 2;
        feature.upper_bound = Some(1);
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::InvalidCardinality { lower: 2, upper: 1 })
        ));
    }

    #[test]
    fn feature_semantics_reject_incomplete_verified_xml_behavior() {
        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].classifiers[0].features[0]
            .xml
            .emit_default = None;
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::IncompleteVerifiedXmlBehavior {
                field: "default emission",
                ..
            })
        ));
    }

    #[test]
    fn feature_semantics_distinguish_verified_absence_from_pending_optional_xml_facts() {
        let mut corpus = feature_semantics_fixture();
        {
            let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
            xml.version_gate = EvidenceValue::Known { value: None };
            xml.delegate = EvidenceValue::Known { value: None };
        }
        assert!(corpus.validate().is_ok());

        {
            let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
            xml.version_gate = EvidenceValue::Pending;
            xml.delegate = EvidenceValue::Pending;
        }
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::IncompleteVerifiedXmlBehavior {
                field: "version gate",
                ..
            })
        ));
        corpus.packages[0].classifiers[0].features[0]
            .xml
            .evidence
            .status = EvidenceStatus::Pending;
        assert!(corpus.validate().is_ok());
    }

    #[test]
    fn feature_semantics_allow_unknown_pending_xml_behavior() {
        let mut corpus = feature_semantics_fixture();
        let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
        xml.qname = None;
        xml.order = None;
        xml.emit_default = None;
        xml.version_gate = EvidenceValue::Pending;
        xml.delegate = EvidenceValue::Pending;
        xml.evidence.status = EvidenceStatus::Pending;
        assert!(corpus.validate().is_ok());
    }

    #[test]
    fn feature_semantics_key_uses_namespace_uri_not_package_name() {
        let mut corpus = feature_semantics_fixture();
        let mut second = corpus.packages[0].clone();
        second.namespace_uri = "http://v8.1c.ru/8.3/xcf/other-form".to_owned();
        second.resource = "model/other-form.xcore".to_owned();
        corpus.packages.push(second);
        corpus.summary.packages = 2;
        corpus.summary.classifiers = 2;
        corpus.summary.features = 2;
        assert!(corpus.validate().is_ok());
        assert!(
            corpus
                .feature(&FeatureSemanticKey {
                    namespace_uri: "http://v8.1c.ru/8.3/xcf/other-form".to_owned(),
                    classifier: "Form".to_owned(),
                    feature: "baseForm".to_owned(),
                })
                .is_some()
        );
    }

    #[test]
    fn feature_semantics_reject_unknown_classifier_kind_and_unportable_sources() {
        let corpus = feature_semantics_fixture();
        let mut json = serde_json::to_value(&corpus).unwrap();
        json["packages"][0]["classifiers"][0]["kind"] = serde_json::json!("unknown");
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&json).unwrap()),
            Err(SchemaError::InvalidJson(_))
        ));

        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].resource = r"C:/EDT/model/form.xcore".to_owned();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
        corpus.packages[0].resource = "file:///C:/EDT/model/form.xcore".to_owned();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
        corpus.packages[0].resource = "model/form.xcore".to_owned();
        corpus.packages[0].classifiers[0].features[0]
            .model_evidence
            .sources = vec![r"\\server\share\form.xcore".to_owned()];
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
    }

    #[test]
    fn verified_feature_evidence_requires_a_source() {
        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].classifiers[0].features[0]
            .model_evidence
            .sources
            .clear();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::EmptyField("verified feature evidence sources"))
        ));
    }

    #[test]
    fn feature_semantics_use_importer_camel_case_and_preserve_base_form_qualifier() {
        let corpus = feature_semantics_fixture();
        let json = serde_json::to_value(&corpus).unwrap();
        let feature = &json["packages"][0]["classifiers"][0]["features"][0];
        assert_eq!(feature["name"], "baseForm");
        assert_eq!(feature["qualifiers"], serde_json::json!(["transient"]));
        assert_eq!(feature["xml"]["qname"], "form:baseForm");
        assert_eq!(feature["xml"]["emitDefault"], false);
        assert_eq!(
            feature["xml"]["versionGate"],
            serde_json::json!({"status": "known", "value": "8.3"})
        );
        assert!(feature["xml"].get("qName").is_none());
    }

    #[test]
    fn bundled_dcs_writer_evidence_exposes_verified_envelopes_and_typed_tail() {
        let corpus = bundled_dcs_writer_evidence().unwrap();
        let feature_semantics = bundled_dcs_list_settings_feature_semantics().unwrap();
        let policy = corpus
            .form_list_settings_tail_policy(&feature_semantics)
            .unwrap();
        assert_eq!(
            policy.namespace_uri(),
            "http://v8.1c.ru/8.1/data-composition-system/settings"
        );
        assert_eq!(
            policy.tail_order(),
            &[
                DcsListSettingsTailField::ItemsViewMode,
                DcsListSettingsTailField::ItemsUserSettingId,
            ]
        );
        assert_eq!(policy.items_view_mode_default(), "QuickAccess");
        assert_eq!(policy.items_user_setting_id_default(), "");
        let envelopes = corpus
            .settings_serialization_policy(&feature_semantics)
            .unwrap();
        assert_eq!(
            envelopes.standalone_document_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}Settings"
        );
        assert_eq!(
            envelopes.form_list_settings_qname(),
            "{http://v8.1c.ru/8.3/xcf/logform}ListSettings"
        );
        assert!(envelopes.type_id_is_absent());
        assert_eq!(corpus.missing_keys.len(), 1);
        assert_eq!(
            corpus.missing_keys[0].status,
            "unsupported-no-lossless-placement"
        );
    }

    #[test]
    fn bundled_platform_dcs_selection_policy_is_exact_and_fails_on_drift() {
        let policy = bundled_dcs_selection_policy().unwrap();
        assert_eq!(
            policy.selection_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}selection"
        );
        assert_eq!(
            policy.field_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}SelectedItemField"
        );
        assert_eq!(
            policy.auto_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}SelectedItemAuto"
        );
        assert!(policy.precedes_order_and_structure_items());
        assert!(policy.empty_selection_is_unsupported());

        let mut drift =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_SELECTION_EVIDENCE_JSON).unwrap();
        drift["policy"]["fieldTypeQname"] = serde_json::json!("{urn:forged}Field");
        assert!(matches!(
            DcsSelectionEvidenceCorpus::parse(&serde_json::to_string(&drift).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("field type QName drifted")
        ));
    }

    #[test]
    fn bundled_platform_dcs_settings_source_owned_policy_binds_exact_standalone_cohort() {
        let policy = bundled_dcs_settings_source_owned_policy().unwrap();
        assert_eq!(
            policy.data_parameters_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}dataParameters"
        );
        assert_eq!(
            policy.data_parameter_item_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/core}item"
        );
        assert_eq!(
            policy.settings_parameter_value_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}SettingsParameterValue"
        );
        assert_eq!(policy.parameter_name(), "Caption");
        assert_eq!(
            policy.value_type_qname(),
            "{http://www.w3.org/2001/XMLSchema}string"
        );
        assert_eq!(policy.value(), "Opaque probe");
        assert_eq!(
            policy.structure_item_group_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}StructureItemGroup"
        );
        assert_eq!(
            policy.order_item_auto_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}OrderItemAuto"
        );
        assert_eq!(
            policy.selected_item_auto_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}SelectedItemAuto"
        );
        assert_eq!(policy.max_data_parameter_items(), 1);
        assert_eq!(policy.max_structure_items(), 1);
        assert!(policy.is_standalone_only());
        assert!(policy.data_parameters_follows_selection_and_precedes_order());
        assert!(policy.structure_item_is_terminal());
        assert!(policy.unknown_children_are_unsupported());
        assert!(policy.generic_opaque_emission_is_forbidden());
    }

    #[test]
    fn dcs_settings_source_owned_evidence_fails_closed_on_shape_hash_and_extra_field_drift() {
        let raw = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_SETTINGS_SOURCE_OWNED_EVIDENCE_JSON,
        )
        .unwrap();

        let mut shape_drift = raw.clone();
        shape_drift["policy"]["structureItemChildOrder"] =
            serde_json::json!(["selection", "order"]);
        assert!(matches!(
            DcsSettingsSourceOwnedEvidenceCorpus::parse(
                &serde_json::to_string(&shape_drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("structure item child order drifted")
        ));

        let mut hash_drift = raw.clone();
        hash_drift["sources"]["dataParameters"]["unpackedBodySha256"] =
            serde_json::json!("0".repeat(64));
        assert!(matches!(
            DcsSettingsSourceOwnedEvidenceCorpus::parse(
                &serde_json::to_string(&hash_drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("dataParameters unpacked body SHA-256 drifted")
        ));

        let mut extra_field = raw;
        extra_field["policy"]["opaqueFallback"] = serde_json::json!(true);
        assert!(matches!(
            DcsSettingsSourceOwnedEvidenceCorpus::parse(
                &serde_json::to_string(&extra_field).unwrap()
            ),
            Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn bundled_platform_dcs_schema_template_envelope_policy_binds_positional_documents() {
        let policy = bundled_dcs_schema_template_envelope_policy().unwrap();
        assert_eq!(
            policy.source_root_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/schema}DataCompositionSchema"
        );
        assert_eq!(
            policy.source_settings_variant_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/schema}settingsVariant"
        );
        assert_eq!(
            policy.source_inline_settings_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}settings"
        );
        assert_eq!(policy.native_schema_file_qname(), "{}SchemaFile");
        assert_eq!(
            policy.native_schema_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/schema}dataCompositionSchema"
        );
        assert_eq!(
            policy.native_settings_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}Settings"
        );
        assert_eq!(policy.header_marker(), 0);
        assert_eq!(policy.settings_count_offset_bytes(), 4);
        assert_eq!(policy.stored_lengths_offset_bytes(), 8);
        assert_eq!(policy.stored_length_width_bytes(), 8);
        assert_eq!(policy.min_attested_settings_variants(), 1);
        assert_eq!(policy.max_attested_settings_variants(), 2);
        assert_eq!(policy.stored_length_count(1), Some(2));
        assert_eq!(policy.stored_length_count(2), Some(3));
        assert_eq!(policy.header_size_bytes(1), Some(24));
        assert_eq!(policy.header_size_bytes(2), Some(32));
        assert_eq!(
            policy.document_role(2, 0),
            Some(DcsSchemaTemplateEnvelopeDocumentRole::PrimarySchemaFile)
        );
        assert_eq!(
            policy.document_role(2, 1),
            Some(DcsSchemaTemplateEnvelopeDocumentRole::Settings)
        );
        assert_eq!(
            policy.document_role(2, 2),
            Some(DcsSchemaTemplateEnvelopeDocumentRole::Settings)
        );
        assert_eq!(
            policy.document_role(2, 3),
            Some(DcsSchemaTemplateEnvelopeDocumentRole::TerminalSchemaFile)
        );
        assert_eq!(policy.document_role(0, 0), None);
        assert_eq!(policy.document_role(3, 0), None);
        assert_eq!(policy.document_role(2, 4), None);
        assert!(policy.settings_count_is_little_endian_u32());
        assert!(policy.stored_lengths_are_little_endian_u64());
        assert!(policy.stored_lengths_cover_primary_and_each_settings());
        assert!(policy.terminal_schema_file_consumes_remaining_bytes());
        assert!(policy.documents_require_utf8_bom());
        assert!(policy.settings_bind_positionally());
        assert!(policy.source_variants_must_be_direct_root_children());
        assert!(policy.terminal_schema_file_is_empty());
    }

    #[test]
    fn dcs_schema_template_envelope_evidence_fails_closed_on_hash_policy_and_nonclaim_drift() {
        let raw = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_SCHEMA_TEMPLATE_ENVELOPE_EVIDENCE_JSON,
        )
        .unwrap();

        let mut hash_drift = raw.clone();
        hash_drift["fixture"]["round2UnpackedSha256"] = serde_json::json!("0".repeat(64));
        assert!(matches!(
            DcsSchemaTemplateEnvelopeEvidenceCorpus::parse(
                &serde_json::to_string(&hash_drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("fixture provenance or artifact hashes drifted")
        ));

        let mut policy_drift = raw.clone();
        policy_drift["policy"]["maximumAttestedSettingsVariants"] = serde_json::json!(3);
        assert!(matches!(
            DcsSchemaTemplateEnvelopeEvidenceCorpus::parse(
                &serde_json::to_string(&policy_drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("framing and positional policy drifted")
        ));

        let mut nonclaim_drift = raw.clone();
        nonclaim_drift["nonClaims"][0] = serde_json::json!("More than two variants are supported.");
        assert!(matches!(
            DcsSchemaTemplateEnvelopeEvidenceCorpus::parse(
                &serde_json::to_string(&nonclaim_drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("non-claims drifted")
        ));

        let mut extra_field = raw;
        extra_field["policy"]["guessMissingSettings"] = serde_json::json!(true);
        assert!(matches!(
            DcsSchemaTemplateEnvelopeEvidenceCorpus::parse(
                &serde_json::to_string(&extra_field).unwrap()
            ),
            Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn bundled_platform_dcs_order_policy_binds_both_contexts_and_fails_on_drift() {
        let policy = bundled_dcs_order_policy().unwrap();
        assert_eq!(
            policy.order_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}order"
        );
        assert_eq!(
            policy.storage_order_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}Order"
        );
        assert_eq!(
            policy.field_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}OrderItemField"
        );
        assert_eq!(policy.supported_order_types(), &["Asc", "Desc"]);
        assert_eq!(policy.supported_use_values(), &[None, Some(false)]);
        assert_eq!(policy.max_emitted_items(), 1);
        assert_eq!(policy.supported_view_modes(), &["Normal"]);
        assert_eq!(
            policy.metadata_only_user_setting_id(),
            "88619765-ccb3-46c6-ac52-38e9c992ebd4"
        );
        assert_eq!(
            policy.storage_record_type_uuid(),
            "11743ff3-2db3-4cfc-9404-90ed8209437f"
        );
        assert!(policy.follows_selection_and_precedes_structure_items());
        assert!(policy.propertyless_empty_order_is_unsupported());
        assert!(policy.metadata_only_order_requires_view_mode_and_user_setting_id());
        assert!(policy.root_auto_is_unsupported());

        let mut drift =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_ORDER_EVIDENCE_JSON).unwrap();
        drift["policy"]["itemChildOrder"] = serde_json::json!(["field", "use?", "orderType"]);
        assert!(matches!(
            DcsOrderEvidenceCorpus::parse(&serde_json::to_string(&drift).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("item child order drifted")
        ));
    }

    #[test]
    fn bundled_platform_dcs_filter_policy_binds_all_physical_contexts_and_defaults() {
        let policy = bundled_dcs_filter_policy().unwrap();
        assert_eq!(
            policy.filter_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}filter"
        );
        assert_eq!(
            policy.storage_filter_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}Filter"
        );
        assert_eq!(
            policy.comparison_item_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}FilterItemComparison"
        );
        assert_eq!(
            policy.left_field_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/core}Field"
        );
        assert_eq!(
            policy.right_string_type_qname(),
            "{http://www.w3.org/2001/XMLSchema}string"
        );
        assert_eq!(policy.supported_comparison_types(), &["Equal"]);
        assert_eq!(policy.supported_use_values(), &[None]);
        assert_eq!(policy.max_emitted_items(), 1);
        assert_eq!(policy.supported_view_modes(), &["Normal"]);
        assert_eq!(
            policy.metadata_only_user_setting_id(),
            "dfcece9d-5077-440b-b6b3-45a5cb4538eb"
        );
        assert_eq!(
            policy.comparison_storage_record_type_uuid(),
            "f6841c6b-6c71-4c82-ae9e-d08b49db326c"
        );
        assert!(policy.follows_selection_and_precedes_order_and_structure_items());
        assert!(policy.propertyless_empty_filter_is_unsupported());
        assert!(policy.metadata_only_filter_requires_view_mode_and_user_setting_id());
        assert!(policy.metadata_only_storage_property_is_absent());

        let mut drift =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_FILTER_EVIDENCE_JSON).unwrap();
        drift["policy"]["itemChildOrder"] = serde_json::json!(["left", "right", "comparisonType"]);
        assert!(matches!(
            DcsFilterEvidenceCorpus::parse(&serde_json::to_string(&drift).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("item child order drifted")
        ));
    }

    #[test]
    fn bundled_platform_dcs_conditional_appearance_policy_binds_dual_form_protocol() {
        let policy = bundled_dcs_conditional_appearance_policy().unwrap();
        assert_eq!(
            policy.conditional_appearance_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}conditionalAppearance"
        );
        assert_eq!(
            policy.storage_conditional_appearance_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}ConditionalAppearance"
        );
        assert_eq!(
            policy.parameter_value_type_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}SettingsParameterValue"
        );
        assert_eq!(
            policy.color_type_qname(),
            "{http://v8.1c.ru/8.1/data/ui}Color"
        );
        assert_eq!(policy.max_emitted_items(), 1);
        assert_eq!(policy.supported_view_modes(), &["Normal"]);
        assert_eq!(policy.storage_property_name(), "Appearance");
        assert_eq!(
            policy.storage_record_type_uuid(),
            "93de27ad-a2d8-4b10-a82b-483c9b0648fe"
        );
        assert!(policy.follows_filter_and_order_and_precedes_structure_items());
        assert!(policy.metadata_only_storage_property_is_absent());
        assert!(policy.empty_nested_filter_is_unsupported());

        let mut drift = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_CONDITIONAL_APPEARANCE_EVIDENCE_JSON,
        )
        .unwrap();
        drift["policy"]["storagePropertyName"] = serde_json::json!("ConditionalAppearance");
        assert!(matches!(
            DcsConditionalAppearanceEvidenceCorpus::parse(
                &serde_json::to_string(&drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("storage property drifted")
        ));
    }

    #[test]
    fn bundled_platform_dcs_form_attributes_conditional_appearance_policy_binds_wrapper() {
        let policy = bundled_dcs_form_attributes_conditional_appearance_policy().unwrap();
        assert_eq!(
            policy.wrapper_qname(),
            "{http://v8.1c.ru/8.3/xcf/logform}ConditionalAppearance"
        );
        assert_eq!(
            policy.storage_root_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}Settings"
        );
        assert_eq!(
            policy.storage_child_qname(),
            "{http://v8.1c.ru/8.1/data-composition-system/settings}conditionalAppearance"
        );
        assert_eq!(policy.max_emitted_items(), 1);
        assert_eq!(policy.storage_container_marker(), "4");
        assert_eq!(policy.storage_absent_container_marker(), "0");
        assert_eq!(policy.storage_inactive_marker(), &["0", "0"]);
        assert_eq!(policy.storage_active_marker(), &["0", "1"]);
        assert_eq!(policy.storage_field_order(), &["selection", "filter"]);
        assert_eq!(policy.storage_selection_type_indexes(), &[26, 9]);
        assert_eq!(policy.storage_filter_type_indexes(), &[26]);
        assert!(policy.follows_all_attributes());
        assert!(policy.uses_unkeyed_direct_base64_tail());
        assert!(!policy.has_storage_record_type_uuid());
        assert!(policy.container_metadata_is_forbidden());

        let mut drift = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_FORM_ATTRIBUTES_CONDITIONAL_APPEARANCE_EVIDENCE_JSON,
        )
        .unwrap();
        drift["policy"]["storageEnvelope"] = serde_json::json!("named-record");
        assert!(matches!(
            DcsFormAttributesConditionalAppearanceEvidenceCorpus::parse(
                &serde_json::to_string(&drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("storage envelope drifted")
        ));

        let mut drift = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_FORM_ATTRIBUTES_CONDITIONAL_APPEARANCE_EVIDENCE_JSON,
        )
        .unwrap();
        drift["policy"]["storageSelectionTypeIndexes"] = serde_json::json!([26, 10]);
        assert!(matches!(
            DcsFormAttributesConditionalAppearanceEvidenceCorpus::parse(
                &serde_json::to_string(&drift).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("selection type indexes drifted")
        ));
    }

    #[test]
    fn bundled_dcs_runtime_feature_slice_matches_full_research_corpus() {
        let runtime = bundled_dcs_list_settings_feature_semantics().unwrap();
        let full = bundled_feature_semantics().unwrap();
        let runtime_raw = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON,
        )
        .unwrap();
        let typed_runtime_raw = serde_json::to_value(&runtime).unwrap();
        assert_eq!(runtime_raw, typed_runtime_raw);
        let is_exact_typed_projection =
            |raw: &serde_json::Value, parsed: &FeatureSemanticsCorpus| {
                raw == &serde_json::to_value(parsed).unwrap()
            };

        let mut root_extra = runtime_raw.clone();
        root_extra["unexpected"] = serde_json::json!("payload");
        let root_extra_parsed =
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&root_extra).unwrap()).unwrap();
        assert!(!is_exact_typed_projection(&root_extra, &root_extra_parsed));

        let mut nested_extra = runtime_raw.clone();
        nested_extra["packages"][0]["classifiers"][0]["features"][0]["unexpected"] =
            serde_json::json!("payload");
        let nested_extra_parsed =
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&nested_extra).unwrap()).unwrap();
        assert!(!is_exact_typed_projection(
            &nested_extra,
            &nested_extra_parsed
        ));

        assert_eq!(runtime.schema_version, 1);
        assert_eq!(runtime.source.product, "1C:EDT");
        assert_eq!(runtime.source.release, "2025.2.3+30");
        assert_eq!(runtime.source.product, full.source.product);
        assert_eq!(runtime.source.release, full.source.release);
        assert_eq!(
            runtime.source.derivation,
            "deterministic runtime projection of the verified Xcore feature semantics corpus"
        );
        assert_eq!(
            runtime.summary,
            FeatureSemanticsSummary {
                packages: 1,
                classifiers: 1,
                features: 1,
            }
        );
        assert_eq!(runtime.packages.len(), 1);
        let package = &runtime.packages[0];
        assert_eq!(package.bundle, "com._1c.g5.v8.dt.dcs.model");
        assert_eq!(package.resource, "model/settings.xcore");
        assert_eq!(package.package_name, "com._1c.g5.v8.dt.dcs.model.settings");
        assert_eq!(package.namespace_uri, DCS_SETTINGS_MODEL_NAMESPACE);
        assert_eq!(package.classifiers.len(), 1);
        let classifier = &package.classifiers[0];
        assert_eq!(classifier.name, DCS_SETTINGS_CLASSIFIER);
        assert_eq!(classifier.kind, FeatureClassifierKind::Class);
        assert_eq!(classifier.features.len(), 1);
        let feature = &classifier.features[0];
        assert_eq!(feature.name, "itemsViewMode");

        let key = FeatureSemanticKey {
            namespace_uri: DCS_SETTINGS_MODEL_NAMESPACE.to_owned(),
            classifier: DCS_SETTINGS_CLASSIFIER.to_owned(),
            feature: "itemsViewMode".to_owned(),
        };
        assert_eq!(Some(feature), full.feature(&key));

        for marker in [
            b"ibcmd.exe".as_slice(),
            b"1cv8.exe",
            b"1cv8c.exe",
            b"\\1cv8\\",
            b"/1cv8/",
            b".jar",
            b"org.eclipse",
            b"JNI_CreateJavaVM",
            b"JNIEnv",
            b"JavaVM",
            b"OSGi",
        ] {
            assert!(
                !BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON
                    .as_bytes()
                    .windows(marker.len())
                    .any(|window| window == marker),
                "runtime DCS feature-semantics slice contains forbidden payload marker `{}`",
                String::from_utf8_lossy(marker)
            );
        }
    }

    #[test]
    fn dcs_tail_model_default_other_fails_closed() {
        let corpus = bundled_dcs_writer_evidence().unwrap();
        let mut feature_semantics = bundled_feature_semantics().unwrap();
        let feature = feature_semantics
            .packages
            .iter_mut()
            .find(|package| package.namespace_uri == DCS_SETTINGS_MODEL_NAMESPACE)
            .and_then(|package| {
                package
                    .classifiers
                    .iter_mut()
                    .find(|classifier| classifier.name == DCS_SETTINGS_CLASSIFIER)
            })
            .and_then(|classifier| {
                classifier
                    .features
                    .iter_mut()
                    .find(|feature| feature.name == "itemsViewMode")
            })
            .unwrap();
        feature.default_value = Some("Other".to_owned());

        assert!(matches!(
            corpus.form_list_settings_tail_policy(&feature_semantics),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("exact default join")
        ));
    }

    #[test]
    fn dcs_tail_writer_constant_other_fails_closed() {
        let mut writer_evidence =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        let items_view_mode_fact = writer_evidence["verifiedFacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|fact| fact["key"].as_str() == Some("dcs.DataCompositionSettings.itemsViewMode"))
            .unwrap();
        items_view_mode_fact["value"]["defaultModelConstant"] = serde_json::json!("OTHER");

        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(
                &serde_json::to_string(&writer_evidence).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("itemsViewMode writer policy drifted")
        ));
    }

    #[test]
    fn dcs_writer_evidence_parser_is_bounded_and_fails_closed_on_drift() {
        let oversized = " ".repeat(MAX_DCS_WRITER_EVIDENCE_JSON_BYTES + 1);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&oversized),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("JSON exceeds")
        ));

        let mut unknown =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        unknown["forged"] = serde_json::json!(true);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&unknown).unwrap()),
            Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let mut duplicate =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        let duplicate_fact = duplicate["verifiedFacts"][0].clone();
        duplicate["verifiedFacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_fact);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&duplicate).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("duplicate verified fact")
        ));

        let mut corrupt_wrapper =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        corrupt_wrapper["verifiedFacts"][0]["value"]["qname"] =
            serde_json::json!("{urn:forged}Settings");
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&corrupt_wrapper).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("standalone Settings QName drifted")
        ));

        let mut corrupt_type =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        corrupt_type["verifiedFacts"][2]["value"]["emission"] = serde_json::json!("xsi:type");
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&corrupt_type).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("TypeId absence drifted")
        ));

        let mut corrupt_opaque =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        corrupt_opaque["missingKeys"][0]["status"] =
            serde_json::json!("not-proven-by-this-extractor");
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&corrupt_opaque).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("unexpected status")
        ));

        let mut corrupt_manual_source =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        corrupt_manual_source["verifiedFacts"][0]["evidence"]["sources"][0] =
            serde_json::json!("tools/report-edt-dcs-writer-evidence.ps1");
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(
                &serde_json::to_string(&corrupt_manual_source).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("provenance sources")
        ));

        let mut corrupt_negative_source =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        corrupt_negative_source["missingKeys"][0]["evidence"]["sources"][1] =
            serde_json::json!("edt-derived://2025.2.3+30/forged/reader#acceptUnknown");
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(
                &serde_json::to_string(&corrupt_negative_source).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("lacks exact negative bytecode evidence")
        ));
    }

    fn canonical_coverage_fixture() -> CanonicalCoverageCorpus {
        CanonicalCoverageCorpus {
            schema_version: 1,
            source: CorpusSource {
                product: "ibcmd-rs".to_owned(),
                release: "2025.2.3+30".to_owned(),
                derivation: "canonical coverage bootstrap".to_owned(),
            },
            summary: CanonicalCoverageSummary {
                entries: 1,
                typed: 1,
                opaque_lossless: 0,
                unsupported: 0,
                platform_only: 0,
            },
            family_aggregates: vec![
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Metadata,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Forms,
                    entries: 1,
                    typed: 1,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Dcs,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Mxl,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Common,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Other,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
            ],
            migration_backlog: vec![],
            entries: vec![CanonicalCoverageEntry {
                key: FeatureSemanticKey {
                    namespace_uri: "http://g5.1c.ru/v8/dt/form".to_owned(),
                    classifier: "Form".to_owned(),
                    feature: "baseForm".to_owned(),
                },
                family: CanonicalCoverageFamily::Forms,
                status: CoverageStatus::Typed,
                canonical_type: Some("CanonicalForm".to_owned()),
                canonical_field: Some("base_form".to_owned()),
                opaque_placement: None,
                diagnostic_code: None,
                evidence: FeatureEvidence {
                    status: EvidenceStatus::Verified,
                    kind: "code-inspection".to_owned(),
                    sources: vec!["crates/ibcmd-core/src/model.rs".to_owned()],
                    note: None,
                },
            }],
        }
    }

    #[test]
    fn canonical_coverage_public_parse_enforces_exact_byte_and_string_limits() {
        let mut json = serde_json::to_string(&canonical_coverage_fixture()).unwrap();
        json.push_str(&" ".repeat(MAX_CANONICAL_COVERAGE_JSON_BYTES - json.len()));
        assert_eq!(json.len(), MAX_CANONICAL_COVERAGE_JSON_BYTES);
        assert!(CanonicalCoverageCorpus::parse(&json).is_ok());

        json.push(' ');
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&json),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 4194304 UTF-8 bytes")
        ));

        let mut value = serde_json::to_value(canonical_coverage_fixture()).unwrap();
        value["source"]["derivation"] =
            serde_json::Value::String("x".repeat(MAX_CANONICAL_COVERAGE_STRING_BYTES));
        assert!(CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()).is_ok());

        value["source"]["derivation"] =
            serde_json::Value::String("x".repeat(MAX_CANONICAL_COVERAGE_STRING_BYTES + 1));
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 4096 UTF-8 bytes")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_enforces_exact_vector_limits() {
        let mut evidence_limit = canonical_coverage_fixture();
        evidence_limit.entries[0].evidence.sources = (0..MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES)
            .map(|index| format!("evidence/source-{index}"))
            .collect();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&evidence_limit).unwrap())
                .is_ok()
        );
        evidence_limit.entries[0]
            .evidence
            .sources
            .push("evidence/overflow".to_owned());
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&evidence_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 16 elements")
        ));

        let mut family_limit = canonical_coverage_fixture();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&family_limit).unwrap()).is_ok()
        );
        family_limit
            .family_aggregates
            .push(family_limit.family_aggregates[0].clone());
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&family_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 6 elements")
        ));

        let mut backlog_limit = canonical_coverage_fixture();
        backlog_limit.migration_backlog = (0..MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES)
            .map(|index| CanonicalMigrationBacklogEntry {
                rule: "unsupported/schema.unmapped".to_owned(),
                family: CanonicalCoverageFamily::Metadata,
                package: format!("package.{index:03}"),
                classifier_kind: FeatureClassifierKind::Class,
                feature_kind: FeatureKind::Attribute,
                features: 1,
            })
            .collect();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&backlog_limit).unwrap()).is_ok()
        );
        backlog_limit
            .migration_backlog
            .push(CanonicalMigrationBacklogEntry {
                rule: "unsupported/schema.unmapped".to_owned(),
                family: CanonicalCoverageFamily::Metadata,
                package: "package.overflow".to_owned(),
                classifier_kind: FeatureClassifierKind::Class,
                feature_kind: FeatureKind::Attribute,
                features: 1,
            });
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&backlog_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 256 elements")
        ));

        let template = canonical_coverage_fixture().entries.remove(0);
        let mut entry_limit = canonical_coverage_fixture();
        entry_limit.entries = (0..MAX_CANONICAL_COVERAGE_ENTRIES)
            .map(|index| {
                let mut entry = template.clone();
                entry.key.feature = format!("feature{index:04}");
                entry
            })
            .collect();
        entry_limit.summary.entries = MAX_CANONICAL_COVERAGE_ENTRIES;
        entry_limit.summary.typed = MAX_CANONICAL_COVERAGE_ENTRIES;
        entry_limit.family_aggregates = recompute_family_aggregates(&entry_limit.entries);
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&entry_limit).unwrap()).is_ok()
        );
        let mut overflow = template;
        overflow.key.feature = "featureOverflow".to_owned();
        entry_limit.entries.push(overflow);
        entry_limit.summary.entries += 1;
        entry_limit.summary.typed += 1;
        entry_limit.family_aggregates = recompute_family_aggregates(&entry_limit.entries);
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&entry_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 5000 elements")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_rejects_forged_and_duplicate_fields() {
        for field in ["unexpected", "uuid", "objectName"] {
            let mut value = serde_json::to_value(canonical_coverage_fixture()).unwrap();
            value["entries"][0]
                .as_object_mut()
                .unwrap()
                .insert(field.to_owned(), serde_json::json!("forged"));
            assert!(matches!(
                CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()),
                Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
            ));
        }

        let json = serde_json::to_string(&canonical_coverage_fixture()).unwrap();
        let duplicate_root = json.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"schemaVersion\":1",
            1,
        );
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&duplicate_root),
            Err(SchemaError::InvalidJson(message)) if message.contains("duplicate field")
        ));
        let duplicate_key = json.replacen(
            "\"feature\":\"baseForm\"",
            "\"feature\":\"baseForm\",\"feature\":\"baseForm\"",
            1,
        );
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&duplicate_key),
            Err(SchemaError::InvalidJson(message)) if message.contains("duplicate field")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_rejects_duplicate_key_map_entries() {
        let mut corpus = canonical_coverage_fixture();
        corpus.entries.push(corpus.entries[0].clone());
        corpus.summary.entries = 2;
        corpus.summary.typed = 2;
        corpus.family_aggregates = recompute_family_aggregates(&corpus.entries);
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&corpus).unwrap()),
            Err(SchemaError::DuplicateValue {
                field: "canonical coverage key",
                ..
            })
        ));
    }

    #[test]
    fn canonical_coverage_requires_status_specific_contracts() {
        let mut corpus = canonical_coverage_fixture();
        corpus.entries[0].canonical_field = None;
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "typed mapping requires canonical type and field",
                ..
            })
        ));

        let entry = &mut corpus.entries[0];
        entry.status = CoverageStatus::OpaqueLossless;
        entry.canonical_type = None;
        entry.canonical_field = None;
        entry.opaque_placement = Some("property-slot".to_owned());
        corpus.summary.typed = 0;
        corpus.summary.opaque_lossless = 1;
        corpus.family_aggregates = recompute_family_aggregates(&corpus.entries);
        assert!(corpus.validate().is_ok());

        corpus.entries[0].evidence.status = EvidenceStatus::Pending;
        corpus.entries[0].evidence.sources.clear();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "coverage mapping requires verified evidence",
                ..
            })
        ));
    }

    #[test]
    fn canonical_coverage_rejects_irrelevant_status_fields() {
        let mut typed = canonical_coverage_fixture();
        typed.entries[0].diagnostic_code = Some("unexpected".to_owned());
        assert!(matches!(
            typed.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "typed mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut opaque = canonical_coverage_fixture();
        opaque.entries[0].status = CoverageStatus::OpaqueLossless;
        opaque.entries[0].opaque_placement = Some("slot".to_owned());
        opaque.summary.typed = 0;
        opaque.summary.opaque_lossless = 1;
        opaque.family_aggregates = recompute_family_aggregates(&opaque.entries);
        assert!(matches!(
            opaque.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "opaque-lossless mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut unsupported = canonical_coverage_fixture();
        unsupported.entries[0].status = CoverageStatus::Unsupported;
        unsupported.entries[0].diagnostic_code = Some("schema.unsupported".to_owned());
        unsupported.summary.typed = 0;
        unsupported.summary.unsupported = 1;
        unsupported.family_aggregates = recompute_family_aggregates(&unsupported.entries);
        assert!(matches!(
            unsupported.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "unsupported mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut platform_only = canonical_coverage_fixture();
        platform_only.entries[0].status = CoverageStatus::PlatformOnly;
        platform_only.entries[0].evidence.note = Some("requires platform runtime".to_owned());
        platform_only.summary.typed = 0;
        platform_only.summary.platform_only = 1;
        platform_only.family_aggregates = recompute_family_aggregates(&platform_only.entries);
        assert!(matches!(
            platform_only.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "platform-only mapping contains irrelevant status fields",
                ..
            })
        ));
    }

    #[test]
    fn canonical_coverage_full_join_rejects_unmapped_and_stale_keys() {
        let mut features = feature_semantics_fixture();
        features.packages[0].package_name = "com._1c.g5.v8.dt.form.model".to_owned();
        features.packages[0].namespace_uri = "http://g5.1c.ru/v8/dt/form".to_owned();
        features.packages[0].classifiers[0].name = "Form".to_owned();
        features.packages[0].classifiers[0].features[0].name = "baseForm".to_owned();

        let coverage = canonical_coverage_fixture();
        assert!(coverage.validate_against(&features).is_ok());

        let mut missing = coverage.clone();
        missing.entries.clear();
        missing.summary.entries = 0;
        missing.summary.typed = 0;
        missing.family_aggregates = recompute_family_aggregates(&missing.entries);
        assert!(matches!(
            missing.validate_against(&features),
            Err(SchemaError::CoverageMismatch {
                kind: "unmapped",
                ..
            })
        ));

        let mut stale = coverage;
        let mut stale_entry = stale.entries[0].clone();
        stale_entry.key.feature = "removedFeature".to_owned();
        stale.entries.push(stale_entry);
        stale.summary.entries = 2;
        stale.summary.typed = 2;
        stale.family_aggregates = recompute_family_aggregates(&stale.entries);
        assert!(matches!(
            stale.validate_against(&features),
            Err(SchemaError::CoverageMismatch { kind: "stale", .. })
        ));
    }

    #[test]
    fn canonical_coverage_rejects_drifted_aggregates_and_backlog() {
        let features = bundled_feature_semantics().unwrap();
        let mut aggregate_drift = bundled_canonical_coverage().unwrap();
        aggregate_drift.family_aggregates[1].entries += 1;
        assert!(matches!(
            aggregate_drift.validate(),
            Err(SchemaError::CoverageDerivedDataMismatch(
                "family aggregates"
            ))
        ));

        let mut backlog_drift = bundled_canonical_coverage().unwrap();
        backlog_drift.migration_backlog[0].features += 1;
        assert!(matches!(
            backlog_drift.validate_against(&features),
            Err(SchemaError::CoverageDerivedDataMismatch(
                "migration backlog"
            ))
        ));
    }

    #[test]
    fn canonical_coverage_unknown_package_classifier_route_fails_closed() {
        let mut features = feature_semantics_fixture();
        features.packages[0].package_name = "unknown.package".to_owned();
        features.packages[0].namespace_uri = "http://g5.1c.ru/v8/dt/form".to_owned();
        features.packages[0].classifiers[0].name = "Form".to_owned();
        features.packages[0].classifiers[0].features[0].name = "baseForm".to_owned();

        assert!(matches!(
            canonical_coverage_fixture().validate_against(&features),
            Err(SchemaError::UnknownCoverageRoute { package, .. })
                if package == "unknown.package"
        ));
    }
}
