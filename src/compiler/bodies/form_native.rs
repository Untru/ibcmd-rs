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

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }
}
