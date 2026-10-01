//! WRITE · spreadsheet (MXL) body — Designer inline `<Settings>` writer plus the shared
//! spreadsheet-children tree reused by the EDT `.mxlx` sidecar (`super::super::mxlx`).

use super::*;

/// Designer `<Settings xmlns:mxl=… xsi:type="mxl:SpreadsheetDocument">` — пустой табличный документ
/// (columns/size, rowsItem/index/row/empty, vgRows). Инлайн-`xmlns:mxl` — атрибут host'а (Designer
/// объявляет его прямо на `<Settings>`, не в корневом ns-блоке).
pub(crate) fn designer_spreadsheet_settings(ss: &MxlSpreadsheetSettings) -> OutElement {
    let mut s = OutElement::branch("", "Settings")
        .attr("xmlns:mxl", MXL_NS_URI)
        .attr("xsi:type", "mxl:SpreadsheetDocument");
    for c in spreadsheet_body_children("mxl", ss) {
        s.push(c);
    }
    s
}

/// Дети-структура табличного документа: БОГАТЫЙ вариант (`full_body`) регенерируется из generic-tree
/// (диалектное написание ХРАНИТСЯ в узлах — `prefix`/порядок атрибутов/инлайн-редекл — ⇒ `prefix`-
/// аргумент для него игнорируется, байт-точно в ТОМ ЖЕ диалекте); МИНИМАЛЬНЫЙ — прежним структурным
/// [`spreadsheet_settings_children`] (SSL/минимум byte-exact не тронут).
pub(crate) fn spreadsheet_body_children(
    prefix: &'static str,
    ss: &MxlSpreadsheetSettings,
) -> Vec<OutElement> {
    if let Some(fb) = &ss.full_body {
        return fb.iter().map(mxl_node_to_out).collect();
    }
    spreadsheet_settings_children(prefix, ss)
}

/// Рекурсивно развернуть [`MxlNode`] generic-tree в [`OutElement`] для byte-exact ре-эмиссии:
/// префикс/local/атрибуты(в порядке)/дети/текст. Пустой самозакрытый узел ⇒ `<tag/>`.
pub(crate) fn mxl_node_to_out(n: &MxlNode) -> OutElement {
    let mut el = OutElement::branch(n.prefix.clone(), n.local.clone());
    for (name, value) in &n.attrs {
        el = el.attr(name.clone(), value.clone());
    }
    if !n.children.is_empty() {
        for c in &n.children {
            el.push(mxl_node_to_out(c));
        }
    } else if !n.text.is_empty() {
        el.text = Some(n.text.clone());
    } else if n.self_closing {
        el.self_closing = true;
    }
    el
}

/// Дети-структура табличного документа — общая ОБЕИМ сериализациям (Designer-инлайн
/// `<Settings>` с `mxl:`-префиксом / EDT-сайдкар `SpreadsheetData.mxlx` с default-ns, `""`):
/// `[languageSettings]`, `columns/size`, `rowsItem/index/row/empty`, `[templateMode]`, `vgRows`
/// — содержимое сверено 1:1 по 11/11 парам корпуса SSL+coverage.
pub(crate) fn spreadsheet_settings_children(
    prefix: &'static str,
    ss: &MxlSpreadsheetSettings,
) -> Vec<OutElement> {
    let mut out = Vec::new();
    // Опциональный `<languageSettings>` ПЕРВЫМ (RE: РедактированиеТабличногоДокумента).
    if let Some(ls) = &ss.language_settings {
        let mut els = OutElement::branch(prefix, "languageSettings");
        els.push(OutElement::leaf(
            prefix,
            "currentLanguage",
            ls.current_language.clone(),
        ));
        els.push(OutElement::leaf(
            prefix,
            "defaultLanguage",
            ls.default_language.clone(),
        ));
        for li in &ls.languages {
            let mut info = OutElement::branch(prefix, "languageInfo");
            info.push(OutElement::leaf(prefix, "id", li.id.clone()));
            info.push(OutElement::leaf(prefix, "code", li.code.clone()));
            info.push(OutElement::leaf(
                prefix,
                "description",
                li.description.clone(),
            ));
            els.push(info);
        }
        out.push(els);
    }
    let mut columns = OutElement::branch(prefix, "columns");
    columns.push(OutElement::leaf(prefix, "size", ss.columns_size.clone()));
    out.push(columns);
    let mut rows_item = OutElement::branch(prefix, "rowsItem");
    rows_item.push(OutElement::leaf(prefix, "index", ss.rows_index.clone()));
    let mut row = OutElement::branch(prefix, "row");
    row.push(OutElement::leaf(
        prefix,
        "empty",
        if ss.row_empty { "true" } else { "false" },
    ));
    rows_item.push(row);
    out.push(rows_item);
    // Опциональный `<templateMode>` между rowsItem и vgRows.
    if let Some(tm) = ss.template_mode {
        out.push(OutElement::leaf(
            prefix,
            "templateMode",
            if tm { "true" } else { "false" },
        ));
    }
    out.push(OutElement::leaf(prefix, "vgRows", ss.vg_rows.clone()));
    out
}
