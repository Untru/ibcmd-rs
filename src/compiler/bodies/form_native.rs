//! Writer for the managed-form item records a native 8.3.27 body stores.
//!
//! The base-free creator in `module_blob` writes the smallest record that
//! names an item and leaves the platform to fill the rest; a body it produces
//! is loadable but is not the body the platform would have stored. This module
//! is the other direction: each function returns the record the platform
//! itself writes, byte for byte, so a loaded configuration exports back to the
//! tree it came from.
//!
//! Every record here is measured against bodies read out of
//! `ibcmd_rs_uha_8327_parity2_20260920` with `mssql-dump-config --inflate`,
//! and the tests below hold the exact strings those bodies carry. A property
//! this module cannot yet write is not defaulted: the caller keeps refusing
//! the item, which is what the blocker model already does.

/// The namespace every form *item* id lives in.
///
/// One uuid across all 3 971 item records of the four ERP УХ 3.3.3.3 form
/// bodies read for this module, from four unrelated families. Form attributes
/// live in a different one, which is why this is an item-only constant.
pub(crate) const FORM_ITEM_NAMESPACE_UUID: &str = "02023637-7868-4a5f-8576-835a76e0c9ba";

/// The palette reference every default appearance slot carries.
const DEFAULT_APPEARANCE_UUID: &str = "48312c09-257f-4b29-b280-284dd89efc1e";

/// The uuid a parent writes before a child to say what kind of child it is.
///
/// One uuid per record wrapper, with no overlap, over the child lists of the
/// first 400 ERP УХ form bodies: 2 353 fields, 2 198 groups, 1 309 buttons,
/// 677 decorations, 180 tables and 8 search additions.
pub(crate) const fn child_kind_uuid(wrapper: u8) -> Option<&'static str> {
    Some(match wrapper {
        37 => "77ffcc29-7f2d-4223-b22f-19666e7250ba",
        22 => "cd5394d0-7dda-4b56-8927-93ccbe967a01",
        31 => "a9f3b1ac-f51b-431e-b102-55a69acdecad",
        12 => "3d3cb80c-508b-41fa-8a18-680cdf5f1712",
        55 => "143c00f7-a42d-4cd7-9189-88e4467dc768",
        5 => "c5259a1d-518a-4afd-b98d-0176027e4feb",
        _ => return None,
    })
}

/// A 1C string literal: quoted, with `"` doubled.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// The `{12,…}` record of an item's `<ExtendedTooltip>`, as the platform
/// stores it when the tooltip carries nothing but its own name.
///
/// A single constant shape: all 85 tooltip records of
/// `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов` and
/// `BusinessProcesses/Задание/Forms/ДействиеВыполнить` normalise to it, with
/// only the id and the name differing.
pub(crate) fn format_extended_tooltip(id: &str, name: &str) -> String {
    format!(
        "{{12,{{{id},{ns}}},0,0,0,0,{name},{{1,0}},{{1,0}},1,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{5,0,0,3,0,{{0,1,0}},{{3,4,{{0}}}},{{3,4,{{0}}}},\
         {{3,0,{{0}},0,1,0,{appearance}}}}},0,1,2,{{1,{{1,0}},0}},0,0,1,0,0,1,0,3,3,0,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        appearance = DEFAULT_APPEARANCE_UUID,
    )
}

/// The `{22,…}` record of a field's `<ContextMenu>`, as the platform stores it
/// when the menu carries nothing but its own name.
pub(crate) fn format_field_context_menu(id: &str, name: &str) -> String {
    format!(
        "{{22,{{{id},{ns}}},0,0,0,8,{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{1,1}},0,1,0,0,0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

/// What a field record needs from the source beyond its own name.
pub(crate) struct NativeFieldItem<'a> {
    pub(crate) id: &'a str,
    /// Which field this is: 1 a label, 2 an input, 3 a check box, 4 a picture,
    /// 5 a radio-button group, 6 a spreadsheet document, 7 an HTML document,
    /// 9 an indicator, 15 a formatted document. Censused over the `{37,…}`
    /// records of the first 800 ERP УХ form bodies: all of them share this one
    /// frame of 59 members -- 60 when the item carries the visibility tuple --
    /// and differ only in this slot, in `after_name`, in the data path and in
    /// the payload.
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Whether the item carries the common `UserVisible` tuple before its
    /// kind, as 527 of the 2 836 kind-2 records do.
    pub(crate) visible_tuple: bool,
    /// The two members that follow the name -- `1,0` for a label or an input,
    /// `4,0` for a check box, `0,0` for a spreadsheet document.
    pub(crate) after_name: &'a str,
    /// The binding the item's `<DataPath>` resolves to, already formatted --
    /// `{1,{2}}` for a form attribute, `{2,{1},{3}}` for a dynamic-list column.
    pub(crate) data_path: &'a str,
    /// The tuple that carries this kind's own properties: `{11,…}` for a
    /// label, `{36,…}` for an input, `{13,…}` for a spreadsheet document,
    /// `{10,…}` for a picture, and so on -- one structure per kind.
    pub(crate) payload: &'a str,
    pub(crate) context_menu_id: &'a str,
    pub(crate) context_menu_name: &'a str,
    pub(crate) extended_tooltip_id: &'a str,
    pub(crate) extended_tooltip_name: &'a str,
}

/// The `{37,…}` record of a field of any kind that carries only its name, its
/// data path, its kind payload and its two default children.
///
/// Measured against `Documents/Лот/Forms/ВыигранныеЛоты`, whose three label
/// fields and one input field are exactly this shape, and against the census
/// of every field kind in the corpus.
pub(crate) fn format_field_item(item: &NativeFieldItem<'_>) -> String {
    let visibility = if item.visible_tuple {
        "1,{0,{0,{\"B\",1},0}}"
    } else {
        "0"
    };
    format!(
        "{{37,{{{id},{ns}}},0,0,{visibility},{kind},{name},{after_name},{{1,0}},{{1,0}},\
         {data_path},{{0}},1,0,2,0,2,{{1,0}},{{1,0}},1,1,0,3,0,3,1,3,0,\
         {{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,\
         {payload},{{0,1,0}},1,\
         {context_menu},1,{{\"Pattern\"}},{{\"Pattern\"}},\"\",\"\",{{0}},0,0,1,{tooltip},3,3,0,0,0,0}}",
        id = item.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = item.kind,
        name = quoted(item.name),
        after_name = item.after_name,
        data_path = item.data_path,
        payload = item.payload,
        context_menu = format_field_context_menu(item.context_menu_id, item.context_menu_name),
        tooltip = format_extended_tooltip(item.extended_tooltip_id, item.extended_tooltip_name),
    )
}

/// The `{11,…}` payload of a label or input field that carries no appearance
/// of its own, with the one slot that separates a field the user edits from
/// one that only shows.
pub(crate) fn format_plain_field_payload(editable: bool) -> String {
    format_label_payload(&NativeLabelPayload::plain(editable))
}

/// The `{11,…}` payload of a label field, member by member.
///
/// Five of its twenty members carry an XML property. They were read off the
/// corpus and then measured against it: a candidate that fills these five and
/// copies the rest reproduces **all 37 078** label payloads of every ERP УХ
/// form body exactly, with no record left over. The other members are the
/// appearance blocks and the event bindings, which are shapes of their own.
pub(crate) struct NativeLabelPayload<'a> {
    /// Slot 1, `0` when the item names no width.
    pub(crate) width: &'a str,
    /// Slot 2, `0` when it names no height.
    pub(crate) height: &'a str,
    /// Slot 3: `false` -> 0, `true` -> 1, absent -> 2.
    pub(crate) horizontal_stretch: Option<bool>,
    /// Slot 6, `{1,0}` when the item names no format.
    pub(crate) format: &'a str,
    /// Slot 7: whether the field is one the user edits.
    pub(crate) editable: bool,
    /// Slot 8, the text colour.
    pub(crate) text_color: &'a str,
    /// Slot 9, the background colour.
    pub(crate) back_color: &'a str,
    /// Slot 10, the font.
    pub(crate) font: &'a str,
    /// Slot 12, the item's own event bindings.
    pub(crate) events: &'a str,
    /// Slot 15: `0` exactly when the item says `AutoMaxWidth` is false.
    pub(crate) auto_max_width: bool,
    /// Slot 16, `0` when the item names no maximum width.
    pub(crate) max_width: &'a str,
}

impl NativeLabelPayload<'_> {
    /// What a label that names no property of its own carries.
    pub(crate) const fn plain(editable: bool) -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: None,
            format: "{1,0}",
            editable,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
            events: "{0,1,0}",
            auto_max_width: true,
            max_width: "0",
        }
    }
}

pub(crate) fn format_label_payload(payload: &NativeLabelPayload<'_>) -> String {
    let stretch = match payload.horizontal_stretch {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{11,{width},{height},{stretch},2,2,{format},{editable},{text_color},{back_color},\
         {font},2,{events},{{3,4,{{0}}}},{{3,0,{{0}},0,1,0,{appearance}}},{auto_max_width},\
         {max_width},0,1,0}}",
        width = payload.width,
        height = payload.height,
        format = payload.format,
        editable = u8::from(payload.editable),
        text_color = payload.text_color,
        back_color = payload.back_color,
        font = payload.font,
        events = payload.events,
        appearance = DEFAULT_APPEARANCE_UUID,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
    )
}

/// The `{36,…}` payload of an input field, member by member.
///
/// Seven of its sixty-six members carry an XML property. Measured the same way
/// as the label: a candidate that fills these seven and copies the rest
/// reproduces 69 452 of the 69 455 input payloads of every ERP УХ form body
/// exactly, and the three it does not are a mask that carries a `;`, which the
/// measurement's own property separator truncated -- not a rule this writer
/// gets wrong.
pub(crate) struct NativeInputPayload<'a> {
    /// Slot 2, `0` when the item names no width.
    pub(crate) width: &'a str,
    /// Slot 3, `0` when it names no height.
    pub(crate) height: &'a str,
    /// Slot 4: `false` -> 0, `true` -> 1, absent -> 2.
    pub(crate) horizontal_stretch: Option<bool>,
    /// Slot 5, the same three ways for `VerticalStretch`.
    pub(crate) vertical_stretch: Option<bool>,
    /// Slot 18, the input mask, `""` when the item names none.
    pub(crate) mask: &'a str,
    /// Slot 49: `0` exactly when the item says `AutoMaxWidth` is false.
    pub(crate) auto_max_width: bool,
    /// Slot 50, `0` when the item names no maximum width.
    pub(crate) max_width: &'a str,
    /// Slot 29, the format, `{1,0}` by default.
    pub(crate) format: &'a str,
    /// Slot 30, the edit format, `{1,0}` by default.
    pub(crate) edit_format: &'a str,
    /// Slot 36, the item's own event bindings.
    pub(crate) events: &'a str,
    /// Slots 37, 38 and 39: the text colour, the background colour and the
    /// border, `{3,4,{0}}` by default.
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    /// Slot 40, the font.
    pub(crate) font: &'a str,
}

impl NativeInputPayload<'_> {
    /// What an input field that names no property of its own carries -- the
    /// payload 24 361 of the corpus's records carry unchanged.
    pub(crate) const fn plain() -> Self {
        Self {
            width: "0",
            height: "0",
            horizontal_stretch: None,
            vertical_stretch: None,
            mask: "",
            auto_max_width: true,
            max_width: "0",
            format: "{1,0}",
            edit_format: "{1,0}",
            events: "{0,1,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_input_payload(payload: &NativeInputPayload<'_>) -> String {
    let stretch = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{36,{{3,0}},{width},{height},{horizontal},{vertical},1,2,2,2,2,2,2,2,2,2,{{\"U\"}},{{\"U\"}},{mask},0,{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},0,0,2,3,00000000-0000-0000-0000-000000000000,{{5006,0}},{{0,0}},2,{format},{edit_format},2,1,0,{{\"Pattern\"}},1,{events},{text_color},{back_color},{border_color},{font},1,{{3,0,0}},0,{{1,0}},2,0,2,0,{auto_max_width},{max_width},0,1,0,0,0,0,0,0,0,0,0,{{0}},0,{{5007,0}},0}}",
        width = payload.width,
        height = payload.height,
        horizontal = stretch(payload.horizontal_stretch),
        vertical = stretch(payload.vertical_stretch),
        mask = quoted(payload.mask),
        format = payload.format,
        edit_format = payload.edit_format,
        events = payload.events,
        text_color = payload.text_color,
        back_color = payload.back_color,
        border_color = payload.border_color,
        font = payload.font,
        auto_max_width = u8::from(payload.auto_max_width),
        max_width = payload.max_width,
    )
}

/// How a `<CheckBoxField>` draws itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeCheckBoxType {
    /// Also what a field that names no type carries.
    Auto,
    CheckBox,
    Tumbler,
    /// A switcher differs from `Auto` in one of the two slots that carry the
    /// type, which is why they cannot share a value.
    Switcher,
}

/// The `{11,…}` payload of a check box field -- the same wrapper a label
/// carries, with thirteen members instead of twenty.
///
/// Two of them carry the check box type. A candidate that fills those two and
/// copies the rest reproduces **all 10 385** check-box payloads of every ERP
/// УХ form body exactly, first time.
pub(crate) struct NativeCheckBoxPayload<'a> {
    pub(crate) check_box_type: NativeCheckBoxType,
    /// Slot 2, the text colour.
    pub(crate) text_color: &'a str,
    /// Slot 3, the background colour.
    pub(crate) back_color: &'a str,
    /// Slot 5, the format.
    pub(crate) format: &'a str,
    /// Slot 6, the border.
    pub(crate) border_color: &'a str,
    /// Slot 7, the font.
    pub(crate) font: &'a str,
}

impl NativeCheckBoxPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            check_box_type: NativeCheckBoxType::Auto,
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            format: "{1,0}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_check_box_payload(payload: &NativeCheckBoxPayload<'_>) -> String {
    let (kind, tail) = match payload.check_box_type {
        NativeCheckBoxType::Auto => ("0", "0"),
        NativeCheckBoxType::CheckBox => ("1", "1"),
        NativeCheckBoxType::Tumbler => ("2", "2"),
        NativeCheckBoxType::Switcher => ("0", "3"),
    };
    format!(
        "{{11,0,{text_color},{back_color},{kind},{format},{border_color},{font},0,0,0,2,{tail}}}",
        text_color = payload.text_color,
        back_color = payload.back_color,
        format = payload.format,
        border_color = payload.border_color,
        font = payload.font,
    )
}

/// How a `<RadioButtonField>` draws itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeRadioButtonType {
    /// Also what a field that names no type carries.
    Auto,
    RadioButtons,
    Tumbler,
}

/// The `{8,…}` payload of a radio-button field.
///
/// Two of its twelve members carry an XML property: the number of columns and
/// the type. A candidate that fills those two and copies the rest reproduces
/// **all 2 282** radio-button payloads of every ERP УХ form body exactly,
/// first time.
pub(crate) struct NativeRadioButtonPayload<'a> {
    /// Slot 2, `0` when the field names no column count.
    pub(crate) columns: &'a str,
    pub(crate) radio_button_type: NativeRadioButtonType,
    /// Slot 1, the choice list the field offers.
    pub(crate) choice_list: &'a str,
    /// Slot 3, the text colour; slot 5, the background; slot 8, the border.
    pub(crate) text_color: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) border_color: &'a str,
    /// Slot 4, the font.
    pub(crate) font: &'a str,
}

impl NativeRadioButtonPayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            columns: "1",
            radio_button_type: NativeRadioButtonType::Auto,
            choice_list: "{3,0}",
            text_color: "{3,4,{0}}",
            back_color: "{3,4,{0}}",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_radio_button_payload(payload: &NativeRadioButtonPayload<'_>) -> String {
    let kind = match payload.radio_button_type {
        NativeRadioButtonType::Auto => "0",
        NativeRadioButtonType::RadioButtons => "1",
        NativeRadioButtonType::Tumbler => "2",
    };
    format!(
        "{{8,{choice_list},{columns},{text_color},{font},{back_color},0,{kind},{border_color},0,0,2}}",
        choice_list = payload.choice_list,
        columns = payload.columns,
        text_color = payload.text_color,
        font = payload.font,
        back_color = payload.back_color,
        border_color = payload.border_color,
    )
}

/// The `{10,…}` payload of a picture field.
///
/// Five of its twenty-four members carry an XML property. All 2 202 records of
/// the corpus are reproduced exactly; the second round added the two
/// `<PictureSize>` spellings the first did not know, `AutoSizeIgnoreScale` and
/// `ByFontSize`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn format_picture_payload(
    width: &str,
    height: &str,
    picture_size: Option<&str>,
    max_width: &str,
    max_height: &str,
    picture: &str,
    title: &str,
    text_color: &str,
    back_color: &str,
    font: &str,
    border: &str,
    events: &str,
    third: &str,
    fourth: &str,
    seventeenth: &str,
    twentieth: &str,
    twenty_second: &str,
) -> String {
    let size = match picture_size {
        Some("Stretch") => "1",
        Some("Proportionally") => "2",
        Some("AutoSize") => "4",
        Some("AutoSizeIgnoreScale") => "6",
        Some("ByFontSize") => "7",
        _ => "0",
    };
    format!(
        "{{10,{width},{height},{third},{fourth},{picture},{size},0,0,{title},{text_color},{back_color},{font},{border},0,{events},{seventeenth},{max_width},0,{twentieth},{max_height},{twenty_second},0,100}}"
    )
}

/// The `{13,…}` payload of a spreadsheet document field.
///
/// Six of its thirty-two members carry an XML property -- the two sizes, the
/// two maxima and the two scroll bars, where a field naming neither scroll bar
/// carries 2. All 942 records of the corpus are reproduced exactly, first
/// time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn format_spreadsheet_payload(
    width: &str,
    height: &str,
    max_width: &str,
    max_height: &str,
    vertical_scroll_bar: Option<bool>,
    horizontal_scroll_bar: Option<bool>,
    middle: &str,
    events: &str,
    tail: &str,
) -> String {
    let bar = |value: Option<bool>| match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "2",
    };
    format!(
        "{{13,{width},{height},{middle},{max_width},0,{max_height},0,0,{events},{tail},{vertical},{horizontal},1,2}}",
        vertical = bar(vertical_scroll_bar),
        horizontal = bar(horizontal_scroll_bar),
    )
}

/// The `{3,…}`, `{5,…}` and `{1,…}` payloads of the HTML, text and formatted
/// document fields.
///
/// Two of their members carry an XML property -- the width and the height, at
/// slots 1 and 2 in all three, with 50 and 10 the values a field that names
/// neither carries. All 135, 94 and 47 records of the corpus are reproduced
/// exactly, first time.
pub(crate) fn format_document_payload(wrapper: u8, width: &str, height: &str, tail: &str) -> String {
    format!("{{{wrapper},{width},{height},{tail}}}")
}

/// The `{31,…}` record of a `<Button>` whose action is a form standard
/// command and which carries nothing but its name and its tooltip.
///
/// One shape across every such button of the bodies read for this module: only
/// the id, the name, the command uuid and the tooltip differ.
pub(crate) fn format_standard_command_button(
    id: &str,
    name: &str,
    command_uuid: &str,
    extended_tooltip_id: &str,
    extended_tooltip_name: &str,
) -> String {
    format!(
        "{{31,{{{id},{ns}}},0,1,{{0,{{0,{{\"B\",1}},0}}}},0,{name},{{1,0}},1,{{1,{command_uuid}}},\
         {{0}},3,0,0,0,2,2,0,0,0,{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},\
         {{0,0,0}},0,{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},1,{{\"Pattern\"}},\"\",2,0,1,{tooltip},\
         {{\"U\"}},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        tooltip = format_extended_tooltip(extended_tooltip_id, extended_tooltip_name),
    )
}

/// The `{12,…}` record of an item's `<ExtendedTooltip>` in the shape the
/// platform writes when the owning item carries the visibility tuple.
///
/// The same record as [`format_extended_tooltip`] with `1,{0,{0,{"B",1},0}}`
/// inserted after the id -- the common `UserVisible` prefix the reader already
/// normalises away. 49 of the 1 602 tooltip records read for this module carry
/// it; the other 1 475 do not.
pub(crate) fn format_visible_extended_tooltip(id: &str, name: &str) -> String {
    format!(
        "{{12,{{{id},{ns}}},0,0,1,{{0,{{0,{{\"B\",1}},0}}}},0,{name},{{1,0}},{{1,0}},1,0,0,2,2,\
         {{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,{{5,0,0,3,0,{{0,1,0}},{{3,4,{{0}}}},\
         {{3,4,{{0}}}},{{3,0,{{0}},0,1,0,{appearance}}}}},0,1,2,{{1,{{1,0}},0}},0,0,1,0,0,1,0,3,3,0,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
        appearance = DEFAULT_APPEARANCE_UUID,
    )
}

/// A `<Title>` that declares one Russian line, in the shape a body stores it.
/// An empty title is `{1,0}`.
pub(crate) fn format_russian_title(text: &str) -> String {
    if text.is_empty() {
        return "{1,0}".to_string();
    }
    format!("{{1,1,{{\"ru\",{}}}}}", quoted(text))
}

/// What a `<LabelDecoration>` record needs beyond its own name.
pub(crate) struct NativeLabelDecoration<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    /// The picture the decoration shows, when it shows one.
    pub(crate) picture_uuid: Option<&'a str>,
    pub(crate) context_menu_id: &'a str,
    pub(crate) context_menu_name: &'a str,
    pub(crate) extended_tooltip_id: &'a str,
    pub(crate) extended_tooltip_name: &'a str,
}

/// The `{12,…}` record of a `<LabelDecoration>` that carries its name, its
/// title and its two default children.
///
/// Measured against `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`,
/// whose decorations are exactly this shape.
pub(crate) fn format_label_decoration(decoration: &NativeLabelDecoration<'_>) -> String {
    let picture = match decoration.picture_uuid {
        Some(uuid) => format!("{{4,1,{{0,{uuid}}},\"\",-1,-1,0,0,\"\"}}"),
        None => "{4,0,{0},\"\",-1,-1,1,0,\"\"}".to_string(),
    };
    format!(
        "{{12,{{{id},{ns}}},0,0,0,1,{name},{title},{{1,0}},1,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{4,{picture},0,0,0,{{1,0}},{{3,4,{{0}}}},\
         {{3,0,{{0}},0,1,0,{appearance}}},0,0,{{0,1,0}},0,100}},1,{context_menu},1,2,\
         {{1,{title},0}},0,1,{tooltip},1,0,0,1,0,3,3,0,0}}",
        id = decoration.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(decoration.name),
        title = decoration.title,
        appearance = DEFAULT_APPEARANCE_UUID,
        context_menu = format_field_context_menu(
            decoration.context_menu_id,
            decoration.context_menu_name
        ),
        tooltip = format_extended_tooltip(
            decoration.extended_tooltip_id,
            decoration.extended_tooltip_name
        ),
    )
}

/// The namespace a form *command* id lives in, which is not the item one.
pub(crate) const FORM_COMMAND_NAMESPACE_UUID: &str = "409b9a53-7f7e-4178-86c1-33176c7c7a7a";

/// The `{9,…}` record of a form attribute.
///
/// The sixteen-member shape, which 7 073 of the 11 700 wrapper-9 records of
/// the first 1 500 ERP УХ form bodies carry. `type_pattern` arrives already
/// formatted -- `{"Pattern"}` for an attribute the form does not type, and the
/// pattern with its reference list for one it does.
pub(crate) fn format_form_attribute(
    id: &str,
    name: &str,
    title: &str,
    type_pattern: &str,
) -> String {
    format!(
        "{{9,{{{id}}},0,{name},{title},{type_pattern},{{0,{{0,{{\"B\",1}},0}}}},\
         {{0,{{0,{{\"B\",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,0}},{{0,0}}}}",
        name = quoted(name),
    )
}

/// The `{9,…}` record of a form command.
///
/// The nineteen-member shape, told apart from an attribute by the namespace in
/// its id tuple. `action` is the tuple that names what the command runs.
pub(crate) fn format_form_command(
    id: &str,
    name: &str,
    title: &str,
    tooltip_title: &str,
    action: &str,
    picture: &str,
    handler: &str,
) -> String {
    format!(
        "{{9,{{{id},{ns}}},{name},{title},{tooltip_title},{{0,{{0,{{\"B\",1}},0}}}},{action},\
         {picture},{handler},3,0,0,{{0,0}},1,0,1,0,0,1}}",
        ns = FORM_COMMAND_NAMESPACE_UUID,
        name = quoted(name),
        handler = quoted(handler),
    )
}

/// How a `<UsualGroup>` arranges what it holds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupArrangement {
    /// `<Group>Vertical</Group>`.
    Vertical,
    /// `<Group>Horizontal</Group>`.
    Horizontal,
    /// `<Group>AlwaysHorizontal</Group>`, which differs from `Horizontal` in
    /// one of the two slots that carry it.
    AlwaysHorizontal,
    /// `<Group>HorizontalIfPossible</Group>`, which is also what a group that
    /// names no arrangement at all carries.
    HorizontalIfPossible,
}

/// How a `<UsualGroup>` behaves.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupBehavior {
    Usual,
    Collapsible,
    PopUp,
    /// What a group that names no behaviour carries, which is not `Usual`.
    Unnamed,
}

/// How a `<UsualGroup>` separates itself from what is around it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroupSeparation {
    None,
    Strong,
    /// Also what a group that names no representation carries.
    Weak,
    Normal,
}

/// The `{29,…}` payload of a usual group, member by member.
///
/// Seven of its twenty-nine members carry an XML property. Measured the same
/// way as the field payloads: a candidate that fills these seven and copies
/// the rest reproduces **every one** of the usual-group payloads it was run
/// against exactly.
///
/// The refinement that got it there was about defaults, not about which slot
/// is which: a group that names no `<Group>` carries what
/// `HorizontalIfPossible` carries, a group that names no `<Representation>`
/// carries what `WeakSeparation` carries, and a group that names no
/// `<Behavior>` carries something that is *not* what `Usual` carries.
pub(crate) struct NativeUsualGroupPayload<'a> {
    pub(crate) separation: NativeGroupSeparation,
    pub(crate) behavior: NativeGroupBehavior,
    /// Slot 19: `Use` -> 0, `DontUse` -> 1, absent -> 2.
    pub(crate) through_align: Option<bool>,
    pub(crate) arrangement: NativeGroupArrangement,
    /// Slot 5, the group's own picture binding, `{0}` by default.
    pub(crate) picture: &'a str,
    /// Slot 9, the background colour, `{3,4,{0}}` by default.
    pub(crate) back_color: &'a str,
    /// Slot 14, the group's title, `{1,0}` by default.
    pub(crate) title: &'a str,
    /// Slot 4, which varies 0/1 over the corpus and whose property is not read
    /// yet, so the caller supplies it.
    pub(crate) fourth: &'a str,
}

impl NativeUsualGroupPayload<'_> {
    /// What a group that names none of these carries.
    pub(crate) const fn plain() -> Self {
        Self {
            separation: NativeGroupSeparation::Weak,
            behavior: NativeGroupBehavior::Unnamed,
            through_align: None,
            arrangement: NativeGroupArrangement::HorizontalIfPossible,
            picture: "{0}",
            back_color: "{3,4,{0}}",
            title: "{1,0}",
            fourth: "0",
        }
    }
}

pub(crate) fn format_usual_group_payload(payload: &NativeUsualGroupPayload<'_>) -> String {
    let separation = match payload.separation {
        NativeGroupSeparation::None => "0",
        NativeGroupSeparation::Strong => "1",
        NativeGroupSeparation::Weak => "2",
        NativeGroupSeparation::Normal => "3",
    };
    let (collapsible, behavior, behavior_tail) = match payload.behavior {
        NativeGroupBehavior::Usual => ("0", "0", "0"),
        NativeGroupBehavior::Collapsible => ("1", "1", "1"),
        NativeGroupBehavior::PopUp => ("1", "2", "2"),
        NativeGroupBehavior::Unnamed => ("1", "0", "3"),
    };
    let through_align = match payload.through_align {
        Some(true) => "0",
        Some(false) => "1",
        None => "2",
    };
    let (arrangement, arrangement_tail) = match payload.arrangement {
        NativeGroupArrangement::Vertical => ("0", "0"),
        NativeGroupArrangement::Horizontal => ("1", "1"),
        NativeGroupArrangement::AlwaysHorizontal => ("1", "3"),
        NativeGroupArrangement::HorizontalIfPossible => ("2", "2"),
    };
    format!(
        "{{29,{arrangement},0,{separation},{fourth},{picture},{{1,0}},{{\"Pattern\"}},\"\",{back_color},{collapsible},0,0,1,{title},0,0,3,3,{through_align},0,1,{arrangement},{{3,4,{{0}}}},{behavior},2,0,{arrangement_tail},{behavior_tail}}}",
        fourth = payload.fourth,
        picture = payload.picture,
        back_color = payload.back_color,
        title = payload.title,
    )
}

/// The `{2,…}` payload of a button group.
///
/// Four members, one of which carries `<Representation>`. All 21 651 of the
/// corpus are reproduced exactly.
pub(crate) fn format_button_group_payload(command_source: &str, representation: Option<&str>) -> String {
    let representation = match representation {
        Some("Usual") => "1",
        Some("Compact") => "2",
        _ => "0",
    };
    format!("{{2,{command_source},2,{representation}}}")
}

/// The `{1,…}` payload of a command bar.
///
/// Three members, one of which carries `<HorizontalLocation>`. All 3 233 of
/// the corpus are reproduced exactly.
pub(crate) fn format_command_bar_payload(horizontal_location: Option<&str>, command_source: &str) -> String {
    let location = match horizontal_location {
        Some("Auto") => "3",
        Some("Right") => "2",
        Some("Center") => "1",
        _ => "0",
    };
    format!("{{1,{location},{command_source}}}")
}

/// The `{4,…}` payload of a `<Pages>` group.
///
/// Six members, two of which carry `<PagesRepresentation>` -- the same value
/// in both, except that a group naming none carries 1 in the first and 6 in
/// the second. All 4 329 of the corpus are reproduced exactly.
pub(crate) fn format_pages_payload(representation: Option<&str>, events: &str, fourth: &str) -> String {
    let code = |value: Option<&str>| match value {
        Some("None") => "0",
        Some("TabsOnTop") => "1",
        Some("TabsOnBottom") => "2",
        Some("TabsOnLeftHorizontal") => "3",
        Some("Swipe") => "5",
        _ => "",
    };
    let first = match code(representation) {
        "" => "1",
        value => value,
    };
    let second = match code(representation) {
        "" => "6",
        value => value,
    };
    format!("{{4,{first},{events},2,{fourth},{second}}}")
}

/// The `{7,…}` payload of a `<Popup>`.
///
/// Nine members, one of which carries `<Representation>`; a popup that names
/// none carries 3. All 10 642 of the corpus are reproduced exactly.
pub(crate) fn format_popup_payload(
    picture: &str,
    command_source: &str,
    representation: Option<&str>,
    back_color: &str,
    border_color: &str,
) -> String {
    let representation = match representation {
        Some("Text") => "0",
        Some("Picture") => "1",
        Some("PictureAndText") => "2",
        _ => "3",
    };
    format!("{{7,{picture},{command_source},2,{representation},0,0,{back_color},{border_color}}}")
}

/// The `{2,…}` payload of a `<ColumnGroup>` -- the same wrapper a button group
/// carries, with twelve members instead of four.
///
/// One of them carries `<Group>`: `Horizontal` is 0, `InCell` is 2, and a
/// group naming none carries 1. All 7 076 of the corpus are reproduced
/// exactly.
pub(crate) fn format_column_group_payload(
    group: Option<&str>,
    second: &str,
    third: &str,
    fourth: &str,
    picture: &str,
    back_color: &str,
    title: &str,
) -> String {
    let group = match group {
        Some("Horizontal") => "0",
        Some("InCell") => "2",
        _ => "1",
    };
    format!(
        "{{2,{group},{second},{third},{fourth},{picture},{back_color},{{0}},{{\"Pattern\"}},\"\",{title},0}}"
    )
}

/// What a `{18,…}` page payload carries.
///
/// Three of its twenty members carry `<Group>`: one says whether the page
/// names an arrangement at all, and two carry which one, differing on
/// `AlwaysHorizontal`. All 11 804 of the corpus are reproduced exactly.
pub(crate) struct NativePagePayload<'a> {
    pub(crate) group: Option<&'a str>,
    pub(crate) picture: &'a str,
    pub(crate) third: &'a str,
    pub(crate) data_path: &'a str,
    pub(crate) title: &'a str,
    pub(crate) sixth: &'a str,
    pub(crate) back_color: &'a str,
    pub(crate) tenth: &'a str,
    pub(crate) eleventh: &'a str,
    pub(crate) twelfth: &'a str,
    pub(crate) thirteenth: &'a str,
    pub(crate) fourteenth: &'a str,
    pub(crate) fifteenth: &'a str,
    pub(crate) border_color: &'a str,
    pub(crate) font: &'a str,
}

impl NativePagePayload<'_> {
    pub(crate) const fn plain() -> Self {
        Self {
            group: None,
            picture: "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
            third: "0",
            data_path: "{0}",
            title: "{1,0}",
            sixth: "1",
            back_color: "{3,4,{0}}",
            tenth: "0",
            eleventh: "0",
            twelfth: "3",
            thirteenth: "3",
            fourteenth: "0",
            fifteenth: "0",
            border_color: "{3,4,{0}}",
            font: "{7,3,0,1,100}",
        }
    }
}

pub(crate) fn format_page_payload(payload: &NativePagePayload<'_>) -> String {
    let named = u8::from(payload.group.is_some());
    let (horizontal, horizontal_tail) = match payload.group {
        Some("Horizontal") => ("1", "1"),
        Some("AlwaysHorizontal") => ("1", "3"),
        Some("HorizontalIfPossible") => ("2", "2"),
        Some("Vertical") | None => ("0", "0"),
        Some(_) => ("0", "0"),
    };
    format!(
        "{{18,{picture},{named},{third},{data_path},{title},{sixth},{{\"Pattern\"}},\"\",{back_color},{tenth},{eleventh},{twelfth},{thirteenth},{fourteenth},{fifteenth},{horizontal},{horizontal_tail},{border_color},{font}}}",
        picture = payload.picture,
        third = payload.third,
        data_path = payload.data_path,
        title = payload.title,
        sixth = payload.sixth,
        back_color = payload.back_color,
        tenth = payload.tenth,
        eleventh = payload.eleventh,
        twelfth = payload.twelfth,
        thirteenth = payload.thirteenth,
        fourteenth = payload.fourteenth,
        fifteenth = payload.fifteenth,
        border_color = payload.border_color,
        font = payload.font,
    )
}

/// What a `{22,…}` group record needs beyond the frame every group shares.
///
/// `kind` is the marker that says which group this is -- 0 command-bar group,
/// 1 submenu, 2 ordinary group, 3 pages, 4 page, 5 usual group, 6 button
/// group, 7 navigator -- and it decides the shape of `payload`. Censused over
/// all 12 515 ERP УХ form bodies: each kind has exactly one payload structure
/// with a fixed member count (29 for kind 5, 20 for kind 4, 4 for kind 6, and
/// so on), varying only in a handful of scalar slots. The payload arrives
/// already formatted for the same reason a field's data path does: the slot
/// that carries an XML property is named where that property is read, not
/// here.
pub(crate) struct NativeGroupItem<'a> {
    pub(crate) id: &'a str,
    pub(crate) kind: u8,
    pub(crate) name: &'a str,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    pub(crate) tooltip_title: &'a str,
    /// The frame's four varying members, in the order they appear.
    pub(crate) frame: NativeGroupFrame,
    pub(crate) payload: &'a str,
    /// `(group uuid, child record)` in the order the body stores them.
    pub(crate) children: &'a [(&'a str, String)],
    /// Already formatted -- see [`format_extended_tooltip`].
    pub(crate) extended_tooltip: &'a str,
}

/// The four members of the group frame that are not constant.
#[derive(Clone, Copy, Default)]
pub(crate) struct NativeGroupFrame {
    pub(crate) first: u32,
    pub(crate) second: u32,
    pub(crate) third: u32,
    pub(crate) fourth: u32,
}

impl NativeGroupFrame {
    /// The values 68 645 of the 68 836 kind-5 records carry.
    pub(crate) const fn usual() -> Self {
        Self {
            first: 0,
            second: 0,
            third: 2,
            fourth: 2,
        }
    }
}

/// The `{22,…}` record of a group, with its children in place.
pub(crate) fn format_group_item(group: &NativeGroupItem<'_>) -> String {
    let mut children = String::new();
    for (group_uuid, record) in group.children {
        children.push(',');
        children.push_str(group_uuid);
        children.push(',');
        children.push_str(record);
    }
    format!(
        "{{22,{{{id},{ns}}},0,0,0,{kind},{name},{title},{tooltip_title},{first},1,0,{second},0,\
         {third},{fourth},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,{payload},{count}{children},\
         1,0,1,{tooltip},0,3,3,0}}",
        id = group.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        kind = group.kind,
        name = quoted(group.name),
        title = group.title,
        tooltip_title = group.tooltip_title,
        first = group.frame.first,
        second = group.frame.second,
        third = group.frame.third,
        fourth = group.frame.fourth,
        payload = group.payload,
        count = group.children.len(),
        tooltip = group.extended_tooltip,
    )
}

/// What the `{50,…}` root layout of a form body carries.
///
/// The childless root is 48 members. Over the roots of the first 600 ERP УХ
/// form bodies that carry no child item, forty of those members are the same
/// in every one; the eight below are not. A root that carries children appends
/// them after the command bar, which this writer does not do yet because the
/// count encoding there is not measured.
pub(crate) struct NativeRootLayout<'a> {
    /// Slot 9, which is `1` on a form that names no title and `0` on one that
    /// does.
    pub(crate) auto_title: bool,
    /// Already formatted -- see [`format_russian_title`].
    pub(crate) title: &'a str,
    /// Slot 17.
    pub(crate) seventeenth: u32,
    /// Slot 19: the form's own event bindings, or `{0,1,0}` when it has none.
    pub(crate) events: &'a str,
    /// Slot 20: the root `<CommandSet>`, `{0}` when the form excludes nothing.
    pub(crate) command_set: &'a str,
    /// Slot 22.
    pub(crate) command_bar: &'a str,
    /// The form's own child items, as `(kind uuid, record)` -- the same
    /// encoding a group uses, with the count first. See [`child_kind_uuid`].
    pub(crate) children: &'a [(&'a str, String)],
    /// Slot 29 of a childless root.
    pub(crate) twenty_ninth: u32,
    /// Slot 39 of a childless root.
    pub(crate) thirty_ninth: u32,
}

/// The `{50,…}` root layout of a form body.
pub(crate) fn format_root_layout(root: &NativeRootLayout<'_>) -> String {
    let mut children = String::new();
    for (kind_uuid, record) in root.children {
        children.push(',');
        children.push_str(kind_uuid);
        children.push(',');
        children.push_str(record);
    }
    format!(
        "{{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,{auto_title},{title},0,0,1,1,1,\
         0,{seventeenth},0,{events},{command_set},1,{command_bar},{count}{children},\"\",\"\",0,1,\
         \"\",{twenty_ninth},0,0,0,0,0,3,3,0,0,{thirty_ninth},100,1,1,0,0,0,{{50,0}},1}}",
        auto_title = u8::from(root.auto_title),
        title = root.title,
        seventeenth = root.seventeenth,
        events = root.events,
        command_set = root.command_set,
        command_bar = root.command_bar,
        count = root.children.len(),
        twenty_ninth = root.twenty_ninth,
        thirty_ninth = root.thirty_ninth,
    )
}

/// The `{5,…}` record of a table's search string, view status or search
/// control addition.
///
/// `kind` is `0` for the search string, `1` for the view status and `2` for
/// the search control, and the record closes by naming the table it belongs to
/// as `{<table id>,<kind>}`. The payload differs per kind -- the view status
/// carries five appearance blocks where the search string carries three -- so
/// it arrives already formatted, like every other slot whose XML property is
/// not yet named.
pub(crate) fn format_search_addition(
    id: &str,
    kind: u8,
    name: &str,
    payload: &str,
    context_menu: &str,
    extended_tooltip: &str,
    table_id: &str,
) -> String {
    format!(
        "{{5,{{{id},{ns}}},0,0,0,{kind},{name},{{1,0}},{{1,0}},1,1,0,1,{payload},1,\
         {context_menu},1,{extended_tooltip},2,{{{table_id},{kind}}},0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

/// What a `{55,…}` table record carries around its six nested children.
///
/// The head grows with what the table shows, but the tail does not. Measured
/// over the `{55,…}` records of the first 800 ERP УХ form bodies: the command
/// bar sits exactly two members after the context menu in all 344 of them, the
/// extended tooltip exactly 26 members from the end in 342, and the first
/// `{5,…}` addition exactly 21 from the end in 335. The three additions follow
/// each other two members apart and sixteen scalars close the record.
///
/// So the parts a caller supplies are the head and the scalar runs; this
/// writer owns the structure between them.
pub(crate) struct NativeTableItem<'a> {
    /// Everything from `55` up to the member before the context menu's flag,
    /// already joined with commas.
    pub(crate) head: &'a str,
    pub(crate) context_menu: &'a str,
    pub(crate) command_bar: &'a str,
    /// The twelve members between the command bar and the tooltip's flag.
    pub(crate) middle: &'a str,
    pub(crate) extended_tooltip: &'a str,
    /// Where the table says what it shows -- the search string, the view
    /// status and the search control -- which is exactly the three members
    /// between the tooltip and the first addition's flag. Measured over all
    /// 7 183 table records of the ERP УХ corpus: a candidate filling these
    /// three, with `<Representation>` and `<SkipOnInput>` which live in the
    /// head and the middle, reproduces every one of them exactly.
    pub(crate) search_string_location: Option<&'a str>,
    pub(crate) view_status_location: Option<&'a str>,
    pub(crate) search_control_location: Option<&'a str>,
    pub(crate) search_string_addition: &'a str,
    pub(crate) view_status_addition: &'a str,
    pub(crate) search_control_addition: &'a str,
    /// The sixteen members that close the record.
    pub(crate) tail: &'a str,
}

/// The codes a table writes for where it shows its search string.
const TABLE_SEARCH_STRING_CODES: &[(&str, &str)] = &[
    ("None", "1"),
    ("CommandBar", "2"),
    ("Top", "3"),
    ("Bottom", "4"),
    ("FormCaption", "5"),
    ("PullFromTop", "6"),
];

/// The codes for where it shows its view status.
const TABLE_VIEW_STATUS_CODES: &[(&str, &str)] = &[("None", "1"), ("Top", "2"), ("Bottom", "3")];

/// The codes for where it shows its search control.
const TABLE_SEARCH_CONTROL_CODES: &[(&str, &str)] = &[("None", "1"), ("CommandBar", "2")];

/// The code a table writes for one of those three; a table that names none
/// writes `0`.
fn table_location_code(
    location: Option<&str>,
    codes: &[(&'static str, &'static str)],
) -> &'static str {
    let Some(location) = location else {
        return "0";
    };
    codes
        .iter()
        .find_map(|(name, code)| (*name == location).then_some(*code))
        .unwrap_or("0")
}

/// The `{55,…}` record of a table, with its six children in place.
pub(crate) fn format_table_item(table: &NativeTableItem<'_>) -> String {
    format!(
        "{{{head},1,{context_menu},1,{command_bar},{middle},1,{tooltip},{search_string_location},{view_status_location},{search_control_location},\
         1,{search_string},1,{view_status},1,{search_control},{tail}}}",
        head = table.head,
        context_menu = table.context_menu,
        command_bar = table.command_bar,
        middle = table.middle,
        tooltip = table.extended_tooltip,
        search_string_location =
            table_location_code(table.search_string_location, TABLE_SEARCH_STRING_CODES),
        view_status_location =
            table_location_code(table.view_status_location, TABLE_VIEW_STATUS_CODES),
        search_control_location =
            table_location_code(table.search_control_location, TABLE_SEARCH_CONTROL_CODES),
        search_string = table.search_string_addition,
        view_status = table.view_status_addition,
        search_control = table.search_control_addition,
        tail = table.tail,
    )
}

/// The `{22,…}` record of an empty `<AutoCommandBar>`.
///
/// The one slot that separates it from a context menu is the marker `9`, and
/// the tail carries `{0,0,1}` where a context menu carries `{1,1}`.
pub(crate) fn format_empty_auto_command_bar(id: &str, name: &str) -> String {
    format!(
        "{{22,{{{id},{ns}}},0,0,0,9,{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{0,0,0}},1,{{0,0,1}},0,1,0,0,0,3,3,0}}",
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(name),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three label fields and the one input field of
    /// `Documents/Лот/Forms/ВыигранныеЛоты`, read out of the stored body.
    #[test]
    fn writes_the_field_records_the_platform_stores() {
        let period = format_field_item(&NativeFieldItem {
            id: "20",
            kind: 1,
            name: "ПериодЗакупок",
            visible_tuple: true,
            after_name: "1,0",
            data_path: "{2,{1},{3}}",
            payload: &format_plain_field_payload(false),
            context_menu_id: "21",
            context_menu_name: "ПериодЗакупокКонтекстноеМеню",
            extended_tooltip_id: "22",
            extended_tooltip_name: "ПериодЗакупокРасширеннаяПодсказка",
        });
        assert_eq!(
            period,
            "{37,{20,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"ПериодЗакупок\",1,0,{1,0},{1,0},{2,{1},{3}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},\"\",-1,-1,1,0,\"\"},{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{11,0,0,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0},{0,1,0},1,{22,{21,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ПериодЗакупокКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{\"Pattern\"},{\"Pattern\"},\"\",\"\",{0},0,0,1,{12,{22,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ПериодЗакупокРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},3,3,0,0,0,0}"
        );

        let supplier = format_field_item(&NativeFieldItem {
            id: "46",
            kind: 1,
            name: "АнкетаПоставщика",
            visible_tuple: true,
            after_name: "1,0",
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu_id: "47",
            context_menu_name: "АнкетаПоставщикаКонтекстноеМеню",
            extended_tooltip_id: "48",
            extended_tooltip_name: "АнкетаПоставщикаРасширеннаяПодсказка",
        });
        assert!(
            supplier.contains("{11,0,0,2,2,2,{1,0},1,"),
            "an editable field carries 1 in the slot a label carries 0"
        );
        assert!(supplier.starts_with(
            "{37,{46,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"АнкетаПоставщика\",1,0,{1,0},{1,0},{1,{2}},"
        ));
        assert!(supplier.ends_with(",3,3,0,0,0,0}"));

        // A field of another kind is the same frame with another kind slot and
        // another payload: the shortest spreadsheet document field of the
        // corpus, without the visibility tuple.
        let result = format_field_item(&NativeFieldItem {
            id: "9",
            kind: 6,
            name: "Результат",
            visible_tuple: false,
            after_name: "0,0",
            data_path: "{1,{3}}",
            payload: "{13,100,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,1,1,1,2}",
            context_menu_id: "10",
            context_menu_name: "РезультатКонтекстноеМеню",
            extended_tooltip_id: "12",
            extended_tooltip_name: "РезультатРасширеннаяПодсказка",
        });
        assert!(result.starts_with(
            "{37,{9,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,\"Результат\",0,0,{1,0},{1,0},{1,{3}},{0},1,0,2,"
        ));
        assert!(result.contains(
            ",{13,100,10,1,1,0,0,1,1,0,0,1,0,0,1,{3,4,{0}},1,1,{0,1,0},0,1,0,0,1,0,0,0,0,1,1,1,2},{0,1,0},1,"
        ));
    }

    #[test]
    fn writes_the_default_children_the_platform_stores() {
        assert_eq!(
            format_extended_tooltip("19", "ЛотРасширеннаяПодсказка"),
            "{12,{19,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ЛотРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0}"
        );
        assert_eq!(
            format_field_context_menu("18", "ЛотКонтекстноеМеню"),
            "{22,{18,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ЛотКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0}"
        );
    }

    /// The two search buttons of
    /// `Documents/Лот/Forms/ВыигранныеЛоты`'s list command bar, and the bar
    /// itself, exactly as that body stores them.
    #[test]
    fn writes_the_command_bar_records_the_platform_stores() {
        assert_eq!(
            format_standard_command_button(
                "26",
                "ФормаНайти",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "27",
                "ФормаНайтиРасширеннаяПодсказка",
            ),
            "{31,{26,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,{0,{0,{\"B\",1},0}},0,\"ФормаНайти\",{1,0},1,{1,c0519548-2a9a-44de-a25e-faf01e089d4d},{0},3,0,0,0,2,2,0,0,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},\"\",-1,-1,1,0,\"\"},1,{\"Pattern\"},\"\",2,0,1,{12,{27,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ФормаНайтиРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},{\"U\"},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}"
        );
        assert_eq!(
            format_standard_command_button(
                "28",
                "ФормаОтменитьПоиск",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "29",
                "ФормаОтменитьПоискРасширеннаяПодсказка",
            ),
            "{31,{28,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,{0,{0,{\"B\",1},0}},0,\"ФормаОтменитьПоиск\",{1,0},1,{1,44ad3ec9-f3c2-4913-9224-5f9fb6418743},{0},3,0,0,0,2,2,0,0,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},\"\",-1,-1,1,0,\"\"},1,{\"Pattern\"},\"\",2,0,1,{12,{29,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ФормаОтменитьПоискРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},{\"U\"},1,0,0,1,0,0,0,3,3,3,0,0,0,0,0,0,1,0}"
        );
        assert_eq!(
            format_empty_auto_command_bar("3", "СписокКоманднаяПанель"),
            "{22,{3,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"СписокКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0}"
        );
    }

    /// Two decorations of
    /// `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`, and the
    /// tooltip shape an item with a visibility tuple carries, exactly as those
    /// bodies store them.
    #[test]
    fn writes_the_decoration_records_the_platform_stores() {
        assert_eq!(
            format_label_decoration(&NativeLabelDecoration {
                id: "1195",
                name: "ДекорацияВниманиеТолькоПросмотр",
                title: &format_russian_title("Внимание"),
                picture_uuid: Some("188d8f0e-94da-44bb-8ed3-21aa01e973b9"),
                context_menu_id: "1196",
                context_menu_name: "ДекорацияВниманиеТолькоПросмотрКонтекстноеМеню",
                extended_tooltip_id: "1197",
                extended_tooltip_name: "ДекорацияВниманиеТолькоПросмотрРасширеннаяПодсказка",
            }),
            "{12,{1195,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,1,\"ДекорацияВниманиеТолькоПросмотр\",{1,1,{\"ru\",\"Внимание\"}},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{4,{4,1,{0,188d8f0e-94da-44bb-8ed3-21aa01e973b9},\"\",-1,-1,0,0,\"\"},0,0,0,{1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},0,0,{0,1,0},0,100},1,{22,{1196,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ДекорацияВниманиеТолькоПросмотрКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,2,{1,{1,1,{\"ru\",\"Внимание\"}},0},0,1,{12,{1197,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ДекорацияВниманиеТолькоПросмотрРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},1,0,0,1,0,3,3,0,0}"
        );

        assert_eq!(
            format_visible_extended_tooltip("6", "СценарийРасширеннаяПодсказка"),
            "{12,{6,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},0,\"СценарийРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0}"
        );

        assert_eq!(format_russian_title(""), "{1,0}");
        assert_eq!(format_russian_title("Внимание"), "{1,1,{\"ru\",\"Внимание\"}}");
    }

    /// One childless record of each group kind, exactly as an ERP УХ form
    /// body stores it. The six together prove the frame: everything outside
    /// the payload is the same in all of them.
    #[test]
    fn writes_one_record_of_every_group_kind() {
        let cases: [(u8, &str, &str, &str, &str, &str, &str, &str); 6] = [
            (
                0,
                "76",
                "ГруппаКоманднаяПанель",
                "Командная панель",
                "{1,0,{0,02023637-7868-4a5f-8576-835a76e0c9ba}}",
                "77",
                "ГруппаКоманднаяПанельРасширеннаяПодсказка",
                "{22,{76,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ГруппаКоманднаяПанель\",{1,1,{\"ru\",\"Командная панель\"}},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,0,{0,02023637-7868-4a5f-8576-835a76e0c9ba}},0,1,0,1,",
            ),
            (
                2,
                "461",
                "ГруппаВидКвоты",
                "",
                "{2,2,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0}",
                "462",
                "ГруппаВидКвотыРасширеннаяПодсказка",
                "{22,{461,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"ГруппаВидКвоты\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{2,2,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0},0,1,0,1,",
            ),
            (
                5,
                "11",
                "ГруппаШапка",
                "",
                "{29,1,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0}",
                "50",
                "ГруппаШапкаРасширеннаяПодсказка",
                "{22,{11,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,\"ГруппаШапка\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{29,1,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0},0,1,0,1,",
            ),
            (
                6,
                "193",
                "ГруппаКнопокДляПрисоединенныхФайлов",
                "",
                "{2,{0},2,0}",
                "194",
                "ГруппаКнопокДляПрисоединенныхФайловРасширеннаяПодсказка",
                "{22,{193,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,\"ГруппаКнопокДляПрисоединенныхФайлов\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{2,{0},2,0},0,1,0,1,",
            ),
            (
                1,
                "482",
                "ПодменюПечать",
                "",
                "{7,{4,1,{-13},\"\",-1,-1,1,0,\"\"},{0},2,3,0,0,{3,4,{0}},{3,4,{0}}}",
                "483",
                "ПодменюПечатьРасширеннаяПодсказка",
                "",
            ),
            (
                4,
                "375",
                "ШаблоныСтраница",
                "",
                "{18,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,{0},{1,0},1,{\"Pattern\"},\"\",{3,4,{0}},0,0,3,3,0,0,0,0,{3,4,{0}},{7,3,0,1,100}}",
                "376",
                "ШаблоныСтраницаРасширеннаяПодсказка",
                "",
            ),
        ];

        for (kind, id, name, title, payload, tooltip_id, tooltip_name, expected_head) in cases {
            let tooltip = format_extended_tooltip(tooltip_id, tooltip_name);
            let record = format_group_item(&NativeGroupItem {
                id,
                kind,
                name,
                title: &format_russian_title(title),
                tooltip_title: "{1,0}",
                frame: NativeGroupFrame::usual(),
                payload,
                children: &[],
                extended_tooltip: &tooltip,
            });
            if !expected_head.is_empty() {
                assert!(
                    record.starts_with(expected_head),
                    "kind {kind} head differs:\n{record}"
                );
            }
            assert!(record.ends_with(&format!("{tooltip},0,3,3,0}}")));
        }
    }

    /// A group writes its children as `(group uuid, record)` pairs after their
    /// count, which is what the list command bar of
    /// `Documents/Лот/Forms/ВыигранныеЛоты` stores.
    #[test]
    fn writes_a_group_with_its_children() {
        let tooltip = format_extended_tooltip("9", "ГруппаРасширеннаяПодсказка");
        let child = format_standard_command_button(
            "26",
            "ФормаНайти",
            "c0519548-2a9a-44de-a25e-faf01e089d4d",
            "27",
            "ФормаНайтиРасширеннаяПодсказка",
        );
        let record = format_group_item(&NativeGroupItem {
            id: "8",
            kind: 0,
            name: "Группа",
            title: "{1,0}",
            tooltip_title: "{1,0}",
            frame: NativeGroupFrame::usual(),
            payload: "{1,0,{0}}",
            children: &[("a9f3b1ac-f51b-431e-b102-55a69acdecad", child.clone())],
            extended_tooltip: &tooltip,
        });

        assert!(record.contains(&format!(
            ",1,a9f3b1ac-f51b-431e-b102-55a69acdecad,{child},1,0,1,"
        )));
    }

    /// An attribute and a command of ERP УХ form bodies, exactly as they are
    /// stored. Both are wrapper 9; only the namespace in the id tuple, and the
    /// member count that follows from it, tell them apart.
    #[test]
    fn writes_the_attribute_and_command_records_the_platform_stores() {
        assert_eq!(
            format_form_attribute("9", "ЦветаФона", "{1,0}", "{\"Pattern\"}"),
            "{9,{9},0,\"ЦветаФона\",{1,0},{\"Pattern\"},{0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}}"
        );
        assert_eq!(
            format_form_command(
                "10",
                "Команда9",
                "{1,0}",
                "{1,0}",
                "{0,57,0}",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "Команда9",
            ),
            "{9,{10,409b9a53-7f7e-4178-86c1-33176c7c7a7a},\"Команда9\",{1,0},{1,0},{0,{0,{\"B\",1},0}},{0,57,0},{4,0,{0},\"\",-1,-1,1,0,\"\"},\"Команда9\",3,0,0,{0,0},1,0,1,0,0,1}"
        );
    }

    /// Splits a record into its top-level members the way the body grammar
    /// does, so a test can say where a child sits.
    fn top_level_members(record: &str) -> Vec<String> {
        let inner = &record[1..record.len() - 1];
        let mut members = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        let mut in_string = false;
        for (index, ch) in inner.char_indices() {
            match ch {
                '"' => in_string = !in_string,
                _ if in_string => {}
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    members.push(inner[start..index].to_string());
                    start = index + ch.len_utf8();
                }
                _ => {}
            }
        }
        members.push(inner[start..].to_string());
        members
    }

    /// The table record keeps its six children where every ERP УХ table keeps
    /// them: the command bar two members after the context menu, the tooltip
    /// 26 from the end, and the three additions 21, 19 and 17 from the end.
    #[test]
    fn writes_a_table_record_with_its_children_where_the_platform_keeps_them() {
        let context_menu = format_field_context_menu("57", "ОтборКонтекстноеМеню");
        let command_bar = format_empty_auto_command_bar("58", "ОтборКоманднаяПанель");
        let tooltip = format_extended_tooltip("59", "ОтборРасширеннаяПодсказка");
        let record = format_table_item(&NativeTableItem {
            head: "55,{56,02023637-7868-4a5f-8576-835a76e0c9ba},0,2,0,\"Отбор\",{0}",
            context_menu: &context_menu,
            command_bar: &command_bar,
            middle: "0,2,2,1,0,{\"Pattern\"},\"\",\"\",2,2,0",
            extended_tooltip: &tooltip,
            search_string_location: None,
            view_status_location: None,
            search_control_location: None,
            search_string_addition: "{5,{60,x},0}",
            view_status_addition: "{5,{63,x},1}",
            search_control_addition: "{5,{66,x},2}",
            tail: "0,1,0,0,1,0,3,3,0,1,0,0,0,0,0,0",
        });

        let members = top_level_members(&record);
        let context_at = members
            .iter()
            .position(|member| member == &context_menu)
            .expect("the context menu is a member");
        let command_at = members
            .iter()
            .position(|member| member == &command_bar)
            .expect("the command bar is a member");
        let tooltip_at = members
            .iter()
            .position(|member| member == &tooltip)
            .expect("the tooltip is a member");
        let addition_at = members
            .iter()
            .position(|member| member == "{5,{60,x},0}")
            .expect("the search string addition is a member");

        assert_eq!(command_at - context_at, 2);
        assert_eq!(members.len() - tooltip_at, 26);
        assert_eq!(members.len() - addition_at, 21);
        assert_eq!(members[addition_at - 1], "1");
        assert_eq!(members[members.len() - 19], "{5,{63,x},1}");
        assert_eq!(members[members.len() - 17], "{5,{66,x},2}");
        assert_eq!(members.len() - (members.len() - 17) - 1, 16);

        // A table that names none of the three writes 0 for each; one that
        // names them writes the codes the corpus gives.
        assert_eq!(&members[members.len() - 25..members.len() - 22], &["0", "0", "0"]);
        let shown = format_table_item(&NativeTableItem {
            head: "55,{56,02023637-7868-4a5f-8576-835a76e0c9ba},0,2,0,\"Отбор\",{0}",
            context_menu: &context_menu,
            command_bar: &command_bar,
            middle: "0,2,2,1,0,{\"Pattern\"},\"\",\"\",2,2,0",
            extended_tooltip: &tooltip,
            search_string_location: Some("PullFromTop"),
            view_status_location: Some("None"),
            search_control_location: Some("CommandBar"),
            search_string_addition: "{5,{60,x},0}",
            view_status_addition: "{5,{63,x},1}",
            search_control_addition: "{5,{66,x},2}",
            tail: "0,1,0,0,1,0,3,3,0,1,0,0,0,0,0,0",
        });
        let shown = top_level_members(&shown);
        assert_eq!(&shown[shown.len() - 25..shown.len() - 22], &["6", "1", "2"]);
    }

    /// The three additions of the `Отбор` table of an ERP УХ form body, each
    /// exactly as that body stores it.
    #[test]
    fn writes_the_search_additions_the_platform_stores() {
        let search_string = format_search_addition(
            "60",
            0,
            "ОтборСтрокаПоиска",
            "{1,0,2,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0}",
            &format_field_context_menu("61", "ОтборСтрокаПоискаКонтекстноеМеню"),
            &format_extended_tooltip("62", "ОтборСтрокаПоискаРасширеннаяПодсказка"),
            "56",
        );
        assert_eq!(
            search_string,
            "{5,{60,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ОтборСтрокаПоиска\",{1,0},{1,0},1,1,0,1,{1,0,2,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0},1,{22,{61,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ОтборСтрокаПоискаКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{12,{62,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ОтборСтрокаПоискаРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},2,{56,0},0,3,3,0}"
        );

        let search_control = format_search_addition(
            "66",
            2,
            "ОтборУправлениеПоиском",
            "{1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,2}",
            &format_field_context_menu("67", "ОтборУправлениеПоискомКонтекстноеМеню"),
            &format_extended_tooltip("68", "ОтборУправлениеПоискомРасширеннаяПодсказка"),
            "56",
        );
        assert!(search_control.starts_with(
            "{5,{66,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"ОтборУправлениеПоиском\",{1,0},{1,0},1,1,0,1,"
        ));
        assert!(search_control.ends_with(",2,{56,2},0,3,3,0}"));
    }

    /// Two childless root layouts of ERP УХ form bodies -- one that names a
    /// title and no event, one that names an event and no title -- exactly as
    /// those bodies store them.
    #[test]
    fn writes_the_root_layouts_the_platform_stores() {
        let bar = format_empty_auto_command_bar("-1", "ФормаКоманднаяПанель");

        assert_eq!(
            format_root_layout(&NativeRootLayout {
                auto_title: false,
                title: "{1,2,{\"ru\",\"Заявление на подключение\"},{\"en\",\"Заявление на подключение\"}}",
                seventeenth: 1,
                events: "{0,1,0}",
                command_set: "{0}",
                command_bar: &bar,
                children: &[],
                twenty_ninth: 2,
                thirty_ninth: 2,
            }),
            "{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,2,{\"ru\",\"Заявление на подключение\"},{\"en\",\"Заявление на подключение\"}},0,0,1,1,1,0,1,0,{0,1,0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"ФормаКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0},0,\"\",\"\",0,1,\"\",2,0,0,0,0,0,3,3,0,0,2,100,1,1,0,0,0,{50,0},1}"
        );

        assert_eq!(
            format_root_layout(&NativeRootLayout {
                auto_title: true,
                title: "{1,0}",
                seventeenth: 0,
                events: "{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"ПриОткрытии\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1}",
                command_set: "{0}",
                command_bar: &bar,
                children: &[],
                twenty_ninth: 2,
                thirty_ninth: 2,
            }),
            "{50,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"ПриОткрытии\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"ФормаКоманднаяПанель\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0},0,\"\",\"\",0,1,\"\",2,0,0,0,0,0,3,3,0,0,2,100,1,1,0,0,0,{50,0},1}"
        );
    }

    /// A root writes its children the way a group does: the count, then a
    /// `(kind uuid, record)` pair each. The kind uuid says what the record is,
    /// one per wrapper with no overlap.
    #[test]
    fn writes_a_root_with_its_children() {
        assert_eq!(child_kind_uuid(37), Some("77ffcc29-7f2d-4223-b22f-19666e7250ba"));
        assert_eq!(child_kind_uuid(22), Some("cd5394d0-7dda-4b56-8927-93ccbe967a01"));
        assert_eq!(child_kind_uuid(31), Some("a9f3b1ac-f51b-431e-b102-55a69acdecad"));
        assert_eq!(child_kind_uuid(12), Some("3d3cb80c-508b-41fa-8a18-680cdf5f1712"));
        assert_eq!(child_kind_uuid(55), Some("143c00f7-a42d-4cd7-9189-88e4467dc768"));
        assert_eq!(child_kind_uuid(5), Some("c5259a1d-518a-4afd-b98d-0176027e4feb"));
        assert_eq!(child_kind_uuid(50), None);

        let bar = format_empty_auto_command_bar("-1", "FormCommandBar");
        let field = format_field_item(&NativeFieldItem {
            id: "1",
            kind: 2,
            name: "A",
            visible_tuple: false,
            after_name: "1,0",
            data_path: "{1,{2}}",
            payload: &format_plain_field_payload(true),
            context_menu_id: "2",
            context_menu_name: "AM",
            extended_tooltip_id: "3",
            extended_tooltip_name: "AT",
        });
        let root = format_root_layout(&NativeRootLayout {
            auto_title: true,
            title: "{1,0}",
            seventeenth: 0,
            events: "{0,1,0}",
            command_set: "{0}",
            command_bar: &bar,
            children: &[(child_kind_uuid(37).expect("a field has a kind uuid"), field.clone())],
            twenty_ninth: 0,
            thirty_ninth: 0,
        });

        assert!(root.contains(&format!(
            ",1,77ffcc29-7f2d-4223-b22f-19666e7250ba,{field},\"\",\"\",0,1,"
        )));
        assert!(root.ends_with(",0,0,0,{50,0},1}"));
    }

    /// Four label payloads of ERP УХ form bodies, each carrying a different
    /// set of the five properties the mapping claims, exactly as stored.
    #[test]
    fn writes_the_label_payloads_the_platform_stores() {
        // A label that names only a height: the height lands in slot 2.
        assert_eq!(
            format_label_payload(&NativeLabelPayload { height: "2", ..NativeLabelPayload::plain(false) }),
            "{11,0,2,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0}"
        );
        // Width and height together.
        assert_eq!(
            format_label_payload(&NativeLabelPayload {
                width: "16",
                height: "1",
                ..NativeLabelPayload::plain(false)
            }),
            "{11,16,1,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0}"
        );
        // `HorizontalStretch` is three-valued: false, true, and not named.
        assert!(format_label_payload(&NativeLabelPayload {
            horizontal_stretch: Some(false),
            ..NativeLabelPayload::plain(false)
        })
        .starts_with("{11,0,0,0,"));
        assert!(format_label_payload(&NativeLabelPayload {
            horizontal_stretch: Some(true),
            ..NativeLabelPayload::plain(false)
        })
        .starts_with("{11,0,0,1,"));
        assert!(format_label_payload(&NativeLabelPayload::plain(false)).starts_with("{11,0,0,2,"));
        // A maximum width switches the auto flag off and lands in slot 16.
        assert!(format_label_payload(&NativeLabelPayload {
            auto_max_width: false,
            max_width: "15",
            ..NativeLabelPayload::plain(false)
        })
        .ends_with(",0,15,0,1,0}"));
    }

    /// The payload 24 361 input fields of the corpus carry unchanged, and the
    /// three slots a form most often moves.
    #[test]
    fn writes_the_input_payloads_the_platform_stores() {
        assert_eq!(
            format_input_payload(&NativeInputPayload::plain()),
            "{36,{3,0},0,0,2,2,1,2,2,2,2,2,2,2,2,2,{\"U\"},{\"U\"},\"\",0,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,2,3,00000000-0000-0000-0000-000000000000,{5006,0},{0,0},2,{1,0},{1,0},2,1,0,{\"Pattern\"},1,{0,1,0},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,{3,0,0},0,{1,0},2,0,2,0,1,0,0,1,0,0,0,0,0,0,0,0,0,{0},0,{5007,0},0}"
        );
        assert!(format_input_payload(&NativeInputPayload {
            width: "25",
            ..NativeInputPayload::plain()
        })
        .starts_with("{36,{3,0},25,0,2,2,"));
        assert!(format_input_payload(&NativeInputPayload {
            auto_max_width: false,
            max_width: "28",
            ..NativeInputPayload::plain()
        })
        .contains(",2,0,2,0,0,28,0,1,0,"));
        assert!(format_input_payload(&NativeInputPayload {
            mask: "999-999-999 99",
            ..NativeInputPayload::plain()
        })
        .contains(",{\"U\"},\"999-999-999 99\",0,"));
    }

    /// The payload of `ГруппаШапка`, a usual group of an ERP УХ form body,
    /// exactly as that body stores it, and the defaults a group that names
    /// nothing carries.
    #[test]
    fn writes_the_usual_group_payloads_the_platform_stores() {
        assert_eq!(
            format_usual_group_payload(&NativeUsualGroupPayload {
                behavior: NativeGroupBehavior::Usual,
                arrangement: NativeGroupArrangement::HorizontalIfPossible,
                separation: NativeGroupSeparation::Weak,
                fourth: "1",
                ..NativeUsualGroupPayload::plain()
            }),
            "{29,2,0,2,1,{0},{1,0},{\"Pattern\"},\"\",{3,4,{0}},0,0,0,1,{1,0},0,0,3,3,2,0,1,2,{3,4,{0}},0,2,0,2,0}"
        );
        // A group that names no arrangement carries what `HorizontalIfPossible`
        // carries, and one that names no behaviour carries what `Usual` does
        // not.
        let plain = format_usual_group_payload(&NativeUsualGroupPayload::plain());
        assert!(plain.starts_with("{29,2,0,2,0,"));
        assert!(plain.ends_with(",1,2,{3,4,{0}},0,2,0,2,3}"));
        let vertical = format_usual_group_payload(&NativeUsualGroupPayload {
            arrangement: NativeGroupArrangement::Vertical,
            ..NativeUsualGroupPayload::plain()
        });
        assert!(vertical.starts_with("{29,0,0,2,0,"));
        assert!(vertical.ends_with(",0,3}"));
    }

    /// The check-box payload 9 906 records carry unchanged, the radio-button
    /// payload, and the slots that carry each field's type.
    #[test]
    fn writes_the_check_box_and_radio_payloads_the_platform_stores() {
        assert_eq!(
            format_check_box_payload(&NativeCheckBoxPayload::plain()),
            "{11,0,{3,4,{0}},{3,4,{0}},0,{1,0},{3,4,{0}},{7,3,0,1,100},0,0,0,2,0}"
        );
        // `Auto` and `Switcher` share the first slot and differ in the last.
        assert!(format_check_box_payload(&NativeCheckBoxPayload {
            check_box_type: NativeCheckBoxType::Switcher,
            ..NativeCheckBoxPayload::plain()
        })
        .ends_with(",0,0,0,2,3}"));
        assert!(format_check_box_payload(&NativeCheckBoxPayload {
            check_box_type: NativeCheckBoxType::Tumbler,
            ..NativeCheckBoxPayload::plain()
        })
        .contains("{3,4,{0}},2,{1,0},"));

        assert_eq!(
            format_radio_button_payload(&NativeRadioButtonPayload::plain()),
            "{8,{3,0},1,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,0,{3,4,{0}},0,0,2}"
        );
        assert!(format_radio_button_payload(&NativeRadioButtonPayload {
            columns: "3",
            radio_button_type: NativeRadioButtonType::Tumbler,
            ..NativeRadioButtonPayload::plain()
        })
        .starts_with("{8,{3,0},3,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,2,"));
    }

    /// The default payload of each of the six group kinds the corpus closed
    /// in one pass, exactly as those bodies store them.
    #[test]
    fn writes_the_remaining_group_payloads_the_platform_stores() {
        assert_eq!(format_button_group_payload("{0}", None), "{2,{0},2,0}");
        assert_eq!(format_button_group_payload("{0}", Some("Compact")), "{2,{0},2,2}");
        assert_eq!(format_command_bar_payload(None, "{0}"), "{1,0,{0}}");
        assert_eq!(format_command_bar_payload(Some("Auto"), "{0}"), "{1,3,{0}}");
        assert_eq!(format_pages_payload(None, "{0,1,0}", "0"), "{4,1,{0,1,0},2,0,6}");
        assert_eq!(
            format_pages_payload(Some("None"), "{0,1,0}", "0"),
            "{4,0,{0,1,0},2,0,0}"
        );
        assert_eq!(
            format_popup_payload(
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "{0}",
                None,
                "{3,4,{0}}",
                "{3,4,{0}}"
            ),
            "{7,{4,0,{0},\"\",-1,-1,1,0,\"\"},{0},2,3,0,0,{3,4,{0}},{3,4,{0}}}"
        );
        assert_eq!(
            format_column_group_payload(
                None,
                "1",
                "0",
                "3",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "{3,4,{0}}",
                "{1,0}"
            ),
            "{2,1,1,0,3,{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{0},{\"Pattern\"},\"\",{1,0},0}"
        );
        assert_eq!(
            format_page_payload(&NativePagePayload::plain()),
            "{18,{4,0,{0},\"\",-1,-1,1,0,\"\"},0,0,{0},{1,0},1,{\"Pattern\"},\"\",{3,4,{0}},0,0,3,3,0,0,0,0,{3,4,{0}},{7,3,0,1,100}}"
        );
        // `AlwaysHorizontal` and `Horizontal` differ in the second of the two
        // slots that carry the arrangement.
        assert!(format_page_payload(&NativePagePayload {
            group: Some("AlwaysHorizontal"),
            ..NativePagePayload::plain()
        })
        .contains(",0,0,1,3,{3,4,{0}},"));
        assert!(format_page_payload(&NativePagePayload {
            group: Some("Horizontal"),
            ..NativePagePayload::plain()
        })
        .contains(",0,0,1,1,{3,4,{0}},"));
    }

    /// The document and picture payloads, exactly as ERP УХ bodies store
    /// them.
    #[test]
    fn writes_the_document_and_picture_payloads_the_platform_stores() {
        assert_eq!(
            format_document_payload(3, "50", "10", "{3,4,{0}},0,{0,1,0},1,0,0,1,0,1,1"),
            "{3,50,10,{3,4,{0}},0,{0,1,0},1,0,0,1,0,1,1}"
        );
        assert_eq!(
            format_document_payload(5, "50", "10", "1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,0,0,1,0,{0,1,0}"),
            "{5,50,10,1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},1,0,0,1,0,{0,1,0}}"
        );
        assert_eq!(
            format_document_payload(1, "50", "10", "1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,1,0"),
            "{1,50,10,1,1,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,1,0},1,0,0,1,0}"
        );

        // The picture size spellings the second round added.
        let picture = |size: Option<&str>| {
            format_picture_payload(
                "2", "0", size, "0", "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}", "{1,0}", "{3,4,{0}}", "{3,4,{0}}",
                "{7,3,0,1,100}", "{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}",
                "{0,1,0}", "1", "1", "1", "1", "0",
            )
        };
        assert!(picture(None).contains("-1,-1,1,0,\"\"},0,0,0,{1,0},"));
        assert!(picture(Some("ByFontSize")).contains("-1,-1,1,0,\"\"},7,0,0,{1,0},"));
        assert!(picture(Some("AutoSizeIgnoreScale")).contains("-1,-1,1,0,\"\"},6,0,0,{1,0},"));

        // A spreadsheet field that names neither scroll bar carries 2 in both
        // of the slots that hold them.
        let sheet = |vertical, horizontal| {
            format_spreadsheet_payload(
                "50", "10", "0", "0", vertical, horizontal,
                "1,1,0", "0,0", "1,1",
            )
        };
        assert!(sheet(None, None).ends_with(",2,2,1,2}"));
        assert!(sheet(Some(true), Some(false)).ends_with(",1,0,1,2}"));
    }

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }
}
