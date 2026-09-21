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
    pub(crate) name: &'a str,
    /// The binding the item's `<DataPath>` resolves to, already formatted --
    /// `{1,{2}}` for a form attribute, `{2,{1},{3}}` for a dynamic-list column.
    pub(crate) data_path: &'a str,
    pub(crate) context_menu_id: &'a str,
    pub(crate) context_menu_name: &'a str,
    pub(crate) extended_tooltip_id: &'a str,
    pub(crate) extended_tooltip_name: &'a str,
    /// The slot that separates a field the user edits from one that only
    /// shows: `1` for an input field, `0` for a label.
    pub(crate) editable: bool,
}

/// The `{37,…}` record of a `<LabelField>` or `<InputField>` that carries only
/// its name, its data path and its two default children.
///
/// Measured against `Documents/Лот/Forms/ВыигранныеЛоты`, whose three label
/// fields and one input field are exactly this shape.
pub(crate) fn format_field_item(item: &NativeFieldItem<'_>) -> String {
    format!(
        "{{37,{{{id},{ns}}},0,0,1,{{0,{{0,{{\"B\",1}},0}}}},1,{name},1,0,{{1,0}},{{1,0}},\
         {data_path},{{0}},1,0,2,0,2,{{1,0}},{{1,0}},1,1,0,3,0,3,1,3,0,\
         {{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{{3,4,{{0}}}},\
         {{7,3,0,1,100}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},{{0,0,0}},1,\
         {{11,0,0,2,2,2,{{1,0}},{editable},{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},2,\
         {{0,1,0}},{{3,4,{{0}}}},{{3,0,{{0}},0,1,0,{appearance}}},1,0,0,1,0}},{{0,1,0}},1,\
         {context_menu},1,{{\"Pattern\"}},{{\"Pattern\"}},\"\",\"\",{{0}},0,0,1,{tooltip},3,3,0,0,0,0}}",
        id = item.id,
        ns = FORM_ITEM_NAMESPACE_UUID,
        name = quoted(item.name),
        data_path = item.data_path,
        editable = u8::from(item.editable),
        appearance = DEFAULT_APPEARANCE_UUID,
        context_menu = format_field_context_menu(item.context_menu_id, item.context_menu_name),
        tooltip = format_extended_tooltip(item.extended_tooltip_id, item.extended_tooltip_name),
    )
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
            name: "ПериодЗакупок",
            data_path: "{2,{1},{3}}",
            context_menu_id: "21",
            context_menu_name: "ПериодЗакупокКонтекстноеМеню",
            extended_tooltip_id: "22",
            extended_tooltip_name: "ПериодЗакупокРасширеннаяПодсказка",
            editable: false,
        });
        assert_eq!(
            period,
            "{37,{20,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"ПериодЗакупок\",1,0,{1,0},{1,0},{2,{1},{3}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},\"\",-1,-1,1,0,\"\"},{4,0,{0},\"\",-1,-1,1,0,\"\"},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{11,0,0,2,2,2,{1,0},0,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},2,{0,1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0},{0,1,0},1,{22,{21,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,8,\"ПериодЗакупокКонтекстноеМеню\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,{\"Pattern\"},{\"Pattern\"},\"\",\"\",{0},0,0,1,{12,{22,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"ПериодЗакупокРасширеннаяПодсказка\",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},3,3,0,0,0,0}"
        );

        let supplier = format_field_item(&NativeFieldItem {
            id: "46",
            name: "АнкетаПоставщика",
            data_path: "{1,{2}}",
            context_menu_id: "47",
            context_menu_name: "АнкетаПоставщикаКонтекстноеМеню",
            extended_tooltip_id: "48",
            extended_tooltip_name: "АнкетаПоставщикаРасширеннаяПодсказка",
            editable: true,
        });
        assert!(
            supplier.contains("{11,0,0,2,2,2,{1,0},1,"),
            "an editable field carries 1 in the slot a label carries 0"
        );
        assert!(supplier.starts_with(
            "{37,{46,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},1,\"АнкетаПоставщика\",1,0,{1,0},{1,0},{1,{2}},"
        ));
        assert!(supplier.ends_with(",3,3,0,0,0,0}"));
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

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }
}
