//! WRITE · Designer DCS writers (`designer_dcs_*`) — dynamic-list attribute, data-set
//! fields, parameters, list settings, filter/order/appearance groups and value codec.

use super::*;

/// Designer `<Settings xsi:type="DynamicList">` реквизита-динсписка. Порядок (корпус):
/// AutoFillAvailableFields? (ЛИШЬ явный false, ПЕРВЫМ), ManualQuery, DynamicDataRead,
/// QueryText?, CalculatedField*, EXTENDED Field*/Parameter*, KeyType?, KeyField* (ПОСЛЕ
/// Field/Parameter — ERP-witness), AutoSaveUserSettings? (ЛИШЬ явный false), MainTable?,
/// ListSettings?.
pub(crate) fn designer_dynamic_list_attr(
    dl: &DynamicListAttrExt,
) -> Result<OutElement, FormError> {
    let mut s = OutElement::branch("", "Settings").attr("xsi:type", "DynamicList");
    // AutoFillAvailableFields: Designer-дефолт true ⇒ эмитим лишь явный false (witness
    // АнализПравДоступа.ВыборСтрокиРегистра).
    if !dl.auto_fill_available_fields {
        s.push(OutElement::leaf("", "AutoFillAvailableFields", "false"));
    }
    // Designer эмитит ManualQuery/DynamicDataRead ВСЕГДА явным true/false (не presence-опускает).
    let bool_leaf = |v: bool| if v { "true" } else { "false" };
    s.push(OutElement::leaf(
        "",
        "ManualQuery",
        bool_leaf(dl.custom_query),
    ));
    s.push(OutElement::leaf(
        "",
        "DynamicDataRead",
        bool_leaf(dl.dynamic_data_read),
    ));
    // QueryText — только у РУЧНОГО запроса (custom_query=true); отсутствует у авто-запроса.
    if let Some(q) = &dl.query_text {
        s.push(OutElement::leaf("", "QueryText", q.clone()));
    }
    // Genuine native UH ordering: fields -> calculated fields -> parameters.
    for f in &dl.fields {
        s.push(designer_dcs_field(f)?);
    }
    for cf in &dl.calculated_fields {
        s.push(designer_dcs_calculated_field(cf)?);
    }
    for p in &dl.parameters {
        s.push(designer_dcs_parameter(p)?);
    }
    // KeyType — ПЕРЕД KeyField (ERP-witness ВыборПрисоединенногоФайла: QueryText→KeyType→KeyField).
    // ОБА идут ПОСЛЕ Field/Parameter — witness СписокДокументов/СтатусыПубликации…Ozon:
    // `…Field*, Parameter*, KeyField*, ListSettings`; ЗастрахованныеЛицаСЭДО: `…Parameter, KeyType,
    // KeyField×3`. У SSL-витнесса ВыбранныеЭлементы Field/Parameter пусты ⇒ байт-идентично.
    if let Some(kt) = &dl.key_type {
        s.push(OutElement::leaf("", "KeyType", kt.clone()));
    }
    // KeyField — ПОВТОРЯЕМЫЙ (по одному тегу на ключ, в исходном порядке).
    for kf in &dl.key_fields {
        s.push(OutElement::leaf("", "KeyField", kf.clone()));
    }
    // MainTable precedes explicit AutoSaveUserSettings (genuine UH case 40).
    if let Some(mt) = &dl.main_table {
        s.push(OutElement::leaf("", "MainTable", mt.clone()));
    }
    if !dl.auto_save_user_settings {
        s.push(OutElement::leaf("", "AutoSaveUserSettings", "false"));
    }
    // GetInvisibleFieldPresentations: Designer-дефолт true ⇒ эмитим лишь явный false
    // (ERP-witness РежимыРаботыСотрудников: MainTable→GetInvisibleFieldPresentations→ListSettings).
    if !dl.get_invisible_field_presentations {
        s.push(OutElement::leaf("", "GetInvisibleFieldPresentations", "false"));
    }
    // `<ListSettings>` несёт КАЖДЫЙ Designer-динсписок (229/229 SSL) — пустые настройки эмитятся
    // самозакрытым `<ListSettings/>` (×2 ВерсииПодсистем*). EDT ЖЕ настроек в `Form.form` не
    // держит вовсе (сайдкар `.dcss`; отсутствие файла ⇒ ПУСТЫЕ настройки ⇒ `list_settings=None`),
    // поэтому None здесь — не «нет тега», а «настройки пусты»: эмитим `<ListSettings/>`, иначе
    // edt→designer молча терял бы тег у этих двух форм.
    let empty;
    let ls = match &dl.list_settings {
        Some(ls) => ls,
        None => {
            empty = DcsListSettings::default();
            &empty
        }
    };
    s.push(designer_list_settings(ls));
    Ok(s)
}

/// Designer `<Field xsi:type="dcssch:DataSetFieldField|dcssch:DataSetFieldNestedDataSet">`:
/// dcssch:dataPath, dcssch:field, опц. title, опц. useRestriction/attributeUseRestriction.
///
/// `pub(super)`: cf-кэш `ServerState` динсписка переиспользует ЭТУ ЖЕ под-IR схему data-set'а
/// ([`super::super::dcss::write_server_state`]) — один writer на оба локуса, а не второй по образцу.
pub(crate) fn designer_dcs_field(f: &DcsField) -> Result<OutElement, FormError> {
    let xt = if f.nested {
        "dcssch:DataSetFieldNestedDataSet"
    } else {
        "dcssch:DataSetFieldField"
    };
    let mut el = OutElement::branch("", "Field").attr("xsi:type", xt);
    el.push(OutElement::leaf("dcssch", "dataPath", f.data_path.clone()));
    el.push(OutElement::leaf("dcssch", "field", f.field.clone()));
    // Independent native witness places co-present title before presentationExpression.
    // The cf ServerState schema cache uses this same writer.
    if let Some(t) = designer_dcs_title(&f.title) {
        el.push(t);
    }
    if let Some(pe) = &f.presentation_expression {
        el.push(OutElement::leaf(
            "dcssch",
            "presentationExpression",
            pe.clone(),
        ));
    }
    for oe in &f.order_expressions { el.push(designer_dcs_order_expression(oe)); }
    // valueType — после title (ERP-witness ВыручкаИСебестоимостьПродаж: dataPath→field→
    // title→valueType; тот же scoped-энкод, что у DCS-параметра; ошибка кодека — громкая).
    if let Some(ts) = &f.value_type {
        el.push(
            crate::type_codec::encode_scoped(
                crate::TypeDialect::Designer,
                "dcssch",
                "valueType",
                ts,
                DESIGNER_FORM_NS,
            )
            .map_err(FormError::Frame)?,
        );
    }
    if let Some(r) = &f.use_restriction {
        el.push(designer_dcs_use_restriction("useRestriction", r));
    }
    if let Some(r) = &f.attribute_use_restriction {
        el.push(designer_dcs_use_restriction("attributeUseRestriction", r));
    }
    // appearance (SC2) — ПОСЛЕ valueType (ERP-witness СостоянияCDNПлощадокИСМП).
    if let Some(app) = designer_dcs_appearance(&f.appearance) {
        el.push(app);
    }
    // availableValue* (SC1) — ПОСЛЕ appearance (ERP-witness СчетНаОплатуКлиенту).
    for av in &f.available_values {
        el.push(designer_dcs_available_value(av));
    }
    Ok(el)
}

/// Designer `<CalculatedField>`: dcssch:dataPath, dcssch:expression, опц. title/useRestriction/
/// presentationExpression/orderExpression*/appearance/valueType (witness НеудаленныеОбъекты;
/// ERP ОперацииСПодключаемымОборудованием).
///
/// `pub(super)` — см. [`designer_dcs_field`]. В cf-кэше тот же узел зовётся `<ExpressionField
/// xsi:type="dcssch:CalculatedField">` (переименование делает [`super::super::dcss::write_server_state`]).
pub(crate) fn designer_dcs_calculated_field(
    cf: &DcsCalculatedField,
) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "CalculatedField");
    el.push(OutElement::leaf("dcssch", "dataPath", cf.data_path.clone()));
    el.push(OutElement::leaf(
        "dcssch",
        "expression",
        cf.expression.clone(),
    ));
    if let Some(t) = designer_dcs_title(&cf.title) {
        el.push(t);
    }
    // Порядок метамодели (models/dcs DataCompositionSchemaCalculatedField): useRestriction →
    // presentationExpression → orderExpression* → appearance → valueType.
    if let Some(r) = &cf.use_restriction {
        el.push(designer_dcs_use_restriction("useRestriction", r));
    }
    if let Some(pe) = &cf.presentation_expression {
        el.push(OutElement::leaf(
            "dcssch",
            "presentationExpression",
            pe.clone(),
        ));
    }
    for oe in &cf.order_expressions {
        el.push(designer_dcs_order_expression(oe));
    }
    if let Some(app) = designer_dcs_appearance(&cf.appearance) {
        el.push(app);
    }
    for av in &cf.available_values { el.push(designer_dcs_available_value(av)); }
    if let Some(ts) = &cf.value_type {
        el.push(
            crate::type_codec::encode_scoped(
                crate::TypeDialect::Designer,
                "dcssch",
                "valueType",
                ts,
                DESIGNER_FORM_NS,
            )
            .map_err(FormError::Frame)?,
        );
    }
    Ok(el)
}

/// Designer `<dcssch:appearance>` DCS-поля/вычисляемого-поля (общий писатель):
/// `dcscor:item xsi:type="dcsset:SettingsParameterValue"`-список. Пусто ⇒ `None` (тег не эмитим).
pub(crate) fn designer_dcs_appearance(items: &[DcsSettingsParameterValue]) -> Option<OutElement> {
    if items.is_empty() {
        return None;
    }
    let mut app = OutElement::branch("dcssch", "appearance");
    for it in items {
        app.push(designer_dcs_settings_parameter_value(it));
    }
    Some(app)
}

/// Designer `<dcssch:orderExpression>`: `<expression>`/`<orderType>`/`<autoOrder>` — ВСЕ ТРИ, в
/// ns `…/data-composition-system/common` (ДЕФОЛТНЫЙ xmlns-оверрайд на КАЖДОМ ребёнке). Witness
/// ОперацииСПодключаемымОборудованием.
pub(crate) fn designer_dcs_order_expression(oe: &DcsOrderExpression) -> OutElement {
    /// Ребёнок common-ns: беспрефиксный + ИНЛАЙН `xmlns="…/common"`.
    fn common_leaf(local: &str, text: impl Into<String>) -> OutElement {
        OutElement::leaf("", local, text.into()).attr("xmlns", DCS_COMMON_NS_URI)
    }
    let mut el = OutElement::branch("dcssch", "orderExpression");
    el.push(common_leaf("expression", oe.expression.clone()));
    el.push(common_leaf("orderType", oe.order_type.clone()));
    el.push(common_leaf("autoOrder", if oe.auto_order { "true" } else { "false" }));
    el
}

/// Designer `<dcssch:availableValue>`: `<dcssch:value xsi:type>` + опц. `<dcssch:presentation>`
/// (title-подобное). Witness СчетНаОплатуКлиенту.
pub(crate) fn designer_dcs_available_value(av: &DcsAvailableValue) -> OutElement {
    let mut el = OutElement::branch("dcssch", "availableValue");
    el.push(designer_dcs_param_value("value", &av.value));
    if let Some(p) = &av.presentation {
        if let Some(pres) = designer_dcs_localized("presentation", p) {
            el.push(pres);
        }
    }
    el
}

/// Designer ограничение использования DCS-поля: ветка dcssch presence-bools.
pub(crate) fn designer_dcs_use_restriction(tag: &str, r: &DcsUseRestriction) -> OutElement {
    let mut el = OutElement::branch("dcssch", tag);
    for (name, v) in [
        ("field", r.field),
        ("condition", r.condition),
        ("group", r.group),
        ("order", r.order),
    ] {
        if v {
            el.push(OutElement::leaf("dcssch", name, "true"));
        }
    }
    el
}

/// Designer DCS-заголовок из IR: `Localized` ⇒ `v8:LocalStringType`; `Str` ⇒
/// `<dcssch:title xsi:type="xs:string">текст`; `None`/пустой ⇒ не эмитим.
pub(crate) fn designer_dcs_title(title: &Option<PropertyValue>) -> Option<OutElement> {
    match title {
        Some(PropertyValue::Localized(pairs)) if !pairs.is_empty() => {
            Some(designer_dcssch_title(pairs))
        }
        Some(PropertyValue::Str(s)) => {
            Some(OutElement::leaf("dcssch", "title", s.clone()).attr("xsi:type", "xs:string"))
        }
        _ => None,
    }
}

/// Designer `<Parameter>` DCS-схемы: dcssch:name, опц. title, valueType, value, useRestriction, флаги.
///
/// `pub(super)` — см. [`designer_dcs_field`]. В cf-кэше тот же узел несёт ДОБАВЛЕННЫЙ
/// `xsi:type="dcssch:Parameter"` (его ставит [`super::super::dcss::write_server_state`]).
pub(crate) fn designer_dcs_parameter(p: &DcsParameter) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "Parameter");
    el.push(OutElement::leaf("dcssch", "name", p.name.clone()));
    if let Some(t) = designer_dcs_title(&p.title) {
        el.push(t);
    }
    match &p.value_type {
        None => {}
        Some(ts) => el.push(
            crate::type_codec::encode_scoped(
                crate::TypeDialect::Designer,
                "dcssch",
                "valueType",
                ts,
                DESIGNER_FORM_NS,
            )
            .map_err(FormError::Frame)?,
        ),
    }
    // `<dcssch:value>` — ОПЦИОНАЛЕН (absent ⇒ None ⇒ не эмитим; 8 параметров ВариантыОтчетов
    // его не несут). Общий писатель значения — см. [`designer_dcs_param_value`].
    if let Some(v) = &p.value {
        el.push(designer_dcs_param_value("value", v));
    }
    for v in &p.additional_values {
        el.push(designer_dcs_param_value("value", v));
    }
    // useRestriction — presence-точная реконструкция три-состояния (true/false/absent).
    if let Some(b) = p
        .use_restriction
        .or_else(|| (!p.designer_omitted_use_restriction).then_some(false))
    {
        el.push(OutElement::leaf(
            "dcssch",
            "useRestriction",
            if b { "true" } else { "false" },
        ));
    }
    if p.value_list_allowed {
        el.push(OutElement::leaf("dcssch", "valueListAllowed", "true"));
    }
    if let Some(expression) = &p.expression { el.push(OutElement::leaf("dcssch", "expression", expression.clone())); }
    if let Some(b) = p.available_as_field {
        el.push(OutElement::leaf(
            "dcssch",
            "availableAsField",
            if b { "true" } else { "false" },
        ));
    }
    if let Some(morph1c_core::ir::form::DcsParameterUse::Always) = p.usage { el.push(OutElement::leaf("dcssch", "use", "Always")); }
    Ok(el)
}

/// Designer DCS-заголовок `<dcssch:title xsi:type="v8:LocalStringType"><v8:item>…`.
pub(crate) fn designer_dcssch_title(pairs: &[(Lang, String)]) -> OutElement {
    let mut t = OutElement::branch("dcssch", "title").attr("xsi:type", "v8:LocalStringType");
    for (lang, text) in pairs {
        let mut item = OutElement::branch("v8", "item");
        item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        item.push(OutElement::leaf("v8", "content", text.clone()));
        t.push(item);
    }
    t
}

/// URI ns DCS-common (`orderExpression`-дети Designer объявляют его ДЕФОЛТНЫМ на КАЖДОМ ребёнке).
const DCS_COMMON_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/common";

/// Designer DCS-значение `<dcssch:{local} …>` (общий писатель): DCS-параметр И доступное значение
/// поля. `nil`/`xs:*`/`v8:UUID`/`v8:Type` — по варианту [`DcsParamValue`]. Пустой `Str` ⇒
/// самозакрытие. `TypeValue` — инлайн types-ns под авто-префиксом `d6p1` (локус DCS-параметра;
/// для availableValue не витнесснут).
pub(crate) fn designer_dcs_param_value(local: &str, v: &DcsParamValue) -> OutElement {
    match v {
        DcsParamValue::DesignTimeValue(text) => designer_dcs_text_value(local, text, "dcscor:DesignTimeValue"),
        DcsParamValue::Undefined => {
            OutElement::self_closing("dcssch", local).attr("xsi:nil", "true")
        }
        DcsParamValue::Boolean(t) => {
            OutElement::leaf("dcssch", local, t.clone()).attr("xsi:type", "xs:boolean")
        }
        DcsParamValue::Str(t) => designer_dcs_text_value(local, t, "xs:string"),
        DcsParamValue::Date(t) => {
            OutElement::leaf("dcssch", local, t.clone()).attr("xsi:type", "xs:dateTime")
        }
        DcsParamValue::Uuid(t) => {
            OutElement::leaf("dcssch", local, t.clone()).attr("xsi:type", "v8:UUID")
        }
        // xs:decimal ⟷ EDT core:NumberValue (ERP-witness Международный.ФормаСписка).
        DcsParamValue::Decimal(t) => {
            OutElement::leaf("dcssch", local, t.clone()).attr("xsi:type", "xs:decimal")
        }
        // Значение-ТИП ⟷ EDT `core:TypeValue`: инлайн types-ns под авто-префиксом ЛОКУСА (`d6p1`
        // у DCS-параметра, глубина 6; ns-объявление ПЕРЕД xsi:type — байт-порядок витнесса).
        DcsParamValue::TypeValue(name) => {
            let mut v = OutElement::leaf("dcssch", local, format!("d6p1:{name}"));
            v.attrs.push((
                "xmlns:d6p1".to_string(),
                "http://v8.1c.ru/8.2/data/types".to_string(),
            ));
            v.attrs.push(("xsi:type".to_string(), "v8:Type".to_string()));
            v
        }
    }
}

/// Designer title-подобный узел с ПАРАМЕТРИЗУЕМЫМ именем (`presentation` availableValue): `Localized`
/// ⇒ `xsi:type="v8:LocalStringType"` с `<v8:item>`-парами; `Str` ⇒ `xsi:type="xs:string"` (пустой ⇒
/// самозакрытие); `None`/пустой ⇒ не эмитим.
pub(crate) fn designer_dcs_localized(local: &str, p: &PropertyValue) -> Option<OutElement> {
    match p {
        PropertyValue::Localized(pairs) if !pairs.is_empty() => {
            let mut el =
                OutElement::branch("dcssch", local).attr("xsi:type", "v8:LocalStringType");
            for (lang, text) in pairs {
                let mut item = OutElement::branch("v8", "item");
                item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                item.push(OutElement::leaf("v8", "content", text.clone()));
                el.push(item);
            }
            Some(el)
        }
        PropertyValue::Str(s) => Some(designer_dcs_text_value(local, s, "xs:string")),
        _ => None,
    }
}

/// Designer `<ListSettings>` (dcsset DCS user-settings). Порядок: filter?, dataParameters?,
/// order?, conditionalAppearance?, item* (структурные), itemsViewMode?, itemsUserSettingID?.
/// ВСЁ пусто ⇒ самозакрытие `<ListSettings/>` (witness ВерсииПодсистемОбластейДанных).
///
/// ПЕРЕИСПОЛЬЗУЕТСЯ EDT-сайдкаром `ListSettings.dcss` ([`super::super::dcss`]) — он снимает префикс
/// `dcsset:` с этого дерева и оборачивает его в корень `<Settings>` (один writer на оба локуса).
pub(crate) fn designer_list_settings(ls: &DcsListSettings) -> OutElement {
    if *ls == DcsListSettings::default() {
        return OutElement::self_closing("", "ListSettings");
    }
    let mut el = OutElement::branch("", "ListSettings");
    if let Some(g) = &ls.filter {
        el.push(designer_dcs_group("filter", g));
    }
    if !ls.data_parameters.is_empty() {
        let mut dp = OutElement::branch("dcsset", "dataParameters");
        for it in &ls.data_parameters {
            dp.push(designer_dcs_settings_parameter_value(it));
        }
        el.push(dp);
    }
    if let Some(g) = &ls.order {
        el.push(designer_dcs_group("order", g));
    }
    if let Some(g) = &ls.conditional_appearance {
        el.push(designer_dcs_group("conditionalAppearance", g));
    }
    for it in &ls.structure_items {
        el.push(designer_dcs_item(it));
    }
    if let Some(v) = &ls.items_view_mode {
        el.push(OutElement::leaf("dcsset", "itemsViewMode", v.clone()));
    }
    if let Some(v) = &ls.items_user_setting_id {
        el.push(OutElement::leaf("dcsset", "itemsUserSettingID", v.clone()));
    }
    // itemsUserSettingPresentation (SC4) — ПОСЛЕ itemsUserSettingID (ERP-witness
    // ПравилаРаспределенияРасходов.ФормаСпискаВручную реквизит ПоказателиКЗаполнению).
    if let Some(p) = &ls.items_user_setting_presentation {
        el.push(designer_dcs_presentation("itemsUserSettingPresentation", p));
    }
    el
}

/// Designer `dcscor:item xsi:type="dcsset:SettingsParameterValue"`: dcscor:use?,
/// dcscor:parameter, dcscor:value, dcsset:userSettingID?.
pub(crate) fn designer_dcs_settings_parameter_value(it: &DcsSettingsParameterValue) -> OutElement {
    let mut el =
        OutElement::branch("dcscor", "item").attr("xsi:type", "dcsset:SettingsParameterValue");
    if let Some(u) = it.used {
        el.push(OutElement::leaf(
            "dcscor",
            "use",
            if u { "true" } else { "false" },
        ));
    }
    el.push(OutElement::leaf(
        "dcscor",
        "parameter",
        it.parameter.clone(),
    ));
    // `value` ОПЦИОНАЛЕН: неиспользуемый dataParameter несёт лишь use+parameter БЕЗ value
    // (§1.0-witness ×7 designer-блоков) ⇒ ничего не эмитим.
    if let Some(v) = &it.value {
        match v {
        DcsCorValue::DesignTimeValue(text) => {
            el.push(designer_dcs_text_value_ns("dcscor", "value", text, "dcscor:DesignTimeValue"));
        }
        DcsCorValue::Nil => {
            el.push(OutElement::self_closing("dcscor", "value").attr("xsi:nil", "true"));
        }
        DcsCorValue::Color(c) => {
            el.push(OutElement::leaf("dcscor", "value", c.clone()).attr("xsi:type", "v8ui:Color"));
        }
        // ERP-witness АналитикиСтатейБюджетов.НастройкаТипаАналитики: булев параметр
        // оформления (`xsi:type="xs:boolean"`).
        DcsCorValue::Boolean(b) => {
            el.push(OutElement::leaf("dcscor", "value", b.clone()).attr("xsi:type", "xs:boolean"));
        }
        // ERP-witness ЗадачиПоЭкземпляруБюджета.ФормаСписка: шрифт-параметр оформления —
        // те же font-атрибуты, что у `<Font>` контролов (общий designer-эмиттер).
        DcsCorValue::Font(f) => {
            let mut v = designer_font_named("value", f);
            v.prefix = "dcscor".into();
            v.attrs.insert(0, ("xsi:type".into(), "v8ui:Font".into()));
            el.push(v);
        }
        // ERP-witness ОтклоненияВСтоимостиТоваров: строковый параметр (Формат=ЧДЦ=2).
        DcsCorValue::Str(s) => {
            el.push(designer_dcs_text_value_ns("dcscor", "value", s, "xs:string"));
        }
        DcsCorValue::LocalString(pairs) => {
            let mut v =
                OutElement::branch("dcscor", "value").attr("xsi:type", "v8:LocalStringType");
            for (lang, text) in pairs {
                let mut item = OutElement::branch("v8", "item");
                item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                item.push(OutElement::leaf("v8", "content", text.clone()));
                v.push(item);
            }
            el.push(v);
        }
        // ERP-witness МероприятияТрудовойДеятельности.ФормаВыбораСобытий: параметр-поле.
        DcsCorValue::Field(f) => {
            el.push(OutElement::leaf("dcscor", "value", f.clone()).attr("xsi:type", "dcscor:Field"));
        }
        // ERP-witness ПечатьЭтикетокИЦенников: горизонтальное положение (`v8ui:HorizontalAlign`).
        DcsCorValue::HorizontalAlign(a) => {
            el.push(
                OutElement::leaf("dcscor", "value", a.clone())
                    .attr("xsi:type", "v8ui:HorizontalAlign"),
            );
        }
        // ERP-witness СреднийЗаработокСЭДО.ФормаСписка dataParameter `Год` (`xs:decimal`).
        DcsCorValue::Decimal(d) => {
            el.push(OutElement::leaf("dcscor", "value", d.clone()).attr("xsi:type", "xs:decimal"));
        }
        }
    }
    if let Some(id) = &it.user_setting_id {
        el.push(OutElement::leaf("dcsset", "userSettingID", id.clone()));
    }
    el
}

/// Designer группа DCS-настроек (`dcsset:filter`/`order`/`conditionalAppearance`): [items] +
/// viewMode? + userSettingID?.
pub(crate) fn designer_dcs_group(tag: &str, g: &DcsSettingsGroup) -> OutElement {
    let mut el = OutElement::branch("dcsset", tag);
    for c in designer_dcs_group_children(g) {
        el.push(c);
    }
    el
}

/// ДЕТИ группы DCS-настроек (items ++ viewMode? ++ userSettingID?) — БЕЗ обёртки-тега.
/// Тот же порядок, что у [`designer_dcs_group`]; выделен, потому что cf-бэг динсписка
/// РЕ-РУТИТ ровно этот список детей под собственный корень секции (`<Order>`/`<Filter>`/
/// `<ConditionalAppearance>`) — см. [`super::super::dcss::write_list_settings_section`].
pub(crate) fn designer_dcs_group_children(g: &DcsSettingsGroup) -> Vec<OutElement> {
    let mut out = Vec::new();
    for it in &g.items {
        out.push(designer_dcs_item(it));
    }
    if let Some(v) = &g.view_mode {
        out.push(OutElement::leaf("dcsset", "viewMode", v.clone()));
    }
    if let Some(v) = &g.user_setting_id {
        out.push(OutElement::leaf("dcsset", "userSettingID", v.clone()));
    }
    // ГРУППОВОЕ `userSettingPresentation` ПОСЛЕ userSettingID (witness ОтветственныеЗа…ФормаСписка).
    // cf-секция ре-рутит ЭТОТ же список — так представление уезжает и в бэг (reprefix).
    if let Some(p) = &g.user_setting_presentation {
        out.push(designer_dcs_presentation("userSettingPresentation", p));
    }
    out
}

/// Designer `<dcsset:item>` (FilterItemComparison / OrderItemField / условное оформление /
/// StructureItemGroup / GroupItemField).
pub(crate) fn designer_dcs_item(it: &DcsItem) -> OutElement {
    match it {
        DcsItem::FilterComparison {
            used,
            left_field,
            left_type,
            comparison_type,
            right,
            presentation,
            view_mode,
            user_setting_id,
            user_setting_presentation,
        } => {
            let mut el = OutElement::branch("dcsset", "item")
                .attr("xsi:type", "dcsset:FilterItemComparison");
            if let Some(u) = used {
                el.push(OutElement::leaf(
                    "dcsset",
                    "use",
                    if *u { "true" } else { "false" },
                ));
            }
            // Тип левого операнда — обычно `dcscor:Field`; пустой (напр. из старого IR) ⇒ дефолт.
            let lt = if left_type.is_empty() {
                "dcscor:Field"
            } else {
                left_type.as_str()
            };
            el.push(OutElement::leaf("dcsset", "left", left_field.clone()).attr("xsi:type", lt));
            el.push(OutElement::leaf(
                "dcsset",
                "comparisonType",
                comparison_type.clone(),
            ));
            for r in right {
                el.push(designer_dcs_right(r));
            }
            if let Some(p) = presentation {
                el.push(designer_dcs_presentation("presentation", p));
            }
            if let Some(v) = view_mode {
                el.push(OutElement::leaf("dcsset", "viewMode", v.clone()));
            }
            if let Some(v) = user_setting_id {
                el.push(OutElement::leaf("dcsset", "userSettingID", v.clone()));
            }
            if let Some(p) = user_setting_presentation {
                el.push(designer_dcs_presentation("userSettingPresentation", p));
            }
            el
        }
        DcsItem::FilterGroup {
            used,
            group_type,
            items,
            presentation,
            view_mode,
            user_setting_id,
        } => {
            let mut el =
                OutElement::branch("dcsset", "item").attr("xsi:type", "dcsset:FilterItemGroup");
            if let Some(u) = used {
                el.push(OutElement::leaf(
                    "dcsset",
                    "use",
                    if *u { "true" } else { "false" },
                ));
            }
            el.push(OutElement::leaf("dcsset", "groupType", group_type.clone()));
            for sub in items {
                el.push(designer_dcs_item(sub));
            }
            if let Some(p) = presentation {
                el.push(designer_dcs_presentation("presentation", p));
            }
            if let Some(v) = view_mode {
                el.push(OutElement::leaf("dcsset", "viewMode", v.clone()));
            }
            if let Some(id) = user_setting_id { el.push(OutElement::leaf("dcsset", "userSettingID", id.clone())); }
            el
        }
        DcsItem::OrderAuto => {
            OutElement::self_closing("dcsset", "item").attr("xsi:type", "dcsset:OrderItemAuto")
        }
        DcsItem::OrderField {
            used,
            field,
            order_type,
            view_mode,
        } => {
            let mut el =
                OutElement::branch("dcsset", "item").attr("xsi:type", "dcsset:OrderItemField");
            if let Some(u) = used {
                el.push(OutElement::leaf(
                    "dcsset",
                    "use",
                    if *u { "true" } else { "false" },
                ));
            }
            el.push(OutElement::leaf("dcsset", "field", field.clone()));
            el.push(OutElement::leaf("dcsset", "orderType", order_type.clone()));
            if let Some(v) = view_mode {
                el.push(OutElement::leaf("dcsset", "viewMode", v.clone()));
            }
            el
        }
        DcsItem::ConditionalAppearance {
            used,
            selection,
            filter,
            appearance,
            source_empty_appearance,
            presentation,
            view_mode,
            user_setting_id,
        } => {
            let mut el = OutElement::branch("dcsset", "item");
            if let Some(u) = used {
                el.push(OutElement::leaf(
                    "dcsset",
                    "use",
                    if *u { "true" } else { "false" },
                ));
            }
            match selection {
                None => el.push(OutElement::self_closing("dcsset", "selection")),
                Some(fields) => {
                    let mut sel = OutElement::branch("dcsset", "selection");
                    for f in fields {
                        let mut si = OutElement::branch("dcsset", "item");
                        if let Some(u) = f.used {
                            si.push(OutElement::leaf(
                                "dcsset",
                                "use",
                                if u { "true" } else { "false" },
                            ));
                        }
                        si.push(OutElement::leaf("dcsset", "field", f.field.clone()));
                        sel.push(si);
                    }
                    el.push(sel);
                }
            }
            // Пустой отбор ⇒ САМОЗАКРЫТЫЙ `<dcsset:filter/>` (ERP-witness НДССостояниеРеализации0
            // «плейсхолдер»-строка); непустой ⇒ ветка с item-ами.
            if filter.is_empty() {
                el.push(OutElement::self_closing("dcsset", "filter"));
            } else {
                let mut flt = OutElement::branch("dcsset", "filter");
                for fi in filter {
                    flt.push(designer_dcs_item(fi));
                }
                el.push(flt);
            }
            // `<dcsset:appearance>` (SC5) — ОПЦИОНАЛЕН: пусто ⇒ тег НЕ эмитим (ERP-witness тот же).
            if !appearance.is_empty() || *source_empty_appearance {
                let mut app = if appearance.is_empty() {
                    OutElement::self_closing("dcsset", "appearance")
                } else {
                    OutElement::branch("dcsset", "appearance")
                };
                for ai in appearance {
                    app.push(designer_dcs_settings_parameter_value(ai));
                }
                el.push(app);
            }
            if let Some(p) = presentation {
                el.push(designer_dcs_presentation("presentation", p));
            }
            if let Some(v) = view_mode {
                el.push(OutElement::leaf("dcsset", "viewMode", v.clone()));
            }
            if let Some(v) = user_setting_id {
                el.push(OutElement::leaf("dcsset", "userSettingID", v.clone()));
            }
            el
        }
        DcsItem::StructureGroup {
            group_items,
            nested,
        } => {
            let mut el =
                OutElement::branch("dcsset", "item").attr("xsi:type", "dcsset:StructureItemGroup");
            let mut gi = OutElement::branch("dcsset", "groupItems");
            for sub in group_items {
                gi.push(designer_dcs_item(sub));
            }
            el.push(gi);
            // ВЛОЖЕННЫЕ под-структуры — прямые дети ПОСЛЕ `groupItems` (рекурсивное дерево).
            for sub in nested {
                el.push(designer_dcs_item(sub));
            }
            el
        }
        DcsItem::GroupField {
            used,
            field,
            group_type,
            period_addition_type,
            period_addition_begin,
            period_addition_end,
        } => {
            let mut el =
                OutElement::branch("dcsset", "item").attr("xsi:type", "dcsset:GroupItemField");
            if let Some(u) = used {
                el.push(OutElement::leaf(
                    "dcsset",
                    "use",
                    if *u { "true" } else { "false" },
                ));
            }
            el.push(OutElement::leaf("dcsset", "field", field.clone()));
            el.push(OutElement::leaf("dcsset", "groupType", group_type.clone()));
            el.push(OutElement::leaf(
                "dcsset",
                "periodAdditionType",
                period_addition_type.clone(),
            ));
            el.push(
                OutElement::leaf(
                    "dcsset",
                    "periodAdditionBegin",
                    period_addition_begin.clone(),
                )
                .attr("xsi:type", "xs:dateTime"),
            );
            el.push(
                OutElement::leaf("dcsset", "periodAdditionEnd", period_addition_end.clone())
                    .attr("xsi:type", "xs:dateTime"),
            );
            el
        }
    }
}

/// Designer `<dcsset:right>` (xs:boolean/decimal/string, dcscor:Field/DesignTimeValue,
/// v8:Type-QName, v8:StandardBeginningDate).
pub(crate) fn designer_dcs_right(right: &DcsRightValue) -> OutElement {
    // Текстовый лист с xsi:type; ПУСТОЙ текст ⇒ самозакрытие (witness
    // `<dcsset:right xsi:type="xs:string"/>` — ПубличныеИдентификаторы).
    let text_leaf = |v: &str, xt: &str| designer_dcs_text_value_ns("dcsset", "right", v, xt);
    match right {
        DcsRightValue::Undefined => OutElement::self_closing("dcsset", "right").attr("xsi:nil", "true"),
        DcsRightValue::Boolean(v) => text_leaf(v, "xs:boolean"),
        DcsRightValue::Decimal(v) => text_leaf(v, "xs:decimal"),
        DcsRightValue::Str(v) => text_leaf(v, "xs:string"),
        DcsRightValue::Field(v) => text_leaf(v, "dcscor:Field"),
        DcsRightValue::DesignTimeValue(v) => text_leaf(v, "dcscor:DesignTimeValue"),
        DcsRightValue::TypeQName(v) => {
            // ИНЛАЙН-ns ПЕРЕД xsi:type (byte-порядок witness Взаимодействия).
            let prefix = v.split_once(':').map_or("d8p1", |(prefix, _)| prefix);
            let mut el = OutElement::leaf("dcsset", "right", v.clone());
            el.attrs.push((
                format!("xmlns:{prefix}"),
                "http://v8.1c.ru/8.2/data/types".to_string(),
            ));
            el.attrs
                .push(("xsi:type".to_string(), "v8:Type".to_string()));
            el
        }
        DcsRightValue::StandardBeginningDate { variant, date } => {
            let mut el =
                OutElement::branch("dcsset", "right").attr("xsi:type", "v8:StandardBeginningDate");
            el.push(
                OutElement::leaf("v8", "variant", variant.clone())
                    .attr("xsi:type", "v8:StandardBeginningDateVariant"),
            );
            if let Some(d) = date {
                el.push(OutElement::leaf("v8", "date", d.clone()));
            }
            el
        }
        DcsRightValue::ValueList { last_id } => {
            // ПУСТОЙ список: `<v8:valueType/>` (самозакрытый) + `<v8:lastId xsi:type="xs:decimal">`.
            let mut el =
                OutElement::branch("dcsset", "right").attr("xsi:type", "v8:ValueListType");
            el.push(OutElement::self_closing("v8", "valueType"));
            el.push(OutElement::leaf("v8", "lastId", last_id.clone()).attr("xsi:type", "xs:decimal"));
            el
        }
    }
}

/// Designer `<dcsset:{local}>`-представление (`presentation`/`userSettingPresentation`):
/// `Str` ⇒ `xsi:type="xs:string"` (пустой ⇒ самозакрытие), `LocalString` ⇒
/// `xsi:type="v8:LocalStringType"` с `<v8:item>`-парами.
pub(crate) fn designer_dcs_presentation(local: &str, p: &DcsPresentation) -> OutElement {
    match p {
        DcsPresentation::Str(s) => designer_dcs_text_value_ns("dcsset", local, s, "xs:string"),
        DcsPresentation::LocalString(pairs) => {
            let mut el =
                OutElement::branch("dcsset", local).attr("xsi:type", "v8:LocalStringType");
            for (lang, text) in pairs {
                let mut item = OutElement::branch("v8", "item");
                item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
                item.push(OutElement::leaf("v8", "content", text.clone()));
                el.push(item);
            }
            el
        }
    }
}

/// Текстовое значение с xsi:type в ns `dcssch`; ПУСТОЙ текст ⇒ самозакрытие.
pub(crate) fn designer_dcs_text_value(local: &str, v: &str, xt: &str) -> OutElement {
    designer_dcs_text_value_ns("dcssch", local, v, xt)
}

/// Текстовое значение с xsi:type; ПУСТОЙ текст ⇒ самозакрытие (`<tag xsi:type="…"/>`).
pub(crate) fn designer_dcs_text_value_ns(prefix: &str, local: &str, v: &str, xt: &str) -> OutElement {
    if v.is_empty() {
        OutElement::self_closing(prefix, local).attr("xsi:type", xt)
    } else {
        OutElement::leaf(prefix, local, v.to_string()).attr("xsi:type", xt)
    }
}
