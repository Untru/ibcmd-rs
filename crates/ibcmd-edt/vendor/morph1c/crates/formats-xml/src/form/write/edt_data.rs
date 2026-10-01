//! WRITE · EDT data-model attributes — `<attributes>`/`<columns>` writers and the
//! dynamic-list requisite ext-info (`DynamicListExtInfo`).

use super::*;

pub(crate) fn edt_data_attribute(a: &FormDataAttribute) -> Result<OutElement, FormError> {
    edt_data_attribute_named(a, "attributes")
}

/// EDT реквизит/колонка (`<attributes>` или вложенная `<columns>`) с одинаковой структурой.
pub(crate) fn edt_data_attribute_named(a: &FormDataAttribute, tag: &str) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", tag);
    el.push(OutElement::leaf("", "name", a.name.clone()));
    if let Some(PropertyValue::Localized(pairs)) = &a.title {
        // Мультиязычный — ПОВТОРЕНИЕ `<title>` по одному на язык (ERP-witness Задание.
        // ДействиеВыполнить; та же конвенция, что toolTip команд). Одноязычный — как прежде.
        if pairs.len() > 1 {
            for pair in pairs.iter() {
                el.push(edt_title("title", std::slice::from_ref(pair)));
            }
        } else if !pairs.is_empty() {
            el.push(edt_title("title", pairs));
        }
    }
    // id=0 (реквизит-дефолт) EDT ОПУСКАЕТ; ⟺ Designer `id="0"` (см. push_edt_id).
    push_edt_id(&mut el, a.id);
    // valueType: present-empty `<valueType/>` либо Type-codec.
    match &a.value_type {
        None => el.push(OutElement::self_closing("", "valueType")),
        Some(ts) => el.push(
            crate::type_codec::encode(crate::TypeDialect::Edt, "", "valueType", ts)
                .map_err(FormError::Frame)?,
        ),
    }
    // view/edit-права: `true` ⇒ `<view><common>true</common></view>`; `false` ⇒ ПУСТОЙ
    // `<view/>` (запрещено; ⟺ Designer `<View><xr:Common>false`). Витнессы УчетныеЗаписи…/
    // Взаимодействия/ВерсииОбъектов.
    let common = |tag: &str, allowed: bool, roles: &[(String, bool)]| {
        // РОЛЕВЫЕ права — `<for><value>B</value><role>R</role></for>×N`. При common=false
        // (ERP-witness БухучетЗарплатыОрганизаций) `<common>` НЕ эмитится — false подразумевается.
        // При common=true эмитится ВЕДУЩИЙ `<common>true</common>` (ERP-witness ФизическиеЛица.
        // ФормаЭлемента `<edit><common>true</common><for>×N`; ⟷ Designer `<xr:Common>true` + роли).
        if !roles.is_empty() {
            let mut v = OutElement::branch("", tag);
            if allowed {
                v.push(OutElement::leaf("", "common", "true"));
            }
            for (role, val) in roles {
                let mut f = OutElement::branch("", "for");
                f.push(OutElement::leaf("", "value", if *val { "true" } else { "false" }));
                f.push(OutElement::leaf("", "role", role.clone()));
                v.push(f);
            }
            return v;
        }
        if !allowed {
            return OutElement::self_closing("", tag);
        }
        let mut v = OutElement::branch("", tag);
        v.push(OutElement::leaf("", "common", "true"));
        v
    };
    el.push(common("view", a.view_common, &a.view_roles));
    el.push(common("edit", a.edit_common, &a.edit_roles));
    if let Some(f) = &a.fill_checking {
        el.push(OutElement::leaf("", "fillChecking", f.clone()));
    }
    // functionalOptions (повторяемый) + notDefaultUseAlwaysAttributes (DataPath) — регион
    // между fillChecking и main (corpus fact).
    for fo in &a.functional_options {
        el.push(OutElement::leaf("", "functionalOptions", fo.clone()));
    }
    for p in &a.not_default_use_always {
        let mut s = OutElement::branch("", "notDefaultUseAlwaysAttributes")
            .attr("xsi:type", "form:DataPath");
        s.push(OutElement::leaf("", "segments", p.clone()));
        el.push(s);
    }
    // settingsSavedData — ПЕРЕД main (SSL: settingsSavedData<main×7, 0 контрпримеров;
    // witness УстановкаОбновлений.Форма «Объект»).
    for p in &a.settings_saved_data {
        let mut s = OutElement::branch("", "settingsSavedData").attr("xsi:type", "form:DataPath");
        s.push(OutElement::leaf("", "segments", p.clone()));
        el.push(s);
    }
    if a.main {
        el.push(OutElement::leaf("", "main", "true"));
    }
    // savedData — ПОСЛЕ settingsSavedData в EDT (SSL: settingsSavedData<savedData×12, 0
    // контрпримеров; напр. НастройкиАвторизацииИнтернетСервисов.ФормаЭлемента). ПРОТИВОПОЛОЖНО
    // Designer (там SavedData ПЕРЕД Save) — пер-форматная дивергенция §1.6.
    if a.saved_data {
        el.push(OutElement::leaf("", "savedData", "true"));
    }
    for col in &a.columns {
        el.push(edt_data_attribute_named(col, "columns")?);
    }
    // Доп-колонки (ValueTable): ПОВТОРЯЕМЫЕ `<additionalColumns>` (сиблинги `<columns>`) —
    // `<tablePath>` (DataPath) + вложенные `<columns>`.
    for ac in &a.additional_columns {
        let mut ace = OutElement::branch("", "additionalColumns");
        let mut tp = OutElement::branch("", "tablePath").attr("xsi:type", "form:DataPath");
        tp.push(OutElement::leaf("", "segments", ac.table_path.clone()));
        ace.push(tp);
        for col in &ac.columns {
            ace.push(edt_data_attribute_named(col, "columns")?);
        }
        el.push(ace);
    }
    // extInfo реквизита (взаимоисключающие) — последним.
    if a.value_list_ext {
        let mut ex = OutElement::branch("", "extInfo").attr("xsi:type", "form:ValueListExtInfo");
        // Пустой item-тип ⇒ самозакрытие `<itemValueType/>` (прежний корпус); заданный ⇒
        // type-codec (тот же путь, что `<valueType>`; пустой набор тоже даёт self-closing).
        let iv = match &a.value_list_item_type {
            None => OutElement::self_closing("", "itemValueType"),
            Some(ts) => crate::type_codec::encode(crate::TypeDialect::Edt, "", "itemValueType", ts)
                .map_err(FormError::Frame)?,
        };
        ex.push(iv);
        el.push(ex);
    } else if a.spreadsheet_ext {
        // form:SpreadsheetDocumentExtInfo — ПОЛНОСТЬЮ пустой (самозакрытие).
        el.push(
            OutElement::self_closing("", "extInfo")
                .attr("xsi:type", "form:SpreadsheetDocumentExtInfo"),
        );
    } else if let Some(marker) = super::super::read::scalar_marker_ext(&a.value_type) {
        // form:ChartExtInfo / form:GanttChartExtInfo / form:GraphicalSchemeExtInfo /
        // form:GeographicalSchemaExtInfo — пустые EDT-only маркеры, ПОЛНОСТЬЮ детерминированные
        // скаляр-типом (в IR не хранятся; регенерация — как spreadsheet_ext; Designer их не
        // несёт). NB: Graphical — «SchemE», Geographical — «SchemA» (оба — как в живых
        // выгрузках); GanttChart — literal (backing GanttChartField, ERP 7 форм).
        let xsi = match marker {
            "Chart" => "form:ChartExtInfo",
            "GanttChart" => "form:GanttChartExtInfo",
            "GeographicalSchema" => "form:GeographicalSchemaExtInfo",
            _ => "form:GraphicalSchemeExtInfo",
        };
        el.push(OutElement::self_closing("", "extInfo").attr("xsi:type", xsi));
    } else if let Some(dl) = &a.dynamic_list {
        el.push(edt_dynamic_list_attr(dl)?);
    }
    Ok(el)
}

/// EDT `<extInfo xsi:type="form:DynamicListExtInfo">` реквизита-динсписка. Порядок: queryText,
/// mainTable, dynamicDataRead, autoFillAvailableFields, customQuery, autoSaveUserSettings,
/// getInvisibleFieldPresentations (presence-true флаги опускают `false`), затем EXTENDED
/// fields/parameters. DCS `list_settings` EDT НЕ эмитит (сайдкар `.dcss`, вне `Form.form`).
pub(crate) fn edt_dynamic_list_attr(dl: &DynamicListAttrExt) -> Result<OutElement, FormError> {
    let mut ex = OutElement::branch("", "extInfo").attr("xsi:type", "form:DynamicListExtInfo");
    // queryText — только у РУЧНОГО запроса; mainTable — только у не-РАСШИРЕННОЙ формы (см. IR).
    if let Some(q) = &dl.query_text {
        ex.push(OutElement::leaf("", "queryText", q.clone()));
    }
    if let Some(mt) = &dl.main_table {
        ex.push(OutElement::leaf("", "mainTable", mt.clone()));
    }
    let push_flag = |ex: &mut OutElement, tag: &str, v: bool| {
        if v {
            ex.push(OutElement::leaf("", tag, "true"));
        }
    };
    push_flag(&mut ex, "dynamicDataRead", dl.dynamic_data_read);
    push_flag(
        &mut ex,
        "autoFillAvailableFields",
        dl.auto_fill_available_fields,
    );
    push_flag(&mut ex, "customQuery", dl.custom_query);
    push_flag(&mut ex, "autoSaveUserSettings", dl.auto_save_user_settings);
    push_flag(
        &mut ex,
        "getInvisibleFieldPresentations",
        dl.get_invisible_field_presentations,
    );
    // Genuine UH and BSP use schema fields, then calculated fields, then parameters.
    for f in &dl.fields {
        ex.push(edt_dcs_field(f)?);
    }
    for cf in &dl.calculated_fields {
        ex.push(edt_dcs_calculated_field(cf)?);
    }
    for p in &dl.parameters {
        ex.push(edt_dcs_parameter(p)?);
    }
    // keyType — ПЕРЕД keyField (⟷ Designer; ERP ×7). ОБА идут ПОСЛЕ fields/parameters —
    // witness ЗастрахованныеЛицаСЭДО: `…parameters, keyType(RowKey), keyField×3`; СписокДокументов:
    // `…parameters, keyField×4`. У SSL-витнесса ВыбранныеЭлементы fields/parameters пусты ⇒ позиция
    // «после флагов» == «после (пустых) параметров» ⇒ байт-идентично (SSL не трогается).
    if let Some(kt) = &dl.key_type {
        ex.push(OutElement::leaf("", "keyType", kt.clone()));
    }
    // keyField — ПОВТОРЯЕМЫЙ (по одному тегу на ключ, в исходном порядке).
    for kf in &dl.key_fields {
        ex.push(OutElement::leaf("", "keyField", kf.clone()));
    }
    Ok(ex)
}
