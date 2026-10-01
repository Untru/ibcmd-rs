//! WRITE · Designer controls — table-driven control writer, additions, decorators,
//! tooltip body, events and data-model attributes (`<Attribute>`/`<Column>`).

use super::*;

/// Общий Designer-писатель таблично-управляемого контрола (FormField/FormGroup): элемент
/// по Designer-тегу `tag`, атрибуты `name`/`id` (+`des_attr`-поля вроде `DisplayImportance`),
/// затем слот-массив Designer-порядка (поля + каркас-маркеры).
pub(crate) fn designer_table_control(
    item: &FormItem,
    tag: &str,
    common: &'static [FieldProj],
    ext: &'static [FieldProj],
    order: &'static [DesSlot],
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", tag)
        .attr("name", item.name.clone())
        .attr("id", item.id.to_string());
    // Designer-атрибуты полей (после name/id; порядок таблицы).
    for entry in common.iter().filter(|e| e.des_attr) {
        if let Some(v) = designer_attr_value(entry, &item.properties) {
            el = el.attr(entry.des, v);
        }
    }
    for slot in order {
        match slot {
            DesSlot::F(id)
                if tag == "Button"
                    && *id == bt::F_BACK_COLOR
                    && item.designer_button_back_color_auto
                    && item.get(*id).is_none() =>
            {
                el.push(OutElement::leaf("", "BackColor", "auto"));
            }
            // Сигила `~` на `RowPictureDataPath` — DESIGNER-ONLY денормализация, снятая на чтении
            // в канон (EDT-написание) и удержанная presence-точным флагом. Возвращаем её ЗДЕСЬ,
            // ровно тем контролам, что её несли (5/312 SSL). См. `FormItem::row_picture_path_unavailable`.
            DesSlot::F(id)
                if *id == tb::F_ROW_PICTURE_DATA_PATH
                    && (item.row_picture_path_unavailable
                        || matches!(item.get(*id), Some(PropertyValue::Ref(path)) if super::super::availability::unavailable(path))) =>
            {
                if let Some(entry) = common.iter().find(|e| e.id == *id) {
                    let sigiled: Vec<(morph1c_core::ir::FieldId, PropertyValue)> = item
                        .properties
                        .iter()
                        .map(|(f, v)| match (f == id, v) {
                            (true, PropertyValue::Ref(p)) => {
                                (*f, PropertyValue::Ref(format!("~{p}")))
                            }
                            _ => (*f, v.clone()),
                        })
                        .collect();
                    emit_field_designer(&mut el, entry, &sigiled)?;
                }
            }
            DesSlot::F(id) => {
                if let Some(entry) = common.iter().find(|e| e.id == *id) {
                    emit_field_designer(&mut el, entry, &item.properties)?;
                } else if let Some(entry) = ext.iter().find(|e| e.id == *id) {
                    emit_field_designer(&mut el, entry, &item.ext_info)?;
                }
            }
            DesSlot::AutoEditMode => emit_designer_auto_edit_mode(&mut el, &item.properties),
            DesSlot::TitleFormatted => {
                // Заголовок декорации: `formatted`-АТРИБУТ несёт канон-поле `formatted`.
                let f = matches!(item.get(ld::F_FORMATTED), Some(PropertyValue::Bool(true)));
                match item.get(ld::F_TITLE) {
                    Some(PropertyValue::Localized(pairs)) if !pairs.is_empty() => {
                        el.push(designer_title_with(
                            pairs,
                            Some(if f { "true" } else { "false" }),
                        ));
                    }
                    // Пустой заголовок + formatted=true: Designer несёт `<Title formatted="true"/>`
                    // самозакрытием (EDT-аналог — `<formatted>true` без `<title>`).
                    _ if f => {
                        el.push(OutElement::self_closing("", "Title").attr("formatted", "true"));
                    }
                    _ => {}
                }
            }
            DesSlot::ContextMenu => {
                if let Some(c) = &item.context_menu {
                    el.push(designer_decorator("ContextMenu", c)?);
                }
            }
            DesSlot::ExtendedTooltip => {
                if let Some(t) = &item.ext_tooltip {
                    el.push(designer_decorator("ExtendedTooltip", t)?);
                }
            }
            DesSlot::Font => {
                if let Some(f) = &item.font {
                    el.push(designer_font(f));
                }
            }
            DesSlot::TitleFont => {
                if let Some(f) = &item.title_font {
                    el.push(designer_font_named("TitleFont", f));
                }
            }
            DesSlot::FooterFont => {
                if let Some(f) = &item.footer_font {
                    el.push(designer_font_named("FooterFont", f));
                }
            }
            DesSlot::Events => push_designer_events(&mut el, item),
            DesSlot::ChildItems => {
                if !item.children.is_empty() {
                    let mut ci = OutElement::branch("", "ChildItems");
                    for child in &item.children {
                        ci.push(designer_item(child)?);
                    }
                    el.push(ci);
                }
            }
            DesSlot::TableCommandSet => {
                if !item.excluded_commands.is_empty() {
                    let mut cs = OutElement::branch("", "CommandSet");
                    for x in &item.excluded_commands {
                        cs.push(OutElement::leaf("", "ExcludedCommand", x.clone()));
                    }
                    el.push(cs);
                }
            }
            DesSlot::TableShowCommandBar => {
                if let Some(v) = &item.show_command_bar {
                    el.push(OutElement::leaf("", "ShowCommandBar", v.clone()));
                }
            }
            DesSlot::TableAutoCommandBar => {
                if let Some(acb) = &item.auto_command_bar {
                    el.push(designer_auto_command_bar(acb)?);
                }
            }
            DesSlot::TableAdditions => {
                for add in &item.additions {
                    el.push(designer_addition(add)?);
                }
            }
            DesSlot::AutoTable => {
                // GanttChartField авто-таблица: прямой ребёнок `<Table>` (обычный Designer-Table).
                if let Some(at) = &item.auto_table {
                    el.push(designer_table_control(
                        at,
                        "Table",
                        tables::TABLE_BODY,
                        &[],
                        tables::DES_TABLE_ORDER,
                    )?);
                }
            }
            DesSlot::TableDynamicList => {
                if let Some(dx) = &item.dynamic_list_ext {
                    for id in tables::DES_DYNAMIC_LIST_ORDER {
                        let entry = tables::DYNAMIC_LIST_EXT
                            .iter()
                            .find(|e| e.id == *id)
                            .expect("DES_DYNAMIC_LIST_ORDER id present in DYNAMIC_LIST_EXT");
                        emit_field_designer(&mut el, entry, &dx.fields)?;
                    }
                }
            }
        }
    }
    Ok(el)
}

/// Designer ДОБАВЛЕНИЕ Таблицы (`<SearchStringAddition name id>`). Порядок (topo по всем
/// SSL-витнессам): [Visible/Enabled], AdditionSource (`<Item>` source + `<Type>` константа
/// вида), [Title], [GroupHorizontalAlign], [Width], [AutoMaxWidth], [HorizontalStretch],
/// ContextMenu, ExtendedTooltip. `AutoMaxWidth` — OppositeBool: эмитится `false` ⟺ bag БЕЗ
/// sparse-true (Designer опускает `true`-дефолт).
pub(crate) fn designer_addition(item: &FormItem) -> Result<OutElement, FormError> {
    let ak = tables::addition_kind(item.kind.as_str()).ok_or_else(|| {
        FormError::Frame(format!(
            "Designer: unknown addition {:?}",
            item.kind.as_str()
        ))
    })?;
    let mut el = OutElement::branch("", ak.kind)
        .attr("name", item.name.clone())
        .attr("id", item.id.to_string());
    // DisplayImportance — АТРИБУТ после name/id (⟺ EDT чилд `<displayImportance>`; witness
    // ФормаВыбораПолейПоиска `VeryHigh`).
    if let Some(PropertyValue::Enum(di)) = item.get(tables::F_ADDITION_DISPLAY_IMPORTANCE) {
        el = el.attr("DisplayImportance", di.as_str().to_string());
    }
    // Каркас видимости (SPARSE head, ДО AdditionSource): эмитим лишь не-дефолт visible/enabled.
    for (fid, tag) in [
        (tb::F_ADDITION_VISIBLE, "Visible"),
        (tb::F_ADDITION_ENABLED, "Enabled"),
    ] {
        if let Some(PropertyValue::Bool(b)) = item.get(fid) {
            el.push(OutElement::leaf("", tag, if *b { "true" } else { "false" }));
        }
    }
    // toolTip — Localized (метамодель Addition#4; ERP ×1; loose до ToolTipRepresentation).
    if let Some(PropertyValue::Localized(pairs)) = item.get(tables::F_ADDITION_TOOL_TIP) {
        if !pairs.is_empty() {
            let mut tt = OutElement::branch("", "ToolTip");
            for (lang, content) in pairs {
                let mut it = OutElement::branch("v8", "item");
                it.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                it.push(OutElement::leaf("v8", "content", content.clone()));
                tt.push(it);
            }
            el.push(tt);
        }
    }
    // toolTipRepresentation — ПЕРЕД AdditionSource (ERP-witness ИнтерфейсДокументовЭДО/
    // ПодключениеКОФД: ToolTipRepresentation→AdditionSource→…).
    if let Some(PropertyValue::Enum(t)) = item.get(tables::F_ADDITION_TOOL_TIP_REPRESENTATION) {
        el.push(OutElement::leaf(
            "",
            "ToolTipRepresentation",
            t.as_str().to_string(),
        ));
    }
    // `<AdditionSource>` эмитим ЛИШЬ у добавления с источником (⟺ EDT `<source>`; witness всегда
    // несёт). Добавление без источника его опускает целиком (симметрично read: source=None).
    if let Some(PropertyValue::Ref(s)) = item.get(tb::F_ADDITION_SOURCE) {
        let mut src = OutElement::branch("", "AdditionSource");
        src.push(OutElement::leaf("", "Item", s.clone()));
        src.push(OutElement::leaf("", "Type", ak.des_source_type));
        el.push(src);
    }
    // title (опц.) — после AdditionSource (⟺ EDT `<title>` первым).
    if let Some(PropertyValue::Localized(pairs)) = item.get(tb::F_ADDITION_TITLE) {
        if !pairs.is_empty() {
            el.push(designer_title(pairs, false));
        }
    }
    if let Some(PropertyValue::Enum(g)) = item.get(tb::F_ADDITION_GROUP_HORIZONTAL_ALIGN) {
        el.push(OutElement::leaf(
            "",
            "GroupHorizontalAlign",
            g.as_str().to_string(),
        ));
    }
    // Width — между GroupHorizontalAlign и AutoMaxWidth (прямой witness лишь
    // AdditionSource<Width<HorizontalStretch; позиция внутри блока — геометрическая
    // конвенция Designer Width<AutoMaxWidth, контрпримеров 0).
    if let Some(PropertyValue::Int(w)) = item.get_ext(tb::F_ADDITION_WIDTH) {
        el.push(OutElement::leaf("", "Width", w.to_string()));
    }
    if item.get_ext(tb::F_ADDITION_AUTO_MAX_WIDTH).is_none() {
        el.push(OutElement::leaf("", "AutoMaxWidth", "false"));
    }
    // MaxWidth — после AutoMaxWidth, ДО HorizontalStretch (метамодель autoMaxWidth→maxWidth;
    // ERP-witness SearchString: AdditionSource→MaxWidth→ContextMenu).
    if let Some(PropertyValue::Int(mw)) = item.get_ext(tables::F_ADDITION_MAX_WIDTH) {
        el.push(OutElement::leaf("", "MaxWidth", mw.to_string()));
    }
    if let Some(PropertyValue::Bool(hs)) = item.get_ext(tb::F_ADDITION_HORIZONTAL_STRETCH) {
        el.push(OutElement::leaf(
            "",
            "HorizontalStretch",
            if *hs { "true" } else { "false" },
        ));
    }
    // HorizontalLocation (ViewStatus-only) — ПОСЛЕ геометрии, ДО ContextMenu (ERP-witness
    // Международный.ФормаСписка: AdditionSource→HorizontalLocation→ContextMenu).
    if let Some(PropertyValue::Enum(hl)) = item.get_ext(tables::F_ADDITION_HORIZONTAL_LOCATION) {
        el.push(OutElement::leaf(
            "",
            "HorizontalLocation",
            hl.as_str().to_string(),
        ));
    }
    if let Some(c) = &item.context_menu {
        el.push(designer_decorator("ContextMenu", c)?);
    }
    if let Some(t) = &item.ext_tooltip {
        el.push(designer_decorator("ExtendedTooltip", t)?);
    }
    // ChildItems — вложенные контролы добавления (ERP ×1; последними, как у Таблицы).
    if !item.children.is_empty() {
        let mut ci = OutElement::branch("", "ChildItems");
        for child in &item.children {
            ci.push(designer_item(child)?);
        }
        el.push(ci);
    }
    Ok(el)
}

/// Designer decorator. Расширенная подсказка несёт СОДЕРЖАТЕЛЬНОЕ тело (вложенный
/// `LabelDecoration`-стаб), если оно непусто; иначе — bare-ref `<X name id/>`. Пустоту тела
/// определяем НАПРЯМУЮ из движка: строим тело и, если оно НЕ дало ни одного ребёнка, эмитим
/// bare-ref (прежде — ручной предикат `designer_tooltip_body_is_empty`, теперь удалён). Контекстное
/// меню несёт `<Autofill>false` у не-авто меню и `<ChildItems>` у меню с явными пунктами;
/// иначе — bare-ref (`Autofill=true` — Designer-дефолт, опускается).
pub(crate) fn designer_decorator(tag: &str, d: &DecoratorRef) -> Result<OutElement, FormError> {
    Ok(match &d.body {
        DecoratorBody::Tooltip(body) => {
            // DisplayImportance — АТРИБУТ после name/id (⟺ EDT чилд `<displayImportance>`; witness
            // кнопка СоздатьДокументы `High`). Кладём и на полный, и на bare-ref элемент.
            let with_head = |mut el: OutElement| {
                if let Some(di) = &body.display_importance {
                    el = el.attr("DisplayImportance", di.clone());
                }
                el
            };
            let mut el = with_head(
                OutElement::branch("", tag)
                    .attr("name", d.name.clone())
                    .attr("id", d.id.to_string()),
            );
            push_designer_tooltip_body(&mut el, body)?;
            // Тело не дало детей ⇒ bare-ref (Designer-дефолты autoMax=true опущены, иного нет).
            if el.children.is_empty() {
                with_head(
                    OutElement::self_closing("", tag)
                        .attr("name", d.name.clone())
                        .attr("id", d.id.to_string()),
                )
            } else {
                el
            }
        }
        // Контекстное меню: Designer опускает auto_fill=true (свой дефолт); эмитит
        // `<Autofill>false` у не-авто меню и `<ChildItems>` у меню с явными пунктами.
        DecoratorBody::ContextMenu(b) if b.auto_fill == Some(false) || !b.items.is_empty() => {
            let mut el = OutElement::branch("", tag)
                .attr("name", d.name.clone())
                .attr("id", d.id.to_string());
            if b.auto_fill == Some(false) {
                el.push(OutElement::leaf("", "Autofill", "false"));
            }
            if !b.items.is_empty() {
                let mut ci = OutElement::branch("", "ChildItems");
                for item in &b.items {
                    ci.push(designer_item(item)?);
                }
                el.push(ci);
            }
            el
        }
        _ => OutElement::self_closing("", tag)
            .attr("name", d.name.clone())
            .attr("id", d.id.to_string()),
    })
}

/// Дописать Designer-тело подсказки в порядке Designer через табличный движок
/// (`emit_field_designer` по `TOOLTIP_BODY`): Width, AutoMaxWidth, Height, MaxWidth(каркас-суппресс),
/// AutoMaxHeight, HorizontalStretch, TextColor, Title(каркас, formatted-атрибут), GroupVertical,
/// GroupHorizontal(порядок групп ОБРАТНЫЙ EDT), VerticalAlign(каркас), Events(каркас). autoMax*:
/// OppositeBool ⇒ движок эмитит `false` при отсутствии в bag (канон-false), опускает `true`.
///
/// БАГФИКС (как EDT): движок эмитит `HorizontalStretch=false` (Symmetric) — прежний ручной писатель
/// эмитил лишь `true`, теряя `<HorizontalStretch>false>` из источника.
pub(crate) fn push_designer_tooltip_body(
    el: &mut OutElement,
    body: &TooltipBody,
) -> Result<(), FormError> {
    let get = |id: morph1c_core::ir::FieldId| {
        body.properties
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v)
    };
    // width — ДО AutoMaxWidth (геометрия LabelDecoration).
    emit_field_designer(el, tt_field(ld::F_WIDTH), &body.properties)?;
    emit_field_designer(el, tt_field(ld::F_AUTO_MAX_WIDTH), &body.properties)?;
    emit_field_designer(el, tt_field(ld::F_HEIGHT), &body.properties)?;
    // MaxWidth — каркас (`0`-суппресс; таблица его не эмитит).
    if let Some(PropertyValue::Int(n)) = get(ld::F_MAX_WIDTH) {
        if *n != 0 {
            el.push(OutElement::leaf("", "MaxWidth", n.to_string()));
        }
    }
    emit_field_designer(el, tt_field(ld::F_AUTO_MAX_HEIGHT), &body.properties)?;
    // maxHeight/verticalStretch — ERP-волна (позиции loose — одиночные witnesses).
    emit_field_designer(el, tt_field(ld::F_MAX_HEIGHT), &body.properties)?;
    emit_field_designer(el, tt_field(ld::F_HORIZONTAL_STRETCH), &body.properties)?;
    emit_field_designer(el, tt_field(ld::F_VERTICAL_STRETCH), &body.properties)?;
    // TextColor — Color, ДО Title (порядок Designer LabelDecoration; witness
    // УчетныеЗаписиЭлектроннойПочты `<TextColor>…</TextColor>` перед `<Title>`).
    emit_field_designer(el, tt_field(ld::F_TEXT_COLOR), &body.properties)?;
    // Font/Hyperlink/TitleHeight/BackColor/BorderColor — ERP-волна (loose позиции; ext-поля
    // подсказки Designer несёт прямыми детьми тела).
    if let Some(f) = &body.font {
        el.push(designer_font(f));
    }
    if let Some((_, PropertyValue::Bool(b))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_HYPERLINK)
    {
        el.push(OutElement::leaf(
            "",
            "Hyperlink",
            if *b { "true" } else { "false" },
        ));
    }
    if let Some((_, PropertyValue::Int(n))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_TITLE_HEIGHT)
    {
        el.push(OutElement::leaf("", "TitleHeight", n.to_string()));
    }
    for (fid, tag) in [
        (ld::F_EXT_BACK_COLOR, "BackColor"),
        (tables::F_LD_BORDER_COLOR, "BorderColor"),
    ] {
        if let Some((_, PropertyValue::Ref(c))) = body.ext_info.iter().find(|(k, _)| *k == fid) {
            el.push(OutElement::leaf(
                "",
                tag,
                super::super::fields::color_to_designer(c, tag)?,
            ));
        }
    }
    // Title — каркас (formatted-АТРИБУТ; FieldProj-кодек его не несёт).
    match get(ld::F_TITLE) {
        Some(PropertyValue::Localized(pairs)) if !pairs.is_empty() => {
            // Контрол-заголовок: formatted="false" (обычно) либо "true" (HTML-флаг подсказки).
            let f = if body.formatted { "true" } else { "false" };
            el.push(designer_title_with(pairs, Some(f)));
        }
        // Пустой заголовок + formatted=true: `<Title formatted="true"/>` самозакрытием
        // (зеркало decoration-пути; witness Пользователи.ФормаЭлемента tooltip с Events).
        _ if body.formatted => {
            el.push(OutElement::self_closing("", "Title").attr("formatted", "true"));
        }
        _ => {}
    }
    // groupVerticalAlign/groupHorizontalAlign — Enum, ПОСЛЕ Title, порядок групп ОБРАТНЫЙ EDT
    // (witness ВнешниеКомпоненты Title→GroupVerticalAlign).
    emit_field_designer(el, tt_field(ld::F_GROUP_VERTICAL_ALIGN), &body.properties)?;
    emit_field_designer(el, tt_field(ld::F_GROUP_HORIZONTAL_ALIGN), &body.properties)?;
    // HorizontalAlign — ext-поле подсказки; Designer опускает `Left` (EDT-каркас-дефолт),
    // эмитит не-дефолт ПОСЛЕ GroupHorizontalAlign (witness СообщениеSMS:
    // GroupHorizontalAlign→HorizontalAlign=Right).
    if let Some((_, PropertyValue::Enum(t))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_HORIZONTAL_ALIGN)
    {
        if t.as_str() != "Left" {
            el.push(OutElement::leaf(
                "",
                "HorizontalAlign",
                t.as_str().to_string(),
            ));
        }
    }
    // VerticalAlign — ext-поле подсказки; Designer эмитит прямым ребёнком ПОСЛЕ Title/групп.
    // Witness НастройкаСтандартногоИнтерфейсаOData.
    if let Some((_, PropertyValue::Enum(t))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_VERTICAL_ALIGN)
    {
        el.push(OutElement::leaf(
            "",
            "VerticalAlign",
            t.as_str().to_string(),
        ));
    }
    if !body.events.is_empty() {
        let mut events = OutElement::branch("", "Events");
        for ev in &body.events {
            events.push(designer_event(ev));
        }
        el.push(events);
    }
    Ok(())
}

pub(crate) fn designer_event(ev: &morph1c_core::ir::FormEvent) -> OutElement {
    // `<Event name="X">handler</Event>` — лист с атрибутом + текстом.
    OutElement::leaf("", "Event", ev.handler.clone()).attr("name", ev.name.clone())
}

pub(crate) fn designer_data_attribute(a: &FormDataAttribute) -> Result<OutElement, FormError> {
    designer_data_attribute_named(a, "Attribute")
}

/// Designer реквизит/колонка (`<Attribute>` или `<Column>`) с одинаковой структурой.
pub(crate) fn designer_data_attribute_named(
    a: &FormDataAttribute,
    tag: &str,
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", tag)
        .attr("name", a.name.clone())
        .attr("id", a.id.to_string());
    if let Some(PropertyValue::Localized(pairs)) = &a.title {
        if !pairs.is_empty() {
            el.push(designer_title(pairs, false));
        }
    }
    match &a.value_type {
        None => el.push(OutElement::self_closing("", "Type")),
        // scoped: envelope формы объявляет dcsset ⇒ без дублирующего инлайн-ns (SSL
        // ФормаНастроекОтчета `<v8:Type>dcsset:SettingsComposer` БЕЗ xmlns-атрибута).
        Some(ts) => el.push(
            crate::type_codec::encode_scoped(
                crate::TypeDialect::Designer,
                "",
                "Type",
                ts,
                DESIGNER_FORM_NS,
            )
            .map_err(FormError::Frame)?,
        ),
    }
    // View/Edit-права — СРАЗУ после Type (витнессы: Type→View / Type→Edit; УчетныеЗаписи…/
    // Взаимодействия/ВерсииОбъектов). Тег эмитится ⟺ доступ снят (Common=false) ЛИБО есть
    // РОЛЕВЫЕ исключения (`<xr:Value name>` на роль). `<xr:Common>` несёт РЕАЛЬНОЕ значение:
    // false = запрет (ERP-witness БухучетЗарплатыОрганизаций), true = общий доступ С ролевыми
    // исключениями (ERP-witness ФизическиеЛица.ФормаЭлемента `<Edit><xr:Common>true</xr:Common>
    // <xr:Value>×N`). (true,∅) неэмитируем (== absent-default) ⇒ условие держит round-trip.
    for (tag, allowed, roles) in [
        ("View", a.view_common, &a.view_roles),
        ("Edit", a.edit_common, &a.edit_roles),
    ] {
        if !allowed || !roles.is_empty() {
            let mut v = OutElement::branch("", tag);
            v.push(OutElement::leaf(
                "xr",
                "Common",
                if allowed { "true" } else { "false" },
            ));
            for (role, val) in roles.iter() {
                v.push(
                    OutElement::leaf("xr", "Value", if *val { "true" } else { "false" })
                        .attr("name", role.clone()),
                );
            }
            el.push(v);
        }
    }
    // Designer-порядок атрибута (сверено попарно, 0 контрпримеров): Type, MainAttribute, SavedData,
    // FillCheck, Save, Settings (MainAttribute<SavedData×155, SavedData<FillCheck×8, SavedData<Save×12,
    // FillCheck<Save×7). ПРОТИВОПОЛОЖНО EDT (там fillChecking, main, savedData) — пер-форматно.
    if a.main {
        el.push(OutElement::leaf("", "MainAttribute", "true"));
    }
    if a.saved_data {
        el.push(OutElement::leaf("", "SavedData", "true"));
    }
    if let Some(f) = &a.fill_checking {
        el.push(OutElement::leaf("", "FillCheck", f.clone()));
    }
    // UseAlways (`<Field>`) — ПЕРЕД Save (SSL Designer: UseAlways<Save×10, UseAlways<
    // Settings×172, 0 контрпримеров). Затем Save, затем FunctionalOptions (Save<
    // FunctionalOptions×7), затем ValueList-Settings (FunctionalOptions<Settings×4 —
    // witness ВнешниеПользователи.ФормаСписка «ВидПользователей»). Порядок ОБРАТНЫЙ
    // EDT-порядку (EDT: functionalOptions ПЕРЕД notDefaultUseAlwaysAttributes; §1.6).
    // Сигила `~` («поле не входит в состав полей динамического списка») — DESIGNER-ONLY
    // денормализация, снятая на чтении в канон (EDT-написание). Возвращаем её РОВНО тем путям,
    // что её несли (`designer_unavailable_paths`). См. `FormDataAttribute` в ir/form.rs.
    let sigil = |p: &String| -> String {
        if a.designer_unavailable_paths.contains(p) || super::super::availability::unavailable(p) {
            format!("~{p}")
        } else {
            p.clone()
        }
    };
    if !a.not_default_use_always.is_empty() {
        let mut u = OutElement::branch("", "UseAlways");
        for p in &a.not_default_use_always {
            u.push(OutElement::leaf("", "Field", sigil(p)));
        }
        el.push(u);
    }
    if !a.settings_saved_data.is_empty() {
        let mut s = OutElement::branch("", "Save");
        for p in &a.settings_saved_data {
            s.push(OutElement::leaf("", "Field", sigil(p)));
        }
        el.push(s);
    }
    if !a.functional_options.is_empty() {
        let mut fo = OutElement::branch("", "FunctionalOptions");
        for r in &a.functional_options {
            fo.push(OutElement::leaf("", "Item", r.clone()));
        }
        el.push(fo);
    }
    // ValueList-extInfo (`<Settings xsi:type="v8:TypeDescription">`) — ПОСЛЕ `<Save>`/
    // FunctionalOptions (сверено: ВыборФорматаВложений/ПодготовкаНовогоПисьма несут
    // `Type,Save,Settings`; ВнешниеПользователи — `FunctionalOptions,Settings`). Пустой
    // item-тип ⇒ самозакрытие (прежний корпус); заданный ⇒ type-codec (как `<Type>`).
    if a.value_list_ext {
        let settings = match &a.value_list_item_type {
            None => OutElement::self_closing("", "Settings").attr("xsi:type", "v8:TypeDescription"),
            Some(ts) => crate::type_codec::encode_scoped(
                crate::TypeDialect::Designer,
                "",
                "Settings",
                ts,
                DESIGNER_FORM_NS,
            )
            .map_err(FormError::Frame)?
            .attr("xsi:type", "v8:TypeDescription"),
        };
        el.push(settings);
    }
    // Табличный документ в настройках (`<Settings xmlns:mxl=… xsi:type="mxl:SpreadsheetDocument">`) —
    // Designer-only структурный блок (EDT несёт лишь пустой маркер spreadsheet_ext). Позиция — как у
    // ValueList-Settings (после Save/Type/SavedData; witness СравнениеТабличныхДокументов).
    if let Some(ss) = &a.spreadsheet_settings {
        el.push(designer_spreadsheet_settings(ss));
    }
    // `<Columns>` — ПОСЛЕ Save/UseAlways/FunctionalOptions (SSL Designer: Save<Columns×7,
    // UseAlways<Columns×5, FunctionalOptions<Columns×2). Прежде эмитился ДО UseAlways — ломал
    // порядок в ValueTable-реквизитах с UseAlways (напр. СтраныМира.Классификатор). Пустой набор
    // доп-колонок ⇒ САМОЗАКРЫТИЕ `<AdditionalColumns table="Путь"/>`.
    if !a.columns.is_empty() || !a.additional_columns.is_empty() {
        let mut cols = OutElement::branch("", "Columns");
        for col in &a.columns {
            cols.push(designer_data_attribute_named(col, "Column")?);
        }
        for ac in &a.additional_columns {
            if ac.columns.is_empty() {
                cols.push(
                    OutElement::self_closing("", "AdditionalColumns")
                        .attr("table", ac.table_path.clone()),
                );
                continue;
            }
            let mut ace =
                OutElement::branch("", "AdditionalColumns").attr("table", ac.table_path.clone());
            for col in &ac.columns {
                ace.push(designer_data_attribute_named(col, "Column")?);
            }
            cols.push(ace);
        }
        el.push(cols);
    }
    // Диаграмма/GanttChart в настройках (`<Settings xmlns:d4p1=… xsi:type="d4p1:Chart">`) —
    // Designer-инлайн Chart-реквизита (EDT держит тело в сайдкаре `Attributes/<attr>/ExtInfo/
    // *.chart`, транскодит pipeline; Designer несёт полное тело здесь). Взаимоисключимо с
    // spreadsheet/dynamic_list — та же `<Settings>`-семья.
    if let Some(cs) = &a.chart_settings {
        el.push(super::super::chart::designer_chart_settings(cs)?);
    }
    // `<Settings xsi:type="DynamicList">` — динамический список, ПОСЛЕДНИМ (после UseAlways/
    // FunctionalOptions; corpus fact ВыборКонтакта/АдреснаяКнига).
    if let Some(dl) = &a.dynamic_list {
        el.push(designer_dynamic_list_attr(dl)?);
    }
    Ok(el)
}
