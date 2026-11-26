//! READ · Designer dialect reader — the control tree: fields, groups, tables (incl.
//! addition regions), buttons, decorations, tooltips, context menus, titles.

use super::*;

/// Designer семейство Decoration (`<LabelDecoration>`/`<PictureDecoration>`): `title` +
/// `formatted`-АТРИБУТ — glue (тот же порядок пуша, что EDT ⇒ X-равенство bag), прочее
/// таблично.
pub(crate) fn read_designer_decoration(
    el: &Element,
    dk: &'static tables::DecorationKind,
) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new(dk.kind), name, id);

    // Title + formatted-атрибут (glue; порядок пуша title→formatted = EDT-сторона).
    if let Some(t) = el.child("Title").filter(|c| c.prefix.is_empty()) {
        let fa = t
            .attr("formatted")
            .ok_or_else(|| FormError::Frame("decoration Title: no formatted attr (§1.0)".into()))?;
        fa.claimed.set(true);
        let formatted = match fa.value.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FormError::Frame(format!(
                    "decoration Title formatted={other:?}"
                )));
            }
        };
        // Пустой `<Title formatted="true"/>` (без содержимого): EDT-аналог несёт лишь
        // `<formatted>true`, БЕЗ `<title>` ⇒ пустой заголовок в bag НЕ кладём (иначе
        // X-дивергенция с EDT, у которого F_TITLE отсутствует); флаг formatted сохраняем.
        // При `formatted="false"` пустой заголовок оставляем как есть (симметрично EDT `<title/>`).
        let title = read_designer_title_pairs(t)?;
        let title_empty = matches!(&title, PropertyValue::Localized(p) if p.is_empty());
        if !(title_empty && formatted) {
            item.properties.push((ld::F_TITLE, title));
        }
        if formatted {
            item.properties
                .push((ld::F_FORMATTED, PropertyValue::Bool(true)));
        }
    }
    read_fields_designer(
        dk.kind,
        el,
        tables::DECORATION_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    read_fields_designer(dk.kind, el, dk.ext, Region::Ext, &mut item.ext_info)?;
    if let Some(fnt) = el.child("Font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_designer_font(fnt)?);
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            item.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }

    expect_only_children_ext(
        el,
        &["Title", "ContextMenu", "ExtendedTooltip", "Events", "Font"],
        tables::DECORATION_BODY
            .iter()
            .filter(|e| !e.des_attr)
            .map(|e| e.des)
            .chain(dk.ext.iter().map(|e| e.des)),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer {} {:?}: {leftover} unconsumed node(s) (§1.0)",
            dk.kind, item.name
        )));
    }
    Ok(item)
}

/// Designer FormField (`<InputField name id>`/`<CheckBoxField>`/`<LabelField>`): общее тело +
/// extInfo-поля INLINE по таблицам; `DisplayImportance` — АТРИБУТ элемента.
pub(crate) fn read_designer_field(
    el: &Element,
    fk: &'static tables::FieldKind,
) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new(fk.kind), name, id);

    read_fields_designer(
        fk.kind,
        el,
        tables::FORM_FIELD_COMMON,
        Region::Body,
        &mut item.properties,
    )?;
    read_fields_designer(fk.kind, el, fk.ext, Region::Ext, &mut item.ext_info)?;
    // TitleFont — композит-шрифт заголовка (Designer: сразу после `<Title>`; witness
    // LabelField/InputField). Общий обоим форматам (X-сравним).
    if let Some(tf) = el.child("TitleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_designer_font(tf)?);
    }
    // `<Font>` — композит-шрифт контрола (EDT-локус: extInfo.font соответствующего вида;
    // witnessed виды — [`tables::field_kind_carries_font`]: Label/Input ×613/Radio ×32/
    // Text ×2/Formatted ×1/Picture ×8, ERP). У прочих видов НЕ витнессирован — остаётся
    // unclaimed ⇒ громкая §1.0-ошибка ниже.
    if tables::field_kind_carries_font(fk.kind) {
        if let Some(fnt) = el.child("Font").filter(|c| c.prefix.is_empty()) {
            item.font = Some(read_designer_font(fnt)?);
        }
    }
    // `<FooterFont>` — шрифт подвала колонки (field-common; ERP ×56, witness
    // РабочееМестоМенеджераПоДоставке: FooterTextColor→FooterFont→Width). cf: {48}-тело
    // cell[36] (абляция s10, проба Кол1).
    if let Some(ff2) = el.child("FooterFont").filter(|c| c.prefix.is_empty()) {
        item.footer_font = Some(read_designer_font(ff2)?);
    }
    // СОБСТВЕННЫЕ исключённые команды (SpreadsheetDocumentField `<CommandSet>`; прочие поля
    // его не несут — аддитивно). Регион между TitleLocation и EditMode (позиция read неважна).
    if let Some(cs) = el.child("CommandSet").filter(|c| c.prefix.is_empty()) {
        cs.claim();
        for x in cs.children.iter() {
            if x.local != "ExcludedCommand" || !x.prefix.is_empty() {
                return Err(FormError::Frame(format!(
                    "{} CommandSet: unexpected child <{}:{}> (§1.0)",
                    fk.kind, x.prefix, x.local
                )));
            }
            x.claim_with_text();
            expect_no_children(x)?;
            item.excluded_commands.push(x.text.clone());
        }
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    // ДОБАВЛЕНИЯ (PDFDocumentField `<ViewStatusAddition>`): прямые дети `<PDFDocumentField>`,
    // тот же Addition-контрол, что у Таблицы (EDT держит их в extInfo).
    if tables::field_kind_has_ext_additions(fk.kind) {
        for ak in tables::ADDITION_KINDS {
            if let Some(a) = el.child(ak.kind).filter(|c| c.prefix.is_empty()) {
                item.additions.push(read_designer_addition(a, ak)?);
            }
        }
    }
    // АВТО-ТАБЛИЦА (GanttChartField `<Table>`): прямой ребёнок `<GanttChartField>` (EDT держит
    // её в `<extInfo><autoTable>`). Читается обычным Designer-Table-движком.
    if tables::field_kind_has_auto_table(fk.kind) {
        if let Some(at) = el.child("Table").filter(|c| c.prefix.is_empty()) {
            let mut table = read_designer_table(at)?;
            // The official TableHolder reader does not store the native child
            // DataPath. Its writer derives that scalar from the current field.
            // Canonicalize only the equal inherited value; a distinct declared
            // value remains authoritative instead of being silently discarded.
            if let Some(parent_path) = item.properties.iter().find(|(id, _)| *id == morph1c_core::spec::forms::controls::form_field::F_DATA_PATH).map(|(_, value)| value) {
                table.properties.retain(|(id, value)| *id != tb::F_DATA_PATH || value != parent_path);
            }
            item.auto_table = Some(Box::new(table));
        }
    }
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            item.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }

    // `Font` — у всех видов с witnessed font-локусом (ERP-волна: Label/Input/Radio/Text/
    // Formatted/Picture; см. field_kind_carries_font). Плюс `Table` (GanttChartField авто-таблица)
    // и addition-теги (PDFDocumentField). Прочие узлы — громкий §1.0.
    let mut allowed: Vec<&str> = vec![
        "AutoEditMode",
        "ContextMenu",
        "ExtendedTooltip",
        "Events",
        "CommandSet",
        "TitleFont",
        "FooterFont",
    ];
    if tables::field_kind_carries_font(fk.kind) {
        allowed.push("Font");
    }
    if tables::field_kind_has_auto_table(fk.kind) {
        allowed.push("Table");
    }
    if tables::field_kind_has_ext_additions(fk.kind) {
        for ak in tables::ADDITION_KINDS {
            allowed.push(ak.kind);
        }
    }
    expect_only_children_ext(
        el,
        &allowed,
        tables::FORM_FIELD_COMMON
            .iter()
            .filter(|e| !e.des_attr)
            .map(|e| e.des)
            .chain(fk.ext.iter().map(|e| e.des)),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer {} {:?}: {leftover} unconsumed node(s) (§1.0)",
            fk.kind, item.name
        )));
    }
    super::super::event_owners::partition_native_field(&mut item)?;
    Ok(item)
}

/// Designer семейство FormGroup (`<UsualGroup>`/`<Pages>`/`<Page>`/`<ButtonGroup>`):
/// общее тело + extInfo-поля INLINE + `<ChildItems>` рекурсия + `<Events>` (Pages).
pub(crate) fn read_designer_group(
    el: &Element,
    gk: &'static tables::GroupKind,
) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new(gk.kind), name, id);

    read_fields_designer(
        gk.kind,
        el,
        tables::FORM_GROUP_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    read_fields_designer(gk.kind, el, gk.ext, Region::Ext, &mut item.ext_info)?;
    // TitleFont — композит-шрифт заголовка группы (Designer: сразу после `<Title>`;
    // witness UsualGroup МашиночитаемыеДоверенности).
    if let Some(tf) = el.child("TitleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_designer_font(tf)?);
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            item.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }
    if let Some(ci) = el.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        item.children = read_designer_child_items(ci)?;
    }

    expect_only_children_ext(
        el,
        &[
            "ContextMenu",
            "ExtendedTooltip",
            "Events",
            "ChildItems",
            "TitleFont",
        ],
        tables::FORM_GROUP_BODY
            .iter()
            .filter(|e| !e.des_attr)
            .map(|e| e.des)
            .chain(gk.ext.iter().map(|e| e.des)),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer {} {:?}: {leftover} unconsumed node(s) (§1.0)",
            gk.kind, item.name
        )));
    }
    Ok(item)
}

/// Designer `<Button name id>`: ПОЛНОЕ тело таблично (LANE-F-2); `<Type>` — ДАННЫЕ-поле
/// (Designer эмитит всегда); `Font` НЕ смоделирован — громкая §1.0-ошибка.
pub(crate) fn read_designer_button(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new("Button"), name, id);

    let color_nodes: Vec<_> = el
        .children
        .iter()
        .filter(|c| c.local == "BackColor" && c.prefix.is_empty())
        .collect();
    if color_nodes.len() > 1 {
        return Err(FormError::Frame("duplicate Button.BackColor".into()));
    }
    if let Some(color) = color_nodes.first().filter(|c| c.text == "auto") {
        if !color.attrs.is_empty() || !color.children.is_empty() {
            return Err(FormError::Frame(
                "Button.BackColor auto must be an attribute-free scalar".into(),
            ));
        }
        color.claim_with_text();
        item.designer_button_back_color_auto = true;
    }
    let button_body: Vec<_> = tables::BUTTON_BODY
        .iter()
        .copied()
        .filter(|entry| !item.designer_button_back_color_auto || entry.des != "BackColor")
        .collect();
    read_fields_designer(
        "Button",
        el,
        &button_body,
        Region::Body,
        &mut item.properties,
    )?;
    if let Some(fnt) = el.child("Font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_designer_font(fnt)?);
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            item.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }

    expect_only_children_ext(
        el,
        &["ContextMenu", "ExtendedTooltip", "Events", "Font"],
        tables::BUTTON_BODY
            .iter()
            .filter(|e| !e.des_attr)
            .map(|e| e.des),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer Button {:?}: {leftover} unconsumed node(s) (§1.0)",
            item.name
        )));
    }
    Ok(item)
}

/// Designer `<Table name id>` (LANE-F-4): тело inline, `<CommandSet>` (собств. исключённые),
/// `<ContextMenu>`/`<ExtendedTooltip>`, `<AutoCommandBar>` (собств.), добавления, `<Events>`,
/// `<ChildItems>` (колонки).
pub(crate) fn read_designer_table(el: &Element) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new("Table"), name, id);

    read_fields_designer(
        "Table",
        el,
        tables::TABLE_BODY,
        Region::Body,
        &mut item.properties,
    )?;
    // Сигила `~` на `RowPictureDataPath` («поле не входит в состав полей динамического списка») —
    // DESIGNER-ONLY денормализация: EDT-экспорт тех же 5 форм пишет путь БЕЗ неё (во всём
    // EDT-корпусе `~` не встречается ни разу), и edt→cf на них byte-exact ⇒ в cf-тело сигила НЕ
    // кодируется. Канонизуем к EDT-написанию, а бит несём presence-точным флагом
    // (`FormItem::row_picture_path_unavailable`; write_designer возвращает сигилу, X зануляет).
    // См. IR-док: точный предикат платформы требует разбора текста запроса ⇒ §1.0 — несём
    // наблюдённый бит, а не угаданное правило.
    item.row_picture_path_unavailable = el.child("RowPictureDataPath")
        .filter(|c| c.prefix.is_empty()).is_some_and(|c| c.text.starts_with('~'));
    // showCommandBar три-состояние: Designer кодирует ОДНИМ `<ShowCommandBar>значение`
    // ({true, false, auto}); EDT — двумя полями (см. read_edt_table_show_command_bar).
    if let Some(scb) = el.child("ShowCommandBar").filter(|c| c.prefix.is_empty()) {
        scb.claim_with_text();
        match scb.text.as_str() {
            "true" | "false" | "auto" => item.show_command_bar = Some(scb.text.clone()),
            other => {
                return Err(FormError::Frame(format!(
                    "Table <ShowCommandBar>={other:?}, want true/false/auto (§1.0)"
                )));
            }
        }
    }
    // extInfo динамического списка (Designer — ИНЛАЙН в `<Table>`): маркер — `<AutoRefresh>`
    // (у обычных таблиц данных отсутствует; весь блок появляется вместе). Читаем ПОЛЯ таблично
    // Обработчики разделяются после чтения единого `<Events>` по runtime-владельцам.
    if el
        .child("AutoRefresh")
        .filter(|c| c.prefix.is_empty())
        .is_some()
    {
        let mut fields = Vec::new();
        read_fields_designer(
            "Table",
            el,
            tables::DYNAMIC_LIST_EXT,
            Region::Ext,
            &mut fields,
        )?;
        item.dynamic_list_ext = Some(DynamicListExt {
            fields,
            events: Vec::new(),
        });
    }
    // Собственные исключённые команды.
    if let Some(cs) = el.child("CommandSet").filter(|c| c.prefix.is_empty()) {
        cs.claim();
        for x in cs.children.iter() {
            if x.local != "ExcludedCommand" || !x.prefix.is_empty() {
                return Err(FormError::Frame(format!(
                    "Table CommandSet: unexpected child <{}:{}> (§1.0)",
                    x.prefix, x.local
                )));
            }
            x.claim_with_text();
            expect_no_children(x)?;
            item.excluded_commands.push(x.text.clone());
        }
    }
    // TitleFont/Font — композит-шрифты Таблицы (ERP-witness ×60/×3; SSL их не несёт).
    if let Some(tf) = el.child("TitleFont").filter(|c| c.prefix.is_empty()) {
        item.title_font = Some(read_designer_font(tf)?);
    }
    if let Some(fnt) = el.child("Font").filter(|c| c.prefix.is_empty()) {
        item.font = Some(read_designer_font(fnt)?);
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    if let Some(acb) = el.child("AutoCommandBar").filter(|c| c.prefix.is_empty()) {
        item.auto_command_bar = Some(read_designer_auto_command_bar(acb)?);
    }
    for ak in tables::ADDITION_KINDS {
        if let Some(a) = el.child(ak.kind).filter(|c| c.prefix.is_empty()) {
            item.additions.push(read_designer_addition(a, ak)?);
        }
    }
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            item.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }
    if let Some(ci) = el.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        item.children = read_designer_child_items(ci)?;
    }

    expect_only_children_ext(
        el,
        &[
            "CommandSet",
            "ContextMenu",
            "ExtendedTooltip",
            "AutoCommandBar",
            "Events",
            "ChildItems",
            "ShowCommandBar",
            "TitleFont",
            "Font",
        ],
        tables::ADDITION_KINDS
            .iter()
            .map(|a| a.kind)
            .chain(
                tables::TABLE_BODY
                    .iter()
                    .filter(|e| !e.des_attr)
                    .map(|e| e.des),
            )
            .chain(tables::DYNAMIC_LIST_EXT.iter().map(|e| e.des)),
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer Table {:?}: {leftover} unconsumed node(s) (no Raw — §1.0): {:?}",
            item.name,
            unclaimed_labels(el)
        )));
    }
    super::super::event_owners::partition_native_table(&mut item)?;
    Ok(item)
}

/// Designer ДОБАВЛЕНИЕ Таблицы (`<SearchStringAddition name id>`): `<AdditionSource>`
/// (`<Item>` source + `<Type>` константа) + bare-ref декораторы. `autoMaxWidth` Designer НЕ
/// несёт — заполняется fill-константой (EDT эмитит всегда). Порядок: AdditionSource,
/// ContextMenu, ExtendedTooltip.
pub(crate) fn read_designer_addition(
    el: &Element,
    ak: &tables::AdditionKind,
) -> Result<FormItem, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut item = FormItem::new(FormControlKind::new(ak.kind), name, id);

    // DisplayImportance — АТРИБУТ (⟺ EDT чилд `<displayImportance>`; witness ФормаВыбораПолейПоиска
    // `VeryHigh`). Клеймим атрибут, иначе — несконсуменный узел (§1.0). cf НЕ витнесснут ⇒ отказ.
    if let Some(a) = el.attr("DisplayImportance") {
        a.claimed.set(true);
        item.properties.push((
            tables::F_ADDITION_DISPLAY_IMPORTANCE,
            PropertyValue::Enum(Token::new(a.value.clone())),
        ));
    }

    // Каркас видимости добавления (SPARSE head, ДО AdditionSource): Designer опускает дефолт
    // `true`, эмитит лишь `false` (witnessed `<Enabled>false`). X-сравним с EDT-стороной.
    // `UserVisible` на Table-именованном добавлении корпусом НЕ витнессирован → §1.0 loud-error.
    for (tag, fid) in [
        ("Visible", tb::F_ADDITION_VISIBLE),
        ("Enabled", tb::F_ADDITION_ENABLED),
    ] {
        if let Some(v) = el.child(tag).filter(|c| c.prefix.is_empty()) {
            let value = read_bool_text(v, tag)?;
            if fid != tb::F_ADDITION_ENABLED || value != PropertyValue::Bool(true) {
                item.properties.push((fid, value));
            }
        }
    }

    // `<AdditionSource>` ОПЦИОНАЛЕН: добавление без источника (не привязанное к таблице) его не
    // несёт вовсе ⇒ `source`-свойство отсутствует. EDT-твин так же опускает `<source>`; cf-энкодер
    // `encode_command_bar_addition` даёт типизированный отказ на добавление без источника (§1.0).
    // Present-вариант: `<Item>` (source) + `<Type>` (константа-дискриминатор вида). Значение
    // ПУШИМ ниже (позиция bag = ПОСЛЕ toolTipRepresentation — X-сравнимо с EDT-ридерами).
    let source = match el.child("AdditionSource").filter(|c| c.prefix.is_empty()) {
        Some(src) => {
            src.claim();
            let source = leaf_text(src, "Item")?;
            let stype = leaf_text(src, "Type")?;
            if stype != ak.des_source_type {
                return Err(FormError::Frame(format!(
                    "{} AdditionSource Type={stype:?}, want {:?} (§1.0)",
                    ak.kind, ak.des_source_type
                )));
            }
            expect_only_children(src, &["Item", "Type"])?;
            Some(source)
        }
        None => None,
    };
    // title (опц., Localized) — после AdditionSource (⟺ EDT `<title>` первым).
    if let Some(t) = el.child("Title").filter(|c| c.prefix.is_empty()) {
        item.properties
            .push((tb::F_ADDITION_TITLE, read_designer_title(t, false)?));
    }
    // toolTip (опц., Localized; метамодель Addition#4 — ДО toolTipRepresentation; ERP ×1
    // SearchString). cf-ячейка НЕ витнесснута — типизированный отказ cf-write.
    if let Some(tt) = el.child("ToolTip").filter(|c| c.prefix.is_empty()) {
        item.properties
            .push((tables::F_ADDITION_TOOL_TIP, read_designer_title(tt, false)?));
    }
    // toolTipRepresentation (опц., ТЕЛО; метамодель Addition#5 — ДО source; ERP-witness
    // МониторингЗаданийEDI `<ToolTipRepresentation>Button`). Позиция пуша = ДО source
    // (зеркально всем трём ридерам добавлений — X-сравнимый bag).
    if let Some(ttr) = el
        .child("ToolTipRepresentation")
        .filter(|c| c.prefix.is_empty())
    {
        ttr.claim_with_text();
        item.properties.push((
            tables::F_ADDITION_TOOL_TIP_REPRESENTATION,
            PropertyValue::Enum(Token::new(ttr.text.clone())),
        ));
    }
    if let Some(source) = source {
        item.properties
            .push((tb::F_ADDITION_SOURCE, PropertyValue::Ref(source)));
    }
    // groupHorizontalAlign (опц., ТЕЛО; Designer после Title — witness ЗагрузкаКурсовВалют).
    if let Some(g) = el
        .child("GroupHorizontalAlign")
        .filter(|c| c.prefix.is_empty())
    {
        g.claim_with_text();
        item.properties.push((
            tb::F_ADDITION_GROUP_HORIZONTAL_ALIGN,
            PropertyValue::Enum(Token::new(g.text.clone())),
        ));
    }
    // ext-геометрия (bag-порядок = канон: width, autoMaxWidth, horizontalStretch).
    if let Some(w) = el.child("Width").filter(|c| c.prefix.is_empty()) {
        w.claim_with_text();
        item.ext_info.push((
            tb::F_ADDITION_WIDTH,
            PropertyValue::Int(parse_int(&w.text)?),
        ));
    }
    // autoMaxWidth — OppositeBool: Designer эмитит лишь `<AutoMaxWidth>false` (⟺ bag БЕЗ
    // поля); отсутствие ⟺ true (bag sparse-true, X-равно EDT-стороне).
    match el.child("AutoMaxWidth").filter(|c| c.prefix.is_empty()) {
        Some(amw) => {
            if !matches!(
                read_bool_text(amw, "AutoMaxWidth")?,
                PropertyValue::Bool(false)
            ) {
                return Err(FormError::Frame(
                    "addition <AutoMaxWidth>true unwitnessed (§1.0 — Designer omits true)".into(),
                ));
            }
        }
        None => item
            .ext_info
            .push((tb::F_ADDITION_AUTO_MAX_WIDTH, PropertyValue::Bool(true))),
    }
    // maxWidth (опц., ext; метамодель autoMaxWidth→maxWidth; ERP-witness SearchString ×13).
    if let Some(mw) = el.child("MaxWidth").filter(|c| c.prefix.is_empty()) {
        mw.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_MAX_WIDTH,
            PropertyValue::Int(parse_int(&mw.text)?),
        ));
    }
    if let Some(hs) = el
        .child("HorizontalStretch")
        .filter(|c| c.prefix.is_empty())
    {
        item.ext_info.push((
            tb::F_ADDITION_HORIZONTAL_STRETCH,
            read_bool_text(hs, "HorizontalStretch")?,
        ));
    }
    // horizontalLocation (опц., ext; ТОЛЬКО ViewStatus — метамодель ViewStatusAdditionExtInfo#6;
    // ERP 3773×Left). На чужом виде добавления — §1.0-ошибка (метамодель поля не несёт).
    if let Some(hl) = el
        .child("HorizontalLocation")
        .filter(|c| c.prefix.is_empty())
    {
        if ak.kind != "ViewStatusAddition" {
            return Err(FormError::Frame(format!(
                "{}: <HorizontalLocation> is a ViewStatusAddition-only property (§1.0)",
                ak.kind
            )));
        }
        hl.claim_with_text();
        item.ext_info.push((
            tables::F_ADDITION_HORIZONTAL_LOCATION,
            PropertyValue::Enum(Token::new(hl.text.clone())),
        ));
    }
    if let Some(c) = el.child("ContextMenu").filter(|c| c.prefix.is_empty()) {
        item.context_menu = Some(read_designer_decorator(c, "ContextMenu", false)?);
    }
    if let Some(t) = el.child("ExtendedTooltip").filter(|c| c.prefix.is_empty()) {
        item.ext_tooltip = Some(read_designer_decorator(t, "ExtendedTooltip", true)?);
    }
    // ChildItems — вложенные контролы добавления (ERP ×1: SearchControlAddition с Button;
    // cf-кодировка НЕ витнесснута — типизированный отказ cf-write).
    if let Some(ci) = el.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        item.children = read_designer_child_items(ci)?;
    }
    expect_only_children(
        el,
        &[
            "Visible",
            "Enabled",
            "AdditionSource",
            "Title",
            "ToolTip",
            "ToolTipRepresentation",
            "GroupHorizontalAlign",
            "Width",
            "AutoMaxWidth",
            "MaxWidth",
            "HorizontalStretch",
            "HorizontalLocation",
            "ContextMenu",
            "ExtendedTooltip",
            "ChildItems",
        ],
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer {} {:?}: {leftover} unconsumed node(s) (§1.0)",
            ak.kind, item.name
        )));
    }
    Ok(item)
}

/// Designer decorator. Designer НЕ-bare: расширенная подсказка несёт СОДЕРЖАТЕЛЬНОЕ тело
/// (вложенный `LabelDecoration`-стаб: `Title`/`MaxWidth`/`AutoMaxWidth`/`AutoMaxHeight`/
/// `HorizontalStretch`/`Events`), которое СОВПАДАЕТ с EDT-инлайн-стабом (после реконсиляции
/// пер-форматных дефолтов) — это ОБЩЕЕ содержимое, X-сравнимое. Bare-ref `<X name id/>`
/// тоже валиден (пустое тело). Контекстное меню несёт `Autofill` (как правило константу
/// пустого авто-меню).
pub(crate) fn read_designer_decorator(
    el: &Element,
    tag: &str,
    is_tooltip: bool,
) -> Result<DecoratorRef, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    if is_tooltip {
        let body = read_designer_tooltip_body(el)?;
        Ok(DecoratorRef {
            name,
            id,
            body: DecoratorBody::Tooltip(body),
        })
    } else {
        let body = read_designer_context_menu_body(el, tag)?;
        Ok(DecoratorRef {
            name,
            id,
            body: DecoratorBody::ContextMenu(body),
        })
    }
}

/// Прочитать Designer-тело расширенной подсказки в [`TooltipBody`]. Поля (вложенный
/// `LabelDecoration`-стаб): `AutoMaxWidth`/`MaxWidth`/`AutoMaxHeight`/`HorizontalStretch`/
/// `Title`/`Events`. Designer опускает свои дефолты (`AutoMaxWidth`/`AutoMaxHeight`-дефолт
/// true) — канонизуем те же opposite-поля, что у обычной надписи, чтобы тело СОВПАЛО с
/// EDT-стороной. `horizontalAlign` Designer в теле подсказки НЕ несёт (это EDT-каркас).
pub(crate) fn read_designer_tooltip_body(el: &Element) -> Result<TooltipBody, FormError> {
    let mut body = TooltipBody::default();
    // DisplayImportance — АТРИБУТ элемента `<ExtendedTooltip>` (⟺ EDT чилд `<displayImportance>`;
    // witness кнопка СоздатьДокументы `High`). Клеймим атрибут, иначе — несконсуменный узел (§1.0).
    if let Some(a) = el.attr("DisplayImportance") {
        a.claimed.set(true);
        body.display_importance = Some(a.value.clone());
    }
    // Title (formatted-АТРИБУТ) — glue (Localized-кодек НЕ клеймит атрибут); пушим ПЕРВЫМ
    // (канон-порядок тела: title перед геометрией), чтобы bag совпал с EDT-стороной без sort.
    if let Some(t) = el.child("Title").filter(|c| c.prefix.is_empty()) {
        // Title несёт formatted-атрибут (контрол-заголовок); formatted="true" ⇒ HTML-флаг.
        let fa = t
            .attr("formatted")
            .ok_or_else(|| FormError::Frame("tooltip Title: no formatted".into()))?;
        body.formatted = match fa.value.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FormError::Frame(format!(
                    "tooltip Title formatted={other:?}"
                )));
            }
        };
        fa.claimed.set(true);
        // Пустой `<Title formatted="true"/>`: EDT-аналог несёт лишь `<formatted>true` БЕЗ
        // `<title>` ⇒ пустой заголовок в bag НЕ кладём (зеркало decoration-пути; X с EDT).
        let title = read_designer_title_pairs(t)?;
        let title_empty = matches!(&title, PropertyValue::Localized(p) if p.is_empty());
        if !(title_empty && body.formatted) {
            body.properties.push((ld::F_TITLE, title));
        }
    }
    // Тело LabelDecoration-стаба (Width/AutoMax*/MaxWidth/Height/HorizontalStretch/GroupAlign/
    // TextColor) — через табличный движок (`TOOLTIP_BODY` в канон-порядке = порядок EDT-стороны).
    // Порядок ЧТЕНИЯ Designer-корпуса варьируется (TextColor до Title и т.п.), но движок ищет
    // каждое поле по тегу и пушит в порядке таблицы ⇒ bag X-равен EDT без пост-сортировки.
    read_fields_designer(
        "LabelDecoration",
        el,
        tables::TOOLTIP_BODY,
        Region::Body,
        &mut body.properties,
    )?;
    // VerticalAlign (опц., Enum) — ext-поле LabelDecoration; Designer эмитит прямым ребёнком
    // подсказки (witness НастройкаСтандартногоИнтерфейсаOData `<VerticalAlign>Top` после Title).
    // Кладём в ext_info (симметрично EDT tooltip extInfo verticalAlign; X нормализует ext_info).
    let vertical_align = el
        .child("VerticalAlign")
        .filter(|c| c.prefix.is_empty())
        .map(|v| {
            v.claim_with_text();
            v.text.clone()
        });
    if let Some(events) = el.child("Events").filter(|c| c.prefix.is_empty()) {
        events.claim();
        for ev in events
            .children
            .iter()
            .filter(|c| c.local == "Event" && c.prefix.is_empty())
        {
            body.events.push(read_designer_event(ev)?);
        }
        expect_only_children(events, &["Event"])?;
    }
    // `<Font>` — композит-шрифт подсказки (ERP ×26; cf tooltip flat[15] — абляция s10).
    if let Some(fnt) = el.child("Font").filter(|c| c.prefix.is_empty()) {
        body.font = Some(read_designer_font(fnt)?);
    }
    // Ext-поля подсказки (LabelDecorationExtInfo): канонический ПОРЯДОК ПУША (= метамодель,
    // зеркально EDT-ридеру): hyperlink → horizontalAlign → verticalAlign → titleHeight →
    // backColor → borderColor (ERP-абляция: суб-рекорд [18] ячейки [1]/[4]/[6]/[7]).
    if let Some(h) = el.child("Hyperlink").filter(|c| c.prefix.is_empty()) {
        body.ext_info
            .push((ld::F_EXT_HYPERLINK, read_bool_text(h, "Hyperlink")?));
    }
    // Канонизация к EDT-flavor (§1.6, approach B): EDT ВСЕГДА эмитит авто-каркас extInfo
    // `form:LabelDecorationExtInfo` с `horizontalAlign` у расширенной подсказки (сверено
    // 731/731 в корпусе; как правило `Left`); Designer несёт `<HorizontalAlign>` в теле
    // подсказки ЛИШЬ не-дефолт (witness СообщениеSMS `Right` после GroupHorizontalAlign),
    // опуская `Left`. Читаем явное значение либо синтезируем `Left`-skeleton (write_designer
    // регенерирует — опускает `Left`, эмитит прочее; ср. normalize_form_for_x, зануляющая
    // ext_info подсказки).
    let horizontal_align = match el.child("HorizontalAlign").filter(|c| c.prefix.is_empty()) {
        Some(h) => {
            h.claim_with_text();
            h.text.clone()
        }
        None => TOOLTIP_EXT_HORIZONTAL_ALIGN.to_string(),
    };
    body.ext_info.push((
        ld::F_EXT_HORIZONTAL_ALIGN,
        PropertyValue::Enum(Token::new(horizontal_align)),
    ));
    if let Some(va) = vertical_align {
        body.ext_info.push((
            ld::F_EXT_VERTICAL_ALIGN,
            PropertyValue::Enum(Token::new(va)),
        ));
    }
    if let Some(th) = el.child("TitleHeight").filter(|c| c.prefix.is_empty()) {
        th.claim_with_text();
        body.ext_info.push((
            ld::F_EXT_TITLE_HEIGHT,
            PropertyValue::Int(parse_int(&th.text)?),
        ));
    }
    if let Some(bc) = el.child("BackColor").filter(|c| c.prefix.is_empty()) {
        bc.claim_with_text();
        body.ext_info.push((
            ld::F_EXT_BACK_COLOR,
            PropertyValue::Ref(crate::form::fields::color_from_designer(
                &bc.text,
                "BackColor",
            )?),
        ));
    }
    if let Some(bc) = el.child("BorderColor").filter(|c| c.prefix.is_empty()) {
        bc.claim_with_text();
        body.ext_info.push((
            tables::F_LD_BORDER_COLOR,
            PropertyValue::Ref(crate::form::fields::color_from_designer(
                &bc.text,
                "BorderColor",
            )?),
        ));
    }
    expect_only_children(
        el,
        &[
            "Width",
            "AutoMaxWidth",
            "MaxWidth",
            "Height",
            "AutoMaxHeight",
            "MaxHeight",
            "HorizontalStretch",
            "VerticalStretch",
            "GroupHorizontalAlign",
            "GroupVerticalAlign",
            "TextColor",
            "Font",
            "Hyperlink",
            "TitleHeight",
            "BackColor",
            "BorderColor",
            "HorizontalAlign",
            "VerticalAlign",
            "Title",
            "Events",
        ],
    )?;
    Ok(body)
}

/// Дефолт `horizontalAlign` авто-каркаса extInfo расширенной подсказки
/// (`form:LabelDecorationExtInfo`). EDT ВСЕГДА эмитит значение (как правило `Left`, сверено
/// 731/731 корпуса); Designer опускает `Left` и несёт `<HorizontalAlign>` лишь не-дефолт
/// (witness СообщениеSMS `Right`). Заполняется в [`read_designer_tooltip_body`],
/// `Left`-омиссия регенерируется в `push_designer_tooltip_body`.
pub(crate) const TOOLTIP_EXT_HORIZONTAL_ALIGN: &str = "Left";

/// Прочитать Designer-тело контекстного меню в [`ContextMenuBody`]. `Autofill` — пер-форматный
/// дефолт: Designer ОПУСКАЕТ его у авто-меню (дефолт `true`), эмитит `<Autofill>false` у
/// не-авто. Заполняем дефолт `true` при отсутствии ⇒ X-равно EDT (которая эмитит `<autoFill>
/// true` ВСЕГДА). Вложенное дерево пунктов — `<ChildItems>` (рекурсивные контролы).
pub(crate) fn read_designer_context_menu_body(
    el: &Element,
    tag: &str,
) -> Result<ContextMenuBody, FormError> {
    let auto_fill = match el.child("Autofill").filter(|c| c.prefix.is_empty()) {
        Some(v) => Some(matches!(
            read_bool_text(v, "Autofill")?,
            PropertyValue::Bool(true)
        )),
        None => Some(CONTEXT_MENU_AUTO_FILL_DEFAULT),
    };
    let items = match el.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        Some(ci) => read_designer_child_items(ci)?,
        None => Vec::new(),
    };
    expect_only_children(el, &["Autofill", "ChildItems"])?;
    if el.unclaimed_count() != 0 {
        return Err(FormError::Frame(format!(
            "{tag}: unmodeled content-bearing context menu (§1.0 — no Raw)"
        )));
    }
    Ok(ContextMenuBody { auto_fill, items })
}

/// Дефолт `autoFill` авто-контекстного-меню: EDT эмитит `true` всегда, Designer опускает.
pub(crate) const CONTEXT_MENU_AUTO_FILL_DEFAULT: bool = true;

/// Прочитать Designer `<Title>`/`<v8:item>` локализацию. `require_formatted`: контрол-
/// заголовки несут `formatted="false"` (сверяется-claim'ится); форм-заголовок — нет.
pub(crate) fn read_designer_title(
    t: &Element,
    require_formatted: bool,
) -> Result<PropertyValue, FormError> {
    if require_formatted {
        // `<Title formatted="false">` — атрибут formatted=false (контрол-заголовок).
        let fa = t
            .attr("formatted")
            .ok_or_else(|| FormError::Frame("Title: no formatted".into()))?;
        if fa.value != "false" {
            return Err(FormError::Frame(format!(
                "Title formatted={:?}, want false",
                fa.value
            )));
        }
        fa.claimed.set(true);
    }
    read_designer_title_pairs(t)
}

/// Прочитать `<v8:item>`-пары `<Title>` (БЕЗ formatted-атрибута — его клеймит вызывающий).
pub(crate) fn read_designer_title_pairs(t: &Element) -> Result<PropertyValue, FormError> {
    t.claim();
    let mut pairs = Vec::new();
    for item in &t.children {
        if item.local != "item" || item.prefix != "v8" {
            return Err(FormError::Frame("Title: expected <v8:item>".into()));
        }
        item.claim();
        let lang = item
            .child("lang")
            .filter(|c| c.prefix == "v8")
            .ok_or_else(|| FormError::Frame("Title: no v8:lang".into()))?;
        let content = item
            .child("content")
            .filter(|c| c.prefix == "v8")
            .ok_or_else(|| FormError::Frame("Title: no v8:content".into()))?;
        lang.claim_with_text();
        content.claim_with_text();
        if item.children.len() != 2 {
            return Err(FormError::Frame(
                "Title: <v8:item> must have exactly lang+content".into(),
            ));
        }
        pairs.push((
            morph1c_core::ir::Lang::new(lang.text.clone()),
            content.text.clone(),
        ));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// Designer view/edit-право реквизита: ОПЦИОНАЛЬНЫЙ `<View>`/`<Edit>` с
/// `<xr:Common>false|true</xr:Common>`; отсутствие ⇒ `true` (общий доступ).
pub(crate) fn read_designer_common_flag(
    parent: &Element,
    tag: &str,
) -> Result<(bool, Vec<(String, bool)>), FormError> {
    let el = match parent.child(tag).filter(|c| c.prefix.is_empty()) {
        Some(el) => el,
        None => return Ok((true, Vec::new())),
    };
    el.claim();
    let c = el
        .children
        .iter()
        .find(|c| c.local == "Common" && c.prefix == "xr")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <xr:Common> (§1.0)")))?;
    c.claim_with_text();
    // РОЛЕВЫЕ права (ERP-witness БухучетЗарплатыОрганизаций.РедактированиеИстории:
    // `<View><xr:Common>false</xr:Common><xr:Value name="Role.X">true</xr:Value>×3`) — та же
    // Common+Value-модель, что Command/Use; ⟷ EDT `<view><for><value>B</value><role>R</role>`.
    let mut roles = Vec::new();
    for v in &el.children {
        if v.prefix == "xr" && v.local == "Common" {
            continue;
        }
        if !(v.prefix == "xr" && v.local == "Value") {
            return Err(FormError::Frame(format!(
                "<{tag}>: unexpected child <{}:{}> (§1.0)",
                v.prefix, v.local
            )));
        }
        let role = attr_value(v, "name")?;
        v.claim_with_text();
        expect_no_children(v)?;
        let val = match v.text.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FormError::Frame(format!(
                    "<{tag}><xr:Value name={role:?}>={other:?}, want bool (§1.0)"
                )));
            }
        };
        roles.push((role, val));
    }
    match c.text.as_str() {
        // Present-false = запрет/ролевое ограничение (общий доступ снят). Present-true = ОБЩИЙ
        // доступ, но с ролевыми исключениями (`<xr:Value>` роли ОБЯЗАТЕЛЬНЫ) — ERP-witness
        // ФизическиеЛица.ФормаЭлемента `<Edit><xr:Common>true</xr:Common><xr:Value>×N` ⟷ EDT
        // `<edit><common>true</common><for>×N`. Байт-точный round-trip держит инвариант writer'а
        // «тег эмитится ⟺ !common ∨ roles≠∅», поэтому present-true-БЕЗ-ролей неотличим от absent
        // (true,[]) ⇒ НЕ витнессирован и НЕ round-trip'ится → §1.0-отказ.
        "false" => Ok((false, roles)),
        "true" if !roles.is_empty() => Ok((true, roles)),
        "true" => Err(FormError::Frame(format!(
            "<{tag}><xr:Common>true</xr:Common> without <xr:Value>-roles is unwitnessed \
             (indistinguishable from absent-default; §1.0)"
        ))),
        other => Err(FormError::Frame(format!(
            "<{tag}><xr:Common>={other:?}, want bool (§1.0)"
        ))),
    }
}
