//! READ · Designer dialect reader — DCS schema fields/params (`dcssch:` ns) of a
//! dynamic-list attribute, mirror of [`super::edt_dcs`] on the Designer input shape.

use super::*;

/// Прочитать Designer `<CalculatedField>` динсписка → [`DcsCalculatedField`]: dcssch:dataPath,
/// dcssch:expression, опц. dcssch:title/useRestriction/presentationExpression/orderExpression*/
/// appearance/valueType (witness НеудаленныеОбъекты; ERP ОперацииСПодключаемымОборудованием).
pub(crate) fn read_designer_dcs_calculated_field(cf: &Element) -> Result<DcsCalculatedField, FormError> {
    cf.claim();
    let data_path = dcssch_leaf_text(cf, "dataPath")?;
    let expression = dcssch_leaf_text(cf, "expression")?;
    let title = match cf.child("title").filter(|c| c.prefix == "dcssch") {
        Some(t) => Some(read_designer_dcssch_title(t)?),
        None => None,
    };
    let use_restriction = read_designer_use_restriction(cf)?;
    // presentationExpression — ⟷ EDT presentationExpression (ERP-witness ОперацииСПодключаемымОборудованием).
    let presentation_expression = cf
        .child("presentationExpression")
        .filter(|c| c.prefix == "dcssch")
        .map(|c| {
            c.claim_with_text();
            c.text.clone()
        });
    // orderExpression* — выражения упорядочивания (⟷ EDT orderExpression; тот же witness).
    let mut order_expressions = Vec::new();
    for oe in cf
        .children
        .iter()
        .filter(|c| c.local == "orderExpression" && c.prefix == "dcssch")
    {
        order_expressions.push(read_designer_dcs_order_expression(oe)?);
    }
    // appearance — общий ридер (ERP-witness ОтклоненияВСтоимостиТоваров).
    let appearance = read_designer_dcs_appearance(cf)?;
    // valueType — тип поля (тот же локус, что у DCS-параметра/DCS-поля; witness — `v8:TypeId`).
    let value_type = read_dcssch_value_type(cf)?;
    let available_values = cf.children.iter().filter(|c| c.local == "availableValue" && c.prefix == "dcssch").map(read_designer_dcs_available_value).collect::<Result<Vec<_>, _>>()?;
    expect_only_dcssch_children(
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
            "availableValue",
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

/// Прочитать Designer `<dcssch:appearance>` DCS-поля/вычисляемого-поля → список
/// [`DcsSettingsParameterValue`] (`dcscor:item xsi:type="dcsset:SettingsParameterValue"` —
/// ОБЩИЙ ридер параметров оформления, как у УО). Отсутствие тега / без items ⇒ пустой список.
pub(crate) fn read_designer_dcs_appearance(
    host: &Element,
) -> Result<Vec<DcsSettingsParameterValue>, FormError> {
    let mut appearance = Vec::new();
    if let Some(app) = host.child("appearance").filter(|c| c.prefix == "dcssch") {
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
                    "<dcssch:appearance>: unexpected child <{}:{}> (§1.0)",
                    c.prefix, c.local
                )));
            }
        }
    }
    Ok(appearance)
}

/// URI ns DCS-common (`orderExpression`-дети Designer объявляют его ДЕФОЛТНЫМ на КАЖДОМ ребёнке).
pub(crate) const DCS_COMMON_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/common";

/// Прочитать Designer `<dcssch:orderExpression>` → [`DcsOrderExpression`]. Дети сидят в
/// ns `…/data-composition-system/common` (ДЕФОЛТНЫЙ xmlns-оверрайд на КАЖДОМ ребёнке, prefix=""):
/// `<expression>` (обяз.), `<orderType>` (дефолт Asc), `<autoOrder>` (дефолт false). Инлайн
/// xmlns-объявление КАЖДОГО ребёнка клеймится (§1.0-тотальность). Witness ОперацииСПодключаемымОборудованием.
pub(crate) fn read_designer_dcs_order_expression(oe: &Element) -> Result<DcsOrderExpression, FormError> {
    oe.claim();
    /// Найти common-ns ребёнка (prefix="", с инлайн xmlns), склеймить его xmlns-объявление и текст.
    fn common_child<'a>(oe: &'a Element, local: &str) -> Option<&'a Element> {
        oe.children
            .iter()
            .find(|c| c.local == local && c.prefix.is_empty())
    }
    let claim_common_ns = |c: &Element| {
        for a in &c.attrs {
            if a.name == "xmlns" && a.value == DCS_COMMON_NS_URI {
                a.claimed.set(true);
            }
        }
    };
    let expr_el = common_child(oe, "expression")
        .ok_or_else(|| FormError::Frame("<dcssch:orderExpression>: no <expression> (§1.0)".into()))?;
    claim_common_ns(expr_el);
    expr_el.claim_with_text();
    let expression = expr_el.text.clone();
    let order_type = match common_child(oe, "orderType") {
        Some(v) => {
            claim_common_ns(v);
            v.claim_with_text();
            v.text.clone()
        }
        None => "Asc".to_string(),
    };
    let auto_order = match common_child(oe, "autoOrder") {
        Some(v) => {
            claim_common_ns(v);
            v.claim_with_text();
            match v.text.as_str() {
                "true" => true,
                "false" => false,
                other => return Err(FormError::Frame(format!("DCS autoOrder={other:?}: want boolean"))),
            }
        }
        None => false,
    };
    // Дети — беспрефиксные (common-ns оверрайд); проверяем состав по local-name.
    for c in &oe.children {
        if !(c.prefix.is_empty()
            && matches!(c.local.as_str(), "expression" | "orderType" | "autoOrder"))
        {
            return Err(FormError::Frame(format!(
                "<dcssch:orderExpression>: unexpected child <{}:{}> (§1.0)",
                c.prefix, c.local
            )));
        }
    }
    if !matches!(order_type.as_str(), "Asc" | "Desc") {
        return Err(FormError::Frame(format!("DCS orderType={order_type:?}: unmodeled direction")));
    }
    Ok(DcsOrderExpression {
        expression,
        order_type,
        auto_order,
    })
}

/// Прочитать Designer `<Field xsi:type="dcssch:DataSetFieldField|
/// dcssch:DataSetFieldNestedDataSet">` → [`DcsField`]: dcssch:dataPath, dcssch:field, опц.
/// dcssch:title, опц. dcssch:useRestriction / dcssch:attributeUseRestriction (ветки).
pub(crate) fn read_designer_dcs_field(f: &Element) -> Result<DcsField, FormError> {
    f.claim();
    let xt = f
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("<Field>: no xsi:type (§1.0)".into()))?;
    let nested = match xt.value.as_str() {
        "dcssch:DataSetFieldField" => false,
        "dcssch:DataSetFieldNestedDataSet" => true,
        other => {
            return Err(FormError::Frame(format!(
                "<Field> xsi:type={other:?}: unmodeled DCS field (§1.0)"
            )))
        }
    };
    xt.claimed.set(true);
    let data_path = dcssch_leaf_text(f, "dataPath")?;
    let field = dcssch_leaf_text(f, "field")?;
    // presentationExpression — выражение представления (опц., за `field`; witness
    // ДокументыПоДоговору; ⟷ EDT `<presentationExpression>`).
    let presentation_expression = f
        .child("presentationExpression")
        .filter(|c| c.prefix == "dcssch")
        .map(|c| {
            c.claim_with_text();
            c.text.clone()
        });
    let title = match f.child("title").filter(|c| c.prefix == "dcssch") {
        Some(t) => Some(read_designer_dcssch_title(t)?),
        None => None,
    };
    // valueType — тип поля (опц.; ERP-witness ВыручкаИСебестоимостьПродаж.ФормаСписка:
    // `<dcssch:valueType><v8:TypeId>{uuid}` ⟷ EDT `<valueType><types>{uuid}`; общий type-codec).
    let value_type = read_dcssch_value_type(f)?;
    let use_restriction = read_designer_use_restriction_tag(f, "useRestriction")?;
    let attribute_use_restriction =
        read_designer_use_restriction_tag(f, "attributeUseRestriction")?;
    // appearance — оформление поля (общий ридер; ERP-witness СостоянияCDNПлощадокИСМП).
    let appearance = read_designer_dcs_appearance(f)?;
    // availableValue* — доступные значения поля (ПОВТОРЯЕМЫЙ; ERP-witness СчетНаОплатуКлиенту).
    let mut available_values = Vec::new();
    for av in f
        .children
        .iter()
        .filter(|c| c.local == "availableValue" && c.prefix == "dcssch")
    {
        available_values.push(read_designer_dcs_available_value(av)?);
    }
    let order_expressions = f.children.iter().filter(|c| c.local == "orderExpression" && c.prefix == "dcssch").map(read_designer_dcs_order_expression).collect::<Result<Vec<_>, _>>()?;
    expect_only_dcssch_children(
        f,
        &[
            "dataPath",
            "field",
            "presentationExpression",
            "title",
            "orderExpression",
            "valueType",
            "useRestriction",
            "attributeUseRestriction",
            "appearance",
            "availableValue",
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

/// Прочитать Designer `<dcssch:availableValue>` → [`DcsAvailableValue`]: `<dcssch:value xsi:type>`
/// + опц. `<dcssch:presentation>` (title-подобное). Witness СчетНаОплатуКлиенту `Состояние`.
pub(crate) fn read_designer_dcs_available_value(av: &Element) -> Result<DcsAvailableValue, FormError> {
    av.claim();
    let value_el = av
        .child("value")
        .filter(|c| c.prefix == "dcssch")
        .ok_or_else(|| FormError::Frame("<dcssch:availableValue>: no <dcssch:value> (§1.0)".into()))?;
    let value = read_designer_dcs_param_value(value_el)?;
    let presentation = match av.child("presentation").filter(|c| c.prefix == "dcssch") {
        Some(p) => Some(read_designer_dcssch_title(p)?),
        None => None,
    };
    expect_only_dcssch_children(av, &["value", "presentation"])?;
    Ok(DcsAvailableValue {
        value,
        presentation,
    })
}

/// Прочитать Designer ограничение использования DCS-поля (`<dcssch:useRestriction>`/
/// `<dcssch:attributeUseRestriction>` — ВЕТКА с presence-bools `dcssch:field`/`condition`/
/// `group`/`order`) → `Some(DcsUseRestriction)`; отсутствие ⇒ `None`.
pub(crate) fn read_designer_use_restriction_tag(
    parent: &Element,
    tag: &str,
) -> Result<Option<DcsUseRestriction>, FormError> {
    let el = match parent.child(tag).filter(|c| c.prefix == "dcssch") {
        Some(el) => el,
        None => return Ok(None),
    };
    el.claim();
    let r = DcsUseRestriction {
        field: read_dcssch_presence_true(el, "field")?,
        condition: read_dcssch_presence_true(el, "condition")?,
        group: read_dcssch_presence_true(el, "group")?,
        order: read_dcssch_presence_true(el, "order")?,
    };
    expect_only_dcssch_children(el, &["field", "condition", "group", "order"])?;
    Ok(Some(r))
}

/// То же для `<CalculatedField>` (несёт лишь `useRestriction`).
pub(crate) fn read_designer_use_restriction(parent: &Element) -> Result<Option<DcsUseRestriction>, FormError> {
    read_designer_use_restriction_tag(parent, "useRestriction")
}

/// Прочитать Designer `<Parameter>` DCS-схемы → [`DcsParameter`].
pub(crate) fn read_designer_dcs_parameter(p: &Element) -> Result<DcsParameter, FormError> {
    p.claim();
    let name = dcssch_leaf_text(p, "name")?;
    let title = match p.child("title").filter(|c| c.prefix == "dcssch") {
        Some(t) => Some(read_designer_dcssch_title(t)?),
        None => None,
    };
    let value_type = read_dcssch_value_type(p)?;
    // `<dcssch:value>` — ОПЦИОНАЛЕН (8 параметров ВариантыОтчетов несут лишь name/title/флаги).
    // Вариации: `xsi:nil="true"` (Undefined) / `xs:boolean` / `xs:string` (пустой ⇒
    // самозакрытие) / `xs:dateTime` / `v8:UUID` — тексты.
    let mut values = p.children.iter()
        .filter(|c| c.local == "value" && c.prefix == "dcssch")
        .map(read_designer_dcs_param_value).collect::<Result<Vec<_>, _>>()?;
    let value = if values.is_empty() { None } else { Some(values.remove(0)) };
    let additional_values = values;
    // useRestriction — три-состояние: Designer НЕРЕГУЛЯРНО эмитит явный `false`
    // (SSL absent×1492/true×72/false×19; mixed внутри одной формы — МашиночитаемыеДоверенности)
    // ⇒ presence-точное чтение {true→Some(true), false→Some(false), absent→None}.
    let use_restriction = match p.child("useRestriction").filter(|c| c.prefix == "dcssch") {
        Some(v) => {
            v.claim_with_text();
            match v.text.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                other => {
                    return Err(FormError::Frame(format!(
                        "<dcssch:useRestriction>={other:?}, want bool (§1.0)"
                    )))
                }
            }
        }
        None => None,
    };
    let value_list_allowed = read_dcssch_presence_true(p, "valueListAllowed")?;
    let available_as_field = read_designer_opt_bool(p, "availableAsField")?;
    let expression = p.child("expression").filter(|c| c.prefix == "dcssch").map(|c| { c.claim_with_text(); c.text.clone() });
    let usage = match p.child("use").filter(|c| c.prefix == "dcssch") {
        None => None,
        Some(c) => {
            c.claim_with_text(); expect_no_children(c)?;
            match c.text.as_str() {
                "Always" => Some(morph1c_core::ir::form::DcsParameterUse::Always),
                other => return Err(FormError::Frame(format!("DCS parameter use={other:?}: unmodeled usage"))),
            }
        }
    };
    expect_only_dcssch_children(
        p,
        &[
            "name",
            "title",
            "valueType",
            "value",
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

/// Прочитать Designer DCS-значение `<dcssch:value xsi:nil|xsi:type>` → [`DcsParamValue`]. Общий
/// декодер: DCS-параметр И доступное значение поля (`<dcssch:value>`; witness СчетНаОплатуКлиенту).
/// `xsi:nil="true"` ⇒ Undefined; иначе диспатч по `xsi:type` (xs:boolean/xs:string/xs:dateTime/
/// v8:UUID/xs:decimal/v8:Type). Текст-несущий лист без детей.
pub(crate) fn read_designer_dcs_param_value(v: &Element) -> Result<DcsParamValue, FormError> {
    v.claim();
    Ok(if let Some(nil) = v.attr("xsi:nil") {
        if nil.value != "true" {
            return Err(FormError::Frame(format!(
                "dcssch:value xsi:nil={:?} (§1.0)",
                nil.value
            )));
        }
        nil.claimed.set(true);
        DcsParamValue::Undefined
    } else {
        let xt = v
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("dcssch:value: no xsi:type (§1.0)".into()))?;
        expect_no_children(v)?;
        v.text_claimed.set(true);
        let out = match xt.value.as_str() {
            "dcscor:DesignTimeValue" => DcsParamValue::DesignTimeValue(v.text.clone()),
            "xs:boolean" => DcsParamValue::Boolean(v.text.clone()),
            "xs:string" => DcsParamValue::Str(v.text.clone()),
            "xs:dateTime" => DcsParamValue::Date(v.text.clone()),
            "v8:UUID" => DcsParamValue::Uuid(v.text.clone()),
            // ERP-witness Международный.ФормаСписка (⟷ EDT core:NumberValue).
            "xs:decimal" => DcsParamValue::Decimal(v.text.clone()),
            // Значение-ТИП ⟷ EDT `core:TypeValue` (witness КлючиРеестраДокументов.ФормаВыбора):
            // инлайн `xmlns:dNpM="…8.2/data/types"` + текст `dNpM:Undefined`. Авто-префикс
            // локус-зависим (d6p1 у DCS-параметра) ⇒ принимаем ЛЮБОЙ, храним беспрефиксное имя.
            "v8:Type" => DcsParamValue::TypeValue(claim_inline_type_qname_local(v)?),
            other => {
                return Err(FormError::Frame(format!(
                    "dcssch:value xsi:type={other:?}: unmodeled DCS value (§1.0)"
                )))
            }
        };
        xt.claimed.set(true);
        out
    })
}

/// Прочитать Designer `<dcssch:valueType>` → тип (present-empty ⇒ `None`). Type-codec.
pub(crate) fn read_dcssch_value_type(p: &Element) -> Result<Option<TypeSpec>, FormError> {
    let host = match p.child("valueType").filter(|c| c.prefix == "dcssch") {
        Some(h) => h,
        None => return Ok(None),
    };
    host.claim();
    type_codec::claim(crate::TypeDialect::Designer, host);
    let v = type_codec::decode(crate::TypeDialect::Designer, host).map_err(FormError::Frame)?;
    match v {
        PropertyValue::Type(ts) if ts.parts.is_empty() => Ok(None),
        PropertyValue::Type(ts) => Ok(Some(ts)),
        other => Err(FormError::Frame(format!(
            "dcssch:valueType: not a Type: {other:?}"
        ))),
    }
}

/// Прочитать Designer DCS-заголовок: `<dcssch:title xsi:type="v8:LocalStringType"><v8:item>…`
/// → `Localized`; либо `<dcssch:title xsi:type="xs:string">текст` → `Str` (witness
/// НаборыДополнительныхРеквизитовИСведений «Показать»).
pub(crate) fn read_designer_dcssch_title(t: &Element) -> Result<PropertyValue, FormError> {
    let xt = t
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("<title>: no xsi:type (§1.0)".into()))?;
    match xt.value.as_str() {
        "v8:LocalStringType" => {
            xt.claimed.set(true);
            read_designer_title_pairs(t) // читает <v8:item>-пары и клеймит t
        }
        "xs:string" => {
            xt.claimed.set(true);
            t.claim();
            t.text_claimed.set(true);
            expect_no_children(t)?;
            Ok(PropertyValue::Str(t.text.clone()))
        }
        other => Err(FormError::Frame(format!(
            "<title> xsi:type={other:?}, want \"v8:LocalStringType\" (§1.0)"
        ))),
    }
}

/// Сверить, что у `el` нет НЕОЖИДАННЫХ детей в ns `dcssch` (кроме `allowed` local-names).
pub(crate) fn expect_only_dcssch_children(el: &Element, allowed: &[&str]) -> Result<(), FormError> {
    for c in &el.children {
        if !(c.prefix == "dcssch" && allowed.contains(&c.local.as_str())) {
            return Err(FormError::Frame(format!(
                "<{}>: unexpected child <{}:{}> (§1.0)",
                el.local, c.prefix, c.local
            )));
        }
    }
    Ok(())
}

/// Текст обязательного `<dcssch:tag>`-листа (claim).
pub(crate) fn dcssch_leaf_text(parent: &Element, tag: &str) -> Result<String, FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix == "dcssch")
        .ok_or_else(|| FormError::Frame(format!("missing <dcssch:{tag}> (§1.0)")))?;
    el.claim_with_text();
    Ok(el.text.clone())
}

/// presence-true bool в ns `dcssch`: `<dcssch:tag>true</dcssch:tag>` ⇒ `true`; отсутствие ⇒
/// `false`; иное — §1.0-ошибка.
pub(crate) fn read_dcssch_presence_true(el: &Element, tag: &str) -> Result<bool, FormError> {
    match el.child(tag).filter(|c| c.prefix == "dcssch") {
        Some(v) => {
            v.claim_with_text();
            if v.text != "true" {
                return Err(FormError::Frame(format!(
                    "<dcssch:{tag}>={:?}, want true (§1.0)",
                    v.text
                )));
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

