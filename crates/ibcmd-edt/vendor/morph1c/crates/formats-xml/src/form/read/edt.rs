//! READ · EDT dialect reader — form envelope/root, service children (command bar, command
//! interface, mobile bar), root ext-info (report/document form) and the data model
//! (attributes, parameters, commands). See §1.0 totality.

use super::*;

// ============================== EDT ==============================

pub(crate) fn read_edt(root: Element) -> Result<FormBody, FormError> {
    if root.prefix != "form" || root.local != "Form" {
        return Err(FormError::Envelope(format!(
            "unexpected root <{}:{}>",
            root.prefix, root.local
        )));
    }
    root.claim();
    claim_root_ns(&root, &[("xmlns:form", FORM_NS_URI)])?;
    // `xmlns:xsi` — ОПЦИОНАЛЬНОЕ объявление (присутствует ⟺ тело использует xsi:-атрибут;
    // сверено 771/772 SSL — БЕЗ xsi лишь минимальная СертификатыКлючей….ПередачаПараметров).
    // Восстанавливается на записи из содержимого, в IR не хранится.
    if let Some(a) = root.attr("xmlns:xsi") {
        if a.value != XSI_NS_URI {
            return Err(FormError::Envelope(format!(
                "root xmlns:xsi={:?} want {XSI_NS_URI:?}",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    // `xmlns:core` — ОПЦИОНАЛЬНОЕ объявление (присутствует ⟺ тело использует core:-тип;
    // сверено 104/104). Восстанавливается на записи из содержимого, в IR не хранится.
    if let Some(a) = root.attr("xmlns:core") {
        if a.value != CORE_NS_URI {
            return Err(FormError::Envelope(format!(
                "root xmlns:core={:?} want {CORE_NS_URI:?}",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    // `xmlns:schema` — ОПЦИОНАЛЬНОЕ (присутствует ⟺ тело несёт DCS-схема-поля реквизита-
    // динсписка; АдреснаяКнига/ФайлыВТоме). Восстанавливается на записи из содержимого.
    if let Some(a) = root.attr("xmlns:schema") {
        if a.value != SCHEMA_NS_URI {
            return Err(FormError::Envelope(format!(
                "root xmlns:schema={:?} want {SCHEMA_NS_URI:?}",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    // `xmlns:settings` — ОПЦИОНАЛЬНОЕ (присутствует ⟺ тело несёт `settings:`-тип: appearance
    // вычисляемого поля динсписка; ERP-witness ОтклоненияВСтоимостиТоваров.ФормаСписка).
    // Восстанавливается на записи из содержимого.
    if let Some(a) = root.attr("xmlns:settings") {
        if a.value != SETTINGS_NS_URI {
            return Err(FormError::Envelope(format!(
                "root xmlns:settings={:?} want {SETTINGS_NS_URI:?}",
                a.value
            )));
        }
        a.claimed.set(true);
    }

    let mut body = FormBody::new();

    // Форм-заголовок `<title>` (опц., прямой ребёнок корня — ДО контрол-дерева).
    // МУЛЬТИЯЗЫЧНЫЙ — ПОВТОРЯЕМЫЙ элемент, по одному на язык (ERP-witness
    // КонтролируемыеСделкиОрганизаций.ФормаДокументовСделки: ru+en) — тот же сбор,
    // что у мульти-<title> реквизита.
    if let Some(t) = root.child("title").filter(|c| c.prefix.is_empty()) {
        body.title = Some(read_edt_title(t)?);
    }
    for t in root
        .children
        .iter()
        .filter(|c| c.local == "title" && c.prefix.is_empty())
        .skip(1)
    {
        match (body.title.as_mut(), read_edt_title(t)?) {
            (Some(PropertyValue::Localized(acc)), PropertyValue::Localized(pairs)) => {
                acc.extend(pairs)
            }
            (got, extra) => {
                return Err(FormError::Frame(format!(
                    "form <title>: unexpected repetition shapes {got:?} + {extra:?} (§1.0)"
                )))
            }
        }
    }

    // Контрол-дерево: `<items>` (прямые дети корня) — диспетчер по xsi:type.
    for el in root
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        body.items.push(read_edt_item(el)?);
    }

    // Форм-атрибуты (claim'ит свои узлы).
    body.attributes = read_form_attrs(FormDialect::Edt, &root).map_err(FormError::Frame)?;

    // autoCommandBar.
    if let Some(acb) = root.child("autoCommandBar").filter(|c| c.prefix.is_empty()) {
        body.auto_command_bar = Some(read_edt_auto_command_bar(acb)?);
    }
    // handlers (форм-события).
    for h in root
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        body.events.push(read_handlers(h)?);
    }
    // ФОРМ-уровневые исключённые команды (`<excludedCommands>X</excludedCommands>`-листы,
    // регион между handlers и форм-атрибутами).
    for x in root
        .children
        .iter()
        .filter(|c| c.local == "excludedCommands" && c.prefix.is_empty())
    {
        x.claim_with_text();
        expect_no_children(x)?;
        body.excluded_commands.push(x.text.clone());
    }
    // Состав командной панели мобильного устройства — ПОВТОРЯЕМЫЙ корневой элемент (каждый
    // несёт РОВНО ОДИН `<value>`; witness ВариантыОтчетов.ФормаСписка ×9 /
    // АктОбУничтоженииПерсональныхДанных ×4; регион ПОСЛЕ форм-атрибутов, ДО данных-модели).
    for m in root
        .children
        .iter()
        .filter(|c| c.local == "mobileDeviceCommandBarContent" && c.prefix.is_empty())
    {
        body.mobile_device_command_bar
            .extend(read_edt_mobile_command_bar(m)?);
    }
    // Заголовок группы кнопок создания (`<createButtonsGroupTitle><key>/<value>` — регион
    // после showCloseButton, ДО данных-модели; witness ПроизводственныеКалендари/Файлы).
    // МНОГОЯЗЫЧНЫЙ — по одному `<createButtonsGroupTitle>` на язык (тот же класс, что title
    // декораций/подсказок; single-язык byte-идентичен прежнему single-read).
    if let Some(t) = read_edt_title_multi(&root, "createButtonsGroupTitle")? {
        body.create_buttons_group_title = Some(t);
    }
    // данные-модель.
    for a in root
        .children
        .iter()
        .filter(|c| c.local == "attributes" && c.prefix.is_empty())
    {
        body.data_attributes.push(read_edt_data_attribute(a)?);
    }
    // Пользовательские команды формы (`<formCommands>`, регион между attributes и parameters).
    for cmd in root
        .children
        .iter()
        .filter(|c| c.local == "formCommands" && c.prefix.is_empty())
    {
        body.commands.push(read_edt_form_command(cmd)?);
    }
    for p in root
        .children
        .iter()
        .filter(|c| c.local == "parameters" && c.prefix.is_empty())
    {
        body.parameters.push(read_edt_parameter(p)?);
    }
    // commandInterface (фикс-блок; navigationPanel/commandBar МОГУТ нести cmiFragmentRecord'ы).
    if let Some(ci) = root
        .child("commandInterface")
        .filter(|c| c.prefix.is_empty())
    {
        let (np, cb) = read_edt_command_interface(ci)?;
        body.command_interface = true;
        body.form_ci_navigation_panel = np;
        body.form_ci_command_bar = cb;
    }
    // Корневой extInfo формы (`<extInfo xsi:type="form:*FormExtInfo">` — ПРЯМОЙ ребёнок корня;
    // отличается от extInfo контролов, которые лежат внутри `<items>`).
    if let Some(ex) = root.child("extInfo").filter(|c| c.prefix.is_empty()) {
        let (rx, report, folders_and_items, document_form, group_list) =
            read_edt_root_ext_info(ex)?;
        body.root_ext_info = Some(rx);
        body.report_form = report;
        body.use_for_folders_and_items = folders_and_items;
        body.document_form = document_form;
        // `groupList` — форм-атрибут F_GROUP_LIST: EDT несёт его в extInfo, Designer — прямым
        // ребёнком корня (read_form_attrs). Кладём в bag в ТАБЛИЧНОЙ позиции (перед F_SETTINGS_STORAGE,
        // иначе в хвост) — bag-порядок совпадает с designer-стороной (X: edt IR == designer IR).
        if let Some(gl) = group_list {
            let pos = body
                .attributes
                .iter()
                .position(|(k, _)| *k == morph1c_core::spec::forms::form_root::F_SETTINGS_STORAGE)
                .unwrap_or(body.attributes.len());
            body.attributes.insert(
                pos,
                (
                    morph1c_core::spec::forms::form_root::F_GROUP_LIST,
                    PropertyValue::Str(gl),
                ),
            );
        }
    }

    // §1.0 тотальность по ВСЕМУ дереву формы.
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{leftover} unconsumed node(s) in EDT form body (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    Ok(body)
}

/// Горизонтальное выравнивание autoCommandBar — 4-состояние {Left,Right,Center,Auto} с
/// ПРОТИВОПОЛОЖНЫМИ пер-форматными омиссиями (SSL cross-census 1481/1481): EDT ОПУСКАЕТ
/// `Auto` (fill=`Auto`, эмитит остальное явно — Left×1238); Designer ОПУСКАЕТ `Left`
/// (fill=`Left`, эмитит остальное явно — Auto×1, ЗагрузкаДанныхИзФайла).
pub(crate) const ACB_HORIZONTAL_ALIGN_DEFAULT: &str = "Left";
/// EDT-fill при отсутствии `<horizontalAlign>` (см. [`ACB_HORIZONTAL_ALIGN_DEFAULT`]).
pub(crate) const ACB_HORIZONTAL_ALIGN_EDT_FILL: &str = "Auto";

pub(crate) fn read_edt_auto_command_bar(el: &Element) -> Result<AutoCommandBar, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    // displayImportance (опц., Enum; метамодель AutoCommandBar: name→id→displayImportance→items;
    // witness УправлениеПодключениемDSS `Low`). Designer несёт АТРИБУТОМ `DisplayImportance`.
    let display_importance = el
        .child("displayImportance")
        .filter(|c| c.prefix.is_empty())
        .map(|c| {
            c.claim_with_text();
            c.text.clone()
        });
    // Дочерние контролы панели (`<items>` — как правило Button-команды).
    let mut items = Vec::new();
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "items" && c.prefix.is_empty())
    {
        items.push(read_edt_item(c)?);
    }
    // horizontalAlign — 4-состояние {Left,Right,Center,Auto} с ПРОТИВОПОЛОЖНЫМИ омиссиями
    // (SSL cross-census 1481/1481: EDT Left×1238+Right×238+Center×4+absent×1 ⟺ Designer
    // absent×1238+Right+Center+Auto×1): EDT ОПУСКАЕТ `Auto`, Designer опускает `Left`.
    let ha = match el.child("horizontalAlign").filter(|c| c.prefix.is_empty()) {
        Some(c) => {
            c.claim_with_text();
            Some(c.text.clone())
        }
        None => Some(ACB_HORIZONTAL_ALIGN_EDT_FILL.to_string()),
    };
    // autoFill — ДАННЫЕ с пер-форматными дефолтами (EDT-дефолт false ⇒ эмитится только
    // `true`; absent ⇒ false). Cross-сверка 104/104 ACB корпуса.
    let auto_fill = match el.child("autoFill").filter(|c| c.prefix.is_empty()) {
        Some(af) => {
            if !matches!(read_bool_text(af, "autoFill")?, PropertyValue::Bool(true)) {
                return Err(FormError::Frame(
                    "autoCommandBar <autoFill> must be true".into(),
                ));
            }
            true
        }
        None => false,
    };
    // `<visible>` — ДАННЫЕ, дефолт `true`; порядок в EDT задан МОДЕЛЬЮ (`models/forms`:
    // AutoCommandBar … items → visible → enabled … horizontalAlign → autoFill).
    let visible = match el.child("visible").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(read_bool_text(v, "visible")?, PropertyValue::Bool(true)),
        None => true,
    };
    expect_only_children(
        el,
        &[
            "name",
            "id",
            "displayImportance",
            "items",
            "visible",
            "horizontalAlign",
            "autoFill",
        ],
    )?;
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

/// Прочитать EDT `<commandInterface>` → (navigationPanel items, commandBar items). Обе панели
/// МОГУТ быть пусты (обычная управляемая форма) ЛИБО нести `<cmiFragmentRecord>`'ы (RE:
/// CommonForm.ВыборИсполнителяБизнесПроцесса). §1.0: атрибуты на панелях не витнессированы →
/// ошибка; иные дети, кроме двух панелей — ошибка.
pub(crate) fn read_edt_command_interface(
    ci: &Element,
) -> Result<(Vec<FormCiItem>, Vec<FormCiItem>), FormError> {
    ci.claim();
    let np = ci
        .child("navigationPanel")
        .ok_or_else(|| FormError::Frame("commandInterface: no navigationPanel".into()))?;
    let cb = ci
        .child("commandBar")
        .ok_or_else(|| FormError::Frame("commandInterface: no commandBar".into()))?;
    if !np.attrs.is_empty() || !cb.attrs.is_empty() {
        return Err(FormError::Frame(
            "commandInterface: navigationPanel/commandBar attrs not modelled (§1.0)".into(),
        ));
    }
    let np_items = read_edt_cmi_panel(np)?;
    let cb_items = read_edt_cmi_panel(cb)?;
    expect_only_children(ci, &["navigationPanel", "commandBar"])?;
    Ok((np_items, cb_items))
}

/// Прочитать одну EDT командную панель (`navigationPanel`/`commandBar`) → её `<cmiFragmentRecord>`'ы.
/// Пустая панель (`<panel/>`) ⇒ пустой список. Каждый фрагмент = `<command>` + опциональные
/// `<type>` (absent ⇒ `Auto`) / `<group>` / `<index>` (НЕЗАВИСИМЫ: group⇒index всегда, index-БЕЗ-
/// группы = размещение по индексу на корне панели; group-БЕЗ-index — §1.0-отказ) / `<userVisible>`
/// (absent ⇒ не задана; пустой ⇒ false; `<common>B` ⇒ B). См. док [`FormCiItem`]. §1.0-strict.
pub(crate) fn read_edt_cmi_panel(panel: &Element) -> Result<Vec<FormCiItem>, FormError> {
    panel.claim();
    let mut out = Vec::new();
    for rec in &panel.children {
        if rec.local != "cmiFragmentRecord" || !rec.prefix.is_empty() {
            return Err(FormError::Frame(format!(
                "commandInterface panel: unexpected child <{}:{}> (only <cmiFragmentRecord> — §1.0)",
                rec.prefix, rec.local
            )));
        }
        rec.claim();
        let command = leaf_text(rec, "command")?;
        let ty = match rec.child("type").filter(|c| c.prefix.is_empty()) {
            Some(t) => {
                t.claim_with_text();
                t.text.clone()
            }
            None => FORM_CI_TYPE_DEFAULT.to_string(),
        };
        let group = match rec.child("group").filter(|c| c.prefix.is_empty()) {
            Some(g) => {
                g.claim_with_text();
                Some(g.text.clone())
            }
            None => None,
        };
        let index = match rec.child("index").filter(|c| c.prefix.is_empty()) {
            Some(i) => {
                i.claim_with_text();
                Some(parse_int(&i.text)?)
            }
            None => None,
        };
        // Census ERP (8194 EDT-фрагментов): group⇒index ВСЕГДА (846, включая index 0); index-БЕЗ-
        // группы — размещение по индексу на КОРНЕ панели (325, значения всегда ≥1); group-БЕЗ-index
        // не встречается (0/8194 — EDT эмитит index при group ВСЕГДА) ⇒ §1.0-отказ (не витнесснут).
        if group.is_some() && index.is_none() {
            return Err(FormError::Frame(format!(
                "cmiFragmentRecord {command:?}: <group> without <index> is unmodelled (§1.0)"
            )));
        }
        let (user_visible, user_visible_roles) = read_edt_user_visible(rec)?;
        expect_only_children(rec, &["command", "type", "group", "index", "userVisible"])?;
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

/// EDT `<userVisible>` визибилити пункта КИ: ОТСУТСТВИЕ == не задана (`None`, []); пустой
/// `<userVisible/>` == common false; `<userVisible><common>BOOL</common></userVisible>` == BOOL.
/// РОЛЕВОЙ вариант — `<userVisible>[<common>true</common>]<for><value>B</value><role>R</role>
/// </for>×N</userVisible>` (общий флаг + пер-ролевые исключения; ⟺ Designer `<Visible>
/// <xr:Common>B</xr:Common><xr:Value name="R">B</xr:Value>×N`; witness ЗаказКлиента.ФормаСписка).
/// Возвращает `(общий флаг, роли)`. §1.0: иной ребёнок → ошибка.
pub(crate) fn read_edt_user_visible(rec: &Element) -> Result<(Option<bool>, Vec<(String, bool)>), FormError> {
    let uv = match rec.child("userVisible").filter(|c| c.prefix.is_empty()) {
        Some(uv) => uv,
        None => return Ok((None, Vec::new())),
    };
    uv.claim();
    // Ролевой пункт: несёт `<for>`-права (± ведущий `<common>true`). EDT эмитит `<common>` лишь
    // при `true`; его отсутствие рядом с `<for>` ⇒ common=false (роли — единственный доступ).
    if uv
        .children
        .iter()
        .any(|c| c.local == "for" && c.prefix.is_empty())
    {
        let common = match uv.child("common").filter(|c| c.prefix.is_empty()) {
            None => false,
            Some(c) => {
                c.claim_with_text();
                if c.text != "true" {
                    return Err(FormError::Frame(format!(
                        "userVisible <common>={:?} alongside <for>-roles, want \"true\" (§1.0)",
                        c.text
                    )));
                }
                true
            }
        };
        let mut roles = Vec::new();
        for f in uv
            .children
            .iter()
            .filter(|c| c.local == "for" && c.prefix.is_empty())
        {
            f.claim();
            let val = matches!(
                read_bool_text(
                    f.child("value").filter(|c| c.prefix.is_empty()).ok_or_else(|| {
                        FormError::Frame("userVisible <for>: no <value> (§1.0)".into())
                    })?,
                    "value"
                )?,
                PropertyValue::Bool(true)
            );
            let role = leaf_text(f, "role")?;
            expect_only_children(f, &["value", "role"])?;
            roles.push((role, val));
        }
        expect_only_children(uv, &["common", "for"])?;
        return Ok((Some(common), roles));
    }
    match uv.children.as_slice() {
        [] => Ok((Some(false), Vec::new())),
        [only] if only.local == "common" && only.prefix.is_empty() => {
            only.claim_with_text();
            match only.text.as_str() {
                "true" => Ok((Some(true), Vec::new())),
                "false" => Ok((Some(false), Vec::new())),
                other => Err(FormError::Frame(format!(
                    "userVisible <common> is {other:?}, want true/false (§1.0)"
                ))),
            }
        }
        _ => Err(FormError::Frame(
            "userVisible carries an unmodelled child (only empty, <common>, or <for>-roles — §1.0)"
                .into(),
        )),
    }
}

/// Общий (обоим форматам) дефолт типа пункта формоуровневого КИ (EDT опускает `Auto`).
pub(crate) const FORM_CI_TYPE_DEFAULT: &str = "Auto";

/// Прочитать ОДИН EDT `<mobileDeviceCommandBarContent>` → значения. Элемент ПОВТОРЯЕМ на
/// корне (по одному на пункт); каждый несёт РОВНО ОДИН `<value xsi:type="core:StringValue">
/// <value>X</value></value>` (через общий value-codec; корпус: children-each всегда 1,
/// но разбор терпим к N — вызывающий копит через extend).
pub(crate) fn read_edt_mobile_command_bar(el: &Element) -> Result<Vec<PropertyValue>, FormError> {
    el.claim();
    let mut out = Vec::new();
    for v in &el.children {
        if v.local != "value" || !v.prefix.is_empty() {
            return Err(FormError::Frame(format!(
                "mobileDeviceCommandBarContent: unexpected child <{}:{}> (§1.0)",
                v.prefix, v.local
            )));
        }
        v.claim();
        out.push(value_codec::decode(ValueDialect::Edt, v).map_err(FormError::Frame)?);
    }
    Ok(out)
}

/// xsi:type маркера корневого extInfo формы отчёта.
pub(crate) const REPORT_FORM_EXT_KIND: &str = "form:ReportFormExtInfo";

/// Результат [`read_edt_root_ext_info`]: корневой extInfo + опциональные форм-уровневые
/// поля отчёта/документа и `useForFoldersAndItems`.
pub(crate) type RootExtInfoRead = (
    FormRootExtInfo,
    Option<ReportFormInfo>,
    Option<String>,
    Option<DocumentFormInfo>,
    // ФОРМ-уровневый `groupList` (форма динамического списка): EDT держит его ВНУТРИ
    // `form:DynamicListFormExtInfo`, Designer — прямым ребёнком корня `<GroupList>` (форм-атрибут
    // F_GROUP_LIST). Читаем здесь, чтобы claim'ить в extInfo; caller кладёт в `body.attributes`.
    Option<String>,
);

/// Прочитать EDT корневой `<extInfo xsi:type="form:*FormExtInfo">` → [`FormRootExtInfo`] +
/// (для формы отчёта) [`ReportFormInfo`]. Несёт дискриминатор типа формы (xsi:type, EDT-only)
/// и СОБСТВЕННЫЕ обработчики формы (`<handlers>` — напр. `AfterWrite` у формы констант;
/// Designer сливает их в корневой `<Events>`). Для `form:ReportFormExtInfo` дополнительно
/// несёт форм-уровневые поля отчёта (`settingsForm`/`showState`/`userSettingsGroup`/
/// `reportResult`/`viewModeApplicationOnSetReportResult`) — они X-сравнимы (Designer несёт их
/// корневыми элементами). Прочие дети — громкая §1.0-ошибка, не Raw.
pub(crate) fn read_edt_root_ext_info(ex: &Element) -> Result<RootExtInfoRead, FormError> {
    ex.claim();
    let xt = ex
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("root extInfo: no xsi:type".into()))?;
    // Дискриминатор типа формы: `form:<Kind>FormExtInfo`.
    if !(xt.value.starts_with("form:") && xt.value.ends_with("FormExtInfo")) {
        return Err(FormError::Frame(format!(
            "root extInfo xsi:type={:?}: not a form:*FormExtInfo (§1.0)",
            xt.value
        )));
    }
    xt.claimed.set(true);
    let kind = xt.value.clone();
    let mut events = Vec::new();
    for h in ex
        .children
        .iter()
        .filter(|c| c.local == "handlers" && c.prefix.is_empty())
    {
        events.push(read_handlers(h)?);
    }
    let report = if kind == REPORT_FORM_EXT_KIND {
        Some(read_edt_report_form(ex)?)
    } else {
        None
    };
    // `userSettingsGroup` формы КОМПОНОВЩИКА НАСТРОЕК (`form:SettingsComposerFormExtInfo`;
    // ⟺ Designer корневой `<CustomSettingsFolder>`). У ФОРМЫ ОТЧЁТА эта группа читается внутри
    // report-блока (`ReportFormInfo`) ⇒ здесь берём её ЛИШЬ для не-отчётных видов, чтобы не
    // задваивать. `None` ⇒ тег отсутствует.
    let user_settings_group = if report.is_some() {
        None
    } else {
        leaf_text_opt(ex, "userSettingsGroup")
    };
    // ФОРМ-уровневое `useForFoldersAndItems` (иерархические Catalog/ChartOfCharacteristicTypes):
    // EDT держит его ВНУТРИ этого extInfo, эмитит лишь non-default `Folders` (дефолт `Items`
    // опущен). Designer несёт прямым ребёнком корня (см. read_designer). См. FormBody.
    let use_for_folders_and_items = leaf_text_opt(ex, "useForFoldersAndItems");
    // ФОРМ-уровневый `groupList` (форма динамического списка `form:DynamicListFormExtInfo`):
    // представление группировки списка (`Дерево`/…). EDT держит его ВНУТРИ этого extInfo,
    // Designer — прямым ребёнком корня `<GroupList>` (форм-атрибут F_GROUP_LIST). ERP-witness
    // Catalog.ВидыОтправляемыхДокументов/РегламентированныеОтчеты (ФормаСписка/ФормаВыбора ×4).
    let group_list = leaf_text_opt(ex, "groupList");
    // Форма объекта-документа: EDT держит autoTime/usePostingMode/repostOnWrite ВНУТРИ этого
    // extInfo, опуская дефолты (autoTime=CurrentOrLast/usePostingMode=Auto никогда не эмитятся;
    // repostOnWrite эмитит лишь `true`). Designer несёт их прямыми детьми корня (см. read_designer).
    let document_form = if kind == "form:DocumentFormExtInfo" {
        let auto_time = leaf_text_opt(ex, "autoTime")
            .unwrap_or_else(|| morph1c_core::ir::DOCUMENT_AUTO_TIME_DEFAULT.to_string());
        let use_posting_mode = leaf_text_opt(ex, "usePostingMode")
            .unwrap_or_else(|| morph1c_core::ir::DOCUMENT_USE_POSTING_MODE_DEFAULT.to_string());
        let repost_on_write = read_presence_true(ex, "repostOnWrite")?;
        Some(DocumentFormInfo {
            auto_time,
            use_posting_mode,
            repost_on_write,
        })
    } else {
        None
    };
    let allowed: &[&str] = if report.is_some() {
        &[
            "handlers",
            "settingsForm",
            "showState",
            "userSettingsGroup",
            "reportResult",
            "detailsInformation",
            "currentVariantPresentationField",
            "reportResultViewMode",
            "viewModeApplicationOnSetReportResult",
            "useForFoldersAndItems",
        ]
    } else if document_form.is_some() {
        &[
            "handlers",
            "useForFoldersAndItems",
            "autoTime",
            "usePostingMode",
            "repostOnWrite",
        ]
    } else {
        // Форма динамического списка (`form:DynamicListFormExtInfo`) / выбора несёт `groupList`.
        &[
            "handlers",
            "useForFoldersAndItems",
            "userSettingsGroup",
            "groupList",
        ]
    };
    expect_only_children(ex, allowed)?;
    Ok((
        FormRootExtInfo {
            kind,
            events,
            user_settings_group,
        },
        report,
        use_for_folders_and_items,
        document_form,
        group_list,
    ))
}

/// Канонический литерал типа формы отчёта `Main` (неявен: EDT его НЕ несёт `<settingsForm>`).
pub(crate) const REPORT_FORM_MAIN: &str = "Main";

/// Прочитать поля формы отчёта из EDT `<extInfo xsi:type="form:ReportFormExtInfo">`.
/// KEEP-поля (`showState`/`reportResult`/`viewModeApplicationOnSetReportResult`) EDT опускает
/// при `Auto` ⇒ отсутствие ⟹ `Auto` (Designer эмитит их всегда). `settingsForm` обязателен.
pub(crate) fn read_edt_report_form(ex: &Element) -> Result<ReportFormInfo, FormError> {
    let opt = |tag: &str| -> Option<String> {
        ex.child(tag).filter(|c| c.prefix.is_empty()).map(|c| {
            c.claim_with_text();
            c.text.clone()
        })
    };
    // `settingsForm` присутствует для `Variant`/`Settings`; ОТСУТСТВИЕ ⟹ тип `Main` (неявен).
    let settings_form = opt("settingsForm").unwrap_or_else(|| REPORT_FORM_MAIN.to_string());
    let show_state = opt("showState").unwrap_or_else(|| REPORT_FORM_AUTO.to_string());
    let user_settings_group = opt("userSettingsGroup");
    let report_result = opt("reportResult");
    let details_data = opt("detailsInformation");
    let variant_appearance = opt("currentVariantPresentationField");
    let report_result_view_mode =
        opt("reportResultViewMode").unwrap_or_else(|| REPORT_FORM_AUTO.to_string());
    let view_mode_application =
        opt("viewModeApplicationOnSetReportResult").unwrap_or_else(|| REPORT_FORM_AUTO.to_string());
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

pub(crate) fn read_edt_data_attribute(el: &Element) -> Result<FormDataAttribute, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    // id ОПЦИОНАЛЕН (EDT опускает дефолт `0`; ⟺ Designer `id="0"`). См. [`read_edt_id`].
    let id = read_edt_id(el)?;
    // title (опц., Localized) — регион между name и id.
    let title = match el.child("title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_edt_title(t)?),
        None => None,
    };
    // Мультиязычный заголовок реквизита — ПОВТОРЯЕМЫЙ `<title>` (по одному на язык;
    // ERP-witness Задание.ДействиеВыполнить ×4). Первый прочитан выше; хвост доклеиваем.
    let mut title = title;
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
                    "attribute <title>: unexpected repetition shapes {got:?} + {extra:?} (§1.0)"
                )))
            }
        }
    }
    let value_type = read_value_type(el, "valueType", crate::TypeDialect::Edt)?;
    // view/edit-права: `<view><common>true</common></view>` = общий (true); ПУСТОЙ `<view/>`
    // = запрещён (false; ⟺ Designer `<View><xr:Common>false</xr:Common></View>`; витнессы
    // УчетныеЗаписи…/Взаимодействия/ВерсииОбъектов).
    let (view_common, view_roles) = read_edt_common_flag(el, "view")?;
    let (edit_common, edit_roles) = read_edt_common_flag(el, "edit")?;
    // fillChecking (опц., Enum).
    let fill_checking = match el.child("fillChecking").filter(|c| c.prefix.is_empty()) {
        Some(f) => {
            f.claim_with_text();
            Some(f.text.clone())
        }
        None => None,
    };
    // main/savedData (опц., presence-true bool).
    let main = match el.child("main").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(read_bool_text(v, "main")?, PropertyValue::Bool(true)),
        None => false,
    };
    let saved_data = match el.child("savedData").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(read_bool_text(v, "savedData")?, PropertyValue::Bool(true)),
        None => false,
    };
    // settingsSavedData (ПОВТОРЯЕМЫЙ, DataPath; dual-encoding: Designer ОДИН `<Save>` с
    // несколькими `<Field>Путь`; witness ГрупповоеИзменениеРеквизитов ×2).
    let mut settings_saved_data = Vec::new();
    for s in el
        .children
        .iter()
        .filter(|c| c.local == "settingsSavedData" && c.prefix.is_empty())
    {
        settings_saved_data.push(read_edt_data_path_elem(s, "settingsSavedData")?);
    }
    // functionalOptions (повторяемый Ref): `<functionalOptions>FunctionalOption.X`.
    let mut functional_options = Vec::new();
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "functionalOptions" && c.prefix.is_empty())
    {
        c.claim_with_text();
        expect_no_children(c)?;
        functional_options.push(c.text.clone());
    }
    // notDefaultUseAlwaysAttributes (ПОВТОРЯЕМЫЙ, DataPath): `<notDefaultUseAlwaysAttributes
    // xsi:type="form:DataPath"><segments>Путь</segments>` — реквизит может нести несколько.
    let mut not_default_use_always = Vec::new();
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "notDefaultUseAlwaysAttributes" && c.prefix.is_empty())
    {
        not_default_use_always.push(read_edt_data_path_elem(c, "notDefaultUseAlwaysAttributes")?);
    }
    // Колонки табличного реквизита (ValueTable): повторяемые `<columns>` — рекурсивно
    // читаются как реквизиты (несут name/title/id/valueType/view/edit).
    let mut columns = Vec::new();
    for c in el
        .children
        .iter()
        .filter(|c| c.local == "columns" && c.prefix.is_empty())
    {
        columns.push(read_edt_data_attribute(c)?);
    }
    // Дополнительные колонки (ValueTable): ПОВТОРЯЕМЫЕ `<additionalColumns>` — сиблинги
    // `<columns>`, каждый несёт `<tablePath xsi:type="form:DataPath"><segments>` + вложенные
    // `<columns>`-дети (рекурсивно как реквизиты).
    let mut additional_columns = Vec::new();
    for ac in el
        .children
        .iter()
        .filter(|c| c.local == "additionalColumns" && c.prefix.is_empty())
    {
        ac.claim();
        let tp = ac
            .child("tablePath")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("additionalColumns: no <tablePath> (§1.0)".into()))?;
        let table_path = read_edt_data_path_elem(tp, "tablePath")?;
        let mut cols = Vec::new();
        for c in ac
            .children
            .iter()
            .filter(|c| c.local == "columns" && c.prefix.is_empty())
        {
            cols.push(read_edt_data_attribute(c)?);
        }
        expect_only_children(ac, &["tablePath", "columns"])?;
        additional_columns.push(AdditionalColumns {
            table_path,
            columns: cols,
        });
    }
    // extInfo реквизита: `form:ValueListExtInfo` (пустой маркер «Список значений»);
    // `form:SpreadsheetDocumentExtInfo` (пустой маркер «Табличный документ», EDT-only);
    // `form:DynamicListExtInfo` (динамический список — query/mainTable/флаги). Иной — §1.0-ошибка.
    let (value_list_ext, value_list_item_type, spreadsheet_ext, dynamic_list, scalar_marker) =
        read_edt_attribute_ext_info(el)?;
    // §1.0-кросс-чек пустых маркеров «Диаграмма»/«Графическая схема» (класс
    // SpreadsheetDocumentExtInfo): маркер ⟺ скаляр-тип Chart/GraphicalSchema (1/1 корпус;
    // Designer маркера не несёт — EDT-write регенерирует из value_type).
    if scalar_marker != scalar_marker_ext(&value_type) {
        return Err(FormError::Frame(format!(
            "attribute {name:?}: Chart/GraphicalScheme extInfo marker {scalar_marker:?} diverges              from its determinant (scalar type = {:?}) — unknown variant (§1.0)",
            scalar_marker_ext(&value_type)
        )));
    }
    // §1.0-кросс-чек детерминанта маркера: `form:SpreadsheetDocumentExtInfo` ⟺ скаляр-тип
    // `SpreadsheetDocument` (37/37 корпус SSL+coverage; у составных типов маркера нет). Designer
    // маркера не несёт и ВЫВОДИТ его из типа тем же предикатом — расхождение здесь означало бы
    // тихую пере/недо-генерацию маркера на кросс-конверсии ⇒ громкий отказ, не глотаем.
    if spreadsheet_ext != is_scalar_spreadsheet_type(&value_type) {
        return Err(FormError::Frame(format!(
            "attribute {name:?}: form:SpreadsheetDocumentExtInfo marker={spreadsheet_ext} \
             diverges from its determinant (scalar SpreadsheetDocument value type = {}) — \
             unknown variant (§1.0)",
            !spreadsheet_ext
        )));
    }
    expect_only_children(
        el,
        &[
            "name",
            "title",
            "id",
            "valueType",
            "view",
            "edit",
            "fillChecking",
            "functionalOptions",
            "notDefaultUseAlwaysAttributes",
            "main",
            "savedData",
            "settingsSavedData",
            "columns",
            "additionalColumns",
            "extInfo",
        ],
    )?;
    Ok(FormDataAttribute {
        chart_settings: None,
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
        // EDT-корпус НИ РАЗУ не несёт designer-сигилу `~` на путях-полях (проверено по всем
        // 876 формам) — EDT-сторона носителя пуста по построению. См. IR-док.
        designer_unavailable_paths: Vec::new(),
        dynamic_list,
        // EDT несёт лишь пустой маркер `form:SpreadsheetDocumentExtInfo` (spreadsheet_ext), сам
        // табличный документ — Designer-only (`<Settings mxl:SpreadsheetDocument>`).
        spreadsheet_settings: None,
    })
}

/// Прочитать EDT `<tag xsi:type="form:DataPath"><segments>Путь</segments></tag>`-элемент → путь.
/// Иной xsi:type / состав — §1.0-ошибка.
pub(crate) fn read_edt_data_path_elem(s: &Element, tag: &str) -> Result<String, FormError> {
    s.claim();
    let xt = s
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no xsi:type")))?;
    if xt.value != "form:DataPath" {
        return Err(FormError::Frame(format!(
            "<{tag}> xsi:type={:?}, want form:DataPath",
            xt.value
        )));
    }
    xt.claimed.set(true);
    let seg = s
        .child("segments")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <segments> (§1.0)")))?;
    seg.claim_with_text();
    if s.children.len() != 1 {
        return Err(FormError::Frame(format!(
            "<{tag}>: expected one <segments> (§1.0)"
        )));
    }
    Ok(seg.text.clone())
}

/// Прочитать EDT `<extInfo>` реквизита →
/// `(value_list_ext, value_list_item_type, spreadsheet_ext, dynamic_list)`.
/// * `form:ValueListExtInfo` — `<itemValueType>` (маркер «Список значений»): пустой ⇒
///   `item_type = None`, НЕ-пустой (`<types>String</types>…`) ⇒ `Some(TypeSpec)` (общий type-codec);
/// * `form:SpreadsheetDocumentExtInfo` — ПОЛНОСТЬЮ пустой (маркер «Табличный документ»,
///   EDT-only, без Designer-аналога);
/// * `form:DynamicListExtInfo` — динамический список (query/mainTable/флаги; см.
///   [`read_edt_dynamic_list_attr`]).
///
/// Отсутствие extInfo ⇒ `(false, None, false, None)`. Чужой/непустой xsi:type — §1.0-ошибка.
pub(crate) fn read_edt_attribute_ext_info(
    el: &Element,
) -> Result<
    (
        bool,
        Option<TypeSpec>,
        bool,
        Option<DynamicListAttrExt>,
        Option<&'static str>,
    ),
    FormError,
> {
    let ex = match el.child("extInfo").filter(|c| c.prefix.is_empty()) {
        Some(e) => e,
        None => return Ok((false, None, false, None, None)),
    };
    ex.claim();
    let xt = ex
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("attribute extInfo: no xsi:type".into()))?;
    match xt.value.as_str() {
        "form:ValueListExtInfo" => {
            xt.claimed.set(true);
            let iv = ex
                .child("itemValueType")
                .filter(|c| c.prefix.is_empty())
                .ok_or_else(|| {
                    FormError::Frame("form:ValueListExtInfo: no <itemValueType> (§1.0)".into())
                })?;
            // Пустой `<itemValueType/>` ⇒ None; НЕ-пустой (`<types>String</types>…`) ⇒
            // TypeSpec через общий type-codec (тот же путь, что `<valueType>`).
            let item_type = decode_type_host(iv, crate::TypeDialect::Edt)?;
            if ex.children.len() != 1 {
                return Err(FormError::Frame(
                    "form:ValueListExtInfo: expected only <itemValueType> (§1.0)".into(),
                ));
            }
            Ok((true, item_type, false, None, None))
        }
        "form:SpreadsheetDocumentExtInfo" => {
            xt.claimed.set(true);
            if !ex.children.is_empty() || !ex.text.is_empty() {
                return Err(FormError::Frame(
                    "form:SpreadsheetDocumentExtInfo: must be empty (§1.0)".into(),
                ));
            }
            Ok((false, None, true, None, None))
        }
        // Пустые EDT-only маркеры «Диаграмма»/«Графическая схема» — тот же класс, что
        // SpreadsheetDocumentExtInfo: ПОЛНОСТЬЮ детерминированы скаляр-типом реквизита
        // (Chart / GraphicalSchema; witness ОценкаПроизводительности / КартаМаршрута,
        // определитель 1/1 в обоих корпусах) ⇒ в IR НЕ хранятся: ридер сверяет с
        // детерминантом (вызывающий), писатель регенерирует из value_type
        // ([`crate::form::write`] `scalar_marker_ext`). Designer маркера не несёт вовсе.
        "form:ChartExtInfo" => {
            xt.claimed.set(true);
            if !ex.children.is_empty() || !ex.text.is_empty() {
                return Err(FormError::Frame(
                    "form:ChartExtInfo: must be empty (§1.0)".into(),
                ));
            }
            Ok((false, None, false, None, Some("Chart")))
        }
        "form:GraphicalSchemeExtInfo" => {
            xt.claimed.set(true);
            if !ex.children.is_empty() || !ex.text.is_empty() {
                return Err(FormError::Frame(
                    "form:GraphicalSchemeExtInfo: must be empty (§1.0)".into(),
                ));
            }
            Ok((false, None, false, None, Some("GraphicalSchema")))
        }
        // «Географическая схема» — тот же класс пустых EDT-only маркеров (ERP-witness
        // БизнесРегионы.ФормаВыбораГеографическогоРегиона: `<extInfo xsi:type=
        // "form:GeographicalSchemaExtInfo"/>` при `<types>GeographicalSchema</types>`).
        "form:GeographicalSchemaExtInfo" => {
            xt.claimed.set(true);
            if !ex.children.is_empty() || !ex.text.is_empty() {
                return Err(FormError::Frame(
                    "form:GeographicalSchemaExtInfo: must be empty (§1.0)".into(),
                ));
            }
            Ok((false, None, false, None, Some("GeographicalSchema")))
        }
        // «Диаграмма Ганта» — тот же класс пустых EDT-only маркеров (backing реквизит
        // GanttChartField). ERP-witness'ы (7 форм, оба диалекта; напр. DataProcessor
        // ДиспетчированиеПроизводства / ДиспетчированиеПроизводстваПооперационное /
        // ДиспетчированиеГрафикаПроизводства) — `<extInfo xsi:type="form:GanttChartExtInfo"/>`
        // при `<types>GanttChart</types>`. Тело диаграммы Designer инлайнит
        // (`<Settings d4p1:GanttChart>`), EDT держит в сайдкаре
        // `Attributes/<attr>/ExtInfo/GanttChart.chart` (chart_settings, транскодит pipeline) — сам
        // маркер ПУСТ и детерминирован скаляр-типом GanttChart (детерминант — scalar_marker_ext).
        "form:GanttChartExtInfo" => {
            xt.claimed.set(true);
            if !ex.children.is_empty() || !ex.text.is_empty() {
                return Err(FormError::Frame(
                    "form:GanttChartExtInfo: must be empty (§1.0)".into(),
                ));
            }
            Ok((false, None, false, None, Some("GanttChart")))
        }
        "form:DynamicListExtInfo" => {
            xt.claimed.set(true);
            let dl = read_edt_dynamic_list_attr(ex)?;
            Ok((false, None, false, Some(dl), None))
        }
        other => Err(FormError::Frame(format!(
            "attribute extInfo xsi:type={other:?}: unmodeled attribute extInfo (§1.0)"
        ))),
    }
}

/// Прочитать EDT `<extInfo xsi:type="form:DynamicListExtInfo">` (SIMPLE-форма) → [`DynamicListAttrExt`].
/// Порядок EDT: queryText, mainTable, dynamicDataRead, autoFillAvailableFields, customQuery,
/// autoSaveUserSettings, getInvisibleFieldPresentations. DCS-настройки (`<ListSettings>`) EDT
/// держит в САЙДКАРЕ `.dcss` (НЕ в `Form.form`) ⇒ list_settings=None. EXTENDED-форма (`<fields>`/
/// `<parameters>` — DataCompositionSchema data-set) пока НЕ моделируется ⇒ expect_only_children
/// эрроритит (§1.0, честный отложенный блокер — АдреснаяКнига/ФайлыВТоме).
pub(crate) fn read_edt_dynamic_list_attr(ex: &Element) -> Result<DynamicListAttrExt, FormError> {
    // queryText/mainTable ОПЦИОНАЛЬНЫ: АВТО-запрос (customQuery=false) не несёт queryText; а
    // РАСШИРЕННАЯ форма (собственный DCS data-set через fields/parameters) не несёт mainTable.
    let query_text = leaf_text_opt(ex, "queryText");
    let main_table = leaf_text_opt(ex, "mainTable");
    let dynamic_data_read = read_presence_true(ex, "dynamicDataRead")?;
    let auto_fill_available_fields = read_presence_true(ex, "autoFillAvailableFields")?;
    let custom_query = read_presence_true(ex, "customQuery")?;
    let auto_save_user_settings = read_presence_true(ex, "autoSaveUserSettings")?;
    let get_invisible_field_presentations =
        read_presence_true(ex, "getInvisibleFieldPresentations")?;
    // keyType — вид ключа списка (ERP ×7 ⟷ Designer `<KeyType>`).
    let key_type = leaf_text_opt(ex, "keyType");
    // keyField — ключевые поля списка (ПОВТОРЯЕМЫЙ; witness ВыбранныеЭлементы ×1, СписокДокументов
    // ×4). Собираем ВСЕ (каждое claim'им), иначе 2-е и далее — несконсуменный узел (§1.0).
    let key_fields = leaf_texts_all(ex, "keyField");
    // Вычисляемые поля (`<calculatedFields>`, повторяемые; witness НеудаленныеОбъекты).
    let mut calculated_fields = Vec::new();
    for cf in ex
        .children
        .iter()
        .filter(|c| c.local == "calculatedFields" && c.prefix.is_empty())
    {
        calculated_fields.push(read_edt_dcs_calculated_field(cf)?);
    }
    // EXTENDED: DataCompositionSchema-поля (`<fields>`) и параметры (`<parameters>`) — ОБА
    // формата несут (X-сравнимы). Порядок: fields ПОСЛЕ флагов, parameters ПОСЛЕ fields.
    let mut fields = Vec::new();
    for f in ex
        .children
        .iter()
        .filter(|c| c.local == "fields" && c.prefix.is_empty())
    {
        fields.push(read_edt_dcs_field(f)?);
    }
    let mut parameters = Vec::new();
    for p in ex
        .children
        .iter()
        .filter(|c| c.local == "parameters" && c.prefix.is_empty())
    {
        parameters.push(read_edt_dcs_parameter(p)?);
    }
    expect_only_children(
        ex,
        &[
            "queryText",
            "mainTable",
            "dynamicDataRead",
            "autoFillAvailableFields",
            "customQuery",
            "autoSaveUserSettings",
            "getInvisibleFieldPresentations",
            "keyType",
            "keyField",
            "calculatedFields",
            "fields",
            "parameters",
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
        get_invisible_field_presentations,
        key_fields,
        calculated_fields,
        list_settings: None,
        fields,
        parameters,
    })
}

/// EDT опциональный явный bool `<tag>true|false</tag>` → `Some(bool)`; отсутствие ⇒ `None`.
pub(crate) fn read_edt_opt_bool(el: &Element, tag: &str) -> Result<Option<bool>, FormError> {
    match el.child(tag).filter(|c| c.prefix.is_empty()) {
        Some(v) => Ok(Some(matches!(
            read_bool_text(v, tag)?,
            PropertyValue::Bool(true)
        ))),
        None => Ok(None),
    }
}

pub(crate) fn read_edt_parameter(el: &Element) -> Result<FormParameter, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    let value_type = read_value_type(el, "valueType", crate::TypeDialect::Edt)?;
    // keyParameter (опц., presence-true bool).
    let key_parameter = match el.child("keyParameter").filter(|c| c.prefix.is_empty()) {
        Some(v) => matches!(
            read_bool_text(v, "keyParameter")?,
            PropertyValue::Bool(true)
        ),
        None => false,
    };
    expect_only_children(el, &["name", "valueType", "keyParameter"])?;
    Ok(FormParameter {
        name,
        value_type,
        key_parameter,
    })
}

/// EDT `<formCommands>` → [`FormCommand`] (спек `spec/forms/command`, порядок = канон).
///
/// Каркас-константы: `<use><common>true</common></use>` (всегда; не в IR), обёртки
/// `<action xsi:type="form:FormCommandHandlerContainer"><handler><name>` и
/// `<associatedTableElementId xsi:type="core:StringValue"><value>`. Пер-форматные дефолты
/// `currentRowUse`/`selectedRowsUse`: EDT опускает `Use` ⇒ absent заполняется `Use`.
pub(crate) fn read_edt_form_command(el: &Element) -> Result<FormCommand, FormError> {
    el.claim();
    let name = leaf_text(el, "name")?;
    let id = leaf_int(el, "id")?;
    let mut cmd = FormCommand::new(name, id);

    // Плоские поля (title/toolTip/shortcut/actionPurpose/representation/modifiesStoredData/
    // currentRowUse/selectedRowsUse) — через табличный движок (COMMAND_BODY). currentRowUse/
    // selectedRowsUse через Keep-политику (EDT заполняет свой дефолт `Use` при отсутствии).
    // Порядок несуществен — коннектор сортирует `sort_props_by_spec` ниже.
    read_fields_edt(
        "FormCommand",
        Some(el),
        tables::COMMAND_BODY,
        Region::Body,
        &mut cmd.properties,
    )?;

    // use: безролевая команда — EDT-константа `<use><common>true</common></use>` (не в IR);
    // РОЛЕВАЯ — `<use>[<common>true</common>]<for><value>B</value><role>Role.X</role></for>…</use>`.
    // common ОТСУТСТВУЕТ ⇒ false (запрет + ролевые исключения; ERP ×371, witness ГруппыСотрудников,
    // зеркально Designer `<Use><xr:Common>false`); common=`true` ⇒ ОБЩИЙ доступ С ролевыми
    // исключениями (ERP ×20, witness ЖурналДокументовБезналичныеПлатежи.ФормаСписка; ⟷ Designer
    // `<Use><xr:Common>true</xr:Common><xr:Value>×N`). Канон — List([Bool(common), List([Str(role),
    // Bool(v)])…]). EDT эмитит `<common>` лишь при `true`.
    let use_el = el
        .child("use")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("formCommands: missing <use>".into()))?;
    let has_roles = use_el
        .children
        .iter()
        .any(|c| c.local == "for" && c.prefix.is_empty());
    if !has_roles {
        read_common_true(use_el, "use")?;
    } else {
        use_el.claim();
        let common = match use_el.child("common").filter(|c| c.prefix.is_empty()) {
            None => false,
            Some(c) => {
                if !matches!(read_bool_text(c, "common")?, PropertyValue::Bool(true)) {
                    return Err(FormError::Frame(
                        "formCommands <use><common>=false alongside <for>-roles is unwitnessed \
                         (EDT omits false; §1.0)"
                            .into(),
                    ));
                }
                true
            }
        };
        let mut entries = vec![PropertyValue::Bool(common)];
        for f in use_el
            .children
            .iter()
            .filter(|c| c.local == "for" && c.prefix.is_empty())
        {
            f.claim();
            // `<value>` ОПЦИОНАЛЕН: EDT опускает дефолт `false` (роль без явного значения ⇒ доступ
            // ЗАПРЕЩЁН; ERP-witness Document.Отпуск.ФормаДокумента команда ПодробнееОРасчетеНДФЛ —
            // весь список ролей БЕЗ `<value>` ⟺ Designer `<xr:Value name=Role>false`), эмитит
            // явно ЛИШЬ `true` (OppositeBool, симметрично прочим EDT-омиссиям дефолта).
            let val = match f.child("value").filter(|c| c.prefix.is_empty()) {
                Some(v) => matches!(read_bool_text(v, "value")?, PropertyValue::Bool(true)),
                None => false,
            };
            let role = leaf_text(f, "role")?;
            expect_only_children(f, &["value", "role"])?;
            entries.push(PropertyValue::List(vec![
                PropertyValue::Str(role),
                PropertyValue::Bool(val),
            ]));
        }
        expect_only_children(use_el, &["common", "for"])?;
        cmd.properties
            .push((tables::F_CMD_USE, PropertyValue::List(entries)));
    }
    // picture: `<picture xsi:type="core:PictureRef"><picture>Ref</picture></picture>` — glue
    // (EDT принимает только core:PictureRef; writer суппрессит пустую ссылку).
    if let Some(p) = el.child("picture").filter(|c| c.prefix.is_empty()) {
        p.claim();
        let xt = p
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("command picture: no xsi:type".into()))?;
        if xt.value != "core:PictureRef" {
            return Err(FormError::Frame(format!(
                "command picture xsi:type={:?}",
                xt.value
            )));
        }
        xt.claimed.set(true);
        let r = leaf_text(p, "picture")?;
        expect_only_children(p, &["picture"])?;
        // EDT command-picture LoadTransparent не несёт ⇒ дерив (симметрично полю-picture).
        let lt = crate::form::fields::picture_lt_default(&r);
        cmd.properties
            .push((fc::F_PICTURE, crate::form::fields::picture_canon(r, lt)));
    }
    // action: `<action xsi:type="form:FormCommandHandlerContainer"><handler><name>X` — glue
    // (EDT handler-контейнер ⟺ Designer текст; несовместимо с FieldProj-кодеком).
    if let Some(a) = el.child("action").filter(|c| c.prefix.is_empty()) {
        a.claim();
        let xt = a
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("command action: no xsi:type".into()))?;
        if xt.value != "form:FormCommandHandlerContainer" {
            return Err(FormError::Frame(format!(
                "command action xsi:type={:?}",
                xt.value
            )));
        }
        xt.claimed.set(true);
        let h = a
            .child("handler")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("command action: no <handler>".into()))?;
        h.claim();
        let handler_name = leaf_text(h, "name")?;
        expect_only_children(h, &["name"])?;
        expect_only_children(a, &["handler"])?;
        cmd.properties
            .push((fc::F_ACTION, PropertyValue::Str(handler_name)));
    }
    // functionalOptions: ПОВТОРЯЕМЫЙ `<functionalOptions>FunctionalOption.X</functionalOptions>`
    // (⟺ Designer контейнер `<FunctionalOptions><Item>…`; SSL 39⟷39) — glue (repeatable).
    let fos: Vec<PropertyValue> = el
        .children
        .iter()
        .filter(|c| c.local == "functionalOptions" && c.prefix.is_empty())
        .map(|c| {
            c.claim_with_text();
            PropertyValue::Ref(c.text.clone())
        })
        .collect();
    if !fos.is_empty() {
        cmd.properties
            .push((fc::F_FUNCTIONAL_OPTIONS, PropertyValue::List(fos)));
    }
    // associatedTableElementId: `<… xsi:type="core:StringValue"><value>X</value>` — glue (обёртка).
    if let Some(a) = el
        .child("associatedTableElementId")
        .filter(|c| c.prefix.is_empty())
    {
        a.claim();
        let xt = a
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("associatedTableElementId: no xsi:type".into()))?;
        if xt.value != "core:StringValue" {
            return Err(FormError::Frame(format!(
                "associatedTableElementId xsi:type={:?}",
                xt.value
            )));
        }
        xt.claimed.set(true);
        let v = leaf_text(a, "value")?;
        expect_only_children(a, &["value"])?;
        cmd.properties
            .push((fc::F_ASSOCIATED_TABLE_ELEMENT_ID, PropertyValue::Str(v)));
    }

    sort_props_by_spec(&mut cmd.properties, fc::form_command());
    expect_only_children(
        el,
        &[
            "name",
            "title",
            "id",
            "toolTip",
            "use",
            "shortcut",
            "picture",
            "action",
            "actionPurpose",
            "functionalOptions",
            "representation",
            "modifiesStoredData",
            "currentRowUse",
            "associatedTableElementId",
            "selectedRowsUse",
        ],
    )?;
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "formCommands {:?}: {leftover} unconsumed node(s) (§1.0)",
            cmd.name
        )));
    }
    Ok(cmd)
}

