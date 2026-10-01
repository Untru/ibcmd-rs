//! READ · Designer dialect reader — form envelope/root, service children (command
//! interface, mobile bar), root ext-info and the data model (attributes, parameters, commands).

use super::*;

/// Прочитать Designer `<CommandInterface>` → (navigationPanel items, commandBar items). Несёт
/// опциональные `<NavigationPanel>`/`<CommandBar>`, каждый — список `<Item>`. §1.0: иные дети —
/// ошибка. Канонизуется X-равно EDT-стороне (тот же [`FormCiItem`]).
pub(crate) fn read_designer_command_interface(
    ci: &Element,
) -> Result<(Vec<FormCiItem>, Vec<FormCiItem>), FormError> {
    ci.claim();
    let mut np = Vec::new();
    let mut cb = Vec::new();
    for panel in &ci.children {
        if !panel.prefix.is_empty() {
            return Err(FormError::Frame(format!(
                "CommandInterface: unexpected child <{}:{}> (§1.0)",
                panel.prefix, panel.local
            )));
        }
        match panel.local.as_str() {
            "NavigationPanel" => np = read_designer_cmi_panel(panel)?,
            "CommandBar" => cb = read_designer_cmi_panel(panel)?,
            other => return Err(FormError::Frame(format!(
                "CommandInterface: unmodelled panel <{other}> (only NavigationPanel/CommandBar — §1.0)"
            ))),
        }
    }
    Ok((np, cb))
}

/// Прочитать одну Designer панель КИ (`NavigationPanel`/`CommandBar`) → её `<Item>`'ы. Каждый
/// `<Item>` = `<Command>` + `<Type>` (всегда) + опциональные `<CommandGroup>`/`<Index>`
/// (Designer опускает `Index` == 0) + видимость (`DefaultVisible`+`Visible` — три-состояние,
/// см. [`FormCiItem`]). §1.0: DefaultVisible проверяется на единственный витнессированный
/// литерал `false`; `Visible` без `DefaultVisible` не витнессирован → ошибка.
pub(crate) fn read_designer_cmi_panel(panel: &Element) -> Result<Vec<FormCiItem>, FormError> {
    panel.claim();
    let mut out = Vec::new();
    for item in &panel.children {
        if item.local != "Item" || !item.prefix.is_empty() {
            return Err(FormError::Frame(format!(
                "CommandInterface panel: unexpected child <{}:{}> (only <Item> — §1.0)",
                item.prefix, item.local
            )));
        }
        item.claim();
        let command = leaf_text(item, "Command")?;
        // Type — Designer эмитит всегда (101/101: Auto/Added).
        let ty_el = item
            .child("Type")
            .ok_or_else(|| FormError::Frame("Item: no <Type> (§1.0)".into()))?;
        ty_el.claim_with_text();
        let ty = ty_el.text.clone();
        let group = match item.child("CommandGroup").filter(|c| c.prefix.is_empty()) {
            Some(g) => {
                g.claim_with_text();
                Some(g.text.clone())
            }
            None => None,
        };
        // Index: Designer опускает `0`. Канон (X-равно EDT): index=Some ⟺ (есть group ЛИБО явный
        // <Index>). Census ERP: index-БЕЗ-группы (325, значения ≥1 — 0 не встречается, поэтому
        // «нет <Index> И нет <CommandGroup>» ОДНОЗНАЧНО = index отсутствует); group-БЕЗ-<Index> =
        // index 0 под группой (346 — Designer опустил свой дефолт 0). Обе комбинации витнесснуты.
        let explicit_index = match item.child("Index").filter(|c| c.prefix.is_empty()) {
            Some(i) => {
                i.claim_with_text();
                Some(parse_int(&i.text)?)
            }
            None => None,
        };
        let index = explicit_index.or_else(|| group.as_ref().map(|_| 0));
        // Видимость: (DefaultVisible, Visible) → три-состояние (см. FormCiItem).
        let dv = match item.child("DefaultVisible").filter(|c| c.prefix.is_empty()) {
            Some(dv) => {
                dv.claim_with_text();
                if dv.text != "false" {
                    return Err(FormError::Frame(format!(
                        "Item <DefaultVisible> is {:?}, only \"false\" modelled (§1.0)",
                        dv.text
                    )));
                }
                true
            }
            None => false,
        };
        // Ролевые исключения видимости пункта: `<Visible>` несёт `<xr:Value name="R">B</xr:Value>`
        // после `<xr:Common>` ⟺ EDT `<userVisible>…<for>` (witness ЗаказКлиента.ФормаСписка). Пусто
        // = безролевой (обычный случай). Канон — Vec<(role, value)> в исходном порядке (X-равно EDT).
        let mut user_visible_roles = Vec::new();
        let vis = match item.child("Visible").filter(|c| c.prefix.is_empty()) {
            Some(vis) => {
                vis.claim();
                let common = vis
                    .child("Common")
                    .ok_or_else(|| FormError::Frame("Item <Visible>: no <Common> (§1.0)".into()))?;
                common.claim_with_text();
                let common_b = match common.text.as_str() {
                    "true" => true,
                    "false" => false,
                    other => {
                        return Err(FormError::Frame(format!(
                            "Item <Visible><Common> is {other:?}, want true/false (§1.0)"
                        )))
                    }
                };
                for v in &vis.children {
                    // `<xr:Common>` уже прочитан (local == "Common"); прочее — только `<xr:Value>`-роли.
                    if v.local == "Common" {
                        continue;
                    }
                    if !(v.prefix == "xr" && v.local == "Value") {
                        return Err(FormError::Frame(format!(
                            "Item <Visible>: unexpected child <{}:{}> (only <xr:Common>/<xr:Value> \
                             — §1.0)",
                            v.prefix, v.local
                        )));
                    }
                    let name = v.attr("name").ok_or_else(|| {
                        FormError::Frame("Item <Visible><xr:Value>: no name (§1.0)".into())
                    })?;
                    name.claimed.set(true);
                    v.claim_with_text();
                    let val = match v.text.as_str() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(FormError::Frame(format!(
                                "Item <Visible><xr:Value name={:?}>={other:?}, want bool (§1.0)",
                                name.value
                            )))
                        }
                    };
                    user_visible_roles.push((name.value.clone(), val));
                }
                Some(common_b)
            }
            None => None,
        };
        let user_visible = match (dv, vis) {
            (false, None) => None,      // видимость не задана (1⟷1 EDT-absent userVisible)
            (true, None) => Some(true), // задана true — Designer опускает <Visible>
            (true, Some(b)) => Some(b), // задана явно (корпус несёт только false)
            (false, Some(_)) => {
                return Err(FormError::Frame(format!(
                    "Item {command:?}: <Visible> without <DefaultVisible> is unmodelled (§1.0)"
                )))
            }
        };
        expect_only_children(
            item,
            &[
                "Command",
                "Type",
                "CommandGroup",
                "Index",
                "DefaultVisible",
                "Visible",
            ],
        )?;
        out.push(FormCiItem {
            command,
            ty,
            group,
            index,
            user_visible,
            user_visible_roles,
        });
    }
    Ok(out)
}

/// Прочитать Designer `<MobileDeviceCommandBarContent>` → список скалярных значений. Каждый
/// пункт — `<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>
/// <xr:Value xsi:type="xs:string">X</xr:Value></xr:Item>` (значение — через общий value-codec).
pub(crate) fn read_designer_mobile_command_bar(el: &Element) -> Result<Vec<PropertyValue>, FormError> {
    el.claim();
    let mut out = Vec::new();
    for item in &el.children {
        if item.local != "Item" || item.prefix != "xr" {
            return Err(FormError::Frame(format!(
                "MobileDeviceCommandBarContent: unexpected child <{}:{}> (§1.0)",
                item.prefix, item.local
            )));
        }
        item.claim();
        let pres = item
            .child("Presentation")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("MobileDeviceCommandBarContent item: no <xr:Presentation>".into())
            })?;
        pres.claim();
        if !pres.children.is_empty() || !pres.text.is_empty() {
            return Err(FormError::Frame(
                "MobileDeviceCommandBarContent: <xr:Presentation> must be empty (§1.0)".into(),
            ));
        }
        let cs = item
            .child("CheckState")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("MobileDeviceCommandBarContent item: no <xr:CheckState>".into())
            })?;
        cs.claim_with_text();
        if cs.text != "0" {
            return Err(FormError::Frame(format!(
                "MobileDeviceCommandBarContent: <xr:CheckState>={:?}, want 0 (§1.0)",
                cs.text
            )));
        }
        let xv = item
            .child("Value")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("MobileDeviceCommandBarContent item: no <xr:Value>".into())
            })?;
        xv.claim();
        let value = value_codec::decode(ValueDialect::Designer, xv).map_err(FormError::Frame)?;
        if item.children.len() != 3 {
            return Err(FormError::Frame(
                "MobileDeviceCommandBarContent <xr:Item>: expected Presentation+CheckState+Value (§1.0)"
                    .into(),
            ));
        }
        out.push(value);
    }
    Ok(out)
}

// ============================== Designer ==============================

pub(crate) fn read_designer(root: Element) -> Result<FormBody, FormError> {
    if !root.prefix.is_empty() || root.local != "Form" {
        return Err(FormError::Envelope(format!(
            "unexpected root <{}>",
            root.local
        )));
    }
    root.claim();
    // Версионный вход РИДЕРА (FORMATS.md §1): версия детектится по корневому `version=`
    // САМОГО файла, ns-блок сверяется против таблицы СВОЕЙ версии (2.20 — без `xmlns:pal`,
    // 2.21 — с ним). Не-witnessed версия → отказ (§1.0). Лишний ns-атрибут чужой версии
    // (напр. `xmlns:pal` в 2.20-файле) не claim'ится и упрётся в тотальность ниже.
    let ver = root
        .attr("version")
        .ok_or_else(|| FormError::Envelope("missing version".into()))?;
    let profile = detect_form_profile(&ver.value).ok_or_else(|| {
        FormError::Envelope(format!(
            "unrecognized Designer form format version {:?} (witnessed: {})",
            ver.value,
            witnessed_form_versions()
        ))
    })?;
    ver.claimed.set(true);
    claim_root_ns(&root, profile.ns_block)?;

    // Версия источника — ВХОД ЧТЕНИЯ и для ТЕЛА формы, а не только для конверта: по ней
    // ридер отличает «тега нет, потому что свойство не задано» (омиссия = дефолт диалекта,
    // восстанавливаем) от «тега нет, потому что в ЭТОЙ версии свойства не существует»
    // (восстанавливать нечего — §1.0). Скоуп виден всему разбору тела, включая контролы.
    morph1c_core::version::with_source_version(Some(profile.format), || {
        read_designer_body(&root)
    })
}

/// Тело Designer-формы (после разбора конверта) — под скоупом версии источника.
fn read_designer_body(root: &Element) -> Result<FormBody, FormError> {
    let mut body = FormBody::new();
    // Форм-заголовок `<Title>` (опц., прямой ребёнок корня; БЕЗ атрибута formatted).
    if let Some(t) = root.child("Title").filter(|c| c.prefix.is_empty()) {
        body.title = Some(read_designer_title(t, false)?);
    }

    body.attributes = read_form_attrs(FormDialect::Designer, &root).map_err(FormError::Frame)?;

    // Состав командной панели мобильного устройства (корневой список; ДО CommandSet).
    if let Some(m) = root
        .child("MobileDeviceCommandBarContent")
        .filter(|c| c.prefix.is_empty())
    {
        body.mobile_device_command_bar = read_designer_mobile_command_bar(m)?;
    }
    // ФОРМ-уровневый `<CommandSet>` (исключённые стандартные команды).
    if let Some(cs) = root.child("CommandSet").filter(|c| c.prefix.is_empty()) {
        cs.claim();
        for x in cs.children.iter() {
            if x.local != "ExcludedCommand" || !x.prefix.is_empty() {
                return Err(FormError::Frame(format!(
                    "CommandSet: unexpected child <{}:{}> (§1.0)",
                    x.prefix, x.local
                )));
            }
            x.claim_with_text();
            expect_no_children(x)?;
            body.excluded_commands.push(x.text.clone());
        }
    }
    if let Some(acb) = root.child("AutoCommandBar").filter(|c| c.prefix.is_empty()) {
        let mut acb = read_designer_auto_command_bar(acb)?;
        // Канонизация к EDT-flavor (§1.6, approach B): ФОРМ-уровневая автокомандная панель — это
        // фиксированная служебная панель, которую Designer сериализует с ПУСТЫМ именем, а EDT — с
        // именем `FormCommandBar`; и EDT-управляемая форма ВСЕГДА несёт фикс-блок
        // `<commandInterface>`, который Designer оставляет неявным. Обе денормализации со-возникают
        // в каждой управляемой форме корпуса ⇒ синтезируем EDT-написание, чтобы под-IR был
        // формат-нейтрален (write_designer регенерирует пустое имя и опускает commandInterface).
        // Связка с presence панели держит синтез в границах витнессированных cf-комбинаций
        // (`(Some(FormCommandBar), true)` / `(None, false)` — form_body/mod.rs).
        if acb.name.is_empty() {
            acb.name = FORM_COMMAND_BAR_NAME.to_string();
        } else if acb.name == FORM_COMMAND_BAR_NAME {
            // Нерегулярность 3/876: Designer сериализовал ЯВНОЕ имя `FormCommandBar` —
            // presence-точный флаг (write_designer НЕ очищает имя; см. IR-док).
            acb.designer_named = true;
        }
        body.auto_command_bar = Some(acb);
        body.command_interface = true;
    }
    // Явный Designer `<CommandInterface>` блок — НАПОЛНЕННЫЙ формоуровневый КИ (Designer его
    // сериализует ТОЛЬКО когда есть пункты; пустой синтезируется из ACB выше). RE:
    // CommonForm.ВыборИсполнителяБизнесПроцесса. Присутствие ⇒ command_interface истинно (даже
    // без ACB — но витнессированно они со-возникают).
    if let Some(ci) = root
        .child("CommandInterface")
        .filter(|c| c.prefix.is_empty())
    {
        let (np, cb) = read_designer_command_interface(ci)?;
        body.command_interface = true;
        body.form_ci_navigation_panel = np;
        body.form_ci_command_bar = cb;
    }
    if let Some(events) = root.child("Events").filter(|c| c.prefix.is_empty()) {
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
    if let Some(ci) = root.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        body.items = read_designer_child_items(ci)?;
    }
    if let Some(attrs) = root.child("Attributes").filter(|c| c.prefix.is_empty()) {
        attrs.claim();
        for a in attrs
            .children
            .iter()
            .filter(|c| c.local == "Attribute" && c.prefix.is_empty())
        {
            body.data_attributes.push(read_designer_data_attribute(a)?);
        }
        // ФОРМ-уровневое условное оформление — Designer-only `<ConditionalAppearance>` с
        // `dcsset:item`-ами ПОСЛЕ всех `<Attribute>` (EDT — сайдкар `.dcssca`; witness
        // РаботаСФайлами.ВерсияПрисоединенногоФайла). Нормализуется ДО X.
        if let Some(ca) = attrs
            .child("ConditionalAppearance")
            .filter(|c| c.prefix.is_empty())
        {
            ca.claim();
            for it in ca
                .children
                .iter()
                .filter(|c| c.local == "item" && c.prefix == "dcsset")
            {
                body.conditional_appearance.push(read_dcs_item(it)?);
            }
            expect_only_dcsset_children(ca, &["item"])?;
        }
        expect_only_children(attrs, &["Attribute", "ConditionalAppearance"])?;
    }
    // Пользовательские команды формы (`<Commands><Command name id>…`).
    if let Some(cmds) = root.child("Commands").filter(|c| c.prefix.is_empty()) {
        cmds.claim();
        for c in cmds
            .children
            .iter()
            .filter(|c| c.local == "Command" && c.prefix.is_empty())
        {
            body.commands.push(read_designer_command(c)?);
        }
        expect_only_children(cmds, &["Command"])?;
    }
    if let Some(params) = root.child("Parameters").filter(|c| c.prefix.is_empty()) {
        params.claim();
        for p in params
            .children
            .iter()
            .filter(|c| c.local == "Parameter" && c.prefix.is_empty())
        {
            body.parameters.push(read_designer_parameter(p)?);
        }
        expect_only_children(params, &["Parameter"])?;
    }
    // Форма отчёта: корневые `ReportFormType`/… (Designer-кодировка `form:ReportFormExtInfo`).
    if root
        .child("ReportFormType")
        .filter(|c| c.prefix.is_empty())
        .is_some()
    {
        body.report_form = Some(read_designer_report_form(&root)?);
    }
    // ФОРМ-уровневое `UseForFoldersAndItems` (иерархические формы). Designer несёт ПРЯМЫМ ребёнком
    // корня ВСЕГДА (`Items`/`Folders`); EDT — внутри корневого extInfo, опуская дефолт `Items`.
    body.use_for_folders_and_items = leaf_text_opt(&root, "UseForFoldersAndItems");
    // Заголовок группы кнопок создания (`<CreateButtonsGroupTitle><v8:item>…` — перед
    // ShowCommandBar/AutoCommandBar; witness ПроизводственныеКалендари/Файлы).
    if let Some(t) = root
        .child("CreateButtonsGroupTitle")
        .filter(|c| c.prefix.is_empty())
    {
        body.create_buttons_group_title = Some(read_designer_title(t, false)?);
    }
    // Форма объекта-документа: Designer несёт AutoTime/UsePostingMode/RepostOnWrite прямыми детьми
    // корня (после CommandSet, перед AutoCommandBar) ВСЕГДА; EDT — внутри form:DocumentFormExtInfo,
    // опуская дефолты (см. read_edt_root_ext_info). Три поля со-возникают (сверено 11/11 Document форм).
    if let Some(at) = root.child("AutoTime").filter(|c| c.prefix.is_empty()) {
        at.claim_with_text();
        let auto_time = at.text.clone();
        let use_posting_mode = leaf_text(&root, "UsePostingMode")?;
        let rw = root
            .child("RepostOnWrite")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("Document form: no <RepostOnWrite> (§1.0)".into()))?;
        let repost_on_write = matches!(
            read_bool_text(rw, "RepostOnWrite")?,
            PropertyValue::Bool(true)
        );
        body.document_form = Some(DocumentFormInfo {
            auto_time,
            use_posting_mode,
            repost_on_write,
        });
    }
    // commandInterface синтезируется выше (вместе с form-level autoCommandBar) — Designer его
    // не сериализует (неявен); без панели presence остаётся false.

    // КОРНЕВОЙ extInfo — ВЫВОДИМ (Designer маркера типа формы не несёт: он неявен). См.
    // [`derive_root_ext_info_kind`]: тип формы — ПЕРФЕКТНАЯ функция типа ОСНОВНОГО реквизита
    // (FD-намайнено по всем 876 формам SSL, 0 конфликтов). Без него cf-энкодер
    // (`form_body::build_ext_pairs` диспатчится РОВНО по этому виду) не эмитит НИ ОДНОЙ ext-пары
    // ⇒ designer→cf молча терял 2·N ячеек корневой группы на КАЖДОЙ форме с extInfo.
    //
    // СОБСТВЕННЫЕ обработчики extInfo (`AfterWrite`/`OnWriteAtServer`/…) Designer сливает в единый
    // корневой `<Events>` — они остаются в [`FormBody::events`] (X сравнивает форм-события как
    // МНОЖЕСТВО, объединяя оба слота; cf-реестр событий — тоже объединение, отсортированное по
    // guid). Поэтому `events` здесь ПУСТ: раскладывать их обратно по слотам не нужно и опасно
    // (designer-писатель регенерирует ОДИН `<Events>`).
    // `<CustomSettingsFolder>` — корневая группа польз. настроек формы КОМПОНОВЩИКА НАСТРОЕК
    // (⟺ EDT `form:SettingsComposerFormExtInfo`>`userSettingsGroup`). У ФОРМЫ ОТЧЁТА этот тег
    // читается внутри `read_designer_report_form` (⇒ уже claimed) ⇒ здесь берём его ЛИШЬ для
    // не-отчётных форм. Позиция в корне — после CommandSet, до AutoCommandBar (witness
    // Catalog.УчетныеЗаписиМаркетплейсов.НастройкаОтборовСписка).
    let user_settings_group = if body.report_form.is_none() {
        leaf_text_opt(&root, "CustomSettingsFolder")
    } else {
        None
    };
    body.root_ext_info =
        derive_root_ext_info_kind(&body.data_attributes).map(|kind| FormRootExtInfo {
            kind,
            events: Vec::new(),
            user_settings_group,
        });

    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{leftover} unconsumed node(s) in Designer form body (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    Ok(body)
}

/// Тип корневого extInfo формы (`form:CatalogFormExtInfo`, `form:DynamicListFormExtInfo`, …) —
/// ВЫВЕДЕННЫЙ из типа ОСНОВНОГО (`main`) реквизита формы.
///
/// EDT сериализует вид явно (`<extInfo xsi:type="form:*FormExtInfo">`); Designer — НЕТ (он неявен).
/// Но вид — ФУНКЦИЯ типа основного реквизита, и обе диалекта этот реквизит несут. FD-намайнено по
/// всем 876 формам SSL (`probe_root_extinfo`): 13 ключей, **0 конфликтов** —
/// голова типа основного реквизита определяет вид ОДНОЗНАЧНО.
///
/// * нет `main`-реквизита (×331) ⇒ `None` (форма без extInfo);
/// * скалярный основной реквизит (`String`, ×19) ⇒ `None`;
/// * `DynamicList` (×189), `CatalogObject` (×65), `DataProcessorObject` (×168), … ⇒ свой вид.
///
/// §1.0: НЕ витнессированная голова типа НЕ угадывается — она возвращается как явный маркер
/// `form:<Head>FormExtInfo?`, на котором cf-энкодер (`build_ext_pairs`) даёт ТИПИЗИРОВАННЫЙ отказ,
/// а не молча пропускает ext-пары (тихо-неверные ячейки — ровно тот класс, который §1.0 и ловит).
pub(crate) fn derive_root_ext_info_kind(attrs: &[FormDataAttribute]) -> Option<String> {
    let main = attrs.iter().find(|a| a.main)?;
    let ty = main.value_type.as_ref()?;
    let head = ty.parts.first()?.id.split('.').next()?;
    let kind = match head {
        "DynamicList" => "form:DynamicListFormExtInfo",
        "CatalogObject" => "form:CatalogFormExtInfo",
        "DocumentObject" => "form:DocumentFormExtInfo",
        "ChartOfCharacteristicTypesObject" => "form:ChartOfCharacteristicTypesFormExtInfo",
        "TaskObject" => "form:TaskFormExtInfo",
        // Опечатка платформы (`Proces`, не `Process`) — витнессирована в EDT-корпусе как есть.
        "BusinessProcessObject" => "form:BusinessProcesFormExtInfo",
        // ChartOfAccounts / ChartOfCalculationTypes ОБЪЕКТНЫЕ формы (ERP-волна). Их корневой extInfo
        // — ТОЛЬКО объект-подобный триплет {24,25,26}, как у Task/BusinessProcess: витнессирован
        // байт-точно на erp.cf (ChartOfAccounts.Хозрасчетный/ФормаСчета `5c68eba9-….0` и
        // ChartOfCalculationTypes.Начисления/ФормаВидаРасчета `872c2f96-….0` — оба несут
        // ext-count=3, `3,24,{"B",0},25,{"U"},26,{"B",1}`, БЕЗ key-0 семейной ячейки Catalog-форм),
        // подтверждён синтетик-оракулом (модульная форма → count=3, безмодульная → count=0).
        "ChartOfAccountsObject" => "form:ChartOfAccountsObjectFormExtInfo",
        "ChartOfCalculationTypesObject" => "form:ChartOfCalculationTypesObjectFormExtInfo",
        "DataProcessorObject" => "form:ObjectFormExtInfo",
        "ReportObject" => "form:ReportFormExtInfo",
        "ConstantsSet" => "form:ConstantsFormExtInfo",
        "InformationRegisterRecordManager" => "form:InformationRegisterManagerFormExtInfo",
        "InformationRegisterRecordSet" => "form:RecordSetFormExtInfo",
        // AccountingRegister record-set форма (основной реквизит `AccountingRegisterRecordSet.*`,
        // witness ОтражениеДокументовВРеглУчете.ПроводкиРегламентированногоУчета) — корневой extInfo
        // ПУСТ (класс RecordSet), НЕ объект-триплет. Байт-точно намайнено на erp.cf носителе
        // `6d466252-2132-4992-a614-5a7f2e9f29b2.0`: форма НЕСЁТ модуль, но ext-count=0 (позиция
        // [18] группы-50, ровно после 17 head-ячеек, перед events_registry) — что и отделяет
        // empty-always класс (RecordSet/Object) от object-triple-when-module (Task/CoA-Object).
        "AccountingRegisterRecordSet" => "form:AccountingRegisterRecordSetFormExtInfo",
        // Форма КОМПОНОВЩИКА НАСТРОЕК (основной реквизит `DataCompositionSettingsComposer`;
        // witness .НастройкаОтборовСписка) — EDT-маркер `form:SettingsComposerFormExtInfo`.
        "DataCompositionSettingsComposer" => "form:SettingsComposerFormExtInfo",
        // Скаляры: основной реквизит примитивного типа ⇒ форма без extInfo (витнесс `String` ×19).
        "String" | "Number" | "Boolean" | "Date" => return None,
        // §1.0 — не витнессировано: пусть отказывает cf-энкодер (см. док выше).
        other => return Some(format!("form:{other}FormExtInfo?")),
    };
    Some(kind.to_string())
}

/// Прочитать корневые Designer-поля формы отчёта → [`ReportFormInfo`]. Designer эмитит
/// `ReportFormType`/`AutoShowState`/`ReportResultViewMode`/`ViewModeApplicationOnSetReportResult`
/// ВСЕГДА (в т.ч. `Auto`), `CustomSettingsFolder` — при наличии. Канон X-равен EDT-стороне
/// (которая опускает `Auto`-дефолты KEEP-полей).
pub(crate) fn read_designer_report_form(root: &Element) -> Result<ReportFormInfo, FormError> {
    let req = |tag: &str| -> Result<String, FormError> { leaf_text(root, tag) };
    let opt = |tag: &str| -> Option<String> {
        root.child(tag).filter(|c| c.prefix.is_empty()).map(|c| {
            c.claim_with_text();
            c.text.clone()
        })
    };
    let settings_form = req("ReportFormType")?;
    let show_state = req("AutoShowState")?;
    let user_settings_group = opt("CustomSettingsFolder");
    let report_result = opt("ReportResult");
    let details_data = opt("DetailsData");
    let variant_appearance = opt("VariantAppearance");
    let report_result_view_mode = req("ReportResultViewMode")?;
    let view_mode_application = req("ViewModeApplicationOnSetReportResult")?;
    Ok(ReportFormInfo {
        settings_form,
        show_state,
        user_settings_group,
        report_result,
        details_data,
        variant_appearance,
        report_result_view_mode,
        view_mode_application,
    })
}

pub(crate) fn read_designer_event(ev: &Element) -> Result<FormEvent, FormError> {
    let name = attr_value(ev, "name")?;
    ev.claim_with_text();
    expect_no_children(ev)?;
    Ok(FormEvent {
        name,
        handler: ev.text.clone(),
    })
}

pub(crate) fn read_designer_auto_command_bar(el: &Element) -> Result<AutoCommandBar, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    // `DisplayImportance` — Designer-АТРИБУТ элемента (⟺ EDT `<displayImportance>`-ребёнок).
    let display_importance = el.attr("DisplayImportance").map(|a| {
        a.claimed.set(true);
        a.value.clone()
    });
    // Дочерние контролы панели `<ChildItems>` (как правило Button-команды).
    let items = match el.child("ChildItems").filter(|c| c.prefix.is_empty()) {
        Some(ci) => read_designer_child_items(ci)?,
        None => Vec::new(),
    };
    // HorizontalAlign: Designer опускает `Left` (его ACB-дефолт) ⇒ заполняем `Left`, чтобы
    // совпасть с EDT (которая эмитит `<horizontalAlign>Left` явно). Иначе — прочитанное.
    let ha = match el.child("HorizontalAlign").filter(|c| c.prefix.is_empty()) {
        Some(c) => {
            c.claim_with_text();
            Some(c.text.clone())
        }
        None => Some(ACB_HORIZONTAL_ALIGN_DEFAULT.to_string()),
    };
    // `<Autofill>` — ДАННЫЕ с пер-форматными дефолтами (Designer-дефолт true ⇒ эмитится
    // только `false`; absent ⇒ true). Cross-сверка 104/104 ACB корпуса.
    let auto_fill = match el.child("Autofill").filter(|c| c.prefix.is_empty()) {
        Some(_) => {
            expect_leaf_value(el, "Autofill", "false")?;
            false
        }
        None => true,
    };
    // `<Visible>` — ДАННЫЕ, Designer-дефолт `true` ⇒ эмитится ТОЛЬКО `false` (witness
    // `integration_subsystem`, CommonForm.инт_Подписчики ACB `ПодписчикиКоманднаяПанель`;
    // единственный `<Visible>` под `<AutoCommandBar>` во всём корпусе — 1/1).
    let visible = match el.child("Visible").filter(|c| c.prefix.is_empty()) {
        Some(_) => {
            expect_leaf_value(el, "Visible", "false")?;
            false
        }
        None => true,
    };
    expect_only_children(el, &["Visible", "HorizontalAlign", "Autofill", "ChildItems"])?;
    Ok(AutoCommandBar {
        name,
        id,
        horizontal_align: ha,
        display_importance,
        auto_fill,
        items,
        visible,
        designer_named: false,
    })
}

/// Designer `<ChildItems>` — диспетчер дочерних контролов по ИМЕНИ элемента.
pub(crate) fn read_designer_child_items(ci: &Element) -> Result<Vec<FormItem>, FormError> {
    ci.claim();
    let mut out = Vec::new();
    for c in ci.children.iter().filter(|c| c.prefix.is_empty()) {
        out.push(match c.local.as_str() {
            "Button" => read_designer_button(c)?,
            "Table" => read_designer_table(c)?,
            k if tables::decoration_kind(k).is_some() => {
                read_designer_decoration(c, tables::decoration_kind(k).expect("checked"))?
            }
            k if tables::group_kind(k).is_some() => {
                read_designer_group(c, tables::group_kind(k).expect("checked"))?
            }
            k if tables::field_kind_by_des_tag(k).is_some() => {
                read_designer_field(c, tables::field_kind_by_des_tag(k).expect("checked"))?
            }
            // Добавление-как-контрол (`<SearchStringAddition>`/…) в дереве (форм-autoCommandBar/
            // contextMenu). Та же модель, что Table-именованное добавление — переиспользуем ридер.
            k if tables::addition_kind(k).is_some() => {
                read_designer_addition(c, tables::addition_kind(k).expect("checked"))?
            }
            other => {
                return Err(FormError::Frame(format!(
                    "ChildItems: unsupported control <{other}> (§1.0 — no Raw)"
                )))
            }
        });
    }
    Ok(out)
}

pub(crate) fn read_designer_data_attribute(el: &Element) -> Result<FormDataAttribute, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    // Title (опц., Localized; БЕЗ formatted-атрибута).
    let title = match el.child("Title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_designer_title(t, false)?),
        None => None,
    };
    let value_type = read_value_type(el, "Type", crate::TypeDialect::Designer)?;
    // View/Edit-права (опц.): `<View><xr:Common>false</xr:Common></View>` = запрещено (false);
    // отсутствие = общий доступ (true) ⟺ EDT `<view/>` / `<view><common>true` (витнессы
    // УчетныеЗаписи…/Взаимодействия/ВерсииОбъектов).
    let (view_common, view_roles) = read_designer_common_flag(el, "View")?;
    let (edit_common, edit_roles) = read_designer_common_flag(el, "Edit")?;
    // FillCheck (опц., Enum).
    let fill_checking = match el.child("FillCheck").filter(|c| c.prefix.is_empty()) {
        Some(f) => {
            f.claim_with_text();
            Some(f.text.clone())
        }
        None => None,
    };
    // MainAttribute/SavedData (опц., presence-true bool).
    let main = match el.child("MainAttribute").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(
            read_bool_text(v, "MainAttribute")?,
            PropertyValue::Bool(true)
        ),
        None => false,
    };
    let saved_data = match el.child("SavedData").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(read_bool_text(v, "SavedData")?, PropertyValue::Bool(true)),
        None => false,
    };
    // Save (опц.): ОДИН `<Save>` с несколькими `<Field>Путь</Field>` ⟺ EDT ПОВТОРЯЕМЫЙ
    // settingsSavedData-DataPath (witness ГрупповоеИзменениеРеквизитов ×2). Порядок сохраняется.
    // Сигила `~` на `<Field>` («поле не входит в состав полей динамического списка») — DESIGNER-ONLY
    // денормализация (EDT её не несёт ни разу; edt→cf byte-exact без неё ⇒ в cf-тело не кодируется).
    // Снимаем в канон, помеченные пути запоминаем в `designer_unavailable_paths` (write_designer
    // возвращает сигилу, normalize_attr_for_x зануляет). См. IR-док.
    let mut designer_unavailable_paths: Vec<String> = Vec::new();
    let mut settings_saved_data = Vec::new();
    if let Some(s) = el.child("Save").filter(|c| c.prefix.is_empty()) {
        s.claim();
        for f in s
            .children
            .iter()
            .filter(|c| c.local == "Field" && c.prefix.is_empty())
        {
            f.claim_with_text();
            expect_no_children(f)?;
            settings_saved_data.push(strip_field_sigil(&f.text, &mut designer_unavailable_paths));
        }
        expect_only_children(s, &["Field"])?;
        if settings_saved_data.is_empty() {
            return Err(FormError::Frame("Attribute Save: no <Field> (§1.0)".into()));
        }
    }
    // FunctionalOptions: `<FunctionalOptions><Item>Ref</Item>…</FunctionalOptions>`.
    let mut functional_options = Vec::new();
    if let Some(fo) = el
        .child("FunctionalOptions")
        .filter(|c| c.prefix.is_empty())
    {
        fo.claim();
        for it in fo
            .children
            .iter()
            .filter(|c| c.local == "Item" && c.prefix.is_empty())
        {
            it.claim_with_text();
            expect_no_children(it)?;
            functional_options.push(it.text.clone());
        }
        expect_only_children(fo, &["Item"])?;
    }
    // UseAlways: ОДИН `<UseAlways>` с несколькими `<Field>Путь</Field>` ⟺ EDT ПОВТОРЯЕМЫЙ
    // notDefaultUseAlwaysAttributes. Порядок сохраняется.
    let mut not_default_use_always = Vec::new();
    if let Some(u) = el.child("UseAlways").filter(|c| c.prefix.is_empty()) {
        u.claim();
        for f in u
            .children
            .iter()
            .filter(|c| c.local == "Field" && c.prefix.is_empty())
        {
            f.claim_with_text();
            expect_no_children(f)?;
            not_default_use_always
                .push(strip_field_sigil(&f.text, &mut designer_unavailable_paths));
        }
        expect_only_children(u, &["Field"])?;
        if not_default_use_always.is_empty() {
            return Err(FormError::Frame(
                "Attribute UseAlways: no <Field> (§1.0)".into(),
            ));
        }
    }
    // Колонки (ValueTable): `<Columns><Column name id>…` — рекурсивно как реквизиты.
    let mut columns = Vec::new();
    let mut additional_columns = Vec::new();
    if let Some(cols) = el.child("Columns").filter(|c| c.prefix.is_empty()) {
        cols.claim();
        for c in cols
            .children
            .iter()
            .filter(|c| c.local == "Column" && c.prefix.is_empty())
        {
            columns.push(read_designer_data_attribute(c)?);
        }
        // `<AdditionalColumns table="Путь">` (ВНУТРИ `<Columns>`, после обычных `<Column>`):
        // группа доп-колонок — `table`-атрибут (табличный путь) + вложенные `<Column>`.
        for ac in cols
            .children
            .iter()
            .filter(|c| c.local == "AdditionalColumns" && c.prefix.is_empty())
        {
            ac.claim();
            let table_path = attr_value(ac, "table")?;
            let mut inner = Vec::new();
            for c in ac
                .children
                .iter()
                .filter(|c| c.local == "Column" && c.prefix.is_empty())
            {
                inner.push(read_designer_data_attribute(c)?);
            }
            expect_only_children(ac, &["Column"])?;
            additional_columns.push(AdditionalColumns {
                table_path,
                columns: inner,
            });
        }
        expect_only_children(cols, &["Column", "AdditionalColumns"])?;
    }
    // `<Settings>` — диспетч по xsi:type: `v8:TypeDescription` (пустой маркер ValueList-extInfo,
    // ⟺ EDT `form:ValueListExtInfo`) ЛИБО `DynamicList` (динамический список; см.
    // [`read_designer_dynamic_list_attr`]). Иной xsi:type — §1.0-ошибка.
    let mut value_list_ext = false;
    let mut value_list_item_type = None;
    let mut dynamic_list = None;
    let mut spreadsheet_settings = None;
    let mut chart_settings = None;
    if let Some(s) = el.child("Settings").filter(|c| c.prefix.is_empty()) {
        let xt = s
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("Attribute Settings: no xsi:type".into()))?;
        match xt.value.as_str() {
            "v8:TypeDescription" => {
                xt.claimed.set(true);
                // Пустой `<Settings/>` ⇒ None; НЕ-пустой (`<v8:Type>xs:string</v8:Type>…`) ⇒
                // TypeSpec через общий type-codec (тот же путь, что `<Type>`). Атрибут `xsi:type`
                // уже склеймлен выше как дискриминатор.
                value_list_item_type = decode_type_host(s, crate::TypeDialect::Designer)?;
                value_list_ext = true;
            }
            "DynamicList" => {
                xt.claimed.set(true);
                dynamic_list = Some(read_designer_dynamic_list_attr(s)?);
            }
            "mxl:SpreadsheetDocument" => {
                xt.claimed.set(true);
                spreadsheet_settings = Some(read_designer_spreadsheet_settings(s)?);
            }
            // Диаграмма/диаграмма Ганта — Designer-инлайн настроек Chart-реквизита (EDT держит
            // их в сайдкаре Attributes/<attr>/ExtInfo/*.chart; ERP-witness'ы: 9 носителей,
            // см. form/chart.rs). Инлайн-`xmlns:d4p1` — атрибут host'а.
            "d4p1:Chart" | "d4p1:GanttChart" => {
                xt.claimed.set(true);
                if let Some(a) = s.attr("xmlns:d4p1") {
                    a.claimed.set(true);
                }
                let kind = xt.value.trim_start_matches("d4p1:").to_string();
                chart_settings = Some(crate::form::chart::read_designer_chart_settings(&kind, s)?);
            }
            other => {
                return Err(FormError::Frame(format!(
                    "Attribute Settings xsi:type={other:?}: unmodeled (§1.0)"
                )));
            }
        }
    }
    expect_only_children(
        el,
        &[
            "Title",
            "Type",
            "View",
            "Edit",
            "FillCheck",
            "MainAttribute",
            "Save",
            "SavedData",
            "Columns",
            "Settings",
            "FunctionalOptions",
            "UseAlways",
        ],
    )?;
    // Designer НЕ несёт EDT-маркера `form:SpreadsheetDocumentExtInfo` физически — но маркер
    // ПОЛНОСТЬЮ детерминирован скаляр-типом `SpreadsheetDocument` (37/37 корпус SSL+coverage;
    // EDT-ридер это §1.0-сверяет) ⇒ КАНОНИЗУЕМ (синтезируем EDT-написание, как
    // commandInterface/autoCommandBar-имя): designer→edt восстанавливает маркер byte-exact.
    // Сам табличный документ Designer держит в `<Settings mxl:SpreadsheetDocument>`
    // (spreadsheet_settings ⟺ EDT-сайдкар `SpreadsheetData.mxlx`, транскодит pipeline).
    let spreadsheet_ext = is_scalar_spreadsheet_type(&value_type);
    Ok(FormDataAttribute {
        chart_settings,
        name,
        id,
        title,
        value_type,
        fill_checking,
        view_common,
        view_roles,
        edit_common,
        edit_roles,
        main,
        saved_data,
        settings_saved_data,
        columns,
        additional_columns,
        value_list_ext,
        value_list_item_type,
        spreadsheet_ext,
        functional_options,
        not_default_use_always,
        designer_unavailable_paths,
        dynamic_list,
        spreadsheet_settings,
    })
}

/// Снять DESIGNER-сигилу `~` с пути-поля (`<Field>~Список.Ограничения</Field>`) и запомнить
/// КАНОНИЧЕСКИЙ (без сигилы) путь в `marked`.
///
/// Сигила означает «поле не входит в состав полей динамического списка». Она DESIGNER-ONLY: во
/// всём EDT-корпусе SSL `~` не встречается ни разу, а edt→cf на тех же формах byte-exact ⇒ в
/// cf-тело она НЕ кодируется. Канон = EDT-написание; бит несёт
/// [`morph1c_core::ir::FormDataAttribute::designer_unavailable_paths`] (write_designer возвращает
/// сигилу, X зануляет). §1.0: точный предикат платформы («лист не выбирает это поле в своём
/// запросе») требует разбора текста запроса — несём НАБЛЮДЁННЫЙ бит, а не угаданное правило.
pub(crate) fn strip_field_sigil(text: &str, marked: &mut Vec<String>) -> String {
    match text.strip_prefix('~') {
        Some(bare) => {
            marked.push(bare.to_string());
            bare.to_string()
        }
        None => text.to_string(),
    }
}

/// Прочитать Designer `<Settings xsi:type="DynamicList">` (SIMPLE-форма) → [`DynamicListAttrExt`].
/// Порядок Designer: AutoFillAvailableFields?, ManualQuery, DynamicDataRead, QueryText?,
/// KeyField?, CalculatedField*, Field*, Parameter*, AutoSaveUserSettings?, MainTable?,
/// ListSettings?. Пер-форматные дефолты флагов ПРОТИВОПОЛОЖНЫ EDT: Designer эмитит ЛИШЬ
/// явный `false` (absent ⇒ true; witness АнализПравДоступа/ВыбранныеЭлементы);
/// `getInvisibleFieldPresentations` Designer не несёт вовсе (канон `true` — 229/229 EDT).
pub(crate) fn read_designer_dynamic_list_attr(
    s: &Element,
) -> Result<DynamicListAttrExt, FormError> {
    s.claim();
    // AutoFillAvailableFields — эмитится ЛИШЬ явным `false` ПЕРВЫМ ребёнком; absent ⇒ true.
    let auto_fill_available_fields = match s
        .child("AutoFillAvailableFields")
        .filter(|c| c.prefix.is_empty())
    {
        Some(v) => matches!(
            read_bool_text(v, "AutoFillAvailableFields")?,
            PropertyValue::Bool(true)
        ),
        None => true,
    };
    // Designer эмитит ManualQuery/DynamicDataRead ВСЕГДА явным true/false (не presence-опускает
    // false как EDT) ⇒ читаем как явные булевы листы. QueryText — только у РУЧНОГО запроса
    // (ManualQuery=true); MainTable отсутствует у РАСШИРЕННОЙ формы (собственный DCS data-set).
    let custom_query = read_bool_leaf(s, "ManualQuery")?;
    let dynamic_data_read = read_bool_leaf(s, "DynamicDataRead")?;
    let query_text = leaf_text_opt(s, "QueryText");
    // KeyType — вид ключа списка (ERP ×7: FieldValue/RowKey; witness ВыборПрисоединенногоФайла:
    // QueryText→KeyType→KeyField). cf-кодировка НЕ витнесснута — типизированный отказ cf-write.
    let key_type = leaf_text_opt(s, "KeyType");
    // KeyField — ключевые поля списка (ПОВТОРЯЕМЫЙ; witness ВыбранныеЭлементы ×1, СписокДокументов
    // ×4). Собираем ВСЕ (каждый claim'им), иначе 2-й и далее — несконсуменный узел (§1.0).
    let key_fields = leaf_texts_all(s, "KeyField");
    // Вычисляемые поля (`<CalculatedField>`; witness НеудаленныеОбъекты).
    let mut calculated_fields = Vec::new();
    for cf in s
        .children
        .iter()
        .filter(|c| c.local == "CalculatedField" && c.prefix.is_empty())
    {
        calculated_fields.push(read_designer_dcs_calculated_field(cf)?);
    }
    // EXTENDED: `<Field>`/`<Parameter>` DataCompositionSchema (после QueryText, ДО MainTable).
    let mut fields = Vec::new();
    for f in s
        .children
        .iter()
        .filter(|c| c.local == "Field" && c.prefix.is_empty())
    {
        fields.push(read_designer_dcs_field(f)?);
    }
    let mut parameters = Vec::new();
    for p in s
        .children
        .iter()
        .filter(|c| c.local == "Parameter" && c.prefix.is_empty())
    {
        parameters.push(read_designer_dcs_parameter(p)?);
    }
    // AutoSaveUserSettings — эмитится ЛИШЬ явным `false` (перед MainTable/ListSettings).
    let auto_save_user_settings = match s
        .child("AutoSaveUserSettings")
        .filter(|c| c.prefix.is_empty())
    {
        Some(v) => matches!(
            read_bool_text(v, "AutoSaveUserSettings")?,
            PropertyValue::Bool(true)
        ),
        None => true,
    };
    // GetInvisibleFieldPresentations — Designer эмитит ЛИШЬ явный `false` (ERP ×14);
    // отсутствие ⇒ `true` (канон EDT-стороны).
    let get_invisible_field_presentations = match s
        .child("GetInvisibleFieldPresentations")
        .filter(|c| c.prefix.is_empty())
    {
        Some(v) => matches!(
            read_bool_text(v, "GetInvisibleFieldPresentations")?,
            PropertyValue::Bool(true)
        ),
        None => true,
    };
    let main_table = leaf_text_opt(s, "MainTable");
    let list_settings = match s.child("ListSettings").filter(|c| c.prefix.is_empty()) {
        Some(ls) => Some(read_designer_list_settings(ls)?),
        None => None,
    };
    expect_only_children(
        s,
        &[
            "AutoFillAvailableFields",
            "ManualQuery",
            "DynamicDataRead",
            "QueryText",
            "KeyField",
            "CalculatedField",
            "Field",
            "Parameter",
            "AutoSaveUserSettings",
            "GetInvisibleFieldPresentations",
            "KeyType",
            "MainTable",
            "ListSettings",
        ],
    )?;
    Ok(DynamicListAttrExt {
        key_type,
        query_text,
        main_table,
        custom_query,
        dynamic_data_read,
        auto_fill_available_fields,
        auto_save_user_settings,
        // Designer несёт ЛИШЬ явный `false` (ERP ×14; SSL — 0 вхождений); отсутствие ⇒ `true`
        // (229/229 EDT-динсписков SSL; нормализуется ДО X, designer→edt восстанавливает
        // EDT-написание `<getInvisibleFieldPresentations>true`).
        get_invisible_field_presentations,
        key_fields,
        calculated_fields,
        list_settings,
        fields,
        parameters,
    })
}

/// Designer опциональный явный bool `<dcssch:tag>true|false</dcssch:tag>` → `Some(bool)`;
/// отсутствие ⇒ `None`.
pub(crate) fn read_designer_opt_bool(el: &Element, tag: &str) -> Result<Option<bool>, FormError> {
    match el.child(tag).filter(|c| c.prefix == "dcssch") {
        Some(v) => Ok(Some(matches!(
            read_bool_text(v, tag)?,
            PropertyValue::Bool(true)
        ))),
        None => Ok(None),
    }
}

pub(crate) fn read_designer_parameter(el: &Element) -> Result<FormParameter, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let value_type = read_value_type(el, "Type", crate::TypeDialect::Designer)?;
    // KeyParameter (опц., presence-true bool).
    let key_parameter = match el.child("KeyParameter").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(
            read_bool_text(v, "KeyParameter")?,
            PropertyValue::Bool(true)
        ),
        None => false,
    };
    expect_only_children(el, &["Type", "KeyParameter"])?;
    Ok(FormParameter {
        name,
        value_type,
        key_parameter,
    })
}

/// Designer `<Command name id>` → [`FormCommand`] (канон-bag спека `spec/forms/command`).
///
/// Каркас-константы/денормализации: `<Picture>` несёт `<xr:LoadTransparent>`, значение
/// которого — функция ВИДА ссылки (`StdPicture.*`→true, иначе false; сверено 261/261) —
/// сверяется-claim'ится, в IR не хранится (несоответствие — типизированная ошибка §1.0).
/// Пер-форматные дефолты `currentRowUse`/`selectedRowsUse`: Designer опускает `Auto` ⇒
/// absent заполняется `Auto` (EDT-сторона заполняет `Use`) — канон-bag несёт всегда.
pub(crate) fn read_designer_command(el: &Element) -> Result<FormCommand, FormError> {
    el.claim();
    let name = attr_value(el, "name")?;
    let id = parse_int(&attr_value(el, "id")?)?;
    let mut cmd = FormCommand::new(name, id);

    // Плоские поля (Title/ToolTip/Shortcut/ActionPurpose/Representation/ModifiesSavedData/
    // CurrentRowUse/SelectedRowsUse) — через табличный движок (COMMAND_BODY). CurrentRowUse/
    // SelectedRowsUse через Keep (Designer заполняет свой дефолт `Auto` при отсутствии).
    // Порядок несуществен — коннектор сортирует `sort_props_by_spec` ниже.
    read_fields_designer(
        "FormCommand",
        el,
        tables::COMMAND_BODY,
        Region::Body,
        &mut cmd.properties,
    )?;

    // Picture: `<Picture><xr:Ref>Ref</xr:Ref><xr:LoadTransparent>B</xr:LoadTransparent>` — glue
    // (денормализация вида ссылки; writer её реконструирует).
    if let Some(p) = el.child("Picture").filter(|c| c.prefix.is_empty()) {
        p.claim();
        let r = p
            .child("Ref")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("Command Picture: no <xr:Ref>".into()))?;
        r.claim_with_text();
        let picture_ref = r.text.clone();
        let lt = p
            .child("LoadTransparent")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("Command Picture: no <xr:LoadTransparent>".into()))?;
        lt.claim_with_text();
        // LoadTransparent — НЕЗАВИСИМЫЙ флаг (ERP witness: ИсточникиЗагрузкиПроизводственнойНСИ.
        // ФормаСписка несёт CommonPicture c LoadTransparent="true"), НЕ дерив-сверка.
        let load_transparent = match lt.text.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FormError::Frame(format!(
                    "Command Picture {picture_ref:?}: LoadTransparent={other:?}, want bool (§1.0)"
                )))
            }
        };
        if p.children.len() != 2 {
            return Err(FormError::Frame(
                "Command Picture: unexpected extra children (§1.0)".into(),
            ));
        }
        cmd.properties.push((
            fc::F_PICTURE,
            crate::form::fields::picture_canon(picture_ref, load_transparent),
        ));
    }
    // Action: `<Action>текст` — glue (Designer плоский текст ⟺ EDT handler-контейнер).
    if let Some(a) = el.child("Action").filter(|c| c.prefix.is_empty()) {
        a.claim_with_text();
        expect_no_children(a)?;
        cmd.properties
            .push((fc::F_ACTION, PropertyValue::Str(a.text.clone())));
    }
    // FunctionalOptions: контейнер `<FunctionalOptions><Item>FunctionalOption.X</Item>…`
    // (⟺ EDT повторяемый `<functionalOptions>`; SSL 39⟷39) — glue. §1.0: иные дети — ошибка.
    if let Some(fo) = el
        .child("FunctionalOptions")
        .filter(|c| c.prefix.is_empty())
    {
        fo.claim();
        let mut list = Vec::new();
        for it in &fo.children {
            if it.local != "Item" || !it.prefix.is_empty() {
                return Err(FormError::Frame(format!(
                    "Command FunctionalOptions: unexpected child <{}:{}> (only <Item> — §1.0)",
                    it.prefix, it.local
                )));
            }
            it.claim_with_text();
            expect_no_children(it)?;
            list.push(PropertyValue::Ref(it.text.clone()));
        }
        if list.is_empty() {
            return Err(FormError::Frame(
                "Command FunctionalOptions: empty container is unmodelled (§1.0)".into(),
            ));
        }
        cmd.properties
            .push((fc::F_FUNCTIONAL_OPTIONS, PropertyValue::List(list)));
    }
    // AssociatedTableElementId: `<… xsi:type="xs:string">текст` — glue (обёртка).
    if let Some(a) = el
        .child("AssociatedTableElementId")
        .filter(|c| c.prefix.is_empty())
    {
        let xt = a
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("AssociatedTableElementId: no xsi:type".into()))?;
        if xt.value != "xs:string" {
            return Err(FormError::Frame(format!(
                "AssociatedTableElementId xsi:type={:?}",
                xt.value
            )));
        }
        xt.claimed.set(true);
        a.claim_with_text();
        expect_no_children(a)?;
        cmd.properties.push((
            fc::F_ASSOCIATED_TABLE_ELEMENT_ID,
            PropertyValue::Str(a.text.clone()),
        ));
    }

    // Use — РОЛЕВОЕ ограничение команды (ERP ×391; witness ГруппыСотрудников
    // «НастроитьКритерииОтбораСотрудников»): `<Use><xr:Common>false</xr:Common>
    // <xr:Value name="Role.X">true</xr:Value>…</Use>`. Канон — List([Bool(common),
    // List([Str(role), Bool(v)])…]); отсутствие `<Use>` ⟺ безролевая команда (как прежде,
    // поля в bag нет). EDT-зеркало — `<use><for><value>B</value><role>R</role></for>…`.
    if let Some(u) = el.child("Use").filter(|c| c.prefix.is_empty()) {
        u.claim();
        let common_el = u
            .child("Common")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("Command Use: no <xr:Common> (§1.0)".into()))?;
        common_el.claim_with_text();
        let common = match common_el.text.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(FormError::Frame(format!(
                    "Command Use <xr:Common>={other:?}, want bool (§1.0)"
                )))
            }
        };
        let mut entries = vec![PropertyValue::Bool(common)];
        for v in &u.children {
            if v.prefix == "xr" && v.local == "Common" {
                continue;
            }
            if !(v.prefix == "xr" && v.local == "Value") {
                return Err(FormError::Frame(format!(
                    "Command Use: unexpected child <{}:{}> (§1.0)",
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
                        "Command Use <xr:Value name={role:?}>={other:?}, want bool (§1.0)"
                    )))
                }
            };
            entries.push(PropertyValue::List(vec![
                PropertyValue::Str(role),
                PropertyValue::Bool(val),
            ]));
        }
        cmd.properties
            .push((tables::F_CMD_USE, PropertyValue::List(entries)));
    }

    sort_props_by_spec(&mut cmd.properties, fc::form_command());
    expect_only_children(
        el,
        &[
            "Title",
            "ToolTip",
            "Shortcut",
            "Use",
            "Picture",
            "Action",
            "FunctionalOptions",
            "Representation",
            "ModifiesSavedData",
            "CurrentRowUse",
            "AssociatedTableElementId",
            "SelectedRowsUse",
            "ActionPurpose",
        ],
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "Designer Command {:?}: {leftover} unconsumed node(s) (§1.0)",
            cmd.name
        )));
    }
    Ok(cmd)
}

