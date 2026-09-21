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

    /// A name that carries a quote is escaped the way every other 1C string in
    /// a body is, rather than being written raw.
    #[test]
    fn escapes_a_quoted_name() {
        assert!(format_extended_tooltip("1", "a\"b").contains("\"a\"\"b\""));
    }
}
