//! WRITE · EDT DCS schema writers (`edt_dcs_*`) — data-set fields, calculated fields,
//! parameters, appearance, order/available values, titles and the DCS value codec.

use super::*;

/// EDT `<fields xsi:type="schema:DataCompositionSchemaDataSetField|…NestedDataSet">`:
/// dataPath, field, опц. title, опц. useRestriction/attributeUseRestriction.
pub(crate) fn edt_dcs_field(f: &DcsField) -> Result<OutElement, FormError> {
    let xt = if f.nested {
        "schema:DataCompositionSchemaNestedDataSet"
    } else {
        "schema:DataCompositionSchemaDataSetField"
    };
    let mut el = OutElement::branch("", "fields").attr("xsi:type", xt);
    el.push(OutElement::leaf("", "dataPath", f.data_path.clone()));
    el.push(OutElement::leaf("", "field", f.field.clone()));
    // presentationExpression — за `field`, до title (единственный witnessed-порядок; ДокументыПоДоговору).
    if let Some(pe) = &f.presentation_expression {
        el.push(OutElement::leaf("", "presentationExpression", pe.clone()));
    }
    if let Some(t) = edt_dcs_title(&f.title) {
        el.push(t);
    }
    // valueType — после title (ERP-witness; ⟷ Designer dcssch:valueType; ошибка — громкая).
    if let Some(ts) = &f.value_type {
        el.push(
            crate::type_codec::encode(crate::TypeDialect::Edt, "", "valueType", ts)
                .map_err(FormError::Frame)?,
        );
    }
    // appearance (SC2) — ПОСЛЕ valueType (ERP-witness СостоянияCDNПлощадокИСМП: valueType→appearance).
    if let Some(app) = edt_dcs_appearance(&f.appearance)? {
        el.push(app);
    }
    // availableValues (SC1) — ПОСЛЕ appearance (ERP-witness СчетНаОплатуКлиенту: title→availableValues).
    for av in &f.available_values {
        el.push(edt_dcs_available_value(av));
    }
    if let Some(r) = &f.use_restriction {
        el.push(edt_dcs_use_restriction("useRestriction", r));
    }
    if let Some(r) = &f.attribute_use_restriction {
        el.push(edt_dcs_use_restriction("attributeUseRestriction", r));
    }
    Ok(el)
}

/// EDT `<calculatedFields>`: dataPath, expression, опц. title/useRestriction/
/// presentationExpression/orderExpression*/appearance/valueType.
pub(crate) fn edt_dcs_calculated_field(cf: &DcsCalculatedField) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "calculatedFields");
    el.push(OutElement::leaf("", "dataPath", cf.data_path.clone()));
    el.push(OutElement::leaf("", "expression", cf.expression.clone()));
    if let Some(t) = edt_dcs_title(&cf.title) {
        el.push(t);
    }
    // Порядок метамодели (см. designer-писатель): useRestriction → presentationExpression →
    // orderExpression* → appearance → valueType (ERP-witness ОперацииСПодключаемымОборудованием).
    if let Some(r) = &cf.use_restriction {
        el.push(edt_dcs_use_restriction("useRestriction", r));
    }
    if let Some(pe) = &cf.presentation_expression {
        el.push(OutElement::leaf("", "presentationExpression", pe.clone()));
    }
    for oe in &cf.order_expressions {
        el.push(edt_dcs_order_expression(oe));
    }
    if let Some(app) = edt_dcs_appearance(&cf.appearance)? {
        el.push(app);
    }
    if let Some(ts) = &cf.value_type {
        el.push(
            crate::type_codec::encode(crate::TypeDialect::Edt, "", "valueType", ts)
                .map_err(FormError::Frame)?,
        );
    }
    Ok(el)
}

/// EDT `<appearance>` DCS-поля/вычисляемого-поля (общий писатель): items-параметры. Пусто ⇒
/// `None` (тег не эмитим). EDT-локус витнесснут БЕЗ use/userSettingID и лишь `core:StringValue` —
/// иное громко отказывает (§1.0). Кодировка: `<items xsi:type="settings:SettingsParameterValue">
/// <parameter><value>Имя</value></parameter><values xsi:type="core:StringValue"><value>…`.
pub(crate) fn edt_dcs_appearance(
    items: &[DcsSettingsParameterValue],
) -> Result<Option<OutElement>, FormError> {
    if items.is_empty() {
        return Ok(None);
    }
    let mut app = OutElement::branch("", "appearance");
    for it in items {
        if it.used.is_some() || it.user_setting_id.is_some() {
            return Err(FormError::Frame(format!(
                "DCS appearance {:?}: use/userSettingID — EDT-кодировка не витнесснута (§1.0)",
                it.parameter
            )));
        }
        let mut item =
            OutElement::branch("", "items").attr("xsi:type", "settings:SettingsParameterValue");
        let mut par = OutElement::branch("", "parameter");
        par.push(OutElement::leaf("", "value", it.parameter.clone()));
        item.push(par);
        match &it.value {
            Some(DcsCorValue::Str(s)) => {
                let mut values =
                    OutElement::branch("", "values").attr("xsi:type", "core:StringValue");
                values.push(OutElement::leaf("", "value", s.clone()));
                item.push(values);
            }
            other => {
                return Err(FormError::Frame(format!(
                    "DCS appearance: EDT-кодировка значения {other:?} не витнесснута \
                     (§1.0 — witnessed core:StringValue)"
                )))
            }
        }
        app.push(item);
    }
    Ok(Some(app))
}

/// EDT `<orderExpression>`: `<expression>` всегда; `<orderType>` лишь если ≠ `Asc`; `<autoOrder>`
/// лишь если `true` (EDT опускает дефолты — witness ОперацииСПодключаемымОборудованием).
pub(crate) fn edt_dcs_order_expression(oe: &DcsOrderExpression) -> OutElement {
    let mut el = OutElement::branch("", "orderExpression");
    el.push(OutElement::leaf("", "expression", oe.expression.clone()));
    if oe.order_type != "Asc" {
        el.push(OutElement::leaf("", "orderType", oe.order_type.clone()));
    }
    if oe.auto_order {
        el.push(OutElement::leaf("", "autoOrder", "true"));
    }
    el
}

/// EDT `<availableValues>`: `<value xsi:type>` + опц. `<presentation><localValue>`
/// (title-подобное). Witness СчетНаОплатуКлиенту.
pub(crate) fn edt_dcs_available_value(av: &DcsAvailableValue) -> OutElement {
    let mut el = OutElement::branch("", "availableValues");
    el.push(edt_dcs_param_value("value", &av.value));
    if let Some(p) = &av.presentation {
        if let Some(pres) = edt_dcs_localized("presentation", p) {
            el.push(pres);
        }
    }
    el
}

/// EDT ограничение использования DCS-поля: ветка presence-bools field/condition/group/order.
pub(crate) fn edt_dcs_use_restriction(tag: &str, r: &DcsUseRestriction) -> OutElement {
    let mut el = OutElement::branch("", tag);
    for (name, v) in [
        ("field", r.field),
        ("condition", r.condition),
        ("group", r.group),
        ("order", r.order),
    ] {
        if v {
            el.push(OutElement::leaf("", name, "true"));
        }
    }
    el
}

/// EDT DCS-заголовок из IR: `Localized` ⇒ `<title><localValue><content>…`; `Str` ⇒
/// `<title><value>текст</value></title>`; `None`/пустой ⇒ не эмитим.
pub(crate) fn edt_dcs_title(title: &Option<PropertyValue>) -> Option<OutElement> {
    match title {
        Some(PropertyValue::Localized(pairs)) if !pairs.is_empty() => {
            Some(edt_dcs_local_title(pairs))
        }
        Some(PropertyValue::Str(s)) => {
            let mut t = OutElement::branch("", "title");
            t.push(OutElement::leaf("", "value", s.clone()));
            Some(t)
        }
        _ => None,
    }
}

/// EDT `<parameters>` DCS-схемы: name, опц. title, valueType, values, useRestriction, опц. флаги.
pub(crate) fn edt_dcs_parameter(p: &DcsParameter) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "parameters");
    el.push(OutElement::leaf("", "name", p.name.clone()));
    if let Some(t) = edt_dcs_title(&p.title) {
        el.push(t);
    }
    // `valueType` ОПЦИОНАЛЕН у DCS-параметра: `None` = ОТСУТСТВУЕТ (корпус не несёт
    // present-empty `<valueType/>` — 0 вхождений), поэтому не эмитим.
    if let Some(ts) = &p.value_type {
        el.push(
            crate::type_codec::encode(crate::TypeDialect::Edt, "", "valueType", ts)
                .map_err(FormError::Frame)?,
        );
    }
    // `<values xsi:type="core:*Value">` — значение; ОПЦИОНАЛЕН (absent ⇒ None). Пустые
    // маркеры (Undefined/Boolean) — самозакрытие; String/Date — `<value>текст</value>`;
    // Uuid — value=-АТРИБУТ.
    if let Some(v) = &p.value {
        el.push(edt_dcs_param_value("values", v));
    }
    // useRestriction — три-состояние (см. IR-док): EDT эмитит ТОЛЬКО `true` (Some(false) =
    // designer-явный false, EDT-аналог — омиссия).
    if p.use_restriction == Some(true) {
        el.push(OutElement::leaf("", "useRestriction", "true"));
    }
    if p.value_list_allowed {
        el.push(OutElement::leaf("", "valueListAllowed", "true"));
    }
    if let Some(b) = p.available_as_field {
        el.push(OutElement::leaf(
            "",
            "availableAsField",
            if b { "true" } else { "false" },
        ));
    }
    Ok(el)
}

/// EDT DCS-локализованный заголовок `<title><localValue><content><key>ru</key><value>…`.
/// Мультиязычный — ПОВТОРЯЕМЫЙ `<content>` по одному на язык (ERP-witness Международный/
/// ВыручкаИСебестоимостьПродаж.ФормаСписка); одноязычный SSL-случай — тот же цикл ×1.
pub(crate) fn edt_dcs_local_title(pairs: &[(Lang, String)]) -> OutElement {
    let mut lv = OutElement::branch("", "localValue");
    for (lang, text) in pairs {
        let mut content = OutElement::branch("", "content");
        content.push(OutElement::leaf("", "key", lang.as_str().to_string()));
        content.push(OutElement::leaf("", "value", text.clone()));
        lv.push(content);
    }
    let mut title = OutElement::branch("", "title");
    title.push(lv);
    title
}

/// EDT DCS-значение `<local xsi:type="core:*Value">` (общий писатель): DCS-параметр (`values`) И
/// доступное значение поля (`value`). Пустые маркеры — самозакрытие; String/Date/Number/
/// TypeValue — `<value>текст</value>`; Uuid — `value=`-АТРИБУТ.
pub(crate) fn edt_dcs_param_value(local: &str, v: &DcsParamValue) -> OutElement {
    match v {
        DcsParamValue::Undefined => {
            OutElement::self_closing("", local).attr("xsi:type", "core:UndefinedValue")
        }
        DcsParamValue::Boolean(_) => {
            OutElement::self_closing("", local).attr("xsi:type", "core:BooleanValue")
        }
        DcsParamValue::Str(s) => {
            let mut values = OutElement::branch("", local).attr("xsi:type", "core:StringValue");
            values.push(OutElement::leaf("", "value", s.clone()));
            values
        }
        DcsParamValue::Date(d) => {
            let mut values = OutElement::branch("", local).attr("xsi:type", "core:DateValue");
            values.push(OutElement::leaf("", "value", d.clone()));
            values
        }
        DcsParamValue::Uuid(u) => OutElement::self_closing("", local)
            .attr("xsi:type", "core:UuidValue")
            .attr("value", u.clone()),
        // xs:decimal ⟷ core:NumberValue (ERP-witness Международный.ФормаСписка: 0⟷0).
        DcsParamValue::Decimal(d) => {
            let mut values = OutElement::branch("", local).attr("xsi:type", "core:NumberValue");
            values.push(OutElement::leaf("", "value", d.clone()));
            values
        }
        // Значение-ТИП: `<… xsi:type="core:TypeValue"><value>Undefined</value></…>`
        // (беспрефиксное имя типа ⟷ Designer `v8:Type">dNpM:Undefined`).
        DcsParamValue::TypeValue(name) => {
            let mut values = OutElement::branch("", local).attr("xsi:type", "core:TypeValue");
            values.push(OutElement::leaf("", "value", name.clone()));
            values
        }
    }
}

/// EDT title-подобный ЛОКАЛИЗОВАННЫЙ/простой узел с ПАРАМЕТРИЗУЕМЫМ именем (`presentation`
/// availableValue): `Localized` ⇒ `<local><localValue><content>…`; `Str` ⇒ `<local><value>текст`;
/// `None`/пустой ⇒ не эмитим.
pub(crate) fn edt_dcs_localized(local: &str, p: &PropertyValue) -> Option<OutElement> {
    match p {
        PropertyValue::Localized(pairs) if !pairs.is_empty() => {
            let mut lv = OutElement::branch("", "localValue");
            for (lang, text) in pairs {
                let mut content = OutElement::branch("", "content");
                content.push(OutElement::leaf("", "key", lang.as_str().to_string()));
                content.push(OutElement::leaf("", "value", text.clone()));
                lv.push(content);
            }
            let mut el = OutElement::branch("", local);
            el.push(lv);
            Some(el)
        }
        PropertyValue::Str(s) => {
            let mut el = OutElement::branch("", local);
            el.push(OutElement::leaf("", "value", s.clone()));
            Some(el)
        }
        _ => None,
    }
}
