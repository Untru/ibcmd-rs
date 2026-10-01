//! READ · DCS user-settings blob parsing (`ListSettings`/`item`/`SettingsParameterValue`,
//! conditional appearance, right-values). Shared: Designer inline `<Settings>` AND the EDT
//! `ListSettings.dcss` sidecar re-prefix into `dcsset:` and call the SAME readers.

use super::*;

/// Прочитать Designer `<ListSettings>` (dcsset DCS user-settings) → [`DcsListSettings`].
/// Порядок: filter?, dataParameters?, order?, conditionalAppearance?, item* (структурные),
/// itemsViewMode?, itemsUserSettingID? — КАЖДЫЙ регион опционален (витнесс пустой
/// `<ListSettings/>` — ВерсииПодсистемОбластейДанных). Типизированный разбор (§1.0 — не Raw).
///
/// ПЕРЕИСПОЛЬЗУЕТСЯ EDT-сайдкаром `ListSettings.dcss` ([`crate::form::dcss`]): тот несёт РОВНО ту же
/// структуру, но с ДЕФОЛТНЫМ settings-ns — сайдкар-кодек перепрефиксирует дерево в `dcsset:` и
/// зовёт ЭТОТ ридер (одна типизация DCS-настроек на оба локуса, не два разбора).
pub(crate) fn read_designer_list_settings(ls: &Element) -> Result<DcsListSettings, FormError> {
    ls.claim();
    let filter = read_dcs_settings_group(ls, "filter")?;
    // dataParameters — `dcscor:item xsi:type="dcsset:SettingsParameterValue"`-список
    // (witness СценарииОбменовДанными.НастройкаРасписанияОбменовДанными).
    let mut data_parameters = Vec::new();
    if let Some(dp) = ls.child("dataParameters").filter(|c| c.prefix == "dcsset") {
        dp.claim();
        for it in dp
            .children
            .iter()
            .filter(|c| c.local == "item" && c.prefix == "dcscor")
        {
            data_parameters.push(read_dcs_settings_parameter_value(it)?);
        }
        for c in &dp.children {
            if !(c.prefix == "dcscor" && c.local == "item") {
                return Err(FormError::Frame(format!(
                    "<dcsset:dataParameters>: unexpected child <{}:{}> (§1.0)",
                    c.prefix, c.local
                )));
            }
        }
    }
    let order = read_dcs_settings_group(ls, "order")?;
    let conditional_appearance = read_dcs_settings_group(ls, "conditionalAppearance")?;
    // Корневые СТРУКТУРНЫЕ `dcsset:item` (StructureItemGroup; witness ДоступныеАнкеты).
    let mut structure_items = Vec::new();
    for it in ls
        .children
        .iter()
        .filter(|c| c.local == "item" && c.prefix == "dcsset")
    {
        structure_items.push(read_dcs_item(it)?);
    }
    let items_view_mode = dcsset_leaf_text_opt(ls, "itemsViewMode");
    let items_user_setting_id = dcsset_leaf_text_opt(ls, "itemsUserSettingID");
    // itemsUserSettingPresentation — ПОСЛЕ itemsUserSettingID (ERP-witness
    // ПравилаРаспределенияРасходов.ФормаСпискаВручную реквизит ПоказателиКЗаполнению).
    let items_user_setting_presentation =
        read_dcs_presentation_opt(ls, "itemsUserSettingPresentation")?;
    expect_only_dcsset_children(
        ls,
        &[
            "filter",
            "dataParameters",
            "order",
            "conditionalAppearance",
            "item",
            "itemsViewMode",
            "itemsUserSettingID",
            "itemsUserSettingPresentation",
        ],
    )?;
    Ok(DcsListSettings {
        filter,
        data_parameters,
        order,
        conditional_appearance,
        structure_items,
        items_view_mode,
        items_user_setting_id,
        items_user_setting_presentation,
        // Флаг конверта EDT-сайдкара: Designer-инлайн его НЕ несёт (ns — на корне Form.xml);
        // выставляется ТОЛЬКО ридером сайдкара (read_list_settings_dcss), здесь — SSL-flavor.
        envelope_without_pal: false,
    })
}

/// Прочитать `dcscor:item xsi:type="dcsset:SettingsParameterValue"` (внутри
/// `dcsset:dataParameters`/`dcsset:appearance`) → [`DcsSettingsParameterValue`]:
/// dcscor:use?, dcscor:parameter, dcscor:value, dcsset:userSettingID?.
pub(crate) fn read_dcs_settings_parameter_value(it: &Element) -> Result<DcsSettingsParameterValue, FormError> {
    it.claim();
    claim_xsi_type(it, "dcsset:SettingsParameterValue")?;
    let used = match it.child("use").filter(|c| c.prefix == "dcscor") {
        Some(v) => {
            v.claim_with_text();
            match v.text.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                other => {
                    return Err(FormError::Frame(format!(
                        "<dcscor:use>={other:?}, want bool (§1.0)"
                    )))
                }
            }
        }
        None => None,
    };
    let parameter = {
        let p = it
            .child("parameter")
            .filter(|c| c.prefix == "dcscor")
            .ok_or_else(|| {
                FormError::Frame("SettingsParameterValue: no <dcscor:parameter> (§1.0)".into())
            })?;
        p.claim_with_text();
        p.text.clone()
    };
    // `<dcscor:value>` ОПЦИОНАЛЕН: неиспользуемый dataParameter (`<dcscor:use>false`) несёт лишь
    // use+parameter БЕЗ value (witness ERP ×7 designer-блоков) ⇒ None. Присутствие ⇒ типизируем.
    let value = match it.child("value").filter(|c| c.prefix == "dcscor") {
        None => None,
        Some(v) => Some({
            v.claim();
            if let Some(nil) = v.attr("xsi:nil") {
        if nil.value != "true" {
            return Err(FormError::Frame(format!(
                "dcscor:value xsi:nil={:?} (§1.0)",
                nil.value
            )));
        }
        nil.claimed.set(true);
        DcsCorValue::Nil
    } else {
        let xt = v
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("dcscor:value: no xsi:type (§1.0)".into()))?;
        match xt.value.as_str() {
            "v8ui:Color" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Color(v.text.clone())
            }
            // Булев параметр оформления (ERP-witness АналитикиСтатейБюджетов.
            // НастройкаТипаАналитики: `<dcscor:value xsi:type="xs:boolean">true`).
            "xs:boolean" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Boolean(v.text.clone())
            }
            // Шрифт-параметр оформления (ERP-witness ЗадачиПоЭкземпляруБюджета.ФормаСписка:
            // `<dcscor:value xsi:type="v8ui:Font" ref="sys:DefaultGUIFont" strikeout="true" …/>`)
            // — те же font-атрибуты, что у `<Font>` контролов (общий read_designer_font).
            "v8ui:Font" => {
                xt.claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Font(read_designer_font(v)?)
            }
            // Строковый параметр оформления (ERP-witness ОтклоненияВСтоимостиТоваров:
            // CalculatedField appearance Формат=`ЧДЦ=2` ⟷ EDT `core:StringValue`).
            "xs:string" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Str(v.text.clone())
            }
            "v8:LocalStringType" => {
                xt.claimed.set(true);
                match read_designer_title_pairs(v)? {
                    PropertyValue::Localized(pairs) => DcsCorValue::LocalString(pairs),
                    other => {
                        return Err(FormError::Frame(format!(
                            "dcscor:value v8:LocalStringType: unexpected {other:?} (§1.0)"
                        )))
                    }
                }
            }
            // ERP-witness МероприятияТрудовойДеятельности.ФормаВыбораСобытий: параметр-поле
            // (`Текст`=`ОписаниеДолжности`, `xsi:type="dcscor:Field"`).
            "dcscor:Field" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Field(v.text.clone())
            }
            // ERP-witness ПечатьЭтикетокИЦенников.ВыборВариантаЗаполненияУпаковками:
            // горизонтальное положение (`xsi:type="v8ui:HorizontalAlign"`, напр. `Justify`).
            "v8ui:HorizontalAlign" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::HorizontalAlign(v.text.clone())
            }
            // ERP-witness СреднийЗаработокСЭДО.ФормаСписка dataParameter `Год`
            // (`xsi:type="xs:decimal"`, напр. `0`).
            "xs:decimal" => {
                xt.claimed.set(true);
                v.text_claimed.set(true);
                expect_no_children(v)?;
                DcsCorValue::Decimal(v.text.clone())
            }
            other => {
                return Err(FormError::Frame(format!(
                    "dcscor:value xsi:type={other:?}: unmodeled (§1.0)"
                )))
            }
        }
    }
        }),
    };
    let user_setting_id = dcsset_leaf_text_opt(it, "userSettingID");
    for c in &it.children {
        let ok = (c.prefix == "dcscor"
            && matches!(c.local.as_str(), "use" | "parameter" | "value"))
            || (c.prefix == "dcsset" && c.local == "userSettingID");
        if !ok {
            return Err(FormError::Frame(format!(
                "SettingsParameterValue: unexpected child <{}:{}> (§1.0)",
                c.prefix, c.local
            )));
        }
    }
    Ok(DcsSettingsParameterValue {
        used,
        parameter,
        value,
        user_setting_id,
    })
}

/// Прочитать ОПЦИОНАЛЬНУЮ группу DCS-настроек (`dcsset:filter`/`order`/
/// `conditionalAppearance`): [items] + viewMode? + userSettingID? (каждая мета опциональна).
pub(crate) fn read_dcs_settings_group(
    parent: &Element,
    tag: &str,
) -> Result<Option<DcsSettingsGroup>, FormError> {
    let g = match parent.child(tag).filter(|c| c.prefix == "dcsset") {
        Some(g) => g,
        None => return Ok(None),
    };
    g.claim();
    let mut items = Vec::new();
    for it in g
        .children
        .iter()
        .filter(|c| c.local == "item" && c.prefix == "dcsset")
    {
        items.push(read_dcs_item(it)?);
    }
    let view_mode = dcsset_leaf_text_opt(g, "viewMode");
    let user_setting_id = dcsset_leaf_text_opt(g, "userSettingID");
    // ГРУППОВОЕ `userSettingPresentation` ПОСЛЕ userSettingID (witness ERP
    // ОтветственныеЗаАктуализациюТокенов…ФормаСписка: `<filter>` несёт xs:string-представление).
    let user_setting_presentation = read_dcs_presentation_opt(g, "userSettingPresentation")?;
    expect_only_dcsset_children(
        g,
        &["item", "viewMode", "userSettingID", "userSettingPresentation"],
    )?;
    Ok(Some(DcsSettingsGroup {
        items,
        view_mode,
        user_setting_id,
        user_setting_presentation,
    }))
}

/// Прочитать `<dcsset:item>` (диспетч по `xsi:type`; БЕЗ xsi:type — элемент УСЛОВНОГО
/// ОФОРМЛЕНИЯ). Иной тип — §1.0-ошибка.
pub(crate) fn read_dcs_item(it: &Element) -> Result<DcsItem, FormError> {
    it.claim();
    let Some(xt) = it.attr("xsi:type") else {
        // Элемент условного оформления: selection + filter + appearance.
        return read_dcs_conditional_appearance_item(it);
    };
    match xt.value.as_str() {
        "dcsset:FilterItemComparison" => {
            xt.claimed.set(true);
            let used = dcsset_bool_opt(it, "use")?;
            let left = it
                .child("left")
                .filter(|c| c.prefix == "dcsset")
                .ok_or_else(|| {
                    FormError::Frame("FilterItemComparison: no <dcsset:left> (§1.0)".into())
                })?;
            // `left` — обычно `dcscor:Field`, но 2 witness ERP несут `xs:boolean` (константный
            // операнд). Клеймим ЛЮБОЙ xsi:type, сохраняя его для byte-exact re-emit.
            let left_type = {
                let lt = left
                    .attr("xsi:type")
                    .ok_or_else(|| FormError::Frame("dcsset:left: no xsi:type (§1.0)".into()))?;
                lt.claimed.set(true);
                lt.value.clone()
            };
            left.claim_with_text();
            let left_field = left.text.clone();
            let comparison_type = dcsset_leaf_text(it, "comparisonType")?;
            // ПОВТОРЯЕМЫЙ `<dcsset:right>` (InList — ERP-witness СогласованиеЗакупки.ФормаСписка:
            // два right'а «Согласовано»/«Не согласовано») — IR несёт Vec (пусто ⇒ тега нет).
            let right = it
                .children
                .iter()
                .filter(|c| c.local == "right" && c.prefix == "dcsset")
                .map(read_dcs_right_value)
                .collect::<Result<Vec<_>, _>>()?;
            let presentation = read_dcs_presentation_opt(it, "presentation")?;
            let view_mode = dcsset_leaf_text_opt(it, "viewMode");
            let user_setting_id = dcsset_leaf_text_opt(it, "userSettingID");
            let user_setting_presentation =
                read_dcs_presentation_opt(it, "userSettingPresentation")?;
            expect_only_dcsset_children(
                it,
                &[
                    "use",
                    "left",
                    "comparisonType",
                    "right",
                    "presentation",
                    "viewMode",
                    "userSettingID",
                    "userSettingPresentation",
                ],
            )?;
            Ok(DcsItem::FilterComparison {
                used,
                left_field,
                left_type,
                comparison_type,
                right,
                presentation,
                view_mode,
                user_setting_id,
                user_setting_presentation,
            })
        }
        "dcsset:FilterItemGroup" => {
            xt.claimed.set(true);
            let used = dcsset_bool_opt(it, "use")?;
            let group_type = dcsset_leaf_text(it, "groupType")?;
            let mut items = Vec::new();
            for sub in it
                .children
                .iter()
                .filter(|c| c.local == "item" && c.prefix == "dcsset")
            {
                items.push(read_dcs_item(sub)?);
            }
            let presentation = read_dcs_presentation_opt(it, "presentation")?;
            let view_mode = dcsset_leaf_text_opt(it, "viewMode");
            expect_only_dcsset_children(
                it,
                &["use", "groupType", "item", "presentation", "viewMode"],
            )?;
            Ok(DcsItem::FilterGroup {
                used,
                group_type,
                items,
                presentation,
                view_mode,
            })
        }
        "dcsset:OrderItemAuto" => {
            xt.claimed.set(true);
            // Пустой самозакрытый элемент — витнесснут БЕЗ детей (23/23). Любой ребёнок — §1.0.
            expect_only_dcsset_children(it, &[])?;
            Ok(DcsItem::OrderAuto)
        }
        "dcsset:OrderItemField" => {
            xt.claimed.set(true);
            let used = dcsset_bool_opt(it, "use")?;
            let field = dcsset_leaf_text(it, "field")?;
            let order_type = dcsset_leaf_text(it, "orderType")?;
            let view_mode = dcsset_leaf_text_opt(it, "viewMode");
            expect_only_dcsset_children(it, &["use", "field", "orderType", "viewMode"])?;
            Ok(DcsItem::OrderField {
                used,
                field,
                order_type,
                view_mode,
            })
        }
        "dcsset:StructureItemGroup" => {
            xt.claimed.set(true);
            let gi = it
                .child("groupItems")
                .filter(|c| c.prefix == "dcsset")
                .ok_or_else(|| {
                    FormError::Frame("StructureItemGroup: no <dcsset:groupItems> (§1.0)".into())
                })?;
            gi.claim();
            let mut group_items = Vec::new();
            for sub in gi
                .children
                .iter()
                .filter(|c| c.local == "item" && c.prefix == "dcsset")
            {
                group_items.push(read_dcs_item(sub)?);
            }
            expect_only_dcsset_children(gi, &["item"])?;
            // ВЛОЖЕННЫЕ под-структуры — прямые `<dcsset:item>`-дети ПОСЛЕ `groupItems`
            // (дерево под-группировок; witness ERP ВидыЦен глубина 5). cf уплощает их в
            // `<GroupItems>`-секции (см. cf-writer); designer/edt держат вложенность.
            let mut nested = Vec::new();
            for sub in it
                .children
                .iter()
                .filter(|c| c.local == "item" && c.prefix == "dcsset")
            {
                nested.push(read_dcs_item(sub)?);
            }
            expect_only_dcsset_children(it, &["groupItems", "item"])?;
            Ok(DcsItem::StructureGroup {
                group_items,
                nested,
            })
        }
        "dcsset:GroupItemField" => {
            xt.claimed.set(true);
            let used = dcsset_bool_opt(it, "use")?;
            let field = dcsset_leaf_text(it, "field")?;
            let group_type = dcsset_leaf_text(it, "groupType")?;
            let period_addition_type = dcsset_leaf_text(it, "periodAdditionType")?;
            let period_addition_begin =
                dcsset_typed_leaf(it, "periodAdditionBegin", "xs:dateTime")?;
            let period_addition_end = dcsset_typed_leaf(it, "periodAdditionEnd", "xs:dateTime")?;
            expect_only_dcsset_children(
                it,
                &[
                    "use",
                    "field",
                    "groupType",
                    "periodAdditionType",
                    "periodAdditionBegin",
                    "periodAdditionEnd",
                ],
            )?;
            Ok(DcsItem::GroupField {
                used,
                field,
                group_type,
                period_addition_type,
                period_addition_begin,
                period_addition_end,
            })
        }
        other => Err(FormError::Frame(format!(
            "dcsset:item xsi:type={other:?}: unmodeled DCS item (§1.0)"
        ))),
    }
}

/// Прочитать элемент УСЛОВНОГО ОФОРМЛЕНИЯ (`dcsset:item` БЕЗ xsi:type): `dcsset:selection`
/// (самозакрытый ⇒ все поля; либо `dcsset:item>dcsset:field`-список), `dcsset:filter`
/// (item-ы), `dcsset:appearance` (`dcscor:item` SettingsParameterValue).
pub(crate) fn read_dcs_conditional_appearance_item(it: &Element) -> Result<DcsItem, FormError> {
    let used = dcsset_bool_opt(it, "use")?;
    let sel = it
        .child("selection")
        .filter(|c| c.prefix == "dcsset")
        .ok_or_else(|| FormError::Frame("CA item: no <dcsset:selection> (§1.0)".into()))?;
    sel.claim();
    let selection = if sel.children.is_empty() {
        None
    } else {
        let mut fields = Vec::new();
        for si in sel
            .children
            .iter()
            .filter(|c| c.local == "item" && c.prefix == "dcsset")
        {
            si.claim();
            // Поле выбора: опц. `<dcsset:use>` + `<dcsset:field>` (1 witness ERP несёт use).
            let field_used = dcsset_bool_opt(si, "use")?;
            let field = dcsset_leaf_text(si, "field")?;
            expect_only_dcsset_children(si, &["use", "field"])?;
            fields.push(morph1c_core::ir::form::DcsSelectionField {
                used: field_used,
                field,
            });
        }
        expect_only_dcsset_children(sel, &["item"])?;
        Some(fields)
    };
    let flt = it
        .child("filter")
        .filter(|c| c.prefix == "dcsset")
        .ok_or_else(|| FormError::Frame("CA item: no <dcsset:filter> (§1.0)".into()))?;
    flt.claim();
    let mut filter = Vec::new();
    for fi in flt
        .children
        .iter()
        .filter(|c| c.local == "item" && c.prefix == "dcsset")
    {
        filter.push(read_dcs_item(fi)?);
    }
    expect_only_dcsset_children(flt, &["item"])?;
    // `<dcsset:appearance>` — ОПЦИОНАЛЕН: CA-элемент может нести лишь selection+filter БЕЗ
    // оформления (ERP-witness НДССостояниеРеализации0.ФормаРабочееМесто — «плейсхолдер»-строка).
    // Отсутствие ⇒ пустой список ⇒ writer тег не эмитит (byte-exact).
    let mut appearance = Vec::new();
    if let Some(app) = it.child("appearance").filter(|c| c.prefix == "dcsset") {
        app.claim();
        for ai in app
            .children
            .iter()
            .filter(|c| c.local == "item" && c.prefix == "dcscor")
        {
            appearance.push(read_dcs_settings_parameter_value(ai)?);
        }
        for c in &app.children {
            if !(c.prefix == "dcscor" && c.local == "item") {
                return Err(FormError::Frame(format!(
                    "<dcsset:appearance>: unexpected child <{}:{}> (§1.0)",
                    c.prefix, c.local
                )));
            }
        }
    }
    let presentation = read_dcs_presentation_opt(it, "presentation")?;
    let view_mode = dcsset_leaf_text_opt(it, "viewMode");
    let user_setting_id = dcsset_leaf_text_opt(it, "userSettingID");
    expect_only_dcsset_children(
        it,
        &[
            "use",
            "selection",
            "filter",
            "appearance",
            "presentation",
            "viewMode",
            "userSettingID",
        ],
    )?;
    Ok(DcsItem::ConditionalAppearance {
        used,
        selection,
        filter,
        appearance,
        presentation,
        view_mode,
        user_setting_id,
    })
}

/// Прочитать ОПЦИОНАЛЬНОЕ `<dcsset:tag>`-представление (`presentation`/`userSettingPresentation`),
/// диспетч по `xsi:type`: `xs:string` — текст (пустой ⇒ самозакрытие), `v8:LocalStringType` —
/// `<v8:item>`-пары. Отсутствие тега ⇒ `None`. Иной `xsi:type` — §1.0-ошибка.
pub(crate) fn read_dcs_presentation_opt(
    parent: &Element,
    tag: &str,
) -> Result<Option<DcsPresentation>, FormError> {
    let el = match parent.child(tag).filter(|c| c.prefix == "dcsset") {
        Some(el) => el,
        None => return Ok(None),
    };
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<dcsset:{tag}>: no xsi:type (§1.0)")))?;
    match xt.value.as_str() {
        "xs:string" => {
            xt.claimed.set(true);
            el.text_claimed.set(true);
            if !el.children.is_empty() {
                return Err(FormError::Frame(format!(
                    "<dcsset:{tag}> xs:string: no children expected (§1.0)"
                )));
            }
            Ok(Some(DcsPresentation::Str(el.text.clone())))
        }
        "v8:LocalStringType" => {
            xt.claimed.set(true);
            match read_designer_title_pairs(el)? {
                PropertyValue::Localized(pairs) => Ok(Some(DcsPresentation::LocalString(pairs))),
                other => Err(FormError::Frame(format!(
                    "<dcsset:{tag}> v8:LocalStringType: unexpected {other:?} (§1.0)"
                ))),
            }
        }
        other => Err(FormError::Frame(format!(
            "<dcsset:{tag}> xsi:type={other:?}: unmodeled presentation (§1.0)"
        ))),
    }
}

/// URI ns типов (`v8:Type`-значение объявляет его ИНЛАЙН авто-префиксом `dNpM`).
pub(crate) const TYPES_NS_82: &str = "http://v8.1c.ru/8.2/data/types";
/// Designer-КАНОН авто-префикса types-ns (см. `dcss::TYPES_PREFIX_DESIGNER`) — под ним хранится
/// QName в IR, чтобы cf-`reprefix` его снял, а sidecar-адаптер пере-префиксовал.
pub(crate) const TYPES_PREFIX_CANON: &str = "d8p1";

/// Найти ИНЛАЙН-объявление types-ns (`xmlns:dNpM="…8.2/data/types"`) на элементе-значении
/// `v8:Type`, склеймить его и вернуть (авто-префикс `dNpM`, ЛОКАЛЬНОЕ ИМЯ типа из QName-текста).
/// Авто-префикс локус-зависим (глубина: `d6p1` у DCS-параметра, `d8p1`/`d10p1` у отбора) —
/// принимаем ЛЮБОЙ (§1.0-тотальность по URI, не по префиксу). Текст-клейм — за вызывающим.
pub(crate) fn claim_inline_type_prefix_local(el: &Element) -> Result<(String, String), FormError> {
    let decl = el
        .attrs
        .iter()
        .find(|a| a.name.starts_with("xmlns:") && a.value == TYPES_NS_82)
        .ok_or_else(|| {
            FormError::Frame("v8:Type value: no inline xmlns for …8.2/data/types (§1.0)".into())
        })?;
    decl.claimed.set(true);
    let prefix = decl.name["xmlns:".len()..].to_string();
    let local = el
        .text
        .strip_prefix(&format!("{prefix}:"))
        .ok_or_else(|| {
            FormError::Frame(format!(
                "v8:Type value text {:?}: not a {prefix:?}-QName (§1.0)",
                el.text
            ))
        })?
        .to_string();
    Ok((prefix, local))
}

/// То же, но возвращает лишь ЛОКАЛЬНОЕ ИМЯ (беспрефиксный канон DCS-параметра-значения).
pub(crate) fn claim_inline_type_qname_local(el: &Element) -> Result<String, FormError> {
    Ok(claim_inline_type_prefix_local(el)?.1)
}

/// Прочитать `<dcsset:right>` (диспетч по `xsi:type`): xs:boolean/xs:decimal/xs:string —
/// тексты; `v8:StandardBeginningDate` — `<v8:variant>` + опц. `<v8:date>`; `dcscor:Field`/
/// `dcscor:DesignTimeValue` — тексты; `v8:Type` — QName с ИНЛАЙН types-ns (ЛЮБОЙ авто-префикс).
pub(crate) fn read_dcs_right_value(right: &Element) -> Result<DcsRightValue, FormError> {
    right.claim();
    let xt = right
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("dcsset:right: no xsi:type (§1.0)".into()))?;
    let text_leaf = |right: &Element| -> Result<String, FormError> {
        right.text_claimed.set(true);
        if !right.children.is_empty() {
            return Err(FormError::Frame(
                "dcsset:right: no children expected (§1.0)".into(),
            ));
        }
        Ok(right.text.clone())
    };
    match xt.value.as_str() {
        "xs:boolean" => {
            xt.claimed.set(true);
            Ok(DcsRightValue::Boolean(text_leaf(right)?))
        }
        "xs:decimal" => {
            xt.claimed.set(true);
            Ok(DcsRightValue::Decimal(text_leaf(right)?))
        }
        "xs:string" => {
            xt.claimed.set(true);
            Ok(DcsRightValue::Str(text_leaf(right)?))
        }
        "dcscor:Field" => {
            xt.claimed.set(true);
            Ok(DcsRightValue::Field(text_leaf(right)?))
        }
        "dcscor:DesignTimeValue" => {
            xt.claimed.set(true);
            Ok(DcsRightValue::DesignTimeValue(text_leaf(right)?))
        }
        "v8:Type" => {
            // `<dcsset:right xmlns:dNpM="http://v8.1c.ru/8.2/data/types" xsi:type="v8:Type">
            // dNpM:Undefined`. Авто-префикс `dNpM` локус-зависим (глубина отбора): Взаимодействия —
            // `d8p1`, ЧекиККМ (вложенная FilterItemGroup) — `d10p1`. Принимаем ЛЮБОЙ и КАНОНИЗИРУЕМ
            // текст к Designer-канону `d8p1:<local>` — под ним sidecar-адаптер и cf-`reprefix`
            // умеют его пере-выводить (§1.0-тотальность по URI, запись пришпилена к d8p1).
            xt.claimed.set(true);
            let (_prefix, local) = claim_inline_type_prefix_local(right)?;
            let _ = text_leaf(right)?; // claim текста + запрет детей
            Ok(DcsRightValue::TypeQName(format!("{TYPES_PREFIX_CANON}:{local}")))
        }
        "v8:StandardBeginningDate" => {
            xt.claimed.set(true);
            let variant = right
                .child("variant")
                .filter(|c| c.prefix == "v8")
                .ok_or_else(|| {
                    FormError::Frame("StandardBeginningDate: no <v8:variant> (§1.0)".into())
                })?;
            claim_xsi_type(variant, "v8:StandardBeginningDateVariant")?;
            variant.claim_with_text();
            // Опц. `<v8:date>` — у варианта `Custom` (witness Взаимодействия).
            let date = right.child("date").filter(|c| c.prefix == "v8").map(|d| {
                d.claim_with_text();
                d.text.clone()
            });
            if right.children.len() != 1 + usize::from(date.is_some()) {
                return Err(FormError::Frame(
                    "StandardBeginningDate: unexpected children (§1.0)".into(),
                ));
            }
            Ok(DcsRightValue::StandardBeginningDate {
                variant: variant.text.clone(),
                date,
            })
        }
        "v8:ValueListType" => {
            // ПУСТОЙ список значений (witness ERP InList): `<v8:valueType/>` (самозакрытый) +
            // `<v8:lastId xsi:type="xs:decimal">last_id`. Непустой список (с `<v8:item>`) —
            // не витнесснут, §1.0-отказ ниже (валидируем ровно два ожидаемых ребёнка).
            xt.claimed.set(true);
            let vt = right
                .child("valueType")
                .filter(|c| c.prefix == "v8")
                .ok_or_else(|| {
                    FormError::Frame("ValueListType: no <v8:valueType> (§1.0)".into())
                })?;
            vt.claim();
            if !vt.children.is_empty() || !vt.text.is_empty() {
                return Err(FormError::Frame(
                    "ValueListType: non-empty <v8:valueType> unmodeled (§1.0)".into(),
                ));
            }
            let last = right.child("lastId").filter(|c| c.prefix == "v8").ok_or_else(|| {
                FormError::Frame("ValueListType: no <v8:lastId> (§1.0)".into())
            })?;
            claim_xsi_type(last, "xs:decimal")?;
            last.claim_with_text();
            let last_id = last.text.clone();
            // Ровно два ребёнка (valueType + lastId) — `<v8:item>`-элементы не витнесснуты.
            if right.children.len() != 2 {
                return Err(FormError::Frame(
                    "ValueListType: expected exactly <v8:valueType>+<v8:lastId> (§1.0)".into(),
                ));
            }
            Ok(DcsRightValue::ValueList { last_id })
        }
        other => Err(FormError::Frame(format!(
            "dcsset:right xsi:type={other:?}: unmodeled DCS value (§1.0)"
        ))),
    }
}

/// Текст обязательного `<dcsset:tag>`-листа (claim).
pub(crate) fn dcsset_leaf_text(parent: &Element, tag: &str) -> Result<String, FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix == "dcsset")
        .ok_or_else(|| FormError::Frame(format!("missing <dcsset:{tag}> (§1.0)")))?;
    el.claim_with_text();
    Ok(el.text.clone())
}

/// Текст ОПЦИОНАЛЬНОГО `<dcsset:tag>`-листа (claim); отсутствие ⇒ `None`.
pub(crate) fn dcsset_leaf_text_opt(parent: &Element, tag: &str) -> Option<String> {
    parent
        .child(tag)
        .filter(|c| c.prefix == "dcsset")
        .map(|el| {
            el.claim_with_text();
            el.text.clone()
        })
}

/// ОПЦИОНАЛЬНЫЙ bool из `<dcsset:tag>true|false</dcsset:tag>`; отсутствие ⇒ `None`.
pub(crate) fn dcsset_bool_opt(parent: &Element, tag: &str) -> Result<Option<bool>, FormError> {
    match dcsset_leaf_text_opt(parent, tag) {
        None => Ok(None),
        Some(s) => match s.as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            other => Err(FormError::Frame(format!(
                "<dcsset:{tag}>={other:?}, want bool (§1.0)"
            ))),
        },
    }
}

/// Текст обязательного `<dcsset:tag xsi:type="…">`-листа с claim'ом xsi:type.
pub(crate) fn dcsset_typed_leaf(parent: &Element, tag: &str, xsi: &str) -> Result<String, FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix == "dcsset")
        .ok_or_else(|| FormError::Frame(format!("missing <dcsset:{tag}> (§1.0)")))?;
    claim_xsi_type(el, xsi)?;
    el.claim_with_text();
    Ok(el.text.clone())
}

/// Claim `xsi:type`-атрибут с ОЖИДАЕМЫМ значением; иное — §1.0-ошибка.
pub(crate) fn claim_xsi_type(el: &Element, want: &str) -> Result<(), FormError> {
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{}>: no xsi:type (§1.0)", el.local)))?;
    if xt.value != want {
        return Err(FormError::Frame(format!(
            "<{}> xsi:type={:?}, want {want:?} (§1.0)",
            el.local, xt.value
        )));
    }
    xt.claimed.set(true);
    Ok(())
}

/// Сверить, что у `el` нет НЕОЖИДАННЫХ детей в ns `dcsset` (кроме `allowed` local-names).
pub(crate) fn expect_only_dcsset_children(el: &Element, allowed: &[&str]) -> Result<(), FormError> {
    for c in &el.children {
        if !(c.prefix == "dcsset" && allowed.contains(&c.local.as_str())) {
            return Err(FormError::Frame(format!(
                "<dcsset:{}>: unexpected child <{}:{}> (§1.0)",
                el.local, c.prefix, c.local
            )));
        }
    }
    Ok(())
}

