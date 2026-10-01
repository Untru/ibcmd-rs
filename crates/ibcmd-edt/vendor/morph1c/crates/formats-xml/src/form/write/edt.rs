//! WRITE · EDT dialect writer — form envelope/root ([`write_edt`]), command-interface
//! panel, root ext-info (report/document form), auto command bar, handlers, parameters
//! and form commands. See §1.0 / §3.2.

use super::*;

// ============================== EDT ==============================

pub(crate) fn write_edt(body: &FormBody) -> Result<Vec<u8>, FormError> {
    let mut root = OutElement::branch("form", "Form").attr("xmlns:form", FORM_NS_URI);

    // 0) форм-заголовок `<title>` (если задан) — ПЕРВЫЙ ребёнок корня. Мультиязычный —
    // ПОВТОРЕНИЕ `<title>` по одному на язык (ERP-witness КонтролируемыеСделкиОрганизаций.
    // ФормаДокументовСделки: ru+en; та же конвенция, что у title реквизита). Одноязычный —
    // как прежде.
    if let Some(PropertyValue::Localized(pairs)) = &body.title {
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                root.push(edt_title("title", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            root.push(edt_title("title", pairs));
        }
    }

    // 1) контрол-дерево (`<items>` прямыми детьми корня) — диспетчер по виду.
    for item in &body.items {
        root.push(edt_item(item)?);
    }

    // 2) commandBarLocation, showCommandBar (форм-атрибуты, идущие ДО autoCommandBar).
    push_edt_attr(&mut root, body, fr::F_COMMAND_BAR_LOCATION);
    push_edt_attr(&mut root, body, fr::F_SHOW_COMMAND_BAR);

    // 3) autoCommandBar (служебный контрол).
    if let Some(acb) = &body.auto_command_bar {
        root.push(edt_auto_command_bar(acb)?);
    }

    // 4) handlers (форм-события).
    for ev in &body.events {
        root.push(edt_handlers(ev));
    }

    // 4a) форм-уровневые исключённые команды (`<excludedCommands>` — регион между
    // handlers и форм-атрибутами; сверено по корпусу).
    for x in &body.excluded_commands {
        root.push(OutElement::leaf("", "excludedCommands", x.clone()));
    }

    // 5) остальные форм-атрибуты в EDT-порядке (сверен topo по 104 CommonForms:
    // width…windowOpeningMode, autoSave/enterKey/saveData, …, verticalScroll, …).
    for id in [
        fr::F_WIDTH,
        // height — сразу после width (метамодель managed-формы; witness АдресныеОбъекты
        // width→height→windowOpeningMode).
        fr::F_HEIGHT,
        fr::F_WINDOW_OPENING_MODE,
        fr::F_AUTO_SAVE_DATA_IN_SETTINGS,
        fr::F_ENTER_KEY_BEHAVIOR,
        fr::F_SAVE_DATA_IN_SETTINGS,
        fr::F_WINDOW_VIEW_MODE,
        fr::F_SAVE_WINDOW_SETTINGS,
        fr::F_AUTO_TITLE,
        fr::F_AUTO_URL,
        fr::F_GROUP,
        fr::F_HORIZONTAL_ALIGN,
        // verticalAlign/horizontalSpacing/verticalSpacing/childItemsWidth — контейнер-геометрия
        // формы, ПОСЛЕ horizontalAlign, ДО autoFillCheck (witness Report group→verticalAlign→
        // enabled; Document group→horizontalSpacing→childItemsWidth→autoFillCheck;
        // verticalSpacing — УничтожениеПерсональныхДанных group→verticalSpacing→autoFillCheck).
        fr::F_VERTICAL_ALIGN,
        fr::F_HORIZONTAL_SPACING,
        fr::F_VERTICAL_SPACING,
        fr::F_CHILD_ITEMS_WIDTH,
        fr::F_AUTO_FILL_CHECK,
        fr::F_ALLOW_FORM_CUSTOMIZE,
        fr::F_ENABLED,
        // scale — после enabled, ДО showTitle (witness ПомощникСозданияОбменаДанными.
        // ВыборТипаТранспорта: enabled→scale=101.0→showTitle).
        fr::F_SCALE,
        fr::F_VERTICAL_SCROLL,
        fr::F_SHOW_TITLE,
        fr::F_SHOW_CLOSE_BUTTON,
        fr::F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT,
        fr::F_CONVERSATIONS_REPRESENTATION,
    ] {
        push_edt_attr(&mut root, body, id);
    }

    // 5a) заголовок группы кнопок создания — сразу ПОСЛЕ showCloseButton-региона (сверено 4/4:
    // showCloseButton→createButtonsGroupTitle→attributes).
    if let Some(PropertyValue::Localized(pairs)) = &body.create_buttons_group_title {
        if !pairs.is_empty() {
            push_edt_title_multi(&mut root, "createButtonsGroupTitle", pairs);
        }
    }

    // 5b) командная панель мобильного устройства (ПОВТОРЯЕМЫЙ корневой элемент, по одному на
    // пункт; ПОСЛЕ форм-атрибутов, ДО данных-модели; сверено ПечатьДокументовOfficeOpen/
    // ФормаОтчета/ВариантыОтчетов×9).
    push_edt_mobile_command_bar(&mut root, &body.mobile_device_command_bar)?;

    // 6) данные-модель: attributes, formCommands, parameters (порядок регионов корпусный).
    for a in &body.data_attributes {
        root.push(edt_data_attribute(a)?);
    }
    for cmd in &body.commands {
        root.push(edt_form_command(cmd)?);
    }
    for p in &body.parameters {
        root.push(edt_parameter(p)?);
    }

    // 7) commandInterface (фикс-блок; панели пусты в обычной форме, ЛИБО несут cmiFragmentRecord'ы).
    if body.command_interface {
        let mut ci = OutElement::branch("", "commandInterface");
        ci.push(edt_cmi_panel(
            "navigationPanel",
            &body.form_ci_navigation_panel,
        ));
        ci.push(edt_cmi_panel("commandBar", &body.form_ci_command_bar));
        root.push(ci);
    }

    // 8) корневой extInfo формы (`<extInfo xsi:type="form:*FormExtInfo">`) — последний ребёнок.
    if let Some(rx) = &body.root_ext_info {
        // groupList — форм-атрибут F_GROUP_LIST, который EDT несёт ВНУТРИ extInfo (не прямым
        // ребёнком корня; edt_table/hardcoded-список его не эмитят). Достаём из bag.
        let group_list = match attr_value(body, fr::F_GROUP_LIST) {
            Some(PropertyValue::Str(s)) => Some(s.as_str()),
            _ => None,
        };
        root.push(edt_root_ext_info(
            rx,
            body.report_form.as_ref(),
            body.use_for_folders_and_items.as_deref(),
            body.document_form.as_ref(),
            group_list,
        ));
    }

    // `xmlns:core` — объявляется РОВНО когда тело несёт хоть один `core:`-тип
    // (`core:ColorRef`/`core:PictureRef`/`core:NumberValue`/`core:StringValue`); позиция
    // фиксирована МЕЖДУ `xmlns:xsi` и `xmlns:form` (сверено 104/104). Выводим из
    // построенного дерева, чтобы не тащить presence-флаг через каждый кодек.
    if out_uses_prefix(&root, "core_1:") {
        root.attrs.insert(0, ("xmlns:core_1".to_string(), "http://g5.1c.ru/v8/dt/data-composition-system/core".to_string()));
    }
    if out_uses_core(&root) {
        root.attrs
            .insert(0, ("xmlns:core".to_string(), CORE_NS_URI.to_string()));
    }
    // `xmlns:xsi` — ПЕРВЫМ, РОВНО когда тело несёт хоть один `xsi:`-АТРИБУТ (`xsi:type`/
    // `xsi:nil`; сверено 771/772 SSL — без xsi лишь минимальная ПередачаПараметров).
    if out_uses_attr_name_prefix(&root, "xsi:") {
        root.attrs
            .insert(0, ("xmlns:xsi".to_string(), XSI_NS_URI.to_string()));
    }
    // `xmlns:schema` — ПОСЛЕДНИМ (после `xmlns:form`), РОВНО когда тело несёт `schema:`-тип
    // (DCS-схема-поля реквизита-динсписка; АдреснаяКнига/ФайлыВТоме).
    if out_uses_prefix(&root, "schema:") {
        root.attrs
            .push(("xmlns:schema".to_string(), SCHEMA_NS_URI.to_string()));
    }
    // `xmlns:settings` — ПОСЛЕДНИМ, РОВНО когда тело несёт `settings:`-тип (appearance
    // вычисляемого поля динсписка; ERP-witness ОтклоненияВСтоимостиТоваров: core, form,
    // settings — schema+settings вместе не витнесснуты, порядок schema→settings — модельный).
    if out_uses_prefix(&root, "settings:") {
        root.attrs
            .push(("xmlns:settings".to_string(), SETTINGS_NS_URI.to_string()));
    }

    Ok(render(&edt_envelope(), &root))
}

/// Собрать EDT панель командного интерфейса (`navigationPanel`/`commandBar`). Пустой список ⇒
/// самозакрывающийся `<panel/>` (обычная форма); иначе `<panel><cmiFragmentRecord>…` (RE:
/// CommonForm.ВыборИсполнителяБизнесПроцесса). Инверсия [`read_edt_cmi_panel`].
pub(crate) fn edt_cmi_panel(local: &str, items: &[FormCiItem]) -> OutElement {
    if items.is_empty() {
        return OutElement::self_closing("", local);
    }
    let mut panel = OutElement::branch("", local);
    for it in items {
        let mut rec = OutElement::branch("", "cmiFragmentRecord");
        rec.push(OutElement::leaf("", "command", it.command.clone()));
        // type: EDT опускает общий дефолт `Auto` (98/101).
        if it.ty != "Auto" {
            rec.push(OutElement::leaf("", "type", it.ty.clone()));
        }
        if let Some(path) = &it.command_parameter {
            let mut parameter = OutElement::branch("", "commandParameter").attr("xsi:type", "form:DataPath");
            parameter.push(OutElement::leaf("", "segments", path.clone()));
            rec.push(parameter);
        }
        // group и index НЕЗАВИСИМЫ (census ERP): EDT эмитит `<index>` ВСЕГДА при group (включая 0)
        // И как index-БЕЗ-группы (размещение по индексу на корне панели, значения ≥1). Инверсия —
        // `read_edt_cmi_panel` (group-БЕЗ-index не витнесснут, ридер его отвергает).
        if let Some(g) = &it.group {
            rec.push(OutElement::leaf("", "group", g.clone()));
        }
        if let Some(i) = it.index {
            rec.push(OutElement::leaf("", "index", i.to_string()));
        }
        match it.user_visible {
            None => {} // видимость не задана — EDT не несёт <userVisible> (1/101).
            // Ролевой пункт: `<userVisible>[<common>true</common>]<for><value>B</value><role>R
            // </role></for>×N</userVisible>` (общий флаг + пер-ролевые исключения; witness
            // ЗаказКлиента.ФормаСписка). Безролевой — прежняя кодировка (байт-идентична).
            Some(common) if !it.user_visible_roles.is_empty() => {
                let mut uv = OutElement::branch("", "userVisible");
                if common {
                    uv.push(OutElement::leaf("", "common", "true"));
                }
                for (role, val) in &it.user_visible_roles {
                    let mut f = OutElement::branch("", "for");
                    if *val { f.push(OutElement::leaf("", "value", "true")); }
                    f.push(OutElement::leaf("", "role", role.clone()));
                    uv.push(f);
                }
                rec.push(uv);
            }
            Some(true) => {
                let mut uv = OutElement::branch("", "userVisible");
                uv.push(OutElement::leaf("", "common", "true"));
                rec.push(uv);
            }
            Some(false) => rec.push(OutElement::self_closing("", "userVisible")),
        }
        panel.push(rec);
    }
    panel
}

/// Несёт ли построенное EDT-дерево хоть один `core:`-тип (по значению любого атрибута,
/// начинающегося с `core:` — это `xsi:type="core:…"`). Рекурсивно.
pub(crate) fn out_uses_core(el: &OutElement) -> bool {
    out_uses_prefix(el, "core:")
}

/// Несёт ли дерево хоть один атрибут-значение с данным префиксом (напр. `xsi:type="schema:…"`).
/// Рекурсивно.
pub(crate) fn out_uses_prefix(el: &OutElement, prefix: &str) -> bool {
    el.attrs.iter().any(|(_, v)| v.starts_with(prefix))
        || el.children.iter().any(|c| out_uses_prefix(c, prefix))
}

/// Несёт ли дерево хоть один атрибут с данным префиксом ИМЕНИ (напр. `xsi:type`/`xsi:nil`).
/// Рекурсивно.
pub(crate) fn out_uses_attr_name_prefix(el: &OutElement, prefix: &str) -> bool {
    el.attrs.iter().any(|(n, _)| n.starts_with(prefix))
        || el
            .children
            .iter()
            .any(|c| out_uses_attr_name_prefix(c, prefix))
}

/// Эмитить один форм-атрибут EDT (пропуская пер-форматный дефолт).
pub(crate) fn push_edt_attr(root: &mut OutElement, body: &FormBody, id: morph1c_core::ir::FieldId) {
    if let Some(node) = edt_attr_node(id, attr_value(body, id)) {
        root.push(node);
    }
}

pub(crate) fn edt_auto_command_bar(acb: &morph1c_core::ir::AutoCommandBar) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "autoCommandBar");
    el.push(OutElement::leaf("", "name", acb.name.clone()));
    el.push(OutElement::leaf("", "id", acb.id.to_string()));
    // displayImportance — после name/id, ДО items (метамодель AutoCommandBar; witness DSS).
    if let Some(di) = &acb.display_importance {
        el.push(OutElement::leaf("", "displayImportance", di.clone()));
    }
    // Дочерние контролы панели (`<items>`).
    for item in &acb.items {
        el.push(edt_item(item)?);
    }
    // `<visible>` — ДАННЫЕ (дефолт true ⇒ эмитим только false). Позиция ДОСЛОВНО из модели
    // EDT (`models/forms/forms.jsonl`, класс AutoCommandBar: … items → visible → enabled …
    // → horizontalAlign → autoFill), а не подобрана: порядок полей EDT задаёт метамодель.
    if !acb.visible {
        el.push(OutElement::leaf("", "visible", "false"));
    }
    // horizontalAlign: EDT ОПУСКАЕТ `Auto` (4-состояние с противоположными омиссиями;
    // SSL cross-census — см. reader), остальное эмитит явно (Left×1238).
    if let Some(h) = &acb.horizontal_align {
        if h != "Auto" {
            el.push(OutElement::leaf("", "horizontalAlign", h.clone()));
        }
    }
    // autoFill: ДАННЫЕ (EDT-дефолт false ⇒ эмитим только true; сверено 104/104 ACB).
    if acb.auto_fill {
        el.push(OutElement::leaf("", "autoFill", "true"));
    }
    Ok(el)
}

pub(crate) fn edt_handlers(ev: &morph1c_core::ir::FormEvent) -> OutElement {
    let mut h = OutElement::branch("", "handlers");
    h.push(OutElement::leaf("", "event", ev.name.clone()));
    h.push(OutElement::leaf("", "name", ev.handler.clone()));
    h
}

/// EDT корневой `<extInfo xsi:type="form:*FormExtInfo">`: тип формы (kind) + СОБСТВЕННЫЕ
/// обработчики (`<handlers>`) + (у формы отчёта) поля отчёта. Пустой ⇒ самозакрытие.
///
/// Порядок детей у формы отчёта (corpus fact): `handlers*`, `settingsForm` (кроме типа `Main` —
/// он неявен ОТСУТСТВИЕМ тега), `showState` (омит `Auto`),
/// `reportResult`/`detailsInformation`/`currentVariantPresentationField` (форма `Main`, если
/// заданы), `userSettingsGroup` (если задан — ПОСЛЕ report-полей, SSL-витнесс),
/// `reportResultViewMode` (омит `Auto`), `viewModeApplicationOnSetReportResult` (омит `Auto`).
pub(crate) fn edt_root_ext_info(
    rx: &morph1c_core::ir::FormRootExtInfo,
    report: Option<&ReportFormInfo>,
    use_for_folders_and_items: Option<&str>,
    document_form: Option<&morph1c_core::ir::DocumentFormInfo>,
    group_list: Option<&str>,
) -> OutElement {
    let mut ex = OutElement::branch("", "extInfo").attr("xsi:type", rx.kind.clone());
    for ev in &rx.events {
        ex.push(edt_handlers(ev));
    }
    // `userSettingsGroup` формы КОМПОНОВЩИКА НАСТРОЕК (`form:SettingsComposerFormExtInfo`) —
    // ПОСЛЕ handlers (witness .НастройкаОтборовСписка). Взаимоисключима с report/document-блоками
    // (те несут свою группу в ReportFormInfo) ⇒ Some ⟺ не-отчётная/не-документная форма.
    if let Some(u) = &rx.user_settings_group {
        ex.push(OutElement::leaf("", "userSettingsGroup", u.clone()));
    }
    // `groupList` (форма динамического списка) — форм-атрибут F_GROUP_LIST, который EDT держит
    // ВНУТРИ extInfo (Designer — прямым ребёнком корня). ПОСЛЕ handlers/userSettingsGroup, ДО
    // document/report-блоков (ERP-witness Catalog.ВидыОтправляемыхДокументов.ФормаСписка: extInfo
    // несёт ЛИШЬ `<groupList>Дерево`).
    if let Some(gl) = group_list {
        ex.push(OutElement::leaf("", "groupList", gl.to_string()));
    }
    // Форма объекта-документа: EDT ОПУСКАЕТ дефолты autoTime=CurrentOrLast/usePostingMode=Auto,
    // эмитит лишь non-default; repostOnWrite эмитит лишь `true` (опускает `false`). После handlers.
    if let Some(d) = document_form {
        if d.auto_time != morph1c_core::ir::DOCUMENT_AUTO_TIME_DEFAULT {
            ex.push(OutElement::leaf("", "autoTime", d.auto_time.clone()));
        }
        if d.use_posting_mode != morph1c_core::ir::DOCUMENT_USE_POSTING_MODE_DEFAULT {
            ex.push(OutElement::leaf(
                "",
                "usePostingMode",
                d.use_posting_mode.clone(),
            ));
        }
        if d.repost_on_write {
            ex.push(OutElement::leaf("", "repostOnWrite", "true"));
        }
    }
    if let Some(r) = report {
        if r.settings_form != REPORT_FORM_MAIN {
            ex.push(OutElement::leaf(
                "",
                "settingsForm",
                r.settings_form.clone(),
            ));
        }
        if r.show_state != REPORT_FORM_AUTO {
            ex.push(OutElement::leaf("", "showState", r.show_state.clone()));
        }
        if let Some(v) = &r.report_result {
            ex.push(OutElement::leaf("", "reportResult", v.clone()));
        }
        if let Some(v) = &r.details_data {
            ex.push(OutElement::leaf("", "detailsInformation", v.clone()));
        }
        if let Some(v) = &r.variant_appearance {
            ex.push(OutElement::leaf(
                "",
                "currentVariantPresentationField",
                v.clone(),
            ));
        }
        // userSettingsGroup — ПОСЛЕ reportResult/detailsInformation/currentVariantPresentationField
        // (SSL EDT: reportResult<userSettingsGroup×2, detailsInformation<userSettingsGroup×2; напр.
        // ДатыЗапретаЗагрузки/ДатыЗапретаИзменения.ФормаОтчета). Прежде эмитился ДО них — ломал порядок.
        if let Some(u) = &r.user_settings_group {
            ex.push(OutElement::leaf("", "userSettingsGroup", u.clone()));
        }
        if r.report_result_view_mode != REPORT_FORM_AUTO {
            ex.push(OutElement::leaf(
                "",
                "reportResultViewMode",
                r.report_result_view_mode.clone(),
            ));
        }
        if r.view_mode_application != REPORT_FORM_AUTO {
            ex.push(OutElement::leaf(
                "",
                "viewModeApplicationOnSetReportResult",
                r.view_mode_application.clone(),
            ));
        }
    }
    // ФОРМ-уровневое useForFoldersAndItems (после handlers/report-полей). EDT ОПУСКАЕТ дефолт
    // `Items` (эмитит лишь `Folders`); присутствует у иерархических форм (не у report-форм).
    if let Some(v) = use_for_folders_and_items {
        if v != morph1c_core::ir::FOLDERS_AND_ITEMS_DEFAULT {
            ex.push(OutElement::leaf("", "useForFoldersAndItems", v.to_string()));
        }
    }
    if ex.children.is_empty() {
        return OutElement::self_closing("", "extInfo").attr("xsi:type", rx.kind.clone());
    }
    ex
}

pub(crate) fn edt_parameter(p: &FormParameter) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "parameters");
    el.push(OutElement::leaf("", "name", p.name.clone()));
    match &p.value_type {
        None => el.push(OutElement::self_closing("", "valueType")),
        Some(ts) => el.push(
            crate::type_codec::encode(crate::TypeDialect::Edt, "", "valueType", ts)
                .map_err(FormError::Frame)?,
        ),
    }
    if p.key_parameter {
        el.push(OutElement::leaf("", "keyParameter", "true"));
    }
    Ok(el)
}

/// EDT `<formCommands>` из [`FormCommand`]. Физический порядок EDT (сверен по 512
/// командам корпуса): name, title, id, toolTip, use, shortcut, picture, action,
/// actionPurpose, representation, modifiesStoredData, currentRowUse,
/// associatedTableElementId, selectedRowsUse. `<use><common>true` — EDT-константа;
/// `currentRowUse`/`selectedRowsUse` опускаются при EDT-дефолте `Use`.
pub(crate) fn edt_form_command(cmd: &FormCommand) -> Result<OutElement, FormError> {
    let get = |id: morph1c_core::ir::FieldId| cmd.get(id);
    let mut el = OutElement::branch("", "formCommands");
    el.push(OutElement::leaf("", "name", cmd.name.clone()));
    if let Some(PropertyValue::Localized(pairs)) = get(fc::F_TITLE) {
        if !pairs.is_empty() {
            push_edt_title_multi(&mut el, "title", pairs);
        }
    }
    el.push(OutElement::leaf("", "id", cmd.id.to_string()));
    if let Some(PropertyValue::Localized(pairs)) = get(fc::F_TOOL_TIP) {
        // Многоязычная подсказка — ПОВТОРЕНИЕ элемента (по одному `<toolTip>` на язык;
        // witness ВводКонтактнойИнформации.ВводАдреса toolTip×6 — как engine-Localized).
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                el.push(edt_title("toolTip", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            el.push(edt_title("toolTip", pairs));
        }
    }
    // use: безролевая — EDT-константа `<common>true`; РОЛЕВАЯ (bag F_CMD_USE) — entries[0] =
    // Bool(common), entries[1..] = роли. common=false ⇒ `<for>×N` БЕЗ `<common>` (ERP ×371);
    // common=true ⇒ ВЕДУЩИЙ `<common>true</common>` + `<for>×N` (общий доступ + ролевые
    // исключения; ERP ×20, witness ЖурналДокументовБезналичныеПлатежи.ФормаСписка ⟷ Designer
    // `<Use><xr:Common>true</xr:Common><xr:Value>×N`).
    match cmd.get(tables::F_CMD_USE) {
        None => {
            let mut use_el = OutElement::branch("", "use");
            use_el.push(OutElement::leaf("", "common", "true"));
            el.push(use_el);
        }
        Some(PropertyValue::List(entries)) => {
            let mut use_el = OutElement::branch("", "use");
            if matches!(entries.first(), Some(PropertyValue::Bool(true))) {
                use_el.push(OutElement::leaf("", "common", "true"));
            }
            for e in entries.iter().skip(1) {
                if let PropertyValue::List(pair) = e {
                    if let (Some(PropertyValue::Str(role)), Some(PropertyValue::Bool(v))) =
                        (pair.first(), pair.get(1))
                    {
                        let mut f = OutElement::branch("", "for");
                        // EDT опускает дефолт `false` (роль без явного значения), эмитит лишь `true`
                        // (ERP-witness Document.Отпуск.ФормаДокумента ПодробнееОРасчетеНДФЛ ×345).
                        if *v {
                            f.push(OutElement::leaf("", "value", "true"));
                        }
                        f.push(OutElement::leaf("", "role", role.clone()));
                        use_el.push(f);
                    }
                }
            }
            el.push(use_el);
        }
        Some(other) => {
            return Err(FormError::Frame(format!(
                "command use: expected List, got {other:?} (§1.0)"
            )))
        }
    }
    if let Some(PropertyValue::Str(s)) = get(fc::F_SHORTCUT) {
        if !s.is_empty() {
            el.push(OutElement::leaf("", "shortcut", s.clone()));
        }
    }
    if let Some(v) = get(fc::F_PICTURE) {
        // Канон `List([Ref, Bool(lt)])`; EDT LoadTransparent не несёт (lt игнор).
        let (r, _lt) = super::super::fields::picture_ref_lt(v)?;
        if !r.is_empty() {
            let mut p = OutElement::branch("", "picture").attr("xsi:type", "core:PictureRef");
            p.push(OutElement::leaf("", "picture", r.to_string()));
            el.push(p);
        }
    }
    if let Some(PropertyValue::Str(h)) = get(fc::F_ACTION) {
        if !h.is_empty() {
            let mut a = OutElement::branch("", "action")
                .attr("xsi:type", "form:FormCommandHandlerContainer");
            let mut hd = OutElement::branch("", "handler");
            hd.push(OutElement::leaf("", "name", h.clone()));
            a.push(hd);
            el.push(a);
        }
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_ACTION_PURPOSE) {
        el.push(OutElement::leaf(
            "",
            "actionPurpose",
            t.as_str().to_string(),
        ));
    }
    // functionalOptions: повторяемый leaf на каждую опцию (метамодель: actionPurpose 8 →
    // functionalOptions 9 → representation 10; сверено 39/39).
    if let Some(PropertyValue::List(fos)) = get(fc::F_FUNCTIONAL_OPTIONS) {
        for fo in fos {
            if let PropertyValue::Ref(r) = fo {
                el.push(OutElement::leaf("", "functionalOptions", r.clone()));
            }
        }
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_REPRESENTATION) {
        el.push(OutElement::leaf(
            "",
            "representation",
            t.as_str().to_string(),
        ));
    }
    if let Some(PropertyValue::Bool(true)) = get(fc::F_MODIFIES_STORED_DATA) {
        el.push(OutElement::leaf("", "modifiesStoredData", "true"));
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_CURRENT_ROW_USE) {
        if t.as_str() != fc::ROW_USE_EDT_DEFAULT {
            el.push(OutElement::leaf(
                "",
                "currentRowUse",
                t.as_str().to_string(),
            ));
        }
    }
    if let Some(PropertyValue::Str(v)) = get(fc::F_ASSOCIATED_TABLE_ELEMENT_ID) {
        if !v.is_empty() {
            let mut a = OutElement::branch("", "associatedTableElementId")
                .attr("xsi:type", "core:StringValue");
            a.push(OutElement::leaf("", "value", v.clone()));
            el.push(a);
        }
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_SELECTED_ROWS_USE) {
        if t.as_str() != fc::ROW_USE_EDT_DEFAULT {
            el.push(OutElement::leaf(
                "",
                "selectedRowsUse",
                t.as_str().to_string(),
            ));
        }
    }
    Ok(el)
}
