//! Перепрефиксовка Designer-поддерева под cf-конвенцию секции/кэша + READ/WRITE ns-адаптеры сайдкара.

use super::*;

/// Designer-префикс ⟼ URI (все префиксы, которыми пользуется [`designer_dcs_item`]).
fn designer_prefix_uri(prefix: &str) -> Option<&'static str> {
    match prefix {
        "dcsset" => Some(DCSSET_NS_URI),
        "dcscor" => Some(DCSCOR_NS_URI),
        "dcssch" => Some(DCSSCH_NS_URI),
        "v8" => Some(V8_NS_URI),
        "v8ui" => Some(V8UI_NS_URI),
        "xs" => Some(XS_NS_URI),
        "xsi" => Some(XSI_NS_URI),
        // Инлайн-ns значения `v8:Type` (Designer-канон — см. модульный docstring).
        TYPES_PREFIX_DESIGNER => Some(TYPES_NS_URI),
        _ => None,
    }
}

/// КАНОНИЧЕСКИЙ (не авто) префикс, которым платформа объявляет этот ns внутри секции/кэша.
/// Только ns самой DCS; всё прочее — авто `d<глубина>p<номер>` (см. docstring секции).
///
/// `dcssch` — ns СХЕМЫ data-set'а: его объявляет ИНЛАЙН каждый ребёнок кэша `ServerState`
/// (`<Field xmlns:dcssch=… xsi:type="dcssch:DataSetFieldField">`, витнесс ВариантыОтчетов).
/// Для СЕКЦИЙ он недостижим (их под-IR схемы не несёт), так что добавление аддитивно.
fn canonical_prefix(uri: &str) -> Option<&'static str> {
    match uri {
        DCSSET_NS_URI => Some(DCSSET),
        DCSCOR_NS_URI => Some("dcscor"),
        DCSSCH_NS_URI => Some("dcssch"),
        _ => None,
    }
}

/// Является ли текст АБСОЛЮТНЫМ цветом `#RRGGBB` (ровно 6 hex-цифр после `#`).
pub(crate) fn is_absolute_rgb_hex(t: &str) -> bool {
    t.strip_prefix('#')
        .is_some_and(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Перепрефиксовать Designer-поддерево под конвенцию секции (см. docstring [`DcsSettingsSection`]).
///
/// `depth` — 1-based глубина ЭТОГО элемента от корня секции (корень = 1); `scope` — связки
/// префикс⟼URI, видимые ЗДЕСЬ (ближайшая связка ПЕРЕКРЫВАЕТ дальнюю — как в XML).
pub(crate) fn reprefix(
    el: &OutElement,
    depth: usize,
    scope: &[(String, &'static str)],
    style_item_uuid: StyleItemUuid<'_>,
) -> Result<OutElement, FormError> {
    /// Префикс, под которым `uri` виден в области `scope` (`""` = дефолтный ns).
    fn lookup<'a>(scope: &'a [(String, &'static str)], uri: &str) -> Option<&'a str> {
        scope
            .iter()
            .find(|(_, u)| *u == uri)
            .map(|(p, _)| p.as_str())
    }
    /// Связать префикс с ns, ПЕРЕКРЫВ прежнюю связку ЭТОГО ЖЕ префикса (иначе старый URI
    /// остался бы «виден» под уже перепривязанным префиксом — так и рождался
    /// `xsi:type="SettingsParameterValue"` вместо `dcsset:SettingsParameterValue`).
    fn push_scope(scope: &mut Vec<(String, &'static str)>, prefix: &str, uri: &'static str) {
        scope.retain(|(p, _)| p != prefix);
        scope.insert(0, (prefix.to_string(), uri));
    }

    let mut scope: Vec<(String, &'static str)> = scope.to_vec();
    let mut decls: Vec<(String, &'static str)> = Vec::new();
    // Счётчик АВТО-префиксов, выданных НА ЭТОМ элементе (канонические его не тратят).
    let mut auto = 0usize;
    let bind = |uri: &'static str,
                scope: &mut Vec<(String, &'static str)>,
                decls: &mut Vec<(String, &'static str)>,
                auto: &mut usize|
     -> String {
        let prefix = match canonical_prefix(uri) {
            Some(p) => p.to_string(),
            None => {
                *auto += 1;
                format!("d{depth}p{auto}")
            }
        };
        decls.push((format!("xmlns:{prefix}"), uri));
        push_scope(scope, &prefix, uri);
        prefix
    };

    // 1. ns САМОГО ЭЛЕМЕНТА. Не виден ⇒ переопределяем ДЕФОЛТНЫЙ ns (`WriteStartElement`
    //    без запрошенного префикса — witness `<item xmlns="…dcs/core">` внутри `<appearance>`).
    //
    //    БЕСПРЕФИКСНЫЙ Designer-элемент живёт в ДЕФОЛТНОМ ns ТОГО документа, куда его кладут, —
    //    а он у локусов РАЗНЫЙ: у секции это settings-ns, у кэша `ServerState` он ПУСТ (дети
    //    сидят вне ns: `<Field>`, `<Parameter>`). Поэтому берём его из scope КОРНЯ, а не из
    //    таблицы префиксов: одна `reprefix` обслуживает оба локуса, и ни один не «знает» другого.
    let el_uri = match designer_prefix_uri(&el.prefix) {
        Some(uri) => uri,
        // Беспрефиксный DCS-common ребёнок `<orderExpression>` (`expression`/`orderType`/`autoOrder`)
        // несёт ИНЛАЙН `xmlns="…/common"` — его РЕАЛЬНЫЙ ns есть common (а НЕ дефолтный ns документа).
        // `el_prefix` ниже пере-объявит его локально (`<expression xmlns="…/common">`), точно как
        // платформенный кэш (witness ОперацииСПодключаемымОборудованием). SSL-инертно: ни один SSL-
        // кэш/сайдкар не несёт такого ребёнка (иначе `check_cache_qnames`/reprefix уже спотыкались бы).
        None if el.prefix.is_empty()
            && el
                .attrs
                .iter()
                .any(|(n, u)| n == "xmlns" && u == DCS_COMMON_NS_URI) =>
        {
            DCS_COMMON_NS_URI
        }
        None if el.prefix.is_empty() => scope
            .iter()
            .find(|(p, _)| p.is_empty())
            .map(|(_, u)| *u)
            .ok_or_else(|| {
            FormError::Frame(format!(
                "dcs settings section: <{}> is unprefixed but the document declares no \
                     default ns (§1.0)",
                el.local
            ))
        })?,
        None => {
            return Err(FormError::Frame(format!(
                "dcs settings section: unknown designer ns prefix {:?} on <{}> (§1.0)",
                el.prefix, el.local
            )))
        }
    };
    let el_prefix = match lookup(&scope, el_uri) {
        Some(p) => p.to_string(),
        None => {
            decls.push(("xmlns".to_string(), el_uri));
            push_scope(&mut scope, "", el_uri);
            String::new()
        }
    };

    // 2. QName-ЗНАЧЕНИЯ атрибутов (`xsi:type`) — им нужен ПРЕФИКС из области видимости.
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut value_is_color = false;
    let mut value_is_font = false;
    for (name, value) in &el.attrs {
        // Designer-дерево несёт ИНЛАЙН-объявление types-ns у `v8:Type` — оно ПЕРЕ-выводится.
        if name.starts_with("xmlns") {
            continue;
        }
        if name == "xsi:type" {
            let (p, local) = value.split_once(':').unwrap_or(("", value.as_str()));
            let uri = designer_prefix_uri(p).ok_or_else(|| {
                FormError::Frame(format!(
                    "dcs settings section: unknown xsi:type ns prefix in {value:?} (§1.0)"
                ))
            })?;
            value_is_color |= uri == V8UI_NS_URI && local == "Color";
            value_is_font |= uri == V8UI_NS_URI && local == "Font";
            let bound = match lookup(&scope, uri) {
                Some(p) => p.to_string(),
                None => bind(uri, &mut scope, &mut decls, &mut auto),
            };
            let q = if bound.is_empty() {
                local.to_string()
            } else {
                format!("{bound}:{local}")
            };
            attrs.push((name.clone(), q));
        } else {
            attrs.push((name.clone(), value.clone()));
        }
    }

    // 3. QName-ТЕКСТ (`v8:Type` ⇒ текст вида `dNpM:Undefined`); маркер — ИНЛАЙН-объявление
    //    types-ns под ЛЮБЫМ авто-префиксом (локус-зависим: `d6p1` DCS-параметра-значения, `d8p1`/
    //    `d10p1` отбора). Снимаем ИСХОДНЫЙ префикс и пере-выводим текст в авто-префикс ТЕКУЩЕЙ
    //    глубины секции/кэша (инлайн-объявление отброшено в п.2 — `bind` вернёт своё).
    let mut text = el.text.clone();
    if let Some(src_prefix) =
        inline_types_prefix(el.attrs.iter().map(|(n, v)| (n.as_str(), v.as_str())))
    {
        let t = text.unwrap_or_default();
        let local = t.strip_prefix(&format!("{src_prefix}:")).ok_or_else(|| {
            FormError::Frame(format!(
                "dcs settings section: `v8:Type` text {t:?} is not a {src_prefix:?}-QName (§1.0)"
            ))
        })?;
        let bound = match lookup(&scope, TYPES_NS_URI) {
            Some(p) => p.to_string(),
            None => bind(TYPES_NS_URI, &mut scope, &mut decls, &mut auto),
        };
        text = Some(if bound.is_empty() {
            local.to_string()
        } else {
            format!("{bound}:{local}")
        });
    }

    // 4. ЗНАЧЕНИЕ `v8ui:Color` в тексте. Две ВИТНЕССЕННЫЕ разновидности:
    //    * `style:<name>` — ссылка на элемент стиля ⟼ `0:<uuid>` (см. [`StyleItemUuid`]).
    //    * `web:<Name>`   — именованный web-цвет ⟼ QName `<auto>:<Name>` под ПЕРЕ-выведенным
    //      web-colors-ns. ERP round 5, витнесс byte-exact (erp.cf `48876d2d-…0`
    //      СчетНаОплатуКлиенту/ФормаСозданияСчетовНаОплату RosyBrown; `f09297af-…`/`6cd48e31-…`/
    //      `620b6bda-…` СогласованиеЗакупки FireBrick):
    //      `<value xmlns:d5p1="…/ui" xmlns:d5p2="…/ui/colors/web" xsi:type="d5p1:Color">d5p2:RosyBrown</value>`
    //      — имя ДОСЛОВНО, БЕЗ rgb/индекса (таблица `web_colors` тут ни при чём). Отличие от style:
    //      ровно одно инлайн-объявление web-ns и verbatim-имя вместо `0:<uuid>`. Прочие флейворы
    //      (`win:`/`sys:`/`pal:`/абсолютный rgb) НЕ витнессены ⇒ громкий отказ (§1.0).
    if value_is_color {
        let t = text.clone().unwrap_or_default();
        if t == "auto" {
            // `auto` — платформа пишет ЛИТЕРАЛ ВЕРБАТИМ, БЕЗ web/style-ns и БЕЗ uuid: остаётся
            // РОВНО инлайн-объявление самой Color-ns (`d5p1`) от xsi:type. Витнесс byte-exact ×2
            // (erp.cf DL-Appearance секция, параметр ЦветТекста):
            //   * `c9ddba23-…694.0` Document.УведомлениеОВвозеМаркированныхТоваровИзЕАЭСГИСМ/
            //     ФормаСпискаДокументов,
            //   * `34e256c5-…487.0` Document.ПланЗакупок/ФормаСписка —
            //   `<value xmlns:d5p1="http://v8.1c.ru/8.1/data/ui" xsi:type="d5p1:Color">auto</value>`.
            // Оставляем `text` как есть (Some("auto")); прочие флейворы ловит else-ветка ниже.
        } else if let Some(name) = t.strip_prefix("style:") {
            match style_item_uuid(name) {
                Some(uuid) => text = Some(format!("0:{uuid}")),
                None => {
                    // Реестр НЕ знает имя ⇒ ПЛАТФОРМЕННЫЙ стиль (тот же дискриминатор, что у
                    // `codecs::color_node`: registry None → platform). Витнесс erp.cf: DL-settings
                    // Appearance `a7402839-…` (ЭлектронныеПеревозочныеДокументы/ФормаСпискаЭПД)
                    // `style:FieldSelectionBackColor` → `d5p2:FieldSelectionBackColor` под
                    // `http://v8.1c.ru/8.1/data/ui/style`; `c8b34c21-…` SpecialTextColor так же.
                    // Отличие от config-StyleItem ровно как web: от style: — style-ns + verbatim-имя
                    // вместо `0:<uuid>` (config StyleItemы ЗНАЕТ реестр → эта ветка dead для них и SSL).
                    let uri = STYLE_NS_URI;
                    let bound = match lookup(&scope, uri) {
                        Some(p) => p.to_string(),
                        None => bind(uri, &mut scope, &mut decls, &mut auto),
                    };
                    text = Some(if bound.is_empty() {
                        name.to_string()
                    } else {
                        format!("{bound}:{name}")
                    });
                }
            }
        } else if let Some(name) = t.strip_prefix("web:") {
            let uri = "http://v8.1c.ru/8.1/data/ui/colors/web";
            let bound = match lookup(&scope, uri) {
                Some(p) => p.to_string(),
                None => bind(uri, &mut scope, &mut decls, &mut auto),
            };
            text = Some(if bound.is_empty() {
                name.to_string()
            } else {
                format!("{bound}:{name}")
            });
        } else if is_absolute_rgb_hex(&t) {
            // АБСОЛЮТНЫЙ `#RRGGBB` — платформа пишет hex-ТЕКСТ ВЕРБАТИМ под инлайн Color-ns (`d5p1`),
            // РОВНО как ветка `auto` (не web/style-QName и не uuid). Витнесс byte-exact на синтетик-
            // DL conditionalAppearance (`#0000C0` ЦветТекста / `#FFFF00` ЦветФона →
            // `<value xmlns:d5p1="http://v8.1c.ru/8.1/data/ui" xsi:type="d5p1:Color">#0000C0</value>`)
            // и на носителях FilterCriterion.ЗадачиПоЭкземпляруБюджета.ФормаСписка (`#FFFF00`) /
            // DataProcessor.ДокументооборотСКонтролирующимиОрганами.УправлениеОбменом (`#0000C0`).
            // `text` остаётся как есть. SSL DL-настройки НЕ несут абсолютного цвета ⇒ путь не задет.
        } else {
            return Err(FormError::Frame(format!(
                "dcs settings section: `v8ui:Color` {t:?} is not a config `style:<name>`, \
                 a `web:<name>`, nor an absolute `#RRGGBB` — the cf blob encoding of any OTHER \
                 color flavour is unwitnessed (§1.0)"
            )));
        }
    }

    // 5. Ссылка `ref` значения `v8ui:Font`. Витнессы erp.cf (section-локус, r31): платформа
    //    объявляет ns ссылки ИНЛАЙН КАНОНИЧЕСКИМ префиксом (в отличие от цветов, где
    //    платформенный стиль едет под АВТО-префиксом `d5p2`) и переписывает ТОЛЬКО
    //    config-StyleItem; прочие атрибуты (`bold`/`italic`/…/`kind`) едут как есть:
    //    * `style:<config>` → `ref="0:<uuid>"`, xmlns:style ПРИ ЭТОМ ОСТАЁТСЯ объявлен
    //      (Catalog.ПоказателиРасчетаЗарплаты/ФормаСписка b62913ec-…:
    //      `style:ЗаголовокУдаленногоРеквизитаШрифт` → `<value xmlns:d5p1="…/ui"
    //       xmlns:style="…/ui/style" xsi:type="d5p1:Font" ref="0:654523be-93d9-4faa-9a35-…"
    //       kind="StyleItem"/>`);
    //    * `style:<платформенный>` → ВЕРБАТИМ + xmlns:style (Catalog.РабочиеМеста/ФормаСписка
    //      aeafdd56-…: `ref="style:TextFont" bold="true" …`);
    //    * `sys:<имя>` → ВЕРБАТИМ + xmlns:sys (Catalog.ВидыБюджетов/ФормаСписка 216df259-…:
    //      `ref="sys:DefaultGUIFont" … kind="WindowsFont"`).
    //    Прочий префикс ссылки не витнесснут → громкий отказ (§1.0). До этого шага ссылка
    //    уезжала БЕЗ инлайн-объявления её ns → платформа шрифт не резолвила
    //    (CompareCfg-класс r36 «Значение:(пусто) → Обычный шрифт текста», ×32).
    if value_is_font {
        for (name, value) in &mut attrs {
            if name != "ref" {
                continue;
            }
            let (ns_uri, canonical): (&'static str, &'static str) = if value.starts_with("style:")
            {
                (STYLE_NS_URI, "style")
            } else if value.starts_with("sys:") {
                (SYS_FONTS_NS_URI, "sys")
            } else if let Some((p, _)) = value.split_once(':') {
                return Err(FormError::Frame(format!(
                    "dcs settings section: `v8ui:Font` ref {value:?} carries the unwitnessed \
                     ns prefix {p:?} — refusing to guess its cf blob encoding (§1.0)"
                )));
            } else {
                continue;
            };
            if lookup(&scope, ns_uri).is_none() {
                decls.push((format!("xmlns:{canonical}"), ns_uri));
                push_scope(&mut scope, canonical, ns_uri);
            }
            if let Some(n) = value.strip_prefix("style:") {
                if let Some(uuid) = style_item_uuid(n) {
                    *value = format!("0:{uuid}");
                }
            }
        }
    }

    // ns-объявления идут ПЕРВЫМИ (byte-порядок витнессов), затем прочие атрибуты.
    let mut out = OutElement {
        prefix: el_prefix,
        local: el.local.clone(),
        attrs: decls
            .into_iter()
            .map(|(n, u)| (n, u.to_string()))
            .chain(attrs)
            .collect(),
        children: Vec::new(),
        text,
        self_closing: el.self_closing,
    };
    for c in &el.children {
        out.push(reprefix(c, depth + 1, &scope, style_item_uuid)?);
    }
    Ok(out)
}

/// READ-адаптер (сайдкар ⟼ Designer-канон), рекурсивно:
/// * беспрефиксный элемент (= settings-ns по умолчанию) ⟼ префикс `dcsset`;
/// * беспрефиксное значение `xsi:type` (= settings-ns тип) ⟼ `dcsset:<тип>`;
/// * ИНЛАЙН-объявление types-ns под ЛЮБЫМ авто-префиксом ⟼ Designer-канон `d8p1` (вместе с
///   QName-текстом, который этот префикс метит).
///
/// Прочие префиксы (`dcscor`/`v8`/`v8ui`/`xs`) — как есть (в сайдкаре они те же, что в Designer).
pub(crate) fn default_ns_to_dcsset(el: &mut Element) {
    if el.prefix.is_empty() {
        el.prefix = DCSSET.to_string();
    }
    // Инлайн types-ns: авто-префикс локус-зависим ⇒ приводим к Designer-канону (см. docstring).
    if let Some(p) =
        inline_types_prefix(el.attrs.iter().map(|a| (a.name.as_str(), a.value.as_str())))
    {
        // Filter right values retain the exact scoped source alias.
        if el.local != "right" && p != TYPES_PREFIX_DESIGNER {
            for a in &mut el.attrs {
                if a.name == format!("xmlns:{p}") {
                    a.name = format!("xmlns:{TYPES_PREFIX_DESIGNER}");
                }
            }
            if let Some(local) = el.text.strip_prefix(&format!("{p}:")) {
                el.text = format!("{TYPES_PREFIX_DESIGNER}:{local}");
            }
        }
    }
    for a in &mut el.attrs {
        if a.name == "xsi:type" && !a.value.contains(':') {
            a.value = format!("{DCSSET}:{}", a.value);
        }
    }
    for c in &mut el.children {
        default_ns_to_dcsset(c);
    }
}

/// WRITE-адаптер: точная обратная биекция к [`default_ns_to_dcsset`] (Designer-канон ⟼ сайдкар).
pub(crate) fn dcsset_to_default_ns(el: &mut OutElement) {
    if el.prefix == DCSSET {
        el.prefix.clear();
    }
    for (name, value) in &mut el.attrs {
        if name == "xsi:type" {
            if let Some(rest) = value.strip_prefix("dcsset:") {
                *value = rest.to_string();
            }
        }
    }
    if el.local != "right"
        && inline_types_prefix(el.attrs.iter().map(|(n, v)| (n.as_str(), v.as_str())))
            .is_some_and(|p| p == TYPES_PREFIX_DESIGNER)
    {
        for (name, _) in &mut el.attrs {
            if name == &format!("xmlns:{TYPES_PREFIX_DESIGNER}") {
                *name = format!("xmlns:{TYPES_PREFIX_DCSS}");
            }
        }
        if let Some(t) = &el.text {
            if let Some(local) = t.strip_prefix(&format!("{TYPES_PREFIX_DESIGNER}:")) {
                el.text = Some(format!("{TYPES_PREFIX_DCSS}:{local}"));
            }
        }
    }
    for c in &mut el.children {
        dcsset_to_default_ns(c);
    }
}

/// Префикс ИНЛАЙН-объявления types-ns на этом элементе (`xmlns:<p>="…/8.2/data/types"`), если есть.
fn inline_types_prefix<'a>(attrs: impl Iterator<Item = (&'a str, &'a str)>) -> Option<String> {
    attrs
        .filter(|(_, v)| *v == TYPES_NS_URI)
        .find_map(|(n, _)| n.strip_prefix("xmlns:").map(str::to_string))
}
