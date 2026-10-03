//! WRITE · EDT controls — item dispatcher, Table (+ auto-table), additions, decorations,
//! form groups/fields, buttons and their ext-info field tables.

use super::*;

/// EDT диспетчер контрола [`FormItem`] → `<items xsi:type="form:X">` по виду.
pub(crate) fn edt_item(item: &FormItem) -> Result<OutElement, FormError> {
    match item.kind.as_str() {
        "Button" => edt_button(item),
        "Table" => edt_table(item),
        k if tables::addition_kind(k).is_some() => edt_addition_control(item),
        k if tables::decoration_kind(k).is_some() => {
            edt_decoration(item, tables::decoration_kind(k).expect("checked"))
        }
        k if tables::group_kind(k).is_some() => {
            edt_form_group(item, tables::group_kind(k).expect("checked"))
        }
        k if tables::field_kind(k).is_some() => {
            edt_form_field(item, tables::field_kind(k).expect("checked"))
        }
        other => Err(FormError::Frame(format!(
            "EDT write: unsupported control kind {other:?}"
        ))),
    }
}

/// EDT `Table` (`<items xsi:type="form:Table">`, LANE-F-4). Порядок (corpus fact): name, id,
/// HEAD_A-поля, excludedCommands, колонки, HEAD_B-поля, autoCommandBar, handlers, добавления,
/// extendedTooltip, contextMenu, TAIL-поля.
pub(crate) fn edt_table(item: &FormItem) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:Table");
    edt_table_body_into(&mut el, item)?;
    Ok(el)
}

/// EDT `<autoTable>` GanttChartField — структурно ПОЛНАЯ Таблица без обёртки
/// `<items xsi:type="form:Table">` (тег `autoTable`, БЕЗ xsi:type). Переиспользует
/// [`edt_table_body_into`] (весь Table-эмиттер). EDT ОПУСКАЕТ dataPath авто-таблицы
/// (наследуется от родителя-диаграммы) — оно не в bag'е (не читалось) ⇒ не эмитится (byte-exact).
pub(crate) fn edt_auto_table(item: &FormItem) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "autoTable");
    edt_table_body_into(&mut el, item)?;
    Ok(el)
}

/// Эмиттер тела EDT-Таблицы в `el` (общий для `<items form:Table>` и `<autoTable>`): name/id +
/// все регионы (поля/колонки/добавления/autoCommandBar/декораторы). Внешнюю обёртку/xsi:type
/// ставит вызывающий.
pub(crate) fn edt_table_body_into(el: &mut OutElement, item: &FormItem) -> Result<(), FormError> {
    super::super::event_owners::validate_table_owned(item)?;
    el.push(OutElement::leaf("", "name", item.name.clone()));
    // id=0 (главная autoTable) EDT ОПУСКАЕТ; ⟺ Designer `id="0"` (см. push_edt_id).
    push_edt_id(el, item.id);
    for entry in &tables::TABLE_BODY[..tables::TABLE_HEAD_A] {
        emit_field_edt(el, entry, &item.properties)?;
        if entry.id == tb::F_TITLE_TEXT_COLOR {
            if let Some(font) = &item.title_font {
                el.push(edt_font_named("titleFont", font));
            }
        }
    }
    for x in &item.excluded_commands {
        el.push(OutElement::leaf("", "excludedCommands", x.clone()));
    }
    // HEAD_MID: toolTip/toolTipRepresentation (ПОСЛЕ excludedCommands, ДО колонок — метамодель
    // `excludedCommands, toolTip, toolTipRepresentation, items`).
    for entry in &tables::TABLE_BODY[tables::TABLE_HEAD_A..tables::TABLE_HEAD_MID] {
        emit_field_edt(el, entry, &item.properties)?;
    }
    for child in &item.children {
        el.push(edt_item(child)?);
    }
    // HEAD_B: commandBarLocation (index HEAD_MID), затем showCommandBar (true/false — сразу
    // после commandBarLocation, corpus fact).
    for (i, entry) in tables::TABLE_BODY[tables::TABLE_HEAD_MID..tables::TABLE_HEAD_B]
        .iter()
        .enumerate()
    {
        emit_field_edt(el, entry, &item.properties)?;
        if i == 0 {
            if let Some(v) = &item.show_command_bar {
                if v == "true" || v == "false" {
                    el.push(OutElement::leaf("", "showCommandBar", v.clone()));
                }
            }
        }
    }
    if let Some(acb) = &item.auto_command_bar {
        el.push(edt_auto_command_bar(acb)?);
    }
    for ev in &item.events {
        el.push(edt_handlers_ctrl(ev));
    }
    for add in &item.additions {
        el.push(edt_addition(add, el.local != "autoTable")?);
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    for entry in &tables::TABLE_BODY[tables::TABLE_HEAD_B..] {
        emit_field_edt(el, entry, &item.properties)?;
        if entry.id == tb::F_BORDER_COLOR {
            if let Some(font) = &item.font {
                el.push(edt_font(font));
            }
        }
    }
    // extInfo динамического списка — ПОСЛЕ TAIL-полей, ДО showCommandBarNeedDereferenced
    // (последний ребёнок `<items>`; corpus fact).
    if let Some(dx) = &item.dynamic_list_ext {
        el.push(edt_dynamic_list_ext(dx)?);
    }
    // showCommandBarNeedDereferenced (`auto`) — В САМОМ КОНЦЕ тела (после TAIL-полей, corpus fact).
    if item.show_command_bar.as_deref() == Some("auto") {
        el.push(OutElement::leaf(
            "",
            "showCommandBarNeedDereferenced",
            "true",
        ));
    }
    Ok(())
}

/// EDT `<extInfo xsi:type="form:DynamicListTableExtInfo">` — обёртка: СОБСТВЕННЫЕ обработчики
/// (`<handlers>`) ПЕРВЫМИ, затем канонические поля в EDT-порядке ([`tables::DYNAMIC_LIST_EXT`]).
/// Designer-ВСЕГДА-поля (`autoRefresh`/…) EDT опускает (Keep-омиссия), поэтому не эмитятся.
pub(crate) fn edt_dynamic_list_ext(dx: &DynamicListExt) -> Result<OutElement, FormError> {
    let mut ext =
        OutElement::branch("", "extInfo").attr("xsi:type", "form:DynamicListTableExtInfo");
    for ev in &dx.events {
        ext.push(edt_handlers_ctrl(ev));
    }
    for entry in tables::DYNAMIC_LIST_EXT {
        emit_field_edt(&mut ext, entry, &dx.fields)?;
    }
    Ok(ext)
}

/// EDT ДОБАВЛЕНИЕ Таблицы. Порядок: name, id, extendedTooltip, contextMenu, [type], source,
/// extInfo(autoMaxWidth).
pub(crate) fn edt_addition(item: &FormItem, enabled_default: bool) -> Result<OutElement, FormError> {
    if item.get(tb::F_ADDITION_ENABLED).is_some_and(|v| !matches!(v, PropertyValue::Bool(_))) {
        return Err(FormError::Frame("Addition.Enabled must be a current boolean".into()));
    }
    let ak = tables::addition_kind(item.kind.as_str()).ok_or_else(|| {
        FormError::Frame(format!("EDT: unknown addition {:?}", item.kind.as_str()))
    })?;
    let mut el = OutElement::branch("", ak.edt_tag);
    // title (опц.) — ПЕРВЫЙ ребёнок (до каркаса видимости). Мультиязычный — ПОВТОРЕНИЕ
    // `<title>` по одному на язык (ERP-witness searchControlAddition ru+en; та же конвенция,
    // что у addition-контрола/корня/реквизита).
    if let Some(PropertyValue::Localized(pairs)) = item.get(tb::F_ADDITION_TITLE) {
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                el.push(edt_title("title", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            el.push(edt_title("title", pairs));
        }
    }
    // Каркас видимости (SPARSE head, ДО name): эмитим лишь присутствующие (не-дефолт)
    // visible/enabled в каноническом порядке.
    for (fid, tag) in [
        (tb::F_ADDITION_VISIBLE, "visible"),
        (tb::F_ADDITION_ENABLED, "enabled"),
    ] {
        if fid == tb::F_ADDITION_ENABLED {
            let current = !matches!(item.get(fid), Some(PropertyValue::Bool(false)));
            if current != enabled_default {
                el.push(OutElement::leaf("", tag, if current { "true" } else { "false" }));
            }
        } else if let Some(PropertyValue::Bool(b)) = item.get(fid) {
            el.push(OutElement::leaf("", tag, if *b { "true" } else { "false" }));
        }
    }
    // toolTip — метамодель Addition#4 (до toolTipRepresentation); мультиязычный —
    // ПОВТОРЕНИЕ по одному на язык (EDT-конвенция).
    if let Some(PropertyValue::Localized(pairs)) = item.get(tables::F_ADDITION_TOOL_TIP) {
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                el.push(edt_title("toolTip", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            el.push(edt_title("toolTip", pairs));
        }
    }
    // toolTipRepresentation — метамодель Addition#5 (между enabled и name).
    if let Some(PropertyValue::Enum(t)) = item.get(tables::F_ADDITION_TOOL_TIP_REPRESENTATION) {
        el.push(OutElement::leaf(
            "",
            "toolTipRepresentation",
            t.as_str().to_string(),
        ));
    }
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    // displayImportance — ГОЛОВА после id (⟺ Designer атрибут; witness ФормаВыбораПолейПоиска
    // `VeryHigh`).
    if let Some(PropertyValue::Enum(di)) = item.get(tables::F_ADDITION_DISPLAY_IMPORTANCE) {
        el.push(OutElement::leaf(
            "",
            "displayImportance",
            di.as_str().to_string(),
        ));
    }
    // Вложенные контролы (`<items>`; метамодель Addition#13 — ДО extendedTooltip).
    for child in &item.children {
        el.push(edt_item(child)?);
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    if let Some(ty) = ak.edt_type {
        el.push(OutElement::leaf("", "type", ty));
    }
    if let Some(PropertyValue::Ref(s)) = item.get(tb::F_ADDITION_SOURCE) {
        el.push(OutElement::leaf("", "source", s.clone()));
    }
    let mut ext = OutElement::branch("", "extInfo").attr("xsi:type", ak.ext_xsi);
    if let Some(PropertyValue::Int(w)) = item.get_ext(tb::F_ADDITION_WIDTH) {
        ext.push(OutElement::leaf("", "width", w.to_string()));
    }
    // autoMaxWidth — OppositeBool (bag sparse-true): эмитим `true` при наличии.
    if matches!(
        item.get_ext(tb::F_ADDITION_AUTO_MAX_WIDTH),
        Some(PropertyValue::Bool(true))
    ) {
        ext.push(OutElement::leaf("", "autoMaxWidth", "true"));
    }
    if let Some(PropertyValue::Int(mw)) = item.get_ext(tables::F_ADDITION_MAX_WIDTH) {
        ext.push(OutElement::leaf("", "maxWidth", mw.to_string()));
    }
    if let Some(PropertyValue::Bool(hs)) = item.get_ext(tb::F_ADDITION_HORIZONTAL_STRETCH) {
        ext.push(OutElement::leaf(
            "",
            "horizontalStretch",
            if *hs { "true" } else { "false" },
        ));
    }
    if let Some(PropertyValue::Enum(hl)) = item.get_ext(tables::F_ADDITION_HORIZONTAL_LOCATION) {
        ext.push(OutElement::leaf(
            "",
            "horizontalLocation",
            hl.as_str().to_string(),
        ));
    }
    // Пустой extInfo (absent autoMaxWidth/width/… — ERP named-addition, 65 SELFCLOSE-носителей)
    // эмитится самозакрытым `<extInfo …/>` (byte-exact edt-write; иначе `></extInfo>`).
    ext.self_closing = ext.children.is_empty();
    el.push(ext);
    Ok(el)
}

/// EDT `<items xsi:type="form:Addition">` — добавление-как-контрол. Каркас видимости:
/// `<visible>true` реконструируется, ОПУСКАЕТСЯ при bag-`visible=false` (sparse; witness
/// ГрупповоеИзменениеРеквизитов); `enabled`/`userVisible` — всегда true. Порядок: title,
/// [visible], enabled, userVisible, name, id, extendedTooltip, contextMenu, [type], source,
/// [groupHorizontalAlign], extInfo([width][autoMaxWidth][horizontalStretch]).
pub(crate) fn edt_addition_control(item: &FormItem) -> Result<OutElement, FormError> {
    if item.get(tb::F_ADDITION_ENABLED).is_some_and(|v| !matches!(v, PropertyValue::Bool(_))) {
        return Err(FormError::Frame("Addition.Enabled must be a current boolean".into()));
    }
    let ak = tables::addition_kind(item.kind.as_str()).ok_or_else(|| {
        FormError::Frame(format!(
            "EDT: unknown addition control {:?}",
            item.kind.as_str()
        ))
    })?;
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:Addition");
    // title (опц.) — ПЕРВЫЙ ребёнок (до каркаса видимости). Мультиязычный — ПОВТОРЕНИЕ
    // `<title>` по одному на язык (ERP-witness ОстаткиАлкогольнойПродукцииЕГАИС.ФормаОстатков;
    // та же конвенция, что у корня/реквизита).
    if let Some(PropertyValue::Localized(pairs)) = item.get(tb::F_ADDITION_TITLE) {
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                el.push(edt_title("title", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            el.push(edt_title("title", pairs));
        }
    }
    if !matches!(
        item.get(tb::F_ADDITION_VISIBLE),
        Some(PropertyValue::Bool(false))
    ) {
        el.push(OutElement::leaf("", "visible", "true"));
    }
    if !matches!(item.get(tb::F_ADDITION_ENABLED), Some(PropertyValue::Bool(false))) {
        el.push(OutElement::leaf("", "enabled", "true"));
    }
    let mut uv = OutElement::branch("", "userVisible");
    uv.push(OutElement::leaf("", "common", "true"));
    el.push(uv);
    // toolTipRepresentation — метамодель Addition#5 (между userVisible и name).
    if let Some(PropertyValue::Enum(t)) = item.get(tables::F_ADDITION_TOOL_TIP_REPRESENTATION) {
        el.push(OutElement::leaf(
            "",
            "toolTipRepresentation",
            t.as_str().to_string(),
        ));
    }
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    // displayImportance — ГОЛОВА после id (⟺ Designer атрибут).
    if let Some(PropertyValue::Enum(di)) = item.get(tables::F_ADDITION_DISPLAY_IMPORTANCE) {
        el.push(OutElement::leaf(
            "",
            "displayImportance",
            di.as_str().to_string(),
        ));
    }
    // Вложенные контролы (`<items>`; метамодель Addition#13 — ДО extendedTooltip). ERP-witness
    // SearchControl-добавление-контрол с Button внутри (зеркально `edt_addition`).
    for child in &item.children {
        el.push(edt_item(child)?);
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    if let Some(ty) = ak.edt_type {
        el.push(OutElement::leaf("", "type", ty));
    }
    if let Some(PropertyValue::Ref(s)) = item.get(tb::F_ADDITION_SOURCE) {
        el.push(OutElement::leaf("", "source", s.clone()));
    }
    if let Some(PropertyValue::Enum(g)) = item.get(tb::F_ADDITION_GROUP_HORIZONTAL_ALIGN) {
        el.push(OutElement::leaf(
            "",
            "groupHorizontalAlign",
            g.as_str().to_string(),
        ));
    }
    let mut ext = OutElement::branch("", "extInfo").attr("xsi:type", ak.ext_xsi);
    if let Some(PropertyValue::Int(w)) = item.get_ext(tb::F_ADDITION_WIDTH) {
        ext.push(OutElement::leaf("", "width", w.to_string()));
    }
    // autoMaxWidth — OppositeBool: bag sparse-true ⇒ эмитим `true` при наличии, опускаем false.
    if matches!(
        item.get_ext(tb::F_ADDITION_AUTO_MAX_WIDTH),
        Some(PropertyValue::Bool(true))
    ) {
        ext.push(OutElement::leaf("", "autoMaxWidth", "true"));
    }
    if let Some(PropertyValue::Int(mw)) = item.get_ext(tables::F_ADDITION_MAX_WIDTH) {
        ext.push(OutElement::leaf("", "maxWidth", mw.to_string()));
    }
    if let Some(PropertyValue::Bool(hs)) = item.get_ext(tb::F_ADDITION_HORIZONTAL_STRETCH) {
        ext.push(OutElement::leaf(
            "",
            "horizontalStretch",
            if *hs { "true" } else { "false" },
        ));
    }
    if let Some(PropertyValue::Enum(hl)) = item.get_ext(tables::F_ADDITION_HORIZONTAL_LOCATION) {
        ext.push(OutElement::leaf(
            "",
            "horizontalLocation",
            hl.as_str().to_string(),
        ));
    }
    // Пустой extInfo (все гео-поля дефолтны, вкл. autoMaxWidth=false ⇒ bag БЕЗ поля) EDT эмитит
    // САМОЗАКРЫТО `<extInfo xsi:type="…"/>` — ERP-witness 65 именованных добавлений (SearchString
    // 61 / SearchControl 3 / ViewStatus 1). Без этого рендер дал бы `<extInfo…></extInfo>` (дифф).
    ext.self_closing = ext.children.is_empty();
    el.push(ext);
    Ok(el)
}

/// EDT локализованный `<tag>` (`<key>/<value>`-пары).
pub(crate) fn edt_title(tag: &str, pairs: &[(Lang, String)]) -> OutElement {
    let mut t = OutElement::branch("", tag);
    for (lang, text) in pairs {
        t.push(OutElement::leaf("", "key", lang.as_str().to_string()));
        t.push(OutElement::leaf("", "value", text.clone()));
    }
    t
}

/// Эмитировать МНОГОЯЗЫЧНЫЙ EDT-заголовок `<tag>` ПОВТОРЕНИЕМ элемента (по одному на язык;
/// как engine-Localized-кодек). ≤1 пара ⇒ один элемент. Зеркально `read_edt_title_multi` —
/// byte-exact для 2+ языков (witness ERP: декорации несут `<title>×2`).
pub(crate) fn push_edt_title_multi(el: &mut OutElement, tag: &str, pairs: &[(Lang, String)]) {
    if pairs.len() > 1 {
        for pair in pairs {
            el.push(edt_title(tag, std::slice::from_ref(pair)));
        }
    } else if !pairs.is_empty() {
        el.push(edt_title(tag, pairs));
    }
}

/// EDT семейство `Decoration` (`<items xsi:type="form:Decoration">`; `<type>Label` у
/// LabelDecoration, у PictureDecoration `<type>` ОТСУТСТВУЕТ). Порядок (corpus fact):
/// name, id, displayImportance, title(glue), головные поля, extendedTooltip, contextMenu,
/// formatted(glue), type, хвостовые поля, extInfo(handlers + поля).
pub(crate) fn edt_decoration(
    item: &FormItem,
    dk: &'static tables::DecorationKind,
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:Decoration");
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    // displayImportance — первое поле тела.
    emit_field_edt(&mut el, &tables::DECORATION_BODY[0], &item.properties)?;
    // title — glue (после displayImportance). МНОГОЯЗЫЧНЫЙ — по одному `<title>` на язык.
    if let Some(PropertyValue::Localized(pairs)) = item.get(ld::F_TITLE) {
        push_edt_title_multi(&mut el, "title", pairs);
    }
    for entry in &tables::DECORATION_BODY[1..tables::DECORATION_EDT_HEAD] {
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    // formatted — glue (после стабов, до type).
    if let Some(PropertyValue::Bool(true)) = item.get(ld::F_FORMATTED) {
        el.push(OutElement::leaf("", "formatted", "true"));
    }
    if dk.kind == "LabelDecoration" {
        el.push(OutElement::leaf("", "type", "Label"));
    }
    for entry in
        &tables::DECORATION_BODY[tables::DECORATION_EDT_HEAD..tables::DECORATION_EDT_FONT_SPLIT]
    {
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    // `<font>` — метамодель Decoration: textColor → font → groupHorizontalAlign (SSL:
    // textColor<font×4, font<groupHorizontalAlign×1, font<groupVerticalAlign×7, font<extInfo×51).
    if let Some(f) = &item.font {
        el.push(edt_font(f));
    }
    for entry in &tables::DECORATION_BODY[tables::DECORATION_EDT_FONT_SPLIT..] {
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    el.push(edt_ext_info(
        dk.ext_xsi,
        dk.ext,
        &item.ext_info,
        &item.events,
    )?);
    Ok(el)
}

/// EDT семейство `FormGroup` (`<items xsi:type="form:FormGroup">` + `<type>` кроме
/// ButtonGroup). Порядок (corpus fact): name, id, displayImportance, ДЕТИ `<items>`,
/// поля тела, extendedTooltip, contextMenu, type, extInfo(handlers + поля).
pub(crate) fn edt_form_group(
    item: &FormItem,
    gk: &'static tables::GroupKind,
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:FormGroup");
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    // displayImportance — единственное поле тела ДО детей.
    emit_field_edt(&mut el, &tables::FORM_GROUP_BODY[0], &item.properties)?;
    // Дети контейнера (в EDT — сразу после id/displayImportance, ДО свойств).
    for child in &item.children {
        el.push(edt_item(child)?);
    }
    for entry in &tables::FORM_GROUP_BODY[1..] {
        // `<titleFont>` — метамодель FormGroup: titleTextColor → titleFont → toolTip
        // (witness МашиночитаемыеДоверенности: title→titleFont→extendedTooltip).
        if entry.id == fg::F_TOOL_TIP {
            if let Some(tf) = &item.title_font {
                el.push(edt_font_named("titleFont", tf));
            }
        }
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    if let Some(ty) = gk.edt_type {
        el.push(OutElement::leaf("", "type", ty));
    }
    el.push(edt_ext_info(
        gk.ext_xsi,
        gk.ext,
        &item.ext_info,
        &item.events,
    )?);
    Ok(el)
}

/// EDT `FormField` (`<items xsi:type="form:FormField">` + `<type>`). Порядок (corpus fact):
/// name, id, головные поля тела, handlers(`OnChange`), extendedTooltip, contextMenu, type,
/// хвостовые поля тела, extInfo(handlers + поля).
pub(crate) fn edt_form_field(
    item: &FormItem,
    fk: &'static tables::FieldKind,
) -> Result<OutElement, FormError> {
    let actual_ext = super::super::event_owners::field_extension_kind(item)?;
    let (body_events, ext_events, extension_attached) =
        super::super::event_owners::field_owned_events(item)?;
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:FormField");
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    let (head, tail) = tables::FORM_FIELD_COMMON.split_at(tables::FORM_FIELD_EDT_HEAD);
    for entry in head {
        // `<titleFont>` — метамодель FormField: titleTextColor → titleFont → visible
        // (witness РаботаСФайлами «Владелец»: title→titleFont→visible).
        if entry.id == ff::F_VISIBLE {
            if let Some(tf) = &item.title_font {
                el.push(edt_font_named("titleFont", tf));
            }
        }
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    // СОБСТВЕННЫЕ исключённые команды (SpreadsheetDocumentField) — после головных полей, ДО
    // decorator-стабов (corpus fact). Пусто у прочих полей ⇒ ничего не эмитится.
    for x in &item.excluded_commands {
        el.push(OutElement::leaf("", "excludedCommands", x.clone()));
    }
    // Тело-события: `OnChange` (общее поле-событие; прочие — extInfo).
    for ev in body_events {
        el.push(edt_handlers_ctrl(ev));
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    if !item.field_type_none { el.push(OutElement::leaf("", "type", fk.kind)); }
    for entry in tail {
        emit_field_edt(&mut el, entry, &item.properties)?;
        // footerFont — сразу ПОСЛЕ строки footerTextColor (метамодель футер-блока;
        // ERP-witness: FooterTextColor→FooterFont; строка-якорь детерминирована даже при
        // отсутствии значения footerTextColor).
        if entry.id == tables::F_FF_FOOTER_TEXT_COLOR {
            if let Some(f) = &item.footer_font {
                el.push(edt_font_named("footerFont", f));
            }
        }
    }
    if extension_attached {
        el.push(edt_ext_info_ref(
            actual_ext.ext_xsi,
            actual_ext.ext,
            &item.ext_info,
            &ext_events,
            item.font.as_ref(),
            item.auto_table.as_deref(),
            &item.additions,
        )?);
    } else {
        let mut defaults = Vec::new();
        super::super::fields::read_fields_edt(actual_ext.kind, None, actual_ext.ext, super::super::fields::Region::Ext, &mut defaults)?;
        if item.ext_info != defaults || item.font.is_some()
            || item.auto_table.is_some() || !item.additions.is_empty()
        {
            return Err(FormError::Frame("absent field extension has current extension data".into()));
        }
    }
    Ok(el)
}

/// Единственное событие ТЕЛА FormField (сверено 172/172 body-handlers корпуса); все прочие
/// события полей живут в extInfo. Разбиение канонического плоского списка на write — по
/// имени события; Designer несёт единый `<Events>` (OnChange первым — сверено).


/// EDT `<extInfo xsi:type="…">`: события (handlers) + таблица полей; пустой ⇒ самозакрытие.
pub(crate) fn edt_ext_info(
    xsi: &str,
    table: &[FieldProj],
    bag: &[(morph1c_core::ir::FieldId, PropertyValue)],
    events: &[morph1c_core::ir::FormEvent],
) -> Result<OutElement, FormError> {
    let refs: Vec<&morph1c_core::ir::FormEvent> = events.iter().collect();
    edt_ext_info_ref(xsi, table, bag, &refs, None, None, &[])
}

/// `font` — композит-шрифт extInfo (LabelFieldExtInfo; метамодель backColor → font → useCopy):
/// эмитится ПЕРЕД строкой `useCopy`. `Some` при отсутствии `useCopy`-строки в таблице —
/// громкая §1.0-ошибка (немоделированный локус, а не молчаливый дроп).
#[allow(clippy::too_many_arguments)]
pub(crate) fn edt_ext_info_ref(
    xsi: &str,
    table: &[FieldProj],
    bag: &[(morph1c_core::ir::FieldId, PropertyValue)],
    events: &[&morph1c_core::ir::FormEvent],
    font: Option<&FontRef>,
    auto_table: Option<&FormItem>,
    additions: &[FormItem],
) -> Result<OutElement, FormError> {
    let mut ext = OutElement::branch("", "extInfo").attr("xsi:type", xsi);
    for ev in events {
        ext.push(edt_handlers_ctrl(ev));
    }
    // Структурное содержимое extInfo ПОСЛЕ handlers, ДО геометрии:
    // * PDFDocumentField — `<viewStatusAddition>` (item.additions);
    // * GanttChartField — `<autoTable>` (item.auto_table).
    // Ни на одном контроле не со-встречаются ⇒ порядок между ними нейтрален.
    for add in additions {
        ext.push(edt_addition(add, true)?);
    }
    if let Some(at) = auto_table {
        ext.push(edt_auto_table(at)?);
    }
    // Пер-видовый ЯКОРЬ эмиссии extInfo-`font` (метамодельная позиция; ERP-волна):
    // Label — ПЕРЕД useCopy (backColor→font→useCopy); Input — ПОСЛЕ textSize (55→56);
    // Radio — ПОСЛЕ horizontalStretch (9→10→textColor); Text — ПОСЛЕ output (модельный хвост);
    // Formatted — ПОСЛЕ borderColor (13→14); Picture — ПОСЛЕ border (21→…→23→fileDragMode).
    // Якорь выбирается по xsi ⇒ чужие таблицы с тем же id-полем не задеваются.
    let font_before: Option<FieldId> = match xsi {
        "form:LabelFieldExtInfo" => Some(ff::F_EXT_USE_COPY),
        _ => None,
    };
    let font_after: Option<FieldId> = match xsi {
        "form:InputFieldExtInfo" => Some(ff::F_EXT_TEXT_SIZE),
        "form:RadioButtonsFieldExtInfo" => Some(rb::F_EXT_HORIZONTAL_STRETCH),
        "form:TextDocFieldExtInfo" => Some(ff::F_EXT_OUTPUT),
        "form:FormattedDocFieldExtInfo" => Some(ff::F_EXT_BORDER_COLOR),
        "form:ImageFieldExtInfo" => Some(ff::F_EXT_BORDER),
        _ => None,
    };
    let mut font_emitted = false;
    for entry in table {
        if Some(entry.id) == font_before && !font_emitted {
            if let Some(f) = font {
                ext.push(edt_font(f));
                font_emitted = true;
            }
        }
        emit_field_edt(&mut ext, entry, bag)?;
        if Some(entry.id) == font_after && !font_emitted {
            if let Some(f) = font {
                ext.push(edt_font(f));
                font_emitted = true;
            }
        }
    }
    if font.is_some() && !font_emitted {
        return Err(FormError::Frame(format!(
            "extInfo {xsi}: font present but no witnessed anchor row (§1.0 — unmodeled locus)"
        )));
    }
    if ext.children.is_empty() {
        ext = OutElement::self_closing("", "extInfo").attr("xsi:type", xsi);
    }
    Ok(ext)
}

/// EDT `Button` (`<items xsi:type="form:Button">`): ПОЛНОЕ тело таблично. Порядок:
/// name, id, головные поля (по skipOnInput), extendedTooltip, contextMenu, хвостовые
/// поля (type — данные-поле в начале хвоста). extInfo кнопка не несёт.
pub(crate) fn edt_button(item: &FormItem) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "items").attr("xsi:type", "form:Button");
    el.push(OutElement::leaf("", "name", item.name.clone()));
    el.push(OutElement::leaf("", "id", item.id.to_string()));
    let (head, tail) = tables::BUTTON_BODY.split_at(tables::BUTTON_EDT_HEAD);
    for entry in head {
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(edt_extended_tooltip(t)?);
    }
    if let Some(c) = &item.context_menu {
        el.push(edt_context_menu(c)?);
    }
    // `<font>` эмитится ПЕРЕД `<picture>` (corpus fact: placementArea, font, picture —
    // ФормаОтчета кнопка с шрифтом И картинкой). Для кнопок без картинки/textColor результат тот
    // же (font сразу после placementArea).
    for entry in tail {
        if entry.id == bt::F_PICTURE {
            if let Some(f) = &item.font {
                el.push(edt_font(f));
            }
        }
        emit_field_edt(&mut el, entry, &item.properties)?;
    }
    Ok(el)
}
