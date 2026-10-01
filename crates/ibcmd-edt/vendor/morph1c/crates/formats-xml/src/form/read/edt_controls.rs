//! READ · EDT dialect reader — the control tree: items, fields, groups, tables (incl.
//! dynamic-list ext + addition regions), buttons, decorations, tooltips, context menus, titles.

use super::*;

/// EDT диспетчер контрола `<items xsi:type="form:X">` → [`FormItem`] по виду.
pub(crate) fn read_edt_item(el: &Element) -> Result<FormItem, FormError> {
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("items: missing xsi:type".into()))?;
    match xt.value.as_str() {
        "form:Decoration" => read_edt_decoration(el),
        "form:FormGroup" => read_edt_form_group(el),
        "form:Button" => read_edt_button(el),
        "form:FormField" => read_edt_form_field(el),
        "form:Table" => read_edt_table(el),
        "form:Addition" => read_edt_addition_control(el),
        other => Err(FormError::Frame(format!(
            "items xsi:type={other:?}: unsupported control kind (§1.0 — no Raw)"
        ))),
    }
}

/// EDT `<items xsi:type="form:Addition">` — ДОБАВЛЕНИЕ-КАК-КОНТРОЛ (список-поиск в дереве,
/// напр. под формой-`autoCommandBar`). В отличие от Table-именованного добавления несёт
/// КАРКАС видимости `visible`/`enabled`/`userVisible` + идентичность + декораторы +
/// дискриминатор `<type>` (ОТСУТСТВИЕ = SearchString) + `source` + `groupHorizontalAlign` +
/// extInfo(`width`/`autoMaxWidth`/`horizontalStretch`). `visible`: присутствие ⟺ `true`
/// (проверяется, НЕ хранится); ОТСУТСТВИЕ ⟺ `false` (хранится sparse — Designer эмитит
/// `<Visible>false`; witness ГрупповоеИзменениеРеквизитов.ВыбранныеЭлементы). `enabled`/
/// `userVisible` — всегда true (иное — §1.0). Модель [`FormItem`] ИДЕНТИЧНА
/// Table-именованному добавлению ⇒ Designer-сторона переиспользует
/// `read_designer_addition`/`designer_addition`. Порядок EDT: title, [visible], enabled,
/// userVisible, name, id, extendedTooltip, contextMenu, [type], source,
/// [groupHorizontalAlign], extInfo.
pub(crate) fn read_edt_addition_control(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("items: missing xsi:type".into()))?;
    xt.claimed.set(true);
    // title (опц., Localized) — ПЕРВЫЙ ребёнок (до каркаса видимости). МУЛЬТИЯЗЫЧНЫЙ —
    // ПОВТОРЯЕМЫЙ элемент по одному на язык (ERP-witness ОстаткиАлкогольнойПродукцииЕГАИС.
    // ФормаОстатков СтрокаПоискаСписка: ru+en) — тот же сбор, что у корня/реквизита.
    let mut title = match el.child("title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_edt_title(t)?),
        None => None,
    };
    for t in el
        .children
        .iter()
        .filter(|c| c.local == "title" && c.prefix.is_empty())
        .skip(1)
    {
        match (title.as_mut(), read_edt_title(t)?) {
            (Some(PropertyValue::Localized(acc)), PropertyValue::Localized(pairs)) => {
                acc.extend(pairs)
            }
            (got, extra) => {
                return Err(FormError::Frame(format!(
                    "form:Addition <title>: unexpected repetition shapes {got:?} + {extra:?} (§1.0)"
                )));
            }
        }
    }
    // Каркас видимости: `visible` присутствует ⟺ true; отсутствует ⟺ false (sparse-хранение,
    // симметрично Designer `<Visible>false`). `enabled` — всегда true.
    let mut visibility = Vec::new();
    match el.child("visible").filter(|c| c.prefix.is_empty()) {
        Some(v) => {
            if !matches!(read_bool_text(v, "visible")?, PropertyValue::Bool(true)) {
                return Err(FormError::Frame(
                    "form:Addition <visible> must be true (§1.0)".into(),
                ));
            }
        }
        None => visibility.push((tb::F_ADDITION_VISIBLE, PropertyValue::Bool(false))),
    }
    let en = el
        .child("enabled")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("form:Addition: no <enabled> (§1.0)".into()))?;
    if !matches!(read_bool_text(en, "enabled")?, PropertyValue::Bool(true)) {
        return Err(FormError::Frame(
            "form:Addition <enabled> must be true (§1.0)".into(),
        ));
    }
    let uv = el
        .child("userVisible")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("form:Addition: no <userVisible> (§1.0)".into()))?;
    read_common_true(uv, "userVisible")?;
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    // Дискриминатор вида: прямой `<type>` (SearchControl/ViewStatus) или ОТСУТСТВИЕ (SearchString).
    let ak = match el.child("type").filter(|c| c.prefix.is_empty()) {
        Some(t) => {
            t.claim_with_text();
            tables::ADDITION_KINDS
                .iter()
                .find(|k| k.edt_type == Some(t.text.as_str()))
                .ok_or_else(|| {
                    FormError::Frame(format!(
                        "form:Addition type={:?}: unsupported control kind (§1.0 — no Raw)",
                        t.text
                    ))
                })?
        }
        None => tables::addition_kind("SearchStringAddition").expect("registered"),
    };
    let mut item = FormItem::new(FormControlKind::new(ak.kind), name, id);
    // displayImportance (опц., ГОЛОВА; ⟺ Designer атрибут) — ПЕРВЫМ в bag (X-сравнимо с
    // designer-ридером). cf НЕ витнесснут ⇒ отказ cf-write.
    if let Some(di) = leaf_text_opt(el, "displayImportance") {
        item.properties.push((
            tables::F_ADDITION_DISPLAY_IMPORTANCE,
            PropertyValue::Enum(Token::new(di)),
        ));
    }
    item.properties.extend(visibility);
    if let Some(t) = title {
        item.properties.push((tb::F_ADDITION_TITLE, t));
    }
    // Вложенные контролы (`<items>`; ДО extendedTooltip — метамодель Addition#13). ERP-witness
    // Document.ВходящийЗапросФССДляРасчетаПособия.ФормаСписка: SearchControl-добавление-контрол
    // с Button внутри. Зеркально Table-именованному `read_edt_addition` (тот тоже читает items).
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        item.children.push(read_edt_item(c)?);
    }
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    // toolTipRepresentation (опц., ТЕЛО; метамодель Addition#5). Пуш ДО source — зеркально
    // остальным ридерам добавлений.
    if let Some(ttr) = el
        .child("toolTipRepresentation")
        .filter(|c| c.prefix.is_empty())
    {
        ttr.claim_with_text();
        item.properties.push((
            tables::F_ADDITION_TOOL_TIP_REPRESENTATION,
            PropertyValue::Enum(Token::new(ttr.text.clone())),
        ));
    }
    // source ОПЦИОНАЛЕН: добавление без источника его не несёт (⟺ Designer без `<AdditionSource>`;
    // cf даёт типизированный отказ). Пушим лишь при наличии.
    if let Some(source) = leaf_text_opt(el, "source") {
        item.properties
            .push((tb::F_ADDITION_SOURCE, PropertyValue::Ref(source)));
    }
    // groupHorizontalAlign (опц., ТЕЛО: метамодель Addition type→source→groupHorizontalAlign;
    // witness ЗагрузкаКурсовВалют `Right`).
    if let Some(g) = el
        .child("groupHorizontalAlign")
        .filter(|c| c.prefix.is_empty())
    {
        g.claim_with_text();
        item.properties.push((
            tb::F_ADDITION_GROUP_HORIZONTAL_ALIGN,
            PropertyValue::Enum(Token::new(g.text.clone())),
        ));
    }
    // extInfo: `<extInfo xsi:type="form:…AdditionExtInfo">[width][autoMaxWidth]
    // [horizontalStretch]</extInfo>` (порядок = метамодель SearchStringAdditionExtInfo).
    let ext = el
        .child("extInfo")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("form:Addition: no extInfo (§1.0)".into()))?;
    ext.claim();
    let xti = ext
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("form:Addition extInfo: no xsi:type".into()))?;
    if xti.value != ak.ext_xsi {
        return Err(FormError::Frame(format!(
            "form:Addition extInfo xsi:type={:?}, want {:?}",
            xti.value, ak.ext_xsi
        )));
    }
    xti.claimed.set(true);
    if let Some(w) = ext.child("width").filter(|c| c.prefix.is_empty()) {
        w.claim_with_text();
        item.ext_info.push((
            tb::F_ADDITION_WIDTH,
            PropertyValue::Int(parse_int(&w.text)?),
        ));
    }
    // autoMaxWidth — OppositeBool: EDT эмитит лишь `true` (bag sparse-true); отсутствие ⟺ false.
    if let Some(amw) = ext.child("autoMaxWidth").filter(|c| c.prefix.is_empty()) {
        if !matches!(
            read_bool_text(amw, "autoMaxWidth")?,
            PropertyValue::Bool(true)
        ) {
            return Err(FormError::Frame(
                "form:Addition extInfo <autoMaxWidth>false unwitnessed (§1.0 — EDT omits false)"
                    .into(),
            ));
        }
        item.ext_info
            .push((tb::F_ADDITION_AUTO_MAX_WIDTH, PropertyValue::Bool(true)));
    }
    // maxWidth (опц., ext; метамодель autoMaxWidth→maxWidth).
    if let Some(mw) = ext.child("maxWidth").filter(|c| c.prefix.is_empty()) {
        mw.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_MAX_WIDTH,
            PropertyValue::Int(parse_int(&mw.text)?),
        ));
    }
    if let Some(hs) = ext
        .child("horizontalStretch")
        .filter(|c| c.prefix.is_empty())
    {
        item.ext_info.push((
            tb::F_ADDITION_HORIZONTAL_STRETCH,
            read_bool_text(hs, "horizontalStretch")?,
        ));
    }
    // horizontalLocation (опц., ext; ТОЛЬКО ViewStatus).
    if let Some(hl) = ext
        .child("horizontalLocation")
        .filter(|c| c.prefix.is_empty())
    {
        if ak.kind != "ViewStatusAddition" {
            return Err(FormError::Frame(format!(
                "{}: <horizontalLocation> is a ViewStatusAddition-only property (§1.0)",
                ak.kind
            )));
        }
        hl.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_HORIZONTAL_LOCATION,
            PropertyValue::Enum(Token::new(hl.text.clone())),
        ));
    }
    expect_only_children(
        ext,
        &[
            "width",
            "autoMaxWidth",
            "maxWidth",
            "horizontalStretch",
            "horizontalLocation",
        ],
    )?;

    expect_only_children(
        el,
        &[
            "title",
            "visible",
            "enabled",
            "userVisible",
            "name",
            "id",
            "displayImportance",
            "items",
            "extendedTooltip",
            "contextMenu",
            "type",
            "toolTipRepresentation",
            "source",
            "groupHorizontalAlign",
            "extInfo",
        ],
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "form:Addition {:?}: {leftover} unconsumed node(s) (§1.0)",
            item.name
        )));
    }
    Ok(item)
}

/// EDT `FormField` (`<items xsi:type="form:FormField">`, дискриминатор `<type>`): общее
/// тело + типо-специфичный `extInfo` по таблицам проекции. Неподдержанный `<type>` —
/// громкая §1.0-ошибка (частотный сигнал свипа).
pub(crate) fn read_edt_form_field(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("FormField: no xsi:type".into()))?;
    xt.claimed.set(true);
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let ftype = leaf_text(el, "type")?;
    let fk = tables::field_kind(&ftype).ok_or_else(|| {
        FormError::Frame(format!(
            "FormField type={ftype:?}: unsupported control kind (§1.0 — no Raw)"
        ))
    })?;
    let mut item = FormItem::new(FormControlKind::new(fk.kind), name, id);

    // Общие поля тела (таблично; политики пер-форматных дефолтов внутри).
    read_fields_edt(
        fk.kind,
        Some(el),
        tables::FORM_FIELD_COMMON,
        Region::Body,
        &mut item.properties,
    )?;
    // titleFont — композит-шрифт заголовка (метамодель FormField: titleTextColor → titleFont →
    // visible; witness РаботаСФайлами «Владелец»).
    if let Some(tf) = el.child("titleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_edt_font(tf)?);
    }
    // footerFont — шрифт подвала колонки (field-common; ERP ×56; ⟷ Designer `<FooterFont>`).
    if let Some(ff2) = el.child("footerFont").filter(|c| c.prefix.is_empty()) {
        item.footer_font = Some(read_edt_font(ff2)?);
    }
    // СОБСТВЕННЫЕ исключённые команды поля (SpreadsheetDocumentField; регион после головных
    // полей, ДО decorator-стабов). Прочие поля их не несут (аддитивно, безопасно).
    for x in el
        .children
        .iter()
        .filter(|c| c.local == "excludedCommands" && c.prefix.is_empty())
    {
        x.claim_with_text();
        expect_no_children(x)?;
        item.excluded_commands.push(x.text.clone());
    }
    // Тело-события (`OnChange`; регион между toolTipRepresentation и extendedTooltip).
    for h in el
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        item.events.push(read_handlers(h)?);
    }
    // Decorator-стабы.
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    // extInfo: типо-специфичные события + поля.
    let ext = el.child("extInfo").filter(|c| c.prefix.is_empty());
    if let Some(ex) = ext {
        ex.claim();
        let xti = ex
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("FormField extInfo: no xsi:type".into()))?;
        if xti.value != fk.ext_xsi {
            return Err(FormError::Frame(format!(
                "FormField extInfo xsi:type={:?}, want {:?}",
                xti.value, fk.ext_xsi
            )));
        }
        xti.claimed.set(true);
        for h in ex
            .children
            .iter()
            .filter(|c| c.local == "handlers" && c.prefix.is_empty())
        {
            item.events.push(read_handlers(h)?);
        }
        read_fields_edt(fk.kind, Some(ex), fk.ext, Region::Ext, &mut item.ext_info)?;
        // АВТО-ТАБЛИЦА (GanttChartField): `<extInfo><autoTable>` — структурно полная Таблица без
        // xsi:type-обёртки. Читается Table-движком в item.auto_table (X-сравнима как обычная таблица).
        if tables::field_kind_has_auto_table(fk.kind) {
            if let Some(at) = ex.child("autoTable").filter(|c| c.prefix.is_empty()) {
                item.auto_table = Some(Box::new(read_edt_auto_table(at)?));
            }
        }
        // ДОБАВЛЕНИЯ extInfo (PDFDocumentField): `<extInfo><viewStatusAddition>` — тот же
        // Addition-контрол, что у Таблицы. Читается в item.additions.
        if tables::field_kind_has_ext_additions(fk.kind) {
            for ak in tables::ADDITION_KINDS {
                if let Some(a) = ex.child(ak.edt_tag).filter(|c| c.prefix.is_empty()) {
                    item.additions.push(read_edt_addition(a, ak)?);
                }
            }
        }
        // extInfo `font` — композит-шрифт (метамодель Label: backColor→font→useCopy; Input:
        // textSize(55)→font(56); Radio: horizontalStretch(9)→font(10); Text/Formatted/Picture —
        // свои хвостовые позиции). Witnessed виды — [`tables::field_kind_carries_font`];
        // у прочих НЕ витнессирован — громкая §1.0-ошибка через expect_only ниже.
        if tables::field_kind_carries_font(fk.kind) {
            if let Some(fnt) = ex.child("font").filter(|c| c.prefix.is_empty()) {
                item.font = Some(read_edt_font(fnt)?);
            }
        }
        // Разрешённые НЕ-полевые дети extInfo: handlers всегда; font — у font-видов;
        // autoTable — у GanttChartField; addition-теги — у PDFDocumentField.
        let mut ext_head: Vec<&str> = vec!["handlers"];
        if tables::field_kind_carries_font(fk.kind) {
            ext_head.push("font");
        }
        if tables::field_kind_has_auto_table(fk.kind) {
            ext_head.push("autoTable");
        }
        if tables::field_kind_has_ext_additions(fk.kind) {
            for ak in tables::ADDITION_KINDS {
                ext_head.push(ak.edt_tag);
            }
        }
        expect_only_children_ext(ex, &ext_head, fk.ext.iter().map(|e| e.edt))?;
    } else {
        read_fields_edt(fk.kind, None, fk.ext, Region::Ext, &mut item.ext_info)?;
    }

    expect_only_children_ext(
        el,
        &[
            "name",
            "id",
            "type",
            "handlers",
            "extendedTooltip",
            "contextMenu",
            "extInfo",
            "excludedCommands",
            "titleFont",
            "footerFont",
        ],
        tables::FORM_FIELD_COMMON.iter().map(|e| e.edt),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "FormField {:?} ({}): {leftover} unconsumed node(s) (§1.0)",
            item.name, fk.kind
        )));
    }
    Ok(item)
}

/// EDT семейство `Decoration` (`<items xsi:type="form:Decoration">`): `<type>Label</type>`
/// ⇒ `LabelDecoration`; ОТСУТСТВИЕ `<type>` ⇒ `PictureDecoration` (corpus fact: 157/100).
/// `title`+`formatted` — каркас-glue (Designer несёт `formatted` АТРИБУТОМ на `<Title>`);
/// прочие поля таблично. `font`/`border` НЕ смоделированы — громкая §1.0-ошибка.
pub(crate) fn read_edt_decoration(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("items: missing xsi:type".into()))?;
    xt.claimed.set(true);
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let dk = match el.child("type").filter(|c| c.prefix.is_empty()) {
        Some(t) => {
            t.claim_with_text();
            if t.text != "Label" {
                return Err(FormError::Frame(format!(
                    "Decoration type={:?}: unsupported control kind (§1.0 — no Raw)",
                    t.text
                )));
            }
            tables::decoration_kind("LabelDecoration").expect("registered")
        }
        None => tables::decoration_kind("PictureDecoration").expect("registered"),
    };
    let mut item = FormItem::new(FormControlKind::new(dk.kind), name, id);

    // title + formatted — glue (порядок пуша title→formatted общий обоим форматам, §1.6).
    // МНОГОЯЗЫЧНЫЙ заголовок — ПОВТОРЯЮЩИЙСЯ `<title>` (по одному на язык); сливаем все.
    if let Some(title) = read_edt_title_multi(el, "title")? {
        item.properties.push((ld::F_TITLE, title));
    }
    if let Some(f) = el.child("formatted").filter(|c| c.prefix.is_empty()) {
        if !matches!(read_bool_text(f, "formatted")?, PropertyValue::Bool(true)) {
            return Err(FormError::Frame(
                "decoration <formatted> must be true (§1.0)".into(),
            ));
        }
        item.properties
            .push((ld::F_FORMATTED, PropertyValue::Bool(true)));
    }
    read_fields_edt(
        dk.kind,
        Some(el),
        tables::DECORATION_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    if let Some(fnt) = el.child("font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_edt_font(fnt)?);
    }
    // extInfo: события + типо-специфичные поля.
    let ext = el.child("extInfo").filter(|c| c.prefix.is_empty());
    if let Some(ex) = ext {
        ex.claim();
        let xti = ex
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("Decoration extInfo: no xsi:type".into()))?;
        if xti.value != dk.ext_xsi {
            return Err(FormError::Frame(format!(
                "Decoration extInfo xsi:type={:?}, want {:?}",
                xti.value, dk.ext_xsi
            )));
        }
        xti.claimed.set(true);
        for h in ex
            .children
            .iter()
            .filter(|c| c.local == "handlers" && c.prefix.is_empty())
        {
            item.events.push(read_handlers(h)?);
        }
        read_fields_edt(dk.kind, Some(ex), dk.ext, Region::Ext, &mut item.ext_info)?;
        expect_only_children_ext(ex, &["handlers"], dk.ext.iter().map(|e| e.edt))?;
    } else {
        read_fields_edt(dk.kind, None, dk.ext, Region::Ext, &mut item.ext_info)?;
    }

    expect_only_children_ext(
        el,
        &[
            "name",
            "id",
            "type",
            "title",
            "formatted",
            "extendedTooltip",
            "contextMenu",
            "extInfo",
            "font",
        ],
        tables::DECORATION_BODY.iter().map(|e| e.edt),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{} {:?}: {leftover} unconsumed node(s) (§1.0)",
            dk.kind, item.name
        )));
    }
    Ok(item)
}

/// EDT семейство `FormGroup` (`<items xsi:type="form:FormGroup">`): дискриминатор —
/// `<type>UsualGroup|Pages|Page</type>`, у `ButtonGroup` `<type>` ОТСУТСТВУЕТ. Общее тело +
/// типо-специфичный extInfo по таблицам; дети `<items>` идут СРАЗУ после `<id>` (corpus fact).
/// Неподдержанный `<type>` (CommandBar/Popup/ColumnGroup/…) — громкая §1.0-ошибка.
pub(crate) fn read_edt_form_group(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("FormGroup: no xsi:type".into()))?;
    xt.claimed.set(true);
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    // Дискриминатор: наличие/значение `<type>`.
    let gk = match el.child("type").filter(|c| c.prefix.is_empty()) {
        Some(t) => {
            t.claim_with_text();
            tables::GROUP_KINDS
                .iter()
                .find(|k| k.edt_type == Some(t.text.as_str()))
                .ok_or_else(|| {
                    FormError::Frame(format!(
                        "FormGroup type={:?}: unsupported control kind (§1.0 — no Raw)",
                        t.text
                    ))
                })?
        }
        None => tables::group_kind("ButtonGroup").expect("ButtonGroup registered"),
    };
    let mut item = FormItem::new(FormControlKind::new(gk.kind), name, id);

    // ПРАВИЛО ПОРЯДКА ДЕТЕЙ ГРУППЫ (corpus fact, byte-exact-критично): в EDT дочерние
    // контролы группы `<items>` идут СРАЗУ после `<name>`/`<id>` (и `<displayImportance>`),
    // ДО остальных свойств тела/extInfo — а НЕ после extInfo (как ошибочно предполагала
    // ранняя модель, сломанная корпусом). Соответственно writer (`edt_form_group`) эмитит
    // детей сразу после `displayImportance`. Тут порядок чтения не критичен (дети собираются
    // по имени тега), но фиксируем правило здесь, чтобы reader/writer не разошлись.
    // Designer держит детей в отдельной обёртке `<ChildItems>` в конце (см. DES_GROUP_ORDER).
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        item.children.push(read_edt_item(c)?);
    }
    // Общие поля тела (таблично).
    read_fields_edt(
        gk.kind,
        Some(el),
        tables::FORM_GROUP_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    // titleFont — композит-шрифт заголовка (метамодель FormGroup: titleTextColor → titleFont →
    // toolTip; witness МашиночитаемыеДоверенности «Лицо, представляющее организацию…»).
    if let Some(tf) = el.child("titleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_edt_font(tf)?);
    }
    // Decorator-стабы.
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    // extInfo: события группы (Pages `OnCurrentPageChange`) + типо-специфичные поля.
    let ext = el.child("extInfo").filter(|c| c.prefix.is_empty());
    if let Some(ex) = ext {
        ex.claim();
        let xti = ex
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("FormGroup extInfo: no xsi:type".into()))?;
        if xti.value != gk.ext_xsi {
            return Err(FormError::Frame(format!(
                "FormGroup extInfo xsi:type={:?}, want {:?}",
                xti.value, gk.ext_xsi
            )));
        }
        xti.claimed.set(true);
        for h in ex
            .children
            .iter()
            .filter(|c| c.local == "handlers" && c.prefix.is_empty())
        {
            item.events.push(read_handlers(h)?);
        }
        read_fields_edt(gk.kind, Some(ex), gk.ext, Region::Ext, &mut item.ext_info)?;
        expect_only_children_ext(ex, &["handlers"], gk.ext.iter().map(|e| e.edt))?;
    } else {
        read_fields_edt(gk.kind, None, gk.ext, Region::Ext, &mut item.ext_info)?;
    }

    expect_only_children_ext(
        el,
        &[
            "name",
            "id",
            "type",
            "items",
            "extendedTooltip",
            "contextMenu",
            "extInfo",
            "titleFont",
        ],
        tables::FORM_GROUP_BODY.iter().map(|e| e.edt),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{} {:?}: {leftover} unconsumed node(s) (§1.0)",
            gk.kind, item.name
        )));
    }
    Ok(item)
}

/// EDT `Button` (`<items xsi:type="form:Button">`): ПОЛНОЕ тело таблично (LANE-F-2).
/// Вид кнопки — ДАННЫЕ-поле `<type>` (EDT-дефолт `CommandBarButton` — омиссия).
/// `font` НЕ смоделирован — громкая §1.0-ошибка.
pub(crate) fn read_edt_button(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("Button: no xsi:type".into()))?;
    xt.claimed.set(true);
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let mut item = FormItem::new(FormControlKind::new("Button"), name, id);

    read_fields_edt(
        "Button",
        Some(el),
        tables::BUTTON_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    if let Some(fnt) = el.child("font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_edt_font(fnt)?);
    }

    expect_only_children_ext(
        el,
        &["name", "id", "extendedTooltip", "contextMenu", "font"],
        tables::BUTTON_BODY.iter().map(|e| e.edt),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Button {:?}: {leftover} unconsumed node(s) (§1.0)",
            item.name
        )));
    }
    Ok(item)
}

/// EDT `Table` (`<items xsi:type="form:Table">`, LANE-F-4) — крупнейший композит. Тело
/// таблично; СОБСТВЕННЫЕ `<excludedCommands>`/`<handlers>`/`<autoCommandBar>`; ТРИ добавления;
/// колонки `<items>` (FormField/ColumnGroup); decorator-стабы. `<extInfo>` (DynamicList) НЕ
/// смоделирована — громкая §1.0-ошибка (форма-блокер).
pub(crate) fn read_edt_table(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("Table: no xsi:type".into()))?;
    xt.claimed.set(true);
    read_edt_table_body(el)
}

/// Прочитать EDT `<autoTable>` GanttChartField — структурно ПОЛНАЯ Таблица, но БЕЗ обёртки
/// `<items xsi:type="form:Table">` (тег `autoTable`, xsi:type отсутствует). Переиспользует
/// [`read_edt_table_body`] (весь Table-движок: тело/добавления/autoCommandBar/декораторы/
/// колонки). Witness — DataProcessor.ДиспетчированиеГрафикаПроизводства.Планирование.
pub(crate) fn read_edt_auto_table(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    read_edt_table_body(el)
}

/// Тело EDT-Таблицы (общее для `<items form:Table>` и `<autoTable>`): всё, кроме внешней
/// обёртки/xsi:type (их claim'ит вызывающий). Порядок чтения тегов неважен (read по имени).
pub(crate) fn read_edt_table_body(el: &Element) -> Result<FormItem, FormError> {
    let name = leaf_text(el, "name")?;
    // id ОПЦИОНАЛЕН: главная `<autoTable>` без `<id>` ⇒ 0 (⟺ Designer `id="0"`). См. [`read_edt_id`].
    let id = read_edt_id(el)?;
    let mut item = FormItem::new(FormControlKind::new("Table"), name, id);

    // Тело (все поля таблично — читаются по имени тега, порядок неважен на read).
    read_fields_edt(
        "Table",
        Some(el),
        tables::TABLE_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    // titleFont/font — композит-шрифты Таблицы (метамодель Table: titleTextColor→titleFont;
    // borderColor→font; ERP-witness ×60/×3). Общие обоим форматам (X-сравнимы).
    if let Some(tf) = el.child("titleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_edt_font(tf)?);
    }
    if let Some(fnt) = el.child("font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_edt_font(fnt)?);
    }
    // showCommandBar три-состояние (см. `read_edt_table_show_command_bar`).
    item.show_command_bar = read_edt_table_show_command_bar(el)?;
    // Собственные исключённые команды.
    for x in el
        .children
        .iter()
        .filter(|c| c.local == "excludedCommands" && c.prefix.is_empty())
    {
        x.claim_with_text();
        expect_no_children(x)?;
        item.excluded_commands.push(x.text.clone());
    }
    // Собственные события таблицы.
    for h in el
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        item.events.push(read_handlers(h)?);
    }
    // Колонки (`<items>` — FormField/ColumnGroup, рекурсивно).
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        item.children.push(read_edt_item(c)?);
    }
    // Собственная командная панель.
    if let Some(acb) = el.child("autoCommandBar").filter(|c| c.prefix.is_empty()) {
        item.auto_command_bar = Some(read_edt_auto_command_bar(acb)?);
    }
    // Добавления (searchString/viewStatus/searchControl).
    for ak in tables::ADDITION_KINDS {
        if let Some(a) = el.child(ak.edt_tag).filter(|c| c.prefix.is_empty()) {
            item.additions.push(read_edt_addition(a, ak)?);
        }
    }
    // Decorator-стабы таблицы.
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    // extInfo динамического списка (`form:DynamicListTableExtInfo`) — обёртка полей + handlers.
    if let Some(ex) = el.child("extInfo").filter(|c| c.prefix.is_empty()) {
        item.dynamic_list_ext = Some(read_edt_dynamic_list_ext(ex)?);
    }

    expect_only_children_ext(
        el,
        &[
            "name",
            "id",
            "excludedCommands",
            "handlers",
            "items",
            "autoCommandBar",
            "extendedTooltip",
            "contextMenu",
            "showCommandBar",
            "showCommandBarNeedDereferenced",
            "extInfo",
            "titleFont",
            "font",
        ],
        tables::ADDITION_KINDS
            .iter()
            .map(|a| a.edt_tag)
            .chain(tables::TABLE_BODY.iter().map(|e| e.edt)),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Table {:?}: {leftover} unconsumed node(s) (no Raw — §1.0): {:?}",
            item.name,
            unclaimed_labels(el)
        )));
    }
    Ok(item)
}

/// xsi:type маркера extInfo Таблицы-динамического-списка.
pub(crate) const DYNAMIC_LIST_EXT_KIND: &str = "form:DynamicListTableExtInfo";

/// Прочитать EDT `<extInfo xsi:type="form:DynamicListTableExtInfo">` → [`DynamicListExt`].
/// Несёт СОБСТВЕННЫЕ обработчики (`<handlers>` — `OnGetDataAtServer`/…; хранятся ОТДЕЛЬНО, для
/// X сливаются в `item.events`) + канонические поля (`autoRefreshPeriod`/`period`/
/// `topLevelParent`/`showRoot`/`allowGettingCurrentRowURL`/`userSettingsGroup`; Keep-политики
/// заполняют дефолты Designer-ВСЕГДА-полей). Чужой xsi:type / незнакомый под-элемент — §1.0-ошибка.
pub(crate) fn read_edt_dynamic_list_ext(ex: &Element) -> Result<DynamicListExt, FormError> {
    ex.claim();
    let xt = ex
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("Table extInfo: no xsi:type".into()))?;
    if xt.value != DYNAMIC_LIST_EXT_KIND {
        return Err(FormError::Frame(format!(
            "Table extInfo xsi:type={:?}, want {DYNAMIC_LIST_EXT_KIND:?} (§1.0 — no Raw)",
            xt.value
        )));
    }
    xt.claimed.set(true);
    let mut events = Vec::new();
    for h in ex
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        events.push(read_handlers(h)?);
    }
    let mut fields = Vec::new();
    read_fields_edt(
        "Table",
        Some(ex),
        tables::DYNAMIC_LIST_EXT,
        Region::Ext,
        &mut fields,
    )?;
    expect_only_children_ext(
        ex,
        &["handlers"],
        tables::DYNAMIC_LIST_EXT.iter().map(|e| e.edt),
    )?;
    let leftover = ex.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "DynamicListTableExtInfo: {leftover} unconsumed node(s) (no Raw — §1.0): {:?}",
            unclaimed_labels(ex)
        )));
    }
    Ok(DynamicListExt { fields, events })
}

/// Прочитать три-состояние показа командной панели Таблицы. EDT кодирует ДВУМЯ полями:
/// `<showCommandBar>true|false` (для `true`/`false`) ИЛИ
/// `<showCommandBarNeedDereferenced>true` (для `auto`). Взаимоисключающие; оба — §1.0-ошибка.
/// Возвращает `None`, если ни одного нет.
pub(crate) fn read_edt_table_show_command_bar(el: &Element) -> Result<Option<String>, FormError> {
    let scb = el.child("showCommandBar").filter(|c| c.prefix.is_empty());
    let need = el
        .child("showCommandBarNeedDereferenced")
        .filter(|c| c.prefix.is_empty());
    match (scb, need) {
        (Some(_), Some(_)) => Err(FormError::Frame(
            "Table: both <showCommandBar> and <showCommandBarNeedDereferenced> (§1.0)".into(),
        )),
        (Some(c), None) => {
            c.claim_with_text();
            match c.text.as_str() {
                "true" | "false" => Ok(Some(c.text.clone())),
                other => Err(FormError::Frame(format!(
                    "<showCommandBar>={other:?}, want true/false (§1.0)"
                ))),
            }
        }
        (None, Some(c)) => {
            c.claim_with_text();
            if c.text != "true" {
                return Err(FormError::Frame(format!(
                    "<showCommandBarNeedDereferenced>={:?}, want true (§1.0)",
                    c.text
                )));
            }
            Ok(Some("auto".to_string()))
        }
        (None, None) => Ok(None),
    }
}

/// EDT ДОБАВЛЕНИЕ Таблицы (`<searchStringAddition>`/…) → [`FormItem`] вида-добавления.
/// Порядок: name, id, extendedTooltip, contextMenu, [type], source, extInfo(autoMaxWidth).
pub(crate) fn read_edt_addition(
    el: &Element,
    ak: &tables::AdditionKind,
) -> Result<FormItem, FormError> {
    el.claim();
    // Каркас видимости добавления (SPARSE head, ДО name): EDT опускает дефолт `true`, эмитит
    // лишь `false` (witnessed `<enabled>false` — АвтономнаяРаботаВМоделиСервиса/
    // НастройкиСинхронизацииДанных). Хранится в properties (X-сравним; расхождение ⇒ X=FALSE).
    // `userVisible` на Table-именованном добавлении корпусом НЕ витнессирован → §1.0 loud-error.
    let mut visibility = Vec::new();
    for (tag, fid) in [
        ("visible", tb::F_ADDITION_VISIBLE),
        ("enabled", tb::F_ADDITION_ENABLED),
    ] {
        if let Some(v) = el.child(tag).filter(|c| c.prefix.is_empty()) {
            visibility.push((fid, read_bool_text(v, tag)?));
        }
    }
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let mut item = FormItem::new(FormControlKind::new(ak.kind), name, id);
    // displayImportance (опц., ГОЛОВА после id; witness ФормаВыбораПолейПоиска `VeryHigh`). ⟺
    // Designer атрибут `DisplayImportance`. Пуш ПЕРВЫМ в bag — X-сравнимо с designer-ридером
    // (тот тоже кладёт DisplayImportance ПЕРВЫМ). cf-ячейка НЕ витнесснута ⇒ отказ cf-write.
    if let Some(di) = leaf_text_opt(el, "displayImportance") {
        item.properties.push((
            tables::F_ADDITION_DISPLAY_IMPORTANCE,
            PropertyValue::Enum(Token::new(di)),
        ));
    }
    item.properties.extend(visibility);
    // title (опц., Localized) — ПЕРВЫЙ ребёнок Table-именованного добавления (до name).
    // МУЛЬТИЯЗЫЧНЫЙ — ПОВТОРЯЕМЫЙ `<title>` по одному на язык (ERP-witness
    // DataProcessor.МобильноеРабочееМестоКладовщика.СписокПриходныхОрдеров searchControlAddition:
    // ru+en). Пуш ПОСЛЕ visibility — X-сравнимо с `read_designer_addition` (title после видимости).
    if let Some(t) = read_edt_title_multi(el, "title")? {
        item.properties.push((tb::F_ADDITION_TITLE, t));
    }
    if let Some(t) = el.child("extendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_edt_extended_tooltip(t)?);
    }
    if let Some(c) = el.child("contextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_edt_context_menu(c)?);
    }
    // Дискриминатор `<type>` (кроме searchString).
    if let Some(want) = ak.edt_type {
        expect_leaf_value(el, "type", want)?;
    }
    // toolTip (опц., Localized; метамодель Addition#4 — пуш ДО toolTipRepresentation).
    // МУЛЬТИЯЗЫЧНЫЙ — ПОВТОРЯЕМЫЙ `<toolTip>` по одному на язык (ERP-witness
    // InformationRegister.НастройкиИсключенийПроверкиДокументов.ФормаНастройкиПроверкиДокументов
    // searchStringAddition: ru+en; ранее читался лишь первый ⇒ 5 неклеймнутых узлов второго).
    if let Some(tt) = read_edt_title_multi(el, "toolTip")? {
        item.properties.push((tables::F_ADDITION_TOOL_TIP, tt));
    }
    // toolTipRepresentation (опц., ТЕЛО; метамодель Addition#5). Пуш ДО source — зеркально
    // Designer-ридеру (X-сравнимый bag).
    if let Some(ttr) = el
        .child("toolTipRepresentation")
        .filter(|c| c.prefix.is_empty())
    {
        ttr.claim_with_text();
        item.properties.push((
            tables::F_ADDITION_TOOL_TIP_REPRESENTATION,
            PropertyValue::Enum(Token::new(ttr.text.clone())),
        ));
    }
    // Вложенные контролы (`<items>`; ERP ×1 — SearchControl с Button).
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        item.children.push(read_edt_item(c)?);
    }
    // source (dataPath-ссылка, текст) — ОПЦИОНАЛЕН: добавление без источника его не несёт
    // (⟺ Designer без `<AdditionSource>`; cf даёт типизированный отказ). Пушим лишь при наличии.
    if let Some(source) = leaf_text_opt(el, "source") {
        item.properties
            .push((tb::F_ADDITION_SOURCE, PropertyValue::Ref(source)));
    }
    // extInfo: `<extInfo xsi:type="…"><autoMaxWidth>true</autoMaxWidth></extInfo>`.
    let ext = el
        .child("extInfo")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("{}: no extInfo", ak.edt_tag)))?;
    ext.claim();
    let xti = ext
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("addition extInfo: no xsi:type".into()))?;
    if xti.value != ak.ext_xsi {
        return Err(FormError::Frame(format!(
            "addition extInfo xsi:type={:?}, want {:?}",
            xti.value, ak.ext_xsi
        )));
    }
    xti.claimed.set(true);
    // Гео-поля extInfo (порядок = метамодель: width, autoMaxWidth, horizontalStretch).
    if let Some(w) = ext.child("width").filter(|c| c.prefix.is_empty()) {
        w.claim_with_text();
        item.ext_info.push((
            tb::F_ADDITION_WIDTH,
            PropertyValue::Int(parse_int(&w.text)?),
        ));
    }
    // autoMaxWidth — OppositeBool: EDT эмитит ЛИШЬ `true` (bag sparse-true), отсутствие ⟺ false
    // (X-равно `read_edt_addition_control`/`read_designer_addition`, где absent-EDT ⟺ Designer
    // `<AutoMaxWidth>false`). ERP-witness: 56 форм несут именованное добавление БЕЗ autoMaxWidth
    // (SearchString 74 / SearchControl 3 / ViewStatus 1 носителя; сверено с Designer-стороной
    // РесурсныеСпецификации.ФормаЭлемента — там `<AutoMaxWidth>false`). Present-`false` EDT НЕ
    // эмитит ⇒ §1.0-отказ (unwitnessed).
    if let Some(amw) = ext.child("autoMaxWidth").filter(|c| c.prefix.is_empty()) {
        if !matches!(
            read_bool_text(amw, "autoMaxWidth")?,
            PropertyValue::Bool(true)
        ) {
            return Err(FormError::Frame(
                "addition extInfo <autoMaxWidth>false unwitnessed (§1.0 — EDT omits false)".into(),
            ));
        }
        item.ext_info
            .push((tb::F_ADDITION_AUTO_MAX_WIDTH, PropertyValue::Bool(true)));
    }
    // maxWidth (опц., ext; метамодель autoMaxWidth→maxWidth).
    if let Some(mw) = ext.child("maxWidth").filter(|c| c.prefix.is_empty()) {
        mw.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_MAX_WIDTH,
            PropertyValue::Int(parse_int(&mw.text)?),
        ));
    }
    // horizontalStretch (опц., Symmetric; witness ГрупповоеИзменениеРеквизитов.ВыбранныеЭлементы
    // именованное searchStringAddition width=40/hs=false).
    if let Some(hs) = ext
        .child("horizontalStretch")
        .filter(|c| c.prefix.is_empty())
    {
        item.ext_info.push((
            tb::F_ADDITION_HORIZONTAL_STRETCH,
            read_bool_text(hs, "horizontalStretch")?,
        ));
    }
    // horizontalLocation (опц., ext; ТОЛЬКО ViewStatus — метамодель ViewStatusAdditionExtInfo#6).
    if let Some(hl) = ext
        .child("horizontalLocation")
        .filter(|c| c.prefix.is_empty())
    {
        if ak.kind != "ViewStatusAddition" {
            return Err(FormError::Frame(format!(
                "{}: <horizontalLocation> is a ViewStatusAddition-only property (§1.0)",
                ak.kind
            )));
        }
        hl.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_HORIZONTAL_LOCATION,
            PropertyValue::Enum(Token::new(hl.text.clone())),
        ));
    }
    expect_only_children(
        ext,
        &[
            "width",
            "autoMaxWidth",
            "maxWidth",
            "horizontalStretch",
            "horizontalLocation",
        ],
    )?;

    expect_only_children(
        el,
        &[
            "title",
            "visible",
            "enabled",
            "name",
            "id",
            "displayImportance",
            "extendedTooltip",
            "contextMenu",
            "items",
            "type",
            "toolTip",
            "toolTipRepresentation",
            "source",
            "extInfo",
        ],
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{} {:?}: {leftover} unconsumed node(s) (§1.0)",
            ak.edt_tag, item.name
        )));
    }
    Ok(item)
}

/// EDT `<extendedTooltip>` — вложенный `LabelDecoration`-стаб, читаемый КАК ДАННЫЕ
/// ([`TooltipBody`]) для byte-exact R (тело варьируется по корпусу — см. `DecoratorRef`).
/// `name`/`id`/`type=Label`/обёртка extInfo — каркас (claim'ится); реальные данные —
/// `title`/`maxWidth`/`autoMaxWidth`/`autoMaxHeight`/`horizontalStretch`/`formatted`/
/// `handlers`/`horizontalAlign`.
pub(crate) fn read_edt_extended_tooltip(el: &Element) -> Result<DecoratorRef, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let mut body = TooltipBody::default();

    // displayImportance (опц., ЧИЛД после id; witness кнопка СоздатьДокументы `High`). ⟺ Designer
    // атрибут `DisplayImportance`. cf-ячейка НЕ витнесснута ⇒ типизированный отказ cf-write.
    body.display_importance = leaf_text_opt(el, "displayImportance");
    // title (опц., МНОГОЯЗЫЧНЫЙ — по одному `<title>` на язык; сливаем все).
    if let Some(title) = read_edt_title_multi(el, "title")? {
        body.properties.push((ld::F_TITLE, title));
    }
    // formatted (опц., presence-bool) — ДО type-дискриминатора.
    if let Some(fm) = el.child("formatted").filter(|c| c.prefix.is_empty()) {
        if !matches!(read_bool_text(fm, "formatted")?, PropertyValue::Bool(true)) {
            return Err(FormError::Frame("tooltip <formatted> must be true".into()));
        }
        body.formatted = true;
    }
    // type=Label дискриминатор (каркас).
    expect_leaf_value(el, "type", "Label")?;
    // Тело LabelDecoration-стаба (width/autoMax*/maxWidth/height/stretch/groupAlign/textColor) —
    // через табличный движок (`TOOLTIP_BODY` в канон-порядке); title/formatted/extInfo — glue вокруг.
    read_fields_edt(
        "LabelDecoration",
        Some(el),
        tables::TOOLTIP_BODY,
        Region::Body,
        &mut body.properties,
    )?;
    // extInfo (form:LabelDecorationExtInfo): handlers + horizontalAlign.
    let ext = el
        .child("extInfo")
        .ok_or_else(|| FormError::Frame("extendedTooltip: no extInfo".into()))?;
    ext.claim();
    let xt = ext
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("tooltip extInfo: no xsi:type".into()))?;
    if xt.value != "form:LabelDecorationExtInfo" {
        return Err(FormError::Frame("tooltip extInfo xsi:type mismatch".into()));
    }
    xt.claimed.set(true);
    for h in ext
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        body.events.push(read_handlers(h)?);
    }
    // Канонический порядок пуша ext-полей (= метамодель, зеркально Designer-ридеру):
    // hyperlink → horizontalAlign → verticalAlign → titleHeight → backColor → borderColor.
    if let Some(h) = ext.child("hyperlink").filter(|c| c.prefix.is_empty()) {
        body.ext_info
            .push((ld::F_EXT_HYPERLINK, read_bool_text(h, "hyperlink")?));
    }
    if let Some(ha) = ext.child("horizontalAlign").filter(|c| c.prefix.is_empty()) {
        ha.claim_with_text();
        body.ext_info.push((
            ld::F_EXT_HORIZONTAL_ALIGN,
            PropertyValue::Enum(Token::new(ha.text.clone())),
        ));
    }
    // verticalAlign (опц., Enum; LabelDecorationExtInfo, ПОСЛЕ horizontalAlign). Witness —
    // DataProcessor НастройкаСтандартногоИнтерфейсаOData (подсказка extInfo verticalAlign=Top).
    if let Some(va) = ext.child("verticalAlign").filter(|c| c.prefix.is_empty()) {
        va.claim_with_text();
        body.ext_info.push((
            ld::F_EXT_VERTICAL_ALIGN,
            PropertyValue::Enum(Token::new(va.text.clone())),
        ));
    }
    if let Some(th) = ext.child("titleHeight").filter(|c| c.prefix.is_empty()) {
        th.claim_with_text();
        body.ext_info.push((
            ld::F_EXT_TITLE_HEIGHT,
            PropertyValue::Int(parse_int(&th.text)?),
        ));
    }
    if let Some(bc) = ext.child("backColor").filter(|c| c.prefix.is_empty()) {
        body.ext_info.push((
            ld::F_EXT_BACK_COLOR,
            crate::form::fields::decode_edt_color(bc, "backColor")?,
        ));
    }
    if let Some(bc) = ext.child("borderColor").filter(|c| c.prefix.is_empty()) {
        body.ext_info.push((
            tables::F_LD_BORDER_COLOR,
            crate::form::fields::decode_edt_color(bc, "borderColor")?,
        ));
    }
    expect_only_children(
        ext,
        &[
            "handlers",
            "hyperlink",
            "horizontalAlign",
            "verticalAlign",
            "titleHeight",
            "backColor",
            "borderColor",
        ],
    )?;
    // `<font>` — композит-шрифт подсказки на ТЕЛЕ стаба (как у декораций; ERP ×26).
    if let Some(fnt) = el.child("font").filter(|c| c.prefix.is_empty()) {
        body.font = Some(read_edt_font(fnt)?);
    }
    expect_only_children(
        el,
        &[
            "name",
            "id",
            "displayImportance",
            "title",
            "formatted",
            "type",
            "width",
            "maxWidth",
            "height",
            "autoMaxWidth",
            "autoMaxHeight",
            "maxHeight",
            "horizontalStretch",
            "verticalStretch",
            "groupHorizontalAlign",
            "groupVerticalAlign",
            "textColor",
            "font",
            "extInfo",
        ],
    )?;
    // Пер-форматные дефолты opposite-полей (autoMax*) + канон-порядок обеспечены табличным
    // движком (OppositeBool + порядок TOOLTIP_BODY) ⇒ тело X-равно Designer-стороне.
    Ok(DecoratorRef {
        name,
        id,
        body: DecoratorBody::Tooltip(body),
    })
}

pub(crate) fn read_edt_context_menu(el: &Element) -> Result<DecoratorRef, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    // Пункты меню — вложенное дерево контролов (`<items>` ДО `<autoFill>`).
    let mut items = Vec::new();
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        items.push(read_edt_item(c)?);
    }
    // `autoFill` — пер-форматный дефолт: EDT эмитит `<autoFill>true>` для авто-меню и
    // ОПУСКАЕТ его у не-авто (сверено — `<autoFill>false>` в корпусе 0/0). Поэтому ОТСУТСТВИЕ
    // ⇒ `false` (не `None`) ⇒ X-равно Designer-стороне, которая у не-авто эмитит `<Autofill>
    // false>` явно (а `true` опускает своим дефолтом).
    let auto_fill = match el.child("autoFill").filter(|c| c.prefix.is_empty()) {
        Some(v) => Some(matches!(
            read_bool_text(v, "autoFill")?,
            PropertyValue::Bool(true)
        )),
        None => Some(false),
    };
    expect_only_children(el, &["name", "id", "items", "autoFill"])?;
    Ok(DecoratorRef {
        name,
        id,
        body: DecoratorBody::ContextMenu(ContextMenuBody { auto_fill, items }),
    })
}

/// Прочитать МНОГОЯЗЫЧНЫЙ EDT-заголовок из ПОВТОРЯЮЩИХСЯ `<tag>`-элементов (EDT кодирует по
/// одному `<title>` НА ЯЗЫК, как engine-Localized-кодек) — слить пары в порядке документа.
/// Отсутствие тега ⇒ `None`. Чтение лишь ПЕРВОГО `<title>` роняло доп-языки в несклеймленные
/// узлы (§1.0; witness ERP PictureDecoration ДекорацияВнимание — `<title>×2` ru+en; класс —
/// 24636 Label + 4136 Picture декораций с двумя заголовками).
pub(crate) fn read_edt_title_multi(
    parent: &Element,
    tag: &str,
) -> Result<Option<PropertyValue>, FormError> {
    let mut pairs = Vec::new();
    for t in parent
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        match read_edt_title(t)? {
            PropertyValue::Localized(p) => pairs.extend(p),
            other => {
                return Err(FormError::Frame(format!(
                    "<{tag}>: localized decode gave {other:?} (bug)"
                )));
            }
        }
    }
    Ok(if pairs.is_empty() {
        None
    } else {
        Some(PropertyValue::Localized(pairs))
    })
}

pub(crate) fn read_edt_title(t: &Element) -> Result<PropertyValue, FormError> {
    t.claim();
    let mut pairs = Vec::new();
    let mut it = t.children.iter();
    while let Some(k) = it.next() {
        if k.local != "key" || !k.prefix.is_empty() {
            return Err(FormError::Frame("title: expected <key>".into()));
        }
        k.claim_with_text();
        let v = it
            .next()
            .ok_or_else(|| FormError::Frame("title: <key> without <value>".into()))?;
        if v.local != "value" || !v.prefix.is_empty() {
            return Err(FormError::Frame("title: expected <value>".into()));
        }
        v.claim_with_text();
        pairs.push((morph1c_core::ir::Lang::new(k.text.clone()), v.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// EDT view/edit-право реквизита: ОБЯЗАТЕЛЬНЫЙ `<tag>`: `<common>true` ⇒ `true`; ПУСТОЙ
/// `<tag/>` ⇒ `false` (запрещено; witness УчетныеЗаписи…/Взаимодействия/ВерсииОбъектов).
pub(crate) fn read_edt_common_flag(
    parent: &Element,
    tag: &str,
) -> Result<(bool, Vec<(String, bool)>), FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("attribute: no <{tag}>")))?;
    // РОЛЕВЫЕ права: `<view><for><value>B</value><role>R</role></for>×N` БЕЗ `<common>`
    // (⟺ Designer `<View><xr:Common>false` + `<xr:Value>`; ERP-witness БухучетЗарплаты…).
    if el
        .children
        .iter()
        .any(|c| c.local == "for" && c.prefix.is_empty())
    {
        el.claim();
        // Опциональный ВЕДУЩИЙ `<common>true</common>` = ОБЩИЙ доступ при ролевых исключениях
        // (ERP-witness ФизическиеЛица.ФормаЭлемента `<edit><common>true</common><for>×N` ⟷
        // Designer `<Edit><xr:Common>true</xr:Common><xr:Value>×N`). Отсутствие = common=false
        // (роли — единственный источник доступа). EDT эмитит `<common>` лишь при `true`.
        let common = match el.child("common").filter(|c| c.prefix.is_empty()) {
            Some(c) => {
                c.claim_with_text();
                if c.text != "true" {
                    return Err(FormError::Frame(format!(
                        "<{tag}><common>={:?} alongside <for>-roles, want \"true\" (§1.0)",
                        c.text
                    )));
                }
                true
            }
            None => false,
        };
        let mut roles = Vec::new();
        for f in el
            .children
            .iter()
            .filter(|c| c.local == "for" && c.prefix.is_empty())
        {
            f.claim();
            let val = match f.child("value").filter(|c| c.prefix.is_empty()) {
                None => false,
                Some(v) => matches!(read_bool_text(v, "value")?, PropertyValue::Bool(true)),
            };
            let role = leaf_text(f, "role")?;
            expect_only_children(f, &["value", "role"])?;
            roles.push((role, val));
        }
        expect_only_children(el, &["common", "for"])?;
        return Ok((common, roles));
    }
    if el.children.is_empty() && el.text.is_empty() {
        el.claim();
        return Ok((false, Vec::new()));
    }
    read_common_true(el, tag)?;
    Ok((true, Vec::new()))
}
