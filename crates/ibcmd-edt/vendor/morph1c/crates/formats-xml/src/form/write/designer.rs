//! WRITE · Designer dialect writer — form envelope/root ([`write_designer`]),
//! command-interface panel, report-form fields, titles, item dispatcher, commands,
//! parameters and control events.

use super::*;

// ============================== Designer ==============================

pub(crate) fn write_designer(body: &FormBody) -> Result<Vec<u8>, FormError> {
    // Версия ПИСАТЕЛЯ (FORMATS.md §1: формат выхода — параметр): конверт (`ns-блок` +
    // `version=`) эмитится ТАРГЕТ-версией из амбьентного round-trip таргета
    // (`with_roundtrip_target`; вне scope — SSL 2.21, прежнее поведение). Не-witnessed
    // таргет → громкая ошибка (§1.0), зеркало `formats_designer::common::emit_root_envelope`.
    let target = morph1c_core::version::current_roundtrip_target()
        .unwrap_or(morph1c_core::version::SSL);
    let profile = form_profile_for(target).ok_or_else(|| {
        FormError::Envelope(format!(
            "no Designer <Form> envelope profile for format {target} (witnessed: {})",
            witnessed_form_versions()
        ))
    })?;
    let mut root = OutElement::branch("", "Form");
    for (n, u) in profile.ns_block {
        root = root.attr(*n, *u);
    }
    root = root.attr("version", profile.version_value);

    // 0) форм-заголовок `<Title>` (если задан) — ПЕРВЫЙ ребёнок (БЕЗ formatted-атрибута).
    if let Some(PropertyValue::Localized(pairs)) = &body.title {
        if !pairs.is_empty() {
            root.push(designer_title(pairs, false));
        }
    }

    // Designer-порядок (сверен topo по 104 CommonForms): Width, WindowOpeningMode,
    // AutoSave/EnterKey/SaveData, SaveWindowSettings, AutoTitle, AutoURL, Group,
    // AutoFillCheck, HorizontalAlign, Customizable, CommandBarLocation, VerticalScroll,
    // ConversationsRepresentation, WindowViewMode, CommandSet, ShowCommandBar,
    // AutoCommandBar, Events, ChildItems, Attributes, Commands, Parameters.
    push_des_attr(&mut root, body, fr::F_WIDTH);
    push_des_attr(&mut root, body, fr::F_HEIGHT);
    push_des_attr(&mut root, body, fr::F_WINDOW_OPENING_MODE);
    push_des_attr(&mut root, body, fr::F_AUTO_SAVE_DATA_IN_SETTINGS);
    push_des_attr(&mut root, body, fr::F_ENTER_KEY_BEHAVIOR);
    push_des_attr(&mut root, body, fr::F_SAVE_DATA_IN_SETTINGS);
    push_des_attr(&mut root, body, fr::F_SAVE_WINDOW_SETTINGS);
    push_des_attr(&mut root, body, fr::F_AUTO_TITLE);
    push_des_attr(&mut root, body, fr::F_AUTO_URL);
    push_des_attr(&mut root, body, fr::F_GROUP);
    // VerticalSpacing — СРАЗУ после Group (witness УничтожениеПерсональныхДанных.
    // ФормаСозданияАктов: Group→VerticalSpacing→CommandBarLocation).
    push_des_attr(&mut root, body, fr::F_VERTICAL_SPACING);
    // Scale — после Group, ДО AutoCommandBar (witness ПомощникСозданияОбменаДанными.
    // ВыборТипаТранспорта: Group→Scale=101→AutoCommandBar).
    push_des_attr(&mut root, body, fr::F_SCALE);
    push_des_attr(&mut root, body, fr::F_AUTO_FILL_CHECK);
    push_des_attr(&mut root, body, fr::F_HORIZONTAL_ALIGN);
    // verticalAlign/horizontalSpacing/childItemsWidth — контейнер-геометрия, ПОСЛЕ HorizontalAlign,
    // ДО Customizable/VerticalScroll (witness Group→HorizontalSpacing→ChildItemsWidth,
    // Group→ChildItemsWidth→VerticalScroll).
    push_des_attr(&mut root, body, fr::F_VERTICAL_ALIGN);
    push_des_attr(&mut root, body, fr::F_HORIZONTAL_SPACING);
    push_des_attr(&mut root, body, fr::F_CHILD_ITEMS_WIDTH);
    push_des_attr(&mut root, body, fr::F_ALLOW_FORM_CUSTOMIZE);
    push_des_attr(&mut root, body, fr::F_COMMAND_BAR_LOCATION);
    push_des_attr(&mut root, body, fr::F_VERTICAL_SCROLL);
    push_des_attr(&mut root, body, fr::F_CONVERSATIONS_REPRESENTATION);
    push_des_attr(&mut root, body, fr::F_WINDOW_VIEW_MODE);
    // Командная панель мобильного устройства (корневой список; ПОСЛЕ ConversationsRepresentation/
    // WindowViewMode, ДО CommandSet; сверено ФормаОтчета/ПечатьДокументовOfficeOpen).
    if !body.mobile_device_command_bar.is_empty() {
        root.push(designer_mobile_command_bar(
            &body.mobile_device_command_bar,
        )?);
    }
    // Форм-уровневый CommandSet (между CommandBarLocation/WindowViewMode и ShowCommandBar).
    if !body.excluded_commands.is_empty() {
        let mut cs = OutElement::branch("", "CommandSet");
        for x in &body.excluded_commands {
            cs.push(OutElement::leaf("", "ExcludedCommand", x.clone()));
        }
        root.push(cs);
    }
    // `GroupList` (форма динамического списка) — прямой ребёнок корня ПОСЛЕ CommandSet, ДО
    // AutoCommandBar (ERP-witness Catalog.ВидыОтправляемыхДокументов.ФормаСписка: VerticalScroll→
    // CommandSet→GroupList→AutoCommandBar). EDT держит это же поле ВНУТРИ extInfo (см.
    // edt_root_ext_info). Форм-атрибут F_GROUP_LIST — эмитим напрямую (не в edt/designer-attr-петле,
    // где он не значится: witness-позиция специфична для дин-списка).
    push_des_attr(&mut root, body, fr::F_GROUP_LIST);
    // Форма объекта-документа: Designer эмитит ВСЕ ТРИ поля прямыми детьми корня СРАЗУ после
    // CommandSet (witness Анкета CommandSet→AutoTime→UsePostingMode→RepostOnWrite→AutoCommandBar);
    // EDT держит их внутри form:DocumentFormExtInfo, опуская дефолты (см. edt_root_ext_info).
    if let Some(d) = &body.document_form {
        root.push(OutElement::leaf("", "AutoTime", d.auto_time.clone()));
        root.push(OutElement::leaf(
            "",
            "UsePostingMode",
            d.use_posting_mode.clone(),
        ));
        root.push(OutElement::leaf(
            "",
            "RepostOnWrite",
            if d.repost_on_write { "true" } else { "false" },
        ));
    }
    // Enabled/ShowTitle/ShowCloseButton — ПОСЛЕ CommandSet, ДО CreateButtonsGroupTitle/
    // ShowCommandBar (SSL Designer-корень: Group<ShowTitle×12, CommandBarLocation<ShowTitle×4,
    // ShowTitle<ShowCommandBar×4, CommandSet<ShowCloseButton×1, ShowCloseButton<ShowCommandBar×1,
    // ShowTitle<AutoCommandBar×12; контрпримеров 0 — прежняя позиция ПОСЛЕ ShowCommandBar
    // была слепой «Designer не эмитит никогда» и ломала ShowTitle→ShowCommandBar).
    push_des_attr(&mut root, body, fr::F_ENABLED);
    push_des_attr(&mut root, body, fr::F_SHOW_TITLE);
    push_des_attr(&mut root, body, fr::F_SHOW_CLOSE_BUTTON);
    // Заголовок группы кнопок создания — ПОСЛЕ VerticalScroll/CommandSet, ДО ShowCommandBar/
    // AutoCommandBar (сверено 4/4: Наборы VerticalScroll→CreateButtonsGroupTitle→ShowCommandBar;
    // Файлы CommandSet→CreateButtonsGroupTitle→AutoCommandBar).
    if let Some(PropertyValue::Localized(pairs)) = &body.create_buttons_group_title {
        if !pairs.is_empty() {
            let mut t = designer_title(pairs, false);
            t.local = "CreateButtonsGroupTitle".to_string();
            root.push(t);
        }
    }
    push_des_attr(&mut root, body, fr::F_SHOW_COMMAND_BAR);
    // Форма отчёта: корневые поля (`form:ReportFormExtInfo` в EDT). Designer эмитит
    // ReportFormType/AutoShowState/[CustomSettingsFolder]/ReportResultViewMode/
    // ViewModeApplicationOnSetReportResult СРАЗУ после ShowCommandBar (corpus fact).
    if let Some(r) = &body.report_form {
        designer_report_form(&mut root, r);
    }
    // `<CustomSettingsFolder>` формы КОМПОНОВЩИКА НАСТРОЕК (не-отчётной; ⟺ EDT
    // `form:SettingsComposerFormExtInfo`>`userSettingsGroup`) — та же позиция, что у отчётного
    // CustomSettingsFolder (после CommandSet/ShowCommandBar, до UseForFoldersAndItems/AutoCommandBar;
    // witness .НастройкаОтборовСписка). Взаимоисключима с report_form (см. read).
    if let Some(u) = body
        .root_ext_info
        .as_ref()
        .and_then(|r| r.user_settings_group.as_ref())
    {
        root.push(OutElement::leaf("", "CustomSettingsFolder", u.clone()));
    }
    // ФОРМ-уровневое UseForFoldersAndItems — прямой ребёнок корня ПЕРЕД AutoCommandBar (corpus
    // fact 69/69). Designer эмитит ВСЕГДА (`Items`/`Folders`), не опуская дефолт (в отличие от EDT).
    if let Some(v) = &body.use_for_folders_and_items {
        root.push(OutElement::leaf("", "UseForFoldersAndItems", v.clone()));
    }

    if let Some(acb) = &body.auto_command_bar {
        // Регенерация Designer-flavor (инверсия канонизации read_designer, §1.6): ФОРМ-уровневая
        // служебная панель сериализуется в Designer с ПУСТЫМ именем (каноническое EDT-имя
        // `FormCommandBar` опускается), а commandInterface неявен (Designer его никогда не
        // эмитит — body.command_interface здесь не читается). Table-уровневые панели идут иным
        // путём (designer_item) и не затрагиваются.
        // `designer_named` (нерегулярность 3/876) — Designer нёс ЯВНОЕ имя: НЕ очищаем.
        let form_acb = if acb.name == FORM_COMMAND_BAR_NAME && !acb.designer_named {
            let mut a = acb.clone();
            a.name.clear();
            a
        } else {
            acb.clone()
        };
        root.push(designer_auto_command_bar(&form_acb)?);
    }
    // Единый корневой `<Events>` = ОБЪЕДИНЕНИЕ двух локусов IR: обычные форм-события
    // (`body.events`) И СОБСТВЕННЫЕ обработчики корневого extInfo (`root_ext_info.events` —
    // EDT-локус `<extInfo><handlers>`: `OnReadAtServer`/`AfterWrite`/`BeforeWrite`/…).
    // Designer их СЛИВАЕТ (статья B `docs/X_LEDGER_FORMS.md`), designer-ридер поэтому
    // оставляет всё в `body.events` и держит `root_ext_info.events` ПУСТЫМ (см.
    // `read::designer`) ⇒ designer→designer тут no-op, byte-exact сохранён. На EDT-производном
    // IR второй слот НЕПУСТ, и до r34 писатель его не читал — 323 обработчика (13 имён)
    // молча исчезали на SSL, 98 тел форм расходились в edt→xml→cf.
    //
    // ПОРЯДОК — платформенный (guid типа события), см. `morph1c_core::ir::merge_form_events`:
    // намайнено по 11 485 формам с событиями (SSL 813 + ERP 10 672), 11 485/11 485,
    // контрпримеров 0. Это НЕ «сначала обычные, потом extInfo»: extInfo-события ложатся
    // ВНУТРЬ обычных (витнесс `Catalog/…/ФормаЭлемента`), а у формы ДОКУМЕНТА порядок ещё и
    // сдвигается документным переопределением guid (`BeforeWrite`/`BeforeWriteAtServer`).
    {
        let ext_events: &[morph1c_core::ir::FormEvent] = body
            .root_ext_info
            .as_ref()
            .map(|r| r.events.as_slice())
            .unwrap_or(&[]);
        let document_ext = body
            .root_ext_info
            .as_ref()
            .is_some_and(|r| r.kind == "form:DocumentFormExtInfo");
        if !body.events.is_empty() || !ext_events.is_empty() {
            let mut events = OutElement::branch("", "Events");
            for ev in morph1c_core::ir::merge_form_events(&body.events, ext_events, document_ext) {
                events.push(designer_event(ev));
            }
            root.push(events);
        }
    }
    if !body.items.is_empty() {
        let mut ci = OutElement::branch("", "ChildItems");
        for item in &body.items {
            ci.push(designer_item(item)?);
        }
        root.push(ci);
    }
    // Designer эмитит `<Attributes>` ВСЕГДА (пустой ⇒ самозакрывающийся `<Attributes/>`;
    // сверено по корпусу: форма без реквизитов несёт `<Attributes/>`). ФОРМ-уровневое
    // условное оформление (`<ConditionalAppearance>` c dcsset:item) — ПОСЛЕ всех Attribute
    // ВНУТРИ `<Attributes>` (witness РаботаСФайлами.ВерсияПрисоединенногоФайла).
    if body.data_attributes.is_empty() && body.conditional_appearance.is_empty() {
        root.push(OutElement::self_closing("", "Attributes"));
    } else {
        let mut attrs = OutElement::branch("", "Attributes");
        for a in &body.data_attributes {
            attrs.push(designer_data_attribute(a)?);
        }
        if !body.conditional_appearance.is_empty() {
            let mut ca = OutElement::branch("", "ConditionalAppearance");
            for it in &body.conditional_appearance {
                ca.push(designer_dcs_item(it));
            }
            attrs.push(ca);
        }
        root.push(attrs);
    }
    // Пользовательские команды (`<Commands>` между Attributes и Parameters; пусто ⇒ нет).
    if !body.commands.is_empty() {
        let mut cmds = OutElement::branch("", "Commands");
        for cmd in &body.commands {
            cmds.push(designer_command(cmd)?);
        }
        root.push(cmds);
    }
    if !body.parameters.is_empty() {
        let mut params = OutElement::branch("", "Parameters");
        for p in &body.parameters {
            params.push(designer_parameter(p)?);
        }
        root.push(params);
    }
    // CommandInterface — ПОСЛЕДНИЙ регион формы; Designer эмитит его ТОЛЬКО когда панель несёт
    // пункты (пустой формоуровневый КИ синтезируется из AutoCommandBar на чтении, не эмитится).
    // Инверсия [`read_designer_command_interface`]. RE: ВыборИсполнителяБизнесПроцесса.
    if !body.form_ci_navigation_panel.is_empty() || !body.form_ci_command_bar.is_empty() {
        let mut ci = OutElement::branch("", "CommandInterface");
        if !body.form_ci_navigation_panel.is_empty() {
            ci.push(designer_cmi_panel(
                "NavigationPanel",
                &body.form_ci_navigation_panel,
            ));
        }
        if !body.form_ci_command_bar.is_empty() {
            ci.push(designer_cmi_panel("CommandBar", &body.form_ci_command_bar));
        }
        root.push(ci);
    }

    Ok(render(&designer_envelope(), &root))
}

/// Собрать Designer панель командного интерфейса (`NavigationPanel`/`CommandBar`) — по `<Item>`
/// на пункт: `<Command>` + `<Type>` (всегда) + `[<CommandGroup>]` + `[<Index>]` (опускается
/// `0`, НЕЗАВИСИМО от group — index-БЕЗ-группы (N≥1) эмитит `<Index>` без `<CommandGroup>`) +
/// видимость (`DefaultVisible=false` ⟺ задана; `<Visible><xr:Common>false` ⟺ задана `false`).
/// Инверсия [`read_designer_cmi_panel`]; правила — док [`FormCiItem`].
pub(crate) fn designer_cmi_panel(local: &str, items: &[FormCiItem]) -> OutElement {
    let mut panel = OutElement::branch("", local);
    for it in items {
        let mut item = OutElement::branch("", "Item");
        item.push(OutElement::leaf("", "Command", it.command.clone()));
        item.push(OutElement::leaf("", "Type", it.ty.clone()));
        if let Some(path) = &it.command_parameter {
            item.push(OutElement::leaf("", "Attribute", path.clone()));
        }
        if let Some(g) = &it.group {
            item.push(OutElement::leaf("", "CommandGroup", g.clone()));
        }
        if let Some(i) = it.index {
            if i != 0 {
                item.push(OutElement::leaf("", "Index", i.to_string()));
            }
        }
        if let Some(uv) = it.user_visible {
            item.push(OutElement::leaf("", "DefaultVisible", "false"));
            // `<Visible>` эмитится ⟺ common=false ЛИБО есть ролевые исключения (иначе Designer
            // опускает; witness: common=true+роли ЗаказКлиента.ФормаСписка несёт `<Visible>
            // <xr:Common>true><xr:Value>×N`). Роли — после `<xr:Common>` (⟺ EDT `<for>`).
            if !uv || !it.user_visible_roles.is_empty() {
                let mut vis = OutElement::branch("", "Visible");
                vis.push(OutElement::leaf("xr", "Common", if uv { "true" } else { "false" }));
                for (role, val) in &it.user_visible_roles {
                    vis.push(
                        OutElement::leaf("xr", "Value", if *val { "true" } else { "false" })
                            .attr("name", role.clone()),
                    );
                }
                item.push(vis);
            }
        }
        panel.push(item);
    }
    panel
}

pub(crate) fn push_des_attr(root: &mut OutElement, body: &FormBody, id: morph1c_core::ir::FieldId) {
    if let Some(node) = designer_attr_node(id, attr_value(body, id)) {
        root.push(node);
    }
}

/// Канонический литерал типа формы отчёта `Main` (неявен на EDT-стороне).
pub(crate) const REPORT_FORM_MAIN: &str = "Main";

/// Designer корневые поля формы отчёта. Порядок (corpus fact): `ReportResult`/`DetailsData`
/// (форма `Main`, если заданы), `ReportFormType` (всегда), `VariantAppearance` (`Main`, если
/// задан), `AutoShowState` (всегда), `CustomSettingsFolder` (если задан), `ReportResultViewMode`
/// (всегда, в т.ч. `Auto`), `ViewModeApplicationOnSetReportResult` (всегда).
pub(crate) fn designer_report_form(root: &mut OutElement, r: &ReportFormInfo) {
    if let Some(v) = &r.report_result {
        root.push(OutElement::leaf("", "ReportResult", v.clone()));
    }
    if let Some(v) = &r.details_data {
        root.push(OutElement::leaf("", "DetailsData", v.clone()));
    }
    root.push(OutElement::leaf(
        "",
        "ReportFormType",
        r.settings_form.clone(),
    ));
    if let Some(v) = &r.variant_appearance {
        root.push(OutElement::leaf("", "VariantAppearance", v.clone()));
    }
    root.push(OutElement::leaf("", "AutoShowState", r.show_state.clone()));
    if let Some(u) = &r.user_settings_group {
        root.push(OutElement::leaf("", "CustomSettingsFolder", u.clone()));
    }
    root.push(OutElement::leaf(
        "",
        "ReportResultViewMode",
        r.report_result_view_mode.clone(),
    ));
    root.push(OutElement::leaf(
        "",
        "ViewModeApplicationOnSetReportResult",
        r.view_mode_application.clone(),
    ));
}

/// Designer `<AutoCommandBar>`: порядок детей (сверен по 104 ACB корпуса) —
/// HorizontalAlign (опускается Left-дефолт), Autofill (опускается true-дефолт),
/// ChildItems. Полностью дефолтная пустая панель ⇒ самозакрывающийся элемент.
pub(crate) fn designer_auto_command_bar(
    acb: &morph1c_core::ir::AutoCommandBar,
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "AutoCommandBar")
        .attr("name", acb.name.clone())
        .attr("id", acb.id.to_string());
    // `DisplayImportance` — Designer-АТРИБУТ (после name/id; witness DSS `Low`).
    if let Some(di) = &acb.display_importance {
        el = el.attr("DisplayImportance", di.clone());
    }
    // `<Visible>` — ДАННЫЕ (Designer-дефолт true ⇒ эмитим только false), ПЕРВЫМ ребёнком:
    // единственный витнесс корпуса (`инт_Подписчики`) несёт его один, а у обычной группы
    // Designer тоже ставит `<Visible>` перед `<Title>` (тот же класс контрола `{22,…}`).
    if !acb.visible {
        el.push(OutElement::leaf("", "Visible", "false"));
    }
    // Designer опускает `Left` (свой ACB-дефолт); эмитит иное.
    if let Some(h) = &acb.horizontal_align {
        if h != "Left" {
            el.push(OutElement::leaf("", "HorizontalAlign", h.clone()));
        }
    }
    // `<Autofill>` — ДАННЫЕ (Designer-дефолт true ⇒ эмитим только false).
    if !acb.auto_fill {
        el.push(OutElement::leaf("", "Autofill", "false"));
    }
    // Дочерние контролы `<ChildItems>` (как правило Button-команды).
    if !acb.items.is_empty() {
        let mut ci = OutElement::branch("", "ChildItems");
        for item in &acb.items {
            ci.push(designer_item(item)?);
        }
        el.push(ci);
    }
    if el.children.is_empty() {
        // Всё-дефолтная пустая панель: `<AutoCommandBar name id/>` (сверено корпусом).
        el = OutElement::self_closing("", "AutoCommandBar")
            .attr("name", acb.name.clone())
            .attr("id", acb.id.to_string());
        if let Some(di) = &acb.display_importance {
            el = el.attr("DisplayImportance", di.clone());
        }
    }
    Ok(el)
}

/// Designer `<Title>` (`<v8:item>` пары). `formatted`: значение атрибута `formatted`
/// (`Some("false")` — контрол-заголовок; `Some("true")` — HTML-подсказка; `None` — без
/// атрибута, форм-заголовок).
pub(crate) fn designer_title_with(pairs: &[(Lang, String)], formatted: Option<&str>) -> OutElement {
    let mut t = OutElement::branch("", "Title");
    if let Some(f) = formatted {
        t = t.attr("formatted", f);
    }
    for (lang, content) in pairs {
        let mut it = OutElement::branch("v8", "item");
        it.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        it.push(OutElement::leaf("v8", "content", content.clone()));
        t.push(it);
    }
    t
}

/// Designer `<Title>` (`<v8:item>` пары). `formatted_false`: контрол-заголовки несут
/// `formatted="false"`; форм-заголовок — нет.
pub(crate) fn designer_title(pairs: &[(Lang, String)], formatted_false: bool) -> OutElement {
    designer_title_with(pairs, if formatted_false { Some("false") } else { None })
}

/// Designer диспетчер контрола [`FormItem`] → элемент по виду.
pub(crate) fn designer_item(item: &FormItem) -> Result<OutElement, FormError> {
    let k = item.kind.as_str();
    match k {
        "Button" => {
            designer_table_control(item, k, tables::BUTTON_BODY, &[], tables::DES_BUTTON_ORDER)
        }
        "Table" => {
            designer_table_control(item, k, tables::TABLE_BODY, &[], tables::DES_TABLE_ORDER)
        }
        // Добавление-как-контрол (`<SearchStringAddition>`/…): та же Designer-форма, что
        // Table-именованное добавление — переиспользуем писатель (visible/enabled/userVisible
        // EDT-каркас, Designer их не несёт).
        _ if tables::addition_kind(k).is_some() => designer_addition(item),
        _ if tables::decoration_kind(k).is_some() => {
            let dk = tables::decoration_kind(k).expect("checked");
            designer_table_control(item, k, tables::DECORATION_BODY, dk.ext, dk.des_order)
        }
        _ if tables::group_kind(k).is_some() => designer_table_control(
            item,
            k,
            tables::FORM_GROUP_BODY,
            tables::group_kind(k).expect("checked").ext,
            tables::DES_GROUP_ORDER,
        ),
        _ if tables::field_kind(k).is_some() => {
            let fk = tables::field_kind(k).expect("checked");
            // Designer-тег может расходиться с каноном (SpreadSheetDocumentField).
            designer_table_control(
                item,
                fk.des_tag,
                tables::FORM_FIELD_COMMON,
                fk.ext,
                fk.des_order,
            )
        }
        other => Err(FormError::Frame(format!(
            "Designer write: unsupported control kind {other:?}"
        ))),
    }
}

/// Designer `<Command name id>` из [`FormCommand`]. Физический порядок Designer (сверен
/// по 512 командам корпуса): Title, ToolTip, Shortcut, Picture, Action, Representation,
/// ModifiesSavedData, CurrentRowUse, AssociatedTableElementId, ActionPurpose,
/// SelectedRowsUse. `currentRowUse`/`selectedRowsUse` опускаются при Designer-дефолте
/// `Auto`; `<xr:LoadTransparent>` картинки — денормализация вида ссылки (StdPicture.*
/// → true, иначе false; реконструируется).
pub(crate) fn designer_command(cmd: &FormCommand) -> Result<OutElement, FormError> {
    let get = |id: morph1c_core::ir::FieldId| cmd.get(id);
    let mut el = OutElement::branch("", "Command")
        .attr("name", cmd.name.clone())
        .attr("id", cmd.id.to_string());
    if let Some(PropertyValue::Localized(pairs)) = get(fc::F_TITLE) {
        if !pairs.is_empty() {
            el.push(designer_title(pairs, false));
        }
    }
    if let Some(PropertyValue::Localized(pairs)) = get(fc::F_TOOL_TIP) {
        if !pairs.is_empty() {
            let mut t = OutElement::branch("", "ToolTip");
            for (lang, content) in pairs {
                let mut it = OutElement::branch("v8", "item");
                it.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                it.push(OutElement::leaf("v8", "content", content.clone()));
                t.push(it);
            }
            el.push(t);
        }
    }
    // Use — РОЛЕВОЕ ограничение (ERP-witness ГруппыСотрудников: Title→Use→Action; метамодель
    // FormCommand#4 после toolTip). `<xr:Common>` + `<xr:Value name>` на роль.
    if let Some(PropertyValue::List(entries)) = get(tables::F_CMD_USE) {
        let mut u = OutElement::branch("", "Use");
        if let Some(PropertyValue::Bool(common)) = entries.first() {
            u.push(OutElement::leaf(
                "xr",
                "Common",
                if *common { "true" } else { "false" },
            ));
        }
        for e in entries.iter().skip(1) {
            if let PropertyValue::List(pair) = e {
                if let (Some(PropertyValue::Str(role)), Some(PropertyValue::Bool(v))) =
                    (pair.first(), pair.get(1))
                {
                    u.push(
                        OutElement::leaf("xr", "Value", if *v { "true" } else { "false" })
                            .attr("name", role.clone()),
                    );
                }
            }
        }
        el.push(u);
    }
    if let Some(PropertyValue::Str(s)) = get(fc::F_SHORTCUT) {
        if !s.is_empty() {
            el.push(OutElement::leaf("", "Shortcut", s.clone()));
        }
    }
    if let Some(v) = get(fc::F_PICTURE) {
        // Канон `List([Ref, Bool(lt)])`; Designer эмитит НЕЗАВИСИМЫЙ LoadTransparent.
        let (r, lt) = super::super::fields::picture_ref_lt(v)?;
        if !r.is_empty() {
            let mut p = OutElement::branch("", "Picture");
            p.push(OutElement::leaf("xr", "Ref", r.to_string()));
            p.push(OutElement::leaf(
                "xr",
                "LoadTransparent",
                if lt { "true" } else { "false" },
            ));
            el.push(p);
        }
    }
    if let Some(PropertyValue::Str(h)) = get(fc::F_ACTION) {
        if !h.is_empty() {
            el.push(OutElement::leaf("", "Action", h.clone()));
        }
    }
    // FunctionalOptions: контейнер `<Item>` на опцию — между Action и Representation (39/39).
    if let Some(PropertyValue::List(fos)) = get(fc::F_FUNCTIONAL_OPTIONS) {
        if !fos.is_empty() {
            let mut c = OutElement::branch("", "FunctionalOptions");
            for fo in fos {
                if let PropertyValue::Ref(r) = fo {
                    c.push(OutElement::leaf("", "Item", r.clone()));
                }
            }
            el.push(c);
        }
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_REPRESENTATION) {
        el.push(OutElement::leaf(
            "",
            "Representation",
            t.as_str().to_string(),
        ));
    }
    if let Some(PropertyValue::Bool(true)) = get(fc::F_MODIFIES_STORED_DATA) {
        el.push(OutElement::leaf("", "ModifiesSavedData", "true"));
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_CURRENT_ROW_USE) {
        if t.as_str() != fc::ROW_USE_DESIGNER_DEFAULT {
            el.push(OutElement::leaf(
                "",
                "CurrentRowUse",
                t.as_str().to_string(),
            ));
        }
    }
    if let Some(PropertyValue::Str(v)) = get(fc::F_ASSOCIATED_TABLE_ELEMENT_ID) {
        if !v.is_empty() {
            el.push(
                OutElement::leaf("", "AssociatedTableElementId", v.clone())
                    .attr("xsi:type", "xs:string"),
            );
        }
    }
    // ActionPurpose — ПЕРЕД SelectedRowsUse (SSL Designer: ActionPurpose<SelectedRowsUse×4, 0
    // контрпримеров; напр. Подписанты.ОтправитьНаПодписание). Оба после CurrentRowUse/
    // AssociatedTableElementId (CurrentRowUse<ActionPurpose×405).
    if let Some(PropertyValue::Enum(t)) = get(fc::F_ACTION_PURPOSE) {
        el.push(OutElement::leaf(
            "",
            "ActionPurpose",
            t.as_str().to_string(),
        ));
    }
    if let Some(PropertyValue::Enum(t)) = get(fc::F_SELECTED_ROWS_USE) {
        if t.as_str() != fc::ROW_USE_DESIGNER_DEFAULT {
            el.push(OutElement::leaf(
                "",
                "SelectedRowsUse",
                t.as_str().to_string(),
            ));
        }
    }
    Ok(el)
}

pub(crate) fn designer_parameter(p: &FormParameter) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "Parameter").attr("name", p.name.clone());
    match &p.value_type {
        None => el.push(OutElement::self_closing("", "Type")),
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
    if p.key_parameter {
        el.push(OutElement::leaf("", "KeyParameter", "true"));
    }
    Ok(el)
}

/// Дописать Designer `<Events>` контрола, если есть.
pub(crate) fn push_designer_events(el: &mut OutElement, item: &FormItem) {
    // Designer держит ВСЕ события контрола в едином `<Events>`. Для Таблицы-динамического-списка
    // канон-IR держит СОБСТВЕННЫЕ обработчики отдельно (`dynamic_list_ext.events`, EDT-обёртка);
    // Designer-запись СЛИВАЕТ их в `<Events>` в ПЛАТФОРМЕННОМ (guid) порядке — тот же ключ, что
    // у формы (`morph1c_core::ir::merge_table_events`, табличное guid-пространство).
    // Designer-round-trip: `dynamic_list_ext.events` пуст (Designer-ридер кладёт всё в
    // `item.events`) ⇒ слияние тождественно, byte-exact сохранён; edt→designer:
    // extInfo-обработчики попадают в `<Events>` (не теряются) на СВОЁ место.
    //
    // ⚠ r34: прежнее «за обычными событиями» (конкатенация) НЕВЕРНО — `OnGetDataAtServer`
    // (9736…) встаёт МЕЖДУ табличными событиями. Ценз по 604 таблицам-ДС (SSL 14 + ERP 590):
    // guid-порядок 604/604, конкатенация ошибалась в 105 (SSL 8 + ERP 97).
    let dyn_events: &[morph1c_core::ir::FormEvent] = item
        .dynamic_list_ext
        .as_ref()
        .map(|d| d.events.as_slice())
        .unwrap_or(&[]);
    if item.events.is_empty() && dyn_events.is_empty() {
        return;
    }
    let mut events = OutElement::branch("", "Events");
    for ev in morph1c_core::ir::merge_table_events(&item.events, dyn_events) {
        events.push(designer_event(ev));
    }
    el.push(events);
}

#[cfg(any())]
mod events_locus_tests {
    use super::*;
    use morph1c_core::ir::{FormEvent, FormRootExtInfo};

    fn ev(name: &str, handler: &str) -> FormEvent {
        FormEvent {
            name: name.to_string(),
            handler: handler.to_string(),
        }
    }

    fn root_events_block(body: &FormBody) -> Vec<String> {
        let text = String::from_utf8(write_designer(body).expect("designer write")).unwrap();
        assert_eq!(
            text.matches("<Events>").count(),
            1,
            "ровно ОДИН корневой <Events>: {text}"
        );
        let inner = text
            .split_once("<Events>")
            .and_then(|(_, t)| t.split_once("</Events>"))
            .map(|(b, _)| b)
            .expect("нет <Events>");
        inner
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// ЛОКУС-АСИММЕТРИЯ СОБЫТИЙ (статья B `docs/X_LEDGER_FORMS.md`): писатель обязан эмитить
    /// ОБЪЕДИНЕНИЕ `body.events` и `root_ext_info.events` в ОДИН корневой `<Events>` — иначе на
    /// EDT-производном IR собственные обработчики корневого extInfo ИСЧЕЗАЮТ (r34: 323
    /// обработчика по 13 именам, 98 расходящихся тел форм в `edt→xml→cf` на SSL).
    ///
    /// Витнесс: `InformationRegister/ПубличныеИдентификаторыСинхронизируемыхОбъектов/
    /// ФормаЗаписи` — `OnOpen` (обычное) + `BeforeWrite` (собственное extInfo) в ЭТОМ порядке
    /// (guid `3ccc650e…` < `9cc34712…`).
    #[test]
    fn root_ext_info_events_merge_into_single_events_block() {
        let mut body = FormBody::new();
        body.events = vec![ev("OnOpen", "ПриОткрытии")];
        body.root_ext_info = Some(FormRootExtInfo {
            kind: "form:InformationRegisterManagerFormExtInfo".into(),
            events: vec![ev("BeforeWrite", "ПередЗаписью")],
            user_settings_group: None,
        });
        assert_eq!(
            root_events_block(&body),
            vec![
                "<Event name=\"OnOpen\">ПриОткрытии</Event>",
                "<Event name=\"BeforeWrite\">ПередЗаписью</Event>",
            ]
        );
    }

    /// Правило — НЕ «сначала обычные, потом extInfo»: extInfo-события ложатся ВНУТРЬ обычных
    /// (витнесс `Catalog/…/ФормаЭлемента`; ценз 11 485/11 485 форм SSL+ERP).
    #[test]
    fn ext_events_interleave_with_own_events() {
        let mut body = FormBody::new();
        body.events = vec![ev("NotificationProcessing", "О1"), ev("OnOpen", "О2")];
        body.root_ext_info = Some(FormRootExtInfo {
            kind: "form:CatalogFormExtInfo".into(),
            events: vec![ev("AfterWrite", "О3"), ev("OnReadAtServer", "О4")],
            user_settings_group: None,
        });
        assert_eq!(
            root_events_block(&body),
            vec![
                "<Event name=\"AfterWrite\">О3</Event>",
                "<Event name=\"NotificationProcessing\">О1</Event>",
                "<Event name=\"OnReadAtServer\">О4</Event>",
                "<Event name=\"OnOpen\">О2</Event>",
            ]
        );
    }

    /// У ФОРМЫ ДОКУМЕНТА порядок сдвигает документное переопределение guid:
    /// `BeforeWriteAtServer` несёт `8f42e083…` (< `9f2e5ddb…` у `OnCreateAtServer`), а не
    /// `bf0ac0e1…`. Переопределение load-bearing и для ПОРЯДКА (ценз: 603 формы).
    #[test]
    fn document_ext_kind_shifts_before_write_at_server() {
        let mut body = FormBody::new();
        body.events = vec![ev("OnCreateAtServer", "О1")];
        let ext = vec![ev("BeforeWriteAtServer", "О2")];
        body.root_ext_info = Some(FormRootExtInfo {
            kind: "form:DocumentFormExtInfo".into(),
            events: ext.clone(),
            user_settings_group: None,
        });
        assert_eq!(
            root_events_block(&body),
            vec![
                "<Event name=\"BeforeWriteAtServer\">О2</Event>",
                "<Event name=\"OnCreateAtServer\">О1</Event>",
            ],
            "документный guid ставит BeforeWriteAtServer ПЕРЕД OnCreateAtServer"
        );
        body.root_ext_info = Some(FormRootExtInfo {
            kind: "form:CatalogFormExtInfo".into(),
            events: ext,
            user_settings_group: None,
        });
        assert_eq!(
            root_events_block(&body),
            vec![
                "<Event name=\"OnCreateAtServer\">О1</Event>",
                "<Event name=\"BeforeWriteAtServer\">О2</Event>",
            ],
            "не-документный guid `bf0ac0e1…` — ПОСЛЕ"
        );
    }

    /// ПУСТОЙ ext-локус (весь Designer→Designer путь) НЕ переупорядочивает `body.events` —
    /// иначе byte-exact designer round-trip держался бы «на счастливой сортируемости».
    #[test]
    fn empty_ext_locus_keeps_source_order() {
        let mut body = FormBody::new();
        body.events = vec![ev("OnOpen", "О1"), ev("AfterWrite", "О2")];
        assert_eq!(
            root_events_block(&body),
            vec![
                "<Event name=\"OnOpen\">О1</Event>",
                "<Event name=\"AfterWrite\">О2</Event>",
            ]
        );
    }

    /// АНАЛОГ ДС: собственный обработчик таблицы-динамического-списка `OnGetDataAtServer`
    /// (guid `97365900…`) встаёт МЕЖДУ табличными событиями, а НЕ в хвост — прежняя
    /// конкатенация ошибалась в 105 из 604 таблиц-ДС (SSL 8 + ERP 97).
    #[test]
    fn dynamic_list_own_handler_lands_in_guid_position() {
        use morph1c_core::ir::{DynamicListExt, FormControlKind, FormItem};
        let mut item = FormItem::new(FormControlKind::new("Table"), "Список", 1);
        item.events = vec![
            ev("OnActivateRow", "О1"),
            ev("OnHover", "О2"),
            ev("OnChange", "О3"),
        ];
        item.dynamic_list_ext = Some(DynamicListExt {
            fields: Vec::new(),
            events: vec![ev("OnGetDataAtServer", "О4")],
        });
        let mut el = OutElement::branch("", "Table");
        push_designer_events(&mut el, &item);
        let names: Vec<&str> = el.children[0]
            .children
            .iter()
            .map(|c| c.attrs[0].1.as_str())
            .collect();
        assert_eq!(
            names,
            ["OnActivateRow", "OnGetDataAtServer", "OnHover", "OnChange"]
        );
    }
}
