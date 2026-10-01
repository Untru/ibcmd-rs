//! READ · EDT dialect reader — DCS schema fields/params of a dynamic-list attribute
//! (`<fields>`/`<calculatedFields>`/`<parameters>` in `schema:`/`core:`/`settings:` ns).

use super::*;

/// Прочитать EDT `<fields xsi:type="schema:DataCompositionSchemaDataSetField|
/// …NestedDataSet">` → [`DcsField`]: dataPath, field, опц. title, опц. useRestriction /
/// attributeUseRestriction.
pub(crate) fn read_edt_dcs_field(f: &Element) -> Result<DcsField, FormError> {
    f.claim();
    let xt = f
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("<fields>: no xsi:type (§1.0)".into()))?;
    let nested = match xt.value.as_str() {
        "schema:DataCompositionSchemaDataSetField" => false,
        "schema:DataCompositionSchemaNestedDataSet" => true,
        other => {
            return Err(FormError::Frame(format!(
                "<fields> xsi:type={other:?}: unmodeled DCS field (§1.0)"
            )))
        }
    };
    xt.claimed.set(true);
    let data_path = leaf_text(f, "dataPath")?;
    let field = leaf_text(f, "field")?;
    // presentationExpression — выражение представления (опц., за `field`; witness
    // ДокументыПоДоговору `НСтр(Дополнительно)`; ⟷ Designer `<dcssch:presentationExpression>`).
    let presentation_expression = f
        .child("presentationExpression")
        .filter(|c| c.prefix.is_empty())
        .map(|c| {
            c.claim_with_text();
            c.text.clone()
        });
    let title = match f.child("title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_edt_dcs_title(t)?),
        None => None,
    };
    // valueType — тип поля (опц.; ⟷ Designer `<dcssch:valueType>`; общий type-codec).
    let value_type = match f.child("valueType").filter(|c| c.prefix.is_empty()) {
        Some(_) => read_value_type(f, "valueType", crate::TypeDialect::Edt)?,
        None => None,
    };
    let use_restriction = read_edt_use_restriction(f, "useRestriction")?;
    let attribute_use_restriction = read_edt_use_restriction(f, "attributeUseRestriction")?;
    // appearance — оформление поля (общий ридер; ERP-witness СостоянияCDNПлощадокИСМП `Формат=ДЛФ=DT`).
    let appearance = read_edt_dcs_appearance(f)?;
    // availableValues* — доступные значения поля (ПОВТОРЯЕМЫЙ; ERP-witness СчетНаОплатуКлиенту).
    let mut available_values = Vec::new();
    for av in f
        .children
        .iter()
        .filter(|c| c.local == "availableValues" && c.prefix.is_empty())
    {
        available_values.push(read_edt_dcs_available_value(av)?);
    }
    let order_expressions = f.children.iter().filter(|c| c.local == "orderExpressions" && c.prefix == "").map(read_edt_dcs_order_expression).collect::<Result<Vec<_>, _>>()?;
    expect_only_children(
        f,
        &[
            "dataPath",
            "field",
            "presentationExpression",
            "title",
            "orderExpressions",
            "valueType",
            "useRestriction",
            "attributeUseRestriction",
            "appearance",
            "availableValues",
        ],
    )?;
    Ok(DcsField {
        order_expressions,
        nested,
        data_path,
        field,
        presentation_expression,
        title,
        value_type,
        use_restriction,
        attribute_use_restriction,
        appearance,
        available_values,
    })
}

/// Прочитать EDT `<availableValues>` DCS-поля → [`DcsAvailableValue`]: `<value xsi:type>` +
/// опц. `<presentation><localValue>` (title-подобное). Witness СчетНаОплатуКлиенту `Состояние`.
pub(crate) fn read_edt_dcs_available_value(av: &Element) -> Result<DcsAvailableValue, FormError> {
    av.claim();
    let value_el = av
        .child("value")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("<availableValues>: no <value> (§1.0)".into()))?;
    let value = read_edt_dcs_param_value(value_el)?;
    let presentation = match av.child("presentation").filter(|c| c.prefix.is_empty()) {
        Some(p) => Some(read_edt_dcs_title(p)?),
        None => None,
    };
    expect_only_children(av, &["value", "presentation"])?;
    Ok(DcsAvailableValue {
        value,
        presentation,
    })
}

/// Прочитать EDT `<calculatedFields>` динсписка → [`DcsCalculatedField`]: dataPath,
/// expression, опц. title, опц. useRestriction/presentationExpression/orderExpression*/
/// appearance/valueType (witness НеудаленныеОбъекты; ERP ОперацииСПодключаемымОборудованием).
pub(crate) fn read_edt_dcs_calculated_field(cf: &Element) -> Result<DcsCalculatedField, FormError> {
    cf.claim();
    let data_path = leaf_text(cf, "dataPath")?;
    let expression = leaf_text(cf, "expression")?;
    let title = match cf.child("title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_edt_dcs_title(t)?),
        None => None,
    };
    let use_restriction = read_edt_use_restriction(cf, "useRestriction")?;
    // presentationExpression — выражение представления (⟷ Designer dcssch:presentationExpression;
    // ERP-witness ОперацииСПодключаемымОборудованием).
    let presentation_expression = cf
        .child("presentationExpression")
        .filter(|c| c.prefix.is_empty())
        .map(|c| {
            c.claim_with_text();
            c.text.clone()
        });
    // orderExpression* — выражения упорядочивания (⟷ Designer dcssch:orderExpression; тот же
    // witness). EDT несёт лишь `<expression>` (orderType/autoOrder — дефолты, опущены).
    let mut order_expressions = Vec::new();
    for oe in cf
        .children
        .iter()
        .filter(|c| c.local == "orderExpression" && c.prefix.is_empty())
    {
        order_expressions.push(read_edt_dcs_order_expression(oe)?);
    }
    // appearance — EDT items-зеркало (общий ридер с DCS-полем; ERP-witness ОтклоненияВСтоимостиТоваров).
    let appearance = read_edt_dcs_appearance(cf)?;
    // valueType — тип поля (⟷ Designer dcssch:valueType; общий type-codec).
    let value_type = match cf.child("valueType").filter(|c| c.prefix.is_empty()) {
        Some(_) => read_value_type(cf, "valueType", crate::TypeDialect::Edt)?,
        None => None,
    };
    let available_values = cf.children.iter().filter(|c| c.local == "availableValues" && c.prefix == "").map(read_edt_dcs_available_value).collect::<Result<Vec<_>, _>>()?;
    expect_only_children(
        cf,
        &[
            "dataPath",
            "expression",
            "title",
            "useRestriction",
            "presentationExpression",
            "orderExpression",
            "appearance",
            "valueType",
            "availableValues",
        ],
    )?;
    Ok(DcsCalculatedField {
        available_values,
        data_path,
        expression,
        title,
        use_restriction,
        presentation_expression,
        order_expressions,
        appearance,
        value_type,
    })
}

/// Прочитать EDT `<appearance>` DCS-поля/вычисляемого-поля → список
/// [`DcsSettingsParameterValue`]. Форма: `<appearance><items xsi:type=
/// "settings:SettingsParameterValue"><parameter><value>Имя</value></parameter>
/// <values xsi:type="core:StringValue"><value>…</value></values></items></appearance>`
/// (ERP-witness ОтклоненияВСтоимостиТоваров / СостоянияCDNПлощадокИСМП). Витнесснут ТОЛЬКО
/// `core:StringValue` — иные `<values>`-xsi §1.0. Отсутствие тега / без items ⇒ пустой список.
pub(crate) fn read_edt_dcs_appearance(host: &Element) -> Result<Vec<DcsSettingsParameterValue>, FormError> {
    let mut appearance = Vec::new();
    if let Some(app) = host.child("appearance").filter(|c| c.prefix.is_empty()) {
        app.claim();
        for ai in &app.children {
            if !(ai.local == "items" && ai.prefix.is_empty()) {
                return Err(FormError::Frame(format!(
                    "DCS <appearance>: unexpected child <{}:{}> (§1.0)",
                    ai.prefix, ai.local
                )));
            }
            let xt = ai.attr("xsi:type").ok_or_else(|| {
                FormError::Frame("DCS appearance <items>: no xsi:type (§1.0)".into())
            })?;
            if xt.value != "settings:SettingsParameterValue" {
                return Err(FormError::Frame(format!(
                    "DCS appearance <items> xsi:type={:?} (§1.0)",
                    xt.value
                )));
            }
            xt.claimed.set(true);
            ai.claim();
            let par = ai
                .child("parameter")
                .filter(|c| c.prefix.is_empty())
                .ok_or_else(|| {
                    FormError::Frame("DCS appearance: no <parameter> (§1.0)".into())
                })?;
            par.claim();
            let parameter = leaf_text(par, "value")?;
            expect_only_children(par, &["value"])?;
            let values = ai
                .child("values")
                .filter(|c| c.prefix.is_empty())
                .ok_or_else(|| FormError::Frame("DCS appearance: no <values> (§1.0)".into()))?;
            let vxt = values.attr("xsi:type").ok_or_else(|| {
                FormError::Frame("DCS appearance <values>: no xsi:type (§1.0)".into())
            })?;
            vxt.claimed.set(true);
            values.claim();
            let value = match vxt.value.as_str() {
                "core:StringValue" => DcsCorValue::Str(leaf_text(values, "value")?),
                "core:ColorValue" => {
                    let inner = values.child("value").filter(|c| c.prefix.is_empty()).ok_or_else(|| FormError::Frame("DCS ColorValue: no value".into()))?;
                    match super::super::decode_edt_color(inner, "value")? {
                        PropertyValue::Ref(c) => DcsCorValue::Color(super::super::color_to_designer(&c, "value")?),
                        other => return Err(FormError::Frame(format!("DCS ColorValue: unexpected {other:?}"))),
                    }
                }
                other => return Err(FormError::Frame(format!("DCS appearance values xsi:type={other:?}: unmodeled"))),
            };
            expect_only_children(values, &["value"])?;
            expect_only_children(ai, &["parameter", "values"])?;
            appearance.push(DcsSettingsParameterValue {
                used: None,
                parameter,
                value: Some(value),
                user_setting_id: None,
            });
        }
    }
    Ok(appearance)
}

/// Прочитать EDT `<orderExpression>` DCS-вычисляемого-поля → [`DcsOrderExpression`]. EDT несёт
/// лишь `<expression>` (orderType/autoOrder опущены при дефолте Asc/false — добиваем); если
/// присутствуют — читаем явно (⟷ Designer common-ns тройка). Витнесс ОперацииСПодключаемымОборудованием.
pub(crate) fn read_edt_dcs_order_expression(oe: &Element) -> Result<DcsOrderExpression, FormError> {
    oe.claim();
    let expression = leaf_text(oe, "expression")?;
    let order_type = match oe.child("orderType").filter(|c| c.prefix.is_empty()) {
        Some(v) => {
            v.claim_with_text();
            v.text.clone()
        }
        None => "Asc".to_string(),
    };
    let auto_order = match oe.child("autoOrder").filter(|c| c.prefix.is_empty()) {
        Some(v) => {
            v.claim_with_text();
            match v.text.as_str() {
                "true" => true,
                "false" => false,
                other => return Err(FormError::Frame(format!("DCS autoOrder={other:?}: want boolean"))),
            }
        }
        None => false,
    };
    expect_only_children(oe, &["expression", "orderType", "autoOrder"])?;
    if !matches!(order_type.as_str(), "Asc" | "Desc") {
        return Err(FormError::Frame(format!("DCS orderType={order_type:?}: unmodeled direction")));
    }
    Ok(DcsOrderExpression {
        expression,
        order_type,
        auto_order,
    })
}

/// Прочитать EDT ограничение использования DCS-поля (`<useRestriction>`/
/// `<attributeUseRestriction>` — ВЕТКА с presence-bools field/condition/group/order) →
/// `Some(DcsUseRestriction)`; отсутствие тега ⇒ `None`.
pub(crate) fn read_edt_use_restriction(
    parent: &Element,
    tag: &str,
) -> Result<Option<DcsUseRestriction>, FormError> {
    let el = match parent.child(tag).filter(|c| c.prefix.is_empty()) {
        Some(el) => el,
        None => return Ok(None),
    };
    el.claim();
    let r = DcsUseRestriction {
        field: read_presence_true(el, "field")?,
        condition: read_presence_true(el, "condition")?,
        group: read_presence_true(el, "group")?,
        order: read_presence_true(el, "order")?,
    };
    expect_only_children(el, &["field", "condition", "group", "order"])?;
    Ok(Some(r))
}

/// Прочитать EDT `<parameters>` DCS-схемы → [`DcsParameter`]: name, опц. title, valueType,
/// values, useRestriction, опц. valueListAllowed/availableAsField.
pub(crate) fn read_edt_dcs_parameter(p: &Element) -> Result<DcsParameter, FormError> {
    p.claim();
    let name = leaf_text(p, "name")?;
    let title = match p.child("title").filter(|c| c.prefix.is_empty()) {
        Some(t) => Some(read_edt_dcs_title(t)?),
        None => None,
    };
    // `<valueType>` ОПЦИОНАЛЕН: некоторые DCS-параметры несут лишь name/title/useRestriction
    // (witness ПустыеПользователи Файлы). Корпус НЕ несёт present-empty `<valueType/>`
    // (0 вхождений), поэтому absent ⇒ None ⇒ на write не эмитим (round-trip byte-exact).
    let value_type = match p.child("valueType").filter(|c| c.prefix.is_empty()) {
        Some(_) => read_value_type(p, "valueType", crate::TypeDialect::Edt)?,
        None => None,
    };
    // `<values xsi:type="core:*Value">` — значение по умолчанию. Пустые маркеры:
    // UndefinedValue / BooleanValue (булев дефолт `false` — EDT несёт лишь тип-маркер).
    // С полезной нагрузкой: StringValue/DateValue — `<value>текст</value>`-ребёнок;
    // UuidValue — `value=`-АТРИБУТ (витнессы МашиночитаемыеДоверенности/ВнешниеПользователи).
    // ОПЦИОНАЛЕН: 12 DCS-параметров корпуса несут лишь name/title/флаги без `<values>` ⇒ None.
    let mut values = p.children.iter()
        .filter(|c| c.local == "values" && c.prefix == "")
        .map(read_edt_dcs_param_value).collect::<Result<Vec<_>, _>>()?;
    let value = if values.is_empty() { None } else { Some(values.remove(0)) };
    let additional_values = values;
    // useRestriction — три-состояние (см. IR-док): EDT несёт ТОЛЬКО `true` (sparse) ⇒
    // present-true→Some(true), absent→None (Some(false) на EDT-стороне не возникает).
    let use_restriction = if read_presence_true(p, "useRestriction")? {
        Some(true)
    } else {
        None
    };
    let value_list_allowed = read_presence_true(p, "valueListAllowed")?;
    let available_as_field = read_edt_opt_bool(p, "availableAsField")?;
    let expression = p.child("expression").filter(|c| c.prefix == "").map(|c| { c.claim_with_text(); c.text.clone() });
    let usage = match p.child("use").filter(|c| c.prefix == "") {
        None => None,
        Some(c) => {
            c.claim_with_text(); expect_no_children(c)?;
            match c.text.as_str() {
                "Always" => Some(morph1c_core::ir::form::DcsParameterUse::Always),
                other => return Err(FormError::Frame(format!("DCS parameter use={other:?}: unmodeled usage"))),
            }
        }
    };
    expect_only_children(
        p,
        &[
            "name",
            "title",
            "valueType",
            "values",
            "useRestriction",
            "valueListAllowed",
            "availableAsField",
            "expression",
            "use",
        ],
    )?;
    Ok(DcsParameter {
        expression,
        usage,
        name,
        title,
        value_type,
        value,
        additional_values,
        use_restriction,
        value_list_allowed,
        available_as_field,
    })
}

/// Прочитать EDT DCS-значение `<tag xsi:type="core:*Value">` → [`DcsParamValue`]. Общий
/// декодер: DCS-параметр (`<values>`) И доступное значение поля (`<value>`; witness
/// СчетНаОплатуКлиенту). Пустые маркеры (Undefined/Boolean) — самозакрытие; String/Date/Number/
/// TypeValue — `<value>текст</value>`-ребёнок; Uuid — `value=`-АТРИБУТ.
pub(crate) fn read_edt_dcs_param_value(values: &Element) -> Result<DcsParamValue, FormError> {
    values.claim();
    let xt = values
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("<values>: no xsi:type (§1.0)".into()))?;
    xt.claimed.set(true);
    // Полезная нагрузка `<value>текст</value>` (StringValue/DateValue).
    let child_value = |values: &Element| -> Result<String, FormError> {
        let v = values
            .child("value")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("<values>: no <value> (§1.0)".into()))?;
        v.claim_with_text();
        expect_no_children(v)?;
        if values.children.len() != 1 {
            return Err(FormError::Frame(
                "<values>: expected one <value> (§1.0)".into(),
            ));
        }
        Ok(v.text.clone())
    };
    let empty = |values: &Element| -> Result<(), FormError> {
        if !values.children.is_empty() || !values.text.is_empty() {
            return Err(FormError::Frame("<values>: must be empty (§1.0)".into()));
        }
        Ok(())
    };
    let v = match xt.value.as_str() {
        "core:UndefinedValue" => {
            empty(values)?;
            DcsParamValue::Undefined
        }
        "core:BooleanValue" => {
            let text = if values.children.is_empty() { empty(values)?; "false".to_string() } else { child_value(values)? };
            if !matches!(text.as_str(), "true" | "false") { return Err(FormError::Frame("DCS BooleanValue: invalid boolean".into())); }
            DcsParamValue::Boolean(text)
        }
        "core:StringValue" => DcsParamValue::Str(child_value(values)?),
        "core:DateValue" => DcsParamValue::Date(child_value(values)?),
        // ⟷ Designer dcssch xs:decimal (ERP-witness Международный.ФормаСписка).
        "core:NumberValue" => DcsParamValue::Decimal(child_value(values)?),
        "core:UuidValue" => {
            let va = values.attr("value").ok_or_else(|| {
                FormError::Frame("core:UuidValue: no value= attr (§1.0)".into())
            })?;
            va.claimed.set(true);
            empty(values)?;
            DcsParamValue::Uuid(va.value.clone())
        }
        // Значение-ТИП: `<value>Undefined</value>`-ребёнок несёт ЛОКАЛЬНОЕ ИМЯ типа в
        // types-ns ⟷ Designer `xsi:type="v8:Type">dNpM:Undefined` (witness
        // КлючиРеестраДокументов.ФормаВыбора). Храним беспрефиксное имя (X-канон).
        "core:TypeValue" => DcsParamValue::TypeValue(child_value(values)?),
        "core_1:DesignTimeValueValue" => {
            let outer = values.child("value").filter(|c| c.prefix.is_empty()).ok_or_else(|| FormError::Frame("DesignTimeValueValue: no outer value".into()))?;
            outer.claim(); expect_only_children(values, &["value"])?;
            DcsParamValue::DesignTimeValue(child_value(outer)?)
        }
        other => {
            return Err(FormError::Frame(format!(
                "<values> xsi:type={other:?}: unmodeled DCS value (§1.0)"
            )))
        }
    };
    Ok(v)
}

/// Прочитать EDT DCS-заголовок: ЛОКАЛИЗОВАННЫЙ `<title><localValue><content><key>ru</key>
/// <value>текст</value></content></localValue></title>` → `Localized`; либо ПРОСТОЙ
/// `<title><value>текст</value></title>` → `Str` (witness
/// НаборыДополнительныхРеквизитовИСведений «Показать»).
pub(crate) fn read_edt_dcs_title(t: &Element) -> Result<PropertyValue, FormError> {
    t.claim();
    if let Some(_lv_probe) = t.child("localValue").filter(|c| c.prefix.is_empty()) {
        let lv = _lv_probe;
        lv.claim();
        // МУЛЬТИЯЗЫЧНЫЙ localValue — ПОВТОРЯЕМЫЙ `<content>` (по одному на язык; ERP-witness
        // Международный/ВыручкаИСебестоимостьПродаж.ФормаСписка: ru+en). Одноязычный SSL-случай
        // (один content) — частный случай того же цикла.
        let mut all: Vec<(morph1c_core::ir::Lang, String)> = Vec::new();
        let mut n_content = 0usize;
        for content in lv
            .children
            .iter()
            .filter(|c| c.local == "content" && c.prefix.is_empty())
        {
            n_content += 1;
            match read_edt_title(content)? {
                PropertyValue::Localized(pairs) => all.extend(pairs),
                other => {
                    return Err(FormError::Frame(format!(
                        "DCS title <content>: unexpected {other:?} (§1.0)"
                    )))
                }
            }
        }
        if n_content == 0 {
            return Err(FormError::Frame("DCS title: no <content> (§1.0)".into()));
        }
        if lv.children.len() != n_content || t.children.len() != 1 {
            return Err(FormError::Frame(
                "DCS title: expected localValue>content* (§1.0)".into(),
            ));
        }
        return Ok(PropertyValue::Localized(all));
    }
    // Простая строка: единственный `<value>текст</value>`-ребёнок.
    let v = t
        .child("value")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame("DCS title: no <localValue>/<value> (§1.0)".into()))?;
    v.claim_with_text();
    expect_no_children(v)?;
    if t.children.len() != 1 {
        return Err(FormError::Frame(
            "DCS title: expected one <value> (§1.0)".into(),
        ));
    }
    Ok(PropertyValue::Str(v.text.clone()))
}

