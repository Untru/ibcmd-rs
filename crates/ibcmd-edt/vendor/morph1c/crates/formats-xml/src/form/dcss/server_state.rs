//! Кэш `ServerState` динамического списка (F-wave 18) — документ `<UniversalListServerOnlyState>`.

use super::*;

// --- кэш `ServerState` динамического списка (F-wave 18) ----------------------------------------

/// Корень документа-кэша `ServerState`.
const SERVER_STATE_ROOT: &str = "UniversalListServerOnlyState";

/// Ns-блок корня кэша — те же ТРИ объявления, что у секции, но ДЕФОЛТНЫЙ ns ПУСТ: дети сидят
/// ВНЕ ns (`<Field>`, не `<dcsset:Field>`), а `dcssch` каждый объявляет ИНЛАЙН у себя.
const SERVER_STATE_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", ""),
    ("xmlns:xs", XS_NS_URI),
    ("xmlns:xsi", XSI_NS_URI),
];

/// Хвост документа-кэша: платформа дописывает ДВА ПРОБЕЛА после корня — и у пустого документа,
/// и у каждого кэша (сверено побайтно на всех 229 витнессах SSL, `probe_server_state`).
const SERVER_STATE_TRAILER: &[u8] = b"  ";

/// Envelope документа-кэша: как у секции, но БЕЗ BOM (и с [`SERVER_STATE_TRAILER`], который
/// дописывает [`write_server_state`] — `render` хвостовых пробелов не умеет).
fn server_state_envelope() -> Envelope {
    Envelope {
        bom: false,
        ..section_envelope()
    }
}

/// Сериализовать КЭШ `ServerState` динамического списка — документ `<UniversalListServerOnlyState>`,
/// который платформа base64-ит в одноимённую ячейку бэга динсписка.
///
/// # Что это
/// НЕ константа, а КЭШ схемы data-set'а списка. Пустая схема ⇒ самозакрытый корень, и это ровно
/// платформенный «пустой документ» (187/187 витнессов SSL несут ОДНО значение — оно же
/// пришпилено тестом [`tests::empty_server_state_is_the_platform_empty_document`], так что
/// «пустой» случай не выделен в отдельную ветку: его ПРОИЗВОДИТ этот же writer).
///
/// # Дети — та же Designer-под-IR схемы, под конвенцией секции ([`reprefix`] на глубине 2)
/// С ДВУМЯ дельтами, которые корпус называет явно (`probe_server_state`):
/// * `<CalculatedField>` (Designer: без `xsi:type`) ⟼ `<ExpressionField xsi:type="dcssch:CalculatedField">`;
/// * `<Parameter>`       (Designer: без `xsi:type`) ⟼ `<Parameter xsi:type="dcssch:Parameter">`;
/// * `<Field …>` — БЕЗ дельты (Designer уже ставит `xsi:type="dcssch:DataSetFieldField"`).
///
/// ПОРЯДОК детей: корпус витнессит лишь `[Field…] ++ [Parameter…]` (×21) и calc-ONLY (×1) — три
/// вида НИКОГДА не сходятся на одном списке, так что 3-сторонний порядок корпусом НЕ ЗАДАН. Берём
/// порядок Designer-локуса (`designer_dynamic_list_attr`: calc, fields, parameters) — он
/// воспроизводит ОБА витнессованных сочетания и не выдумывает третьего правила.
///
/// # §1.0-предохранители (иначе кэш молча разъехался бы с формой)
/// `valueType` параметра эмитит [`super::super::write::designer_dcs_parameter`] в ПРЕФИКСАХ КОРНЯ ФОРМЫ
/// (`DESIGNER_FORM_NS`, 18 объявлений) — а корень кэша объявляет лишь `xs`/`xsi`. Поэтому
/// QName-текст вроде `cfg:CatalogRef.X` здесь ПОВИС БЫ (префикс не связан), а АВТО-ns-тип
/// (`Chart`/`Picture`/`FormattedString`) принёс бы инлайн-объявление с глубиной ФОРМЫ (`d5p1`),
/// а не кэша. Корпус витнессит РОВНО две формы `valueType` — `xs:`-примитив и сырой `TypeId`-uuid
/// — поэтому [`check_cache_qnames`] громко отказывает на всём прочем, вместо тихой порчи.
pub fn write_server_state(
    fields: &[DcsField],
    calculated_fields: &[DcsCalculatedField],
    parameters: &[DcsParameter],
    style_item_uuid: StyleItemUuid<'_>,
) -> Result<Vec<u8>, FormError> {
    let mut root = OutElement::self_closing("", SERVER_STATE_ROOT);
    for (name, uri) in SERVER_STATE_ROOT_NS {
        root = root.attr(*name, *uri);
    }
    // Область видимости корня: ДЕФОЛТНЫЙ ns ПУСТ (дети — вне ns), плюс xs/xsi.
    let scope: Vec<(String, &'static str)> = vec![
        (String::new(), ""),
        ("xs".into(), XS_NS_URI),
        ("xsi".into(), XSI_NS_URI),
    ];

    let mut children: Vec<OutElement> = Vec::new();
    // ПОРЯДОК детей: `[Field…]` ПЕРЕД `[ExpressionField…]` — witness erp.cf
    // InformationRegister.ОперацииСПодключаемымОборудованием/ФормаСписка (22 `<Field>` затем один
    // `<ExpressionField>`). SSL-инертно: ни один SSL-кэш не несёт fields И calc одновременно
    // (корпус витнессит лишь `[Field]++[Parameter]` ×21 и calc-ONLY ×1 — 3-сторонний порядок им
    // НЕ задан), поэтому перестановка calc↔fields байт-не-трогает ни один SSL-документ.
    for f in fields {
        children.push(designer_dcs_field(f)?);
    }
    for cf in calculated_fields {
        let mut el = designer_dcs_calculated_field(cf)?;
        // Дельта кэша: ПЕРЕИМЕНОВАНИЕ + ДОБАВЛЕННЫЙ тип (`xsi:type` идёт ПЕРВЫМ атрибутом —
        // ns-объявления перед ним расставит `reprefix`).
        el.local = "ExpressionField".into();
        el.attrs
            .insert(0, ("xsi:type".into(), "dcssch:CalculatedField".into()));
        children.push(el);
    }
    for p in parameters {
        let mut el = designer_dcs_parameter(p)?;
        el.attrs
            .insert(0, ("xsi:type".into(), "dcssch:Parameter".into()));
        force_use_restriction(&mut el);
        children.push(el);
    }

    for child in &mut children {
        drop_v8_type_prefix(child);
        check_cache_qnames(child)?;
        root.self_closing = false;
        root.push(reprefix(child, 2, &scope, style_item_uuid)?);
    }
    let mut out = render(&server_state_envelope(), &root);
    out.extend_from_slice(SERVER_STATE_TRAILER);
    Ok(out)
}

/// Дельта КЭША для `<Parameter>`: `useRestriction` ЭМИТИТСЯ ВСЕГДА (F-wave 21).
///
/// Форменный XML хранит его как presence-точное ТРИ-состояние (`true`/`false`/absent —
/// [`super::super::write::designer_dcs_parameter`]), и менять это нельзя: на нём стоит survey-инвариант
/// round-trip'а. Но КЭШ так не делает — он материализует дефолт: **91/91** `<Parameter>`-блоков
/// всех 42 закэшированных SSL-документов несут `<dcssch:useRestriction>` (72 `true`, 19 `false`;
/// `probe_server_state`, corpus-wide count), в том числе там, где IR не несёт ничего.
///
/// Witness: `Catalog/РассылкиОтчетов/ФормаСписка`, параметр `ОтчетОтбор` — IR `None`, оракул
/// `<dcssch:useRestriction>false</dcssch:useRestriction>` (ровно те 56 байт, на которые наш
/// документ был короче). Форма отказывала до этой волны, поэтому свидетель был недостижим.
///
/// Позиция: сразу за `<value>`, ПЕРЕД `valueListAllowed`/`availableAsField` — тот же порядок, что и
/// у ветки, где IR его несёт.
fn force_use_restriction(el: &mut OutElement) {
    if el
        .children
        .iter()
        .any(|c| c.local == "useRestriction" && c.prefix == "dcssch")
    {
        return;
    }
    let at = el
        .children
        .iter()
        .position(|c| c.local == "valueListAllowed" || c.local == "availableAsField")
        .unwrap_or(el.children.len());
    el.children.insert(
        at,
        OutElement::leaf("dcssch", "useRestriction", "false".to_string()),
    );
}

/// Кэш пишет `v8`-CORE-тип по ИМЕНИ, без префикса: `reprefix` кладёт `<v8:Type>` в ДЕФОЛТНЫЙ ns
/// `…/data/core` (`<Type xmlns="http://v8.1c.ru/8.1/data/core">`), так что префикс `v8:` в тексте
/// QName избыточен — и оракул его не пишет (F-wave 23).
///
/// Витнесс: `Catalog/{Пользователи,ВнешниеПользователи}/ФормаСписка` — по два `UUID`-параметра
/// списка; оракул кэша несёт `<Type …>UUID</Type>`, а мы писали `<TypeId>fc01b5df-…</TypeId>`
/// (реестр знает `UUID` как платформенный BUILTIN — см. `form_body::data_attrs::is_config_type_id`).
/// Обе формы отказывали (STAR-select) до этой волны, поэтому витнесс был недостижим.
///
/// `xs:`-примитив (`xs:boolean`) НЕ трогаем: его префикс связан в КОРНЕ кэша, и оракул пишет его
/// с префиксом.
fn drop_v8_type_prefix(el: &mut OutElement) {
    if el.prefix == "v8" && (el.local == "Type" || el.local == "TypeSet") {
        if let Some(t) = el.text.as_mut() {
            if let Some(bare) = t.strip_prefix("v8:") {
                *t = bare.to_string();
            }
        }
    }
    for c in &mut el.children {
        drop_v8_type_prefix(c);
    }
}

/// §1.0-предохранитель кэша: ни один узел не смеет опираться на ns-префикс, которого в корне
/// кэша НЕТ (см. docstring [`write_server_state`]).
///
/// Два запрета, оба — на РЕАЛЬНЫЕ пути порчи, а не на гипотетические:
/// 1. ИНЛАЙН-объявление `xmlns:*` на узле — так себя эмитит АВТО-ns-тип (`d5p1:Chart`), а
///    `reprefix` инлайн-объявления ОТБРАСЫВАЕТ ⇒ префикс в тексте повис бы;
/// 2. QName-ТЕКСТ `<v8:Type>`/`<v8:TypeSet>` с префиксом, не связанным в корне кэша (связаны
///    лишь `xs`/`xsi`) — напр. `cfg:CatalogRef.X`.
///
/// Витнессы корпуса проходят оба: `<Type …>xs:boolean` и `<TypeId …>a7a5262c-…` (без префикса).
fn check_cache_qnames(el: &OutElement) -> Result<(), FormError> {
    // Ловим ЛЮБОЕ инлайн-объявление ns — и префиксное `xmlns:*` (авто-ns-тип Chart/Picture/…),
    // и БЕСПРЕФИКСНЫЙ дефолтный `xmlns` (common-ns дети `orderExpression` DCS-вычисляемого-поля:
    // `reprefix` их ns не знает и МОЛЧА положил бы их в дефолтный ns кэша — SC3). Оба — §1.0-отказ
    // вместо тихой порчи.
    //
    // ЕДИНСТВЕННОЕ ЛЕГИТИМНОЕ инлайн-объявление — types-ns у значения `v8:Type`: `reprefix` шаг 3
    // (`inline_types_prefix`) ОТБРАСЫВАЕТ этот инлайн и ПЕРЕ-выводит его глубино-корректно (в кэше
    // — `d3p2`), так что он НЕ повисает. Витнесс byte-exact ×3 (erp.cf ServerState-кэш, параметр
    // `ТипЗначенияКлюча`): `83f6b94e-…432b.0` Catalog.КлючиРеестраДокументов/ФормаВыбора,
    // `aec88eb2-…b503.0` .../ФормаСписка, `f65b0d3e-…1eac.0` Document.ДокументЭДОБЗК/ФормаСписка —
    //   `<dcssch:value xmlns:d3p1="http://v8.1c.ru/8.1/data/core"
    //     xmlns:d3p2="http://v8.1c.ru/8.2/data/types" xsi:type="d3p1:Type">d3p2:Undefined</dcssch:value>`
    // (`d6p1`, который несёт Designer-дерево, — ФОРМ-глубинный префикс; `reprefix` его пере-выводит).
    // Прочие инлайн-ns (авто-ns-тип, дефолтный `xmlns` orderExpression) по-прежнему отказ — у
    // `reprefix` нет шага, который бы их пере-вывел.
    // NB variant-B (round 14, WITNESSED, NOT YET CLOSED): a DCS-common default `xmlns="…/common"` on
    // the `<orderExpression>` children (`expression`/`orderType`/`autoOrder`) is KEPT verbatim by the
    // platform cache (erp.cf InformationRegister.ОперацииСПодключаемымОборудованием/ФормаСписка +
    // synthetic wG). Merely allowing it here is NOT enough — `reprefix` still DROPS the inline decl
    // (→ empty ns) AND `write_server_state` emits calc-fields BEFORE fields whereas the oracle orders
    // Field→ExpressionField. Closing it needs a reprefix common-ns passthrough + the field/calc order
    // flip, both on the byte-exact SSL cache path; left REFUSING (§1.0) until those two land.
    // Пропускаем ДВА легитимных инлайн-ns: (1) types-ns у `v8:Type` ([`reprefix`] шаг 3 пере-выводит),
    // (2) DCS-common дефолтный `xmlns` у детей `orderExpression` DCS-вычисляемого-поля (reprefix
    // пере-объявляет локально — witness ОперацииСПодключаемымОборудованием). Прочее (авто-ns-тип
    // Chart/Picture/FormattedString) по-прежнему §1.0-отказ.
    if let Some((name, _)) = el.attrs.iter().find(|(n, u)| {
        n.starts_with("xmlns")
            && u.as_str() != TYPES_NS_URI
            && u.as_str() != DCS_COMMON_NS_URI
    }) {
        return Err(FormError::Frame(format!(
            "ServerState cache: <{}> carries an INLINE ns declaration {name:?} (an auto-ns type \
             such as Chart/Picture/FormattedString) — the cache root binds only xs/xsi and the ns \
             would dangle / be silently remapped; no SSL witness has one (§1.0)",
            el.local
        )));
    }
    if el.prefix == "v8" && (el.local == "Type" || el.local == "TypeSet") {
        if let Some((p, _)) = el.text.as_deref().and_then(|t| t.split_once(':')) {
            if p != "xs" {
                return Err(FormError::Frame(format!(
                    "ServerState cache: <v8:{}> QName {:?} uses ns prefix {p:?}, which the cache \
                     root does not bind (only `xs:`-primitives and raw `TypeId` uuids are \
                     witnessed in the SSL corpus) (§1.0)",
                    el.local,
                    el.text.as_deref().unwrap_or_default()
                )));
            }
        }
    }
    for c in &el.children {
        check_cache_qnames(c)?;
    }
    Ok(())
}
