//! ЧЕТВЁРТЫЙ ЛОКУС: EDT-сайдкар `ConditionalAppearance.dcssca` + cf-ячейка `<Settings>` root[3].

use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────────
// ЧЕТВЁРТЫЙ ЛОКУС: EDT-САЙДКАР `ConditionalAppearance.dcssca` (ФОРМ-уровневое УО)
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Полный фиксированный ns-блок корня `<ConditionalAppearance>` сайдкара (13 объявлений;
/// settings-ns — ДЕФОЛТНЫЙ, как и в `.dcss`). Порядок и URI — с ЕДИНСТВЕННОГО witness'а SSL
/// (`DataProcessors/РаботаСФайлами/Forms/ВерсияПрисоединенногоФайла`); он же ЕДИНСТВЕННАЯ форма
/// корпуса с форм-уровневым УО.
const DCSSCA_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", DCSSET_NS_URI),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", XSI_NS_URI),
    (
        "xmlns:dcscor",
        "http://v8.1c.ru/8.1/data-composition-system/core",
    ),
    (
        "xmlns:lf",
        "http://v8.1c.ru/8.2/managed-application/logform",
    ),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
];

/// Опциональная ПАРА объявлений корня `.dcssca`: ERP-flavored сайдкары (ценз 506/506 файлов
/// ERP-корпуса — ЕДИНСТВЕННЫЙ второй флавор) идут без обоих; SSL-flavor несёт полный блок.
/// Порядок остальных 11 объявлений в обоих флаворах ОДИН (сверено цензом) ⇒ presence-бит +
/// фильтр по [`DCSSCA_ROOT_NS`] воспроизводит оба корня byte-exact.
const DCSSCA_OPTIONAL_NS: &[&str] = &["xmlns:lf", "xmlns:pal"];

/// ЧЕТВЁРТЫЙ ЛОКУС тех же настроек: КОНЕЧНАЯ ячейка области root[3] формы в `.cf` —
/// цельный `<Settings>`-документ, base64.
///
/// Корень объявляет ТЕ ЖЕ 11 ns, что и сайдкар `.dcss`, но в ДРУГОМ порядке: дефолтный
/// settings-ns, затем префиксы ПО АЛФАВИТУ (`dcscor` `pal` `style` `sys` `v8` `v8ui` `web` `win`
/// `xs` `xsi`). Порядок снят с оракула (F-wave 24) и перепроверяется тестом ниже: писатель обязан
/// побайтово воспроизвести и ПУСТОЙ блоб (самозакрытый `<Settings/>`, 875 форм SSL), и
/// ЕДИНСТВЕННЫЙ непустой (`DataProcessor/РаботаСФайлами/ВерсияПрисоединенногоФайла`).
///
/// Набор ФИКСИРОВАН (не used-only: `style`/`win`/`web` объявлены и без употребления), но
/// ВЕРСИОНЕН: старый (2.20 / 8.3.27-writer, V50) ростер НЕ несёт `xmlns:pal` — 10 объявлений
/// (r31-витнессы erp.cf: ПУСТОЙ блоб — ExchangePlan.МобильноеПриложениеЗаказыКлиентов/
/// ФормаГлавногоУзла `77cbc40e-…`; непустой — ФормаОтправкиPushУведомления `4d3af4d2-…`);
/// новый (2.21+/8.5.1, V59) — полные 11 С `pal` (876/876 SSL byte-exact). См.
/// [`CF_SETTINGS_V50_DROPPED_NS`].
///
/// Конверт: BOM + декларация, БЕЗ завершающего перевода строки (в отличие от сайдкаров).
const CF_SETTINGS_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", DCSSET_NS_URI),
    (
        "xmlns:dcscor",
        "http://v8.1c.ru/8.1/data-composition-system/core",
    ),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", STYLE_NS_URI),
    ("xmlns:sys", SYS_FONTS_NS_URI),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", XSI_NS_URI),
];

/// Объявления [`CF_SETTINGS_ROOT_NS`], которые СТАРЫЙ (V50) писатель НЕ эмитит.
const CF_SETTINGS_V50_DROPPED_NS: &[&str] = &["xmlns:pal"];

/// Конверт cf-блоба `<Settings>`: BOM, CRLF, таб, БЕЗ завершающего EOL.
fn cf_settings_envelope() -> Envelope {
    Envelope {
        bom: true,
        eol: "\r\n",
        indent_unit: "\t",
        decl: DCSS_DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Записать КОНЕЧНУЮ ячейку области root[3] формы (`.cf`): `<Settings>`-документ, несущий
/// форм-уровневое условное оформление. Пустое УО ⇒ САМОЗАКРЫТЫЙ `<Settings/>` — ровно тот
/// байт-в-байт блоб, который 875 форм SSL несут константой.
///
/// `without_pal` — флавор ns-ростера корня: `true` = V50-таргет (2.20 / 8.3.27-writer, БЕЗ
/// `xmlns:pal`), `false` = V59 (полный 11-ns ростер). См. [`CF_SETTINGS_ROOT_NS`].
///
/// `style_item_uuid` — резолв config-`StyleItem`-ссылок цветов/шрифтов УО в `0:<uuid>`
/// ([`resolve_config_style_refs`]); переписывание НЕ версионно (r31: ssl.cf 8.5.1 переписывает
/// цвет секции МашиночитаемыеДоверенности ТАК ЖЕ, как erp.cf 8.3.27 — витнесс
/// `fe63db47-…` == uuid StyleItem.ТекстЗапрещеннойЯчейкиЦвет), гейт не нужен; SSL-канар
/// инертен — единственный SSL-носитель форм-уровневого УО style-ссылок не несёт.
pub fn write_form_settings_blob(
    conditional_appearance: &[DcsItem],
    without_pal: bool,
    style_item_uuid: StyleItemUuid<'_>,
) -> Vec<u8> {
    let mut root = OutElement::branch("", "Settings");
    for (name, uri) in CF_SETTINGS_ROOT_NS {
        if without_pal && CF_SETTINGS_V50_DROPPED_NS.contains(name) {
            continue;
        }
        root = root.attr(*name, *uri);
    }
    if conditional_appearance.is_empty() {
        // No appearance ⇒ the platform SELF-CLOSES the root (`<Settings …/>`), it does not emit an
        // empty pair — that is the byte the 875 non-carrier SSL forms hold.
        root.self_closing = true;
    } else {
        let mut ca = OutElement::branch("", "conditionalAppearance");
        for it in conditional_appearance {
            let mut el = designer_dcs_item(it);
            resolve_config_style_refs(&mut el, style_item_uuid);
            dcsset_to_default_ns(&mut el);
            ca.push(el);
        }
        root.push(ca);
    }
    render(&cf_settings_envelope(), &root)
}

/// Переписать config-`StyleItem`-ссылки поддерева УО в `0:<uuid>` — форма, которой их держит
/// cf-блоб `<Settings>` (Designer/сайдкары держат символическую `style:<Имя>`).
///
/// Витнессы (r31, побайтно):
/// * ЦВЕТ: `<dcscor:value xsi:type="v8ui:Color">style:ЦветТекстаОтмененнойСтрокиДокумента` ⟼
///   `0:fc27350f-e87d-4b7b-bc03-ef0417edab45` (erp.cf ExchangePlan.МобильноеПриложение
///   ЗаказыКлиентов/ФормаОтправкиPushУведомления `4d3af4d2-…0`);
/// * ШРИФТ: `<dcscor:value xsi:type="v8ui:Font" ref="style:ВажнаяНадписьШрифт"
///   kind="StyleItem"/>` ⟼ `ref="0:fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc"`, прочие атрибуты
///   как есть (erp.cf DataProcessor.СервисShare/ВыборФайловКПубликации `342ec436-…0`).
///
/// НЕразрешимое имя = ПЛАТФОРМЕННЫЙ стиль — ВЕРБАТИМ (erp.cf Catalog.ОтправкиОтчетности/
/// ФормаЭлемента `82a09bc8-…0`: `style:SpecialTextColor`; Catalog.ПравилаИнтеграции
/// С1СДокументооборотом/ВыборРеквизитаПотребителя `8d247cc7-…0`: `ref="style:NormalTextFont"
/// bold="true" …`). Ссылки под другими ns (`sys:DefaultGUIFont`, `web:`/`win:`-цвета,
/// `#RRGGBB`, `auto`) не трогаем — В ЭТОМ локусе их ns объявлены ростером корня
/// ([`CF_SETTINGS_ROOT_NS`]), инлайн-переобъявления секционного локуса тут нет.
fn resolve_config_style_refs(el: &mut OutElement, style_item_uuid: StyleItemUuid<'_>) {
    match el
        .attrs
        .iter()
        .find(|(n, _)| n == "xsi:type")
        .map(|(_, v)| v.as_str())
    {
        Some("v8ui:Color") => {
            if let Some(name) = el.text.as_deref().and_then(|t| t.strip_prefix("style:")) {
                if let Some(uuid) = style_item_uuid(name) {
                    el.text = Some(format!("0:{uuid}"));
                }
            }
        }
        Some("v8ui:Font") => {
            for (name, value) in &mut el.attrs {
                if name != "ref" {
                    continue;
                }
                if let Some(n) = value.strip_prefix("style:") {
                    if let Some(uuid) = style_item_uuid(n) {
                        *value = format!("0:{uuid}");
                    }
                }
            }
        }
        _ => {}
    }
    for c in &mut el.children {
        resolve_config_style_refs(c, style_item_uuid);
    }
}

/// Прочитать байты `ConditionalAppearance.dcssca` → форм-уровневые `<item>`-ы УО
/// (byte-exact-обратимо; тот же default-ns⟼`dcsset:` адаптер, что и у `.dcss`).
///
/// EDT выносит форм-уровневое УО в сайдкар, тогда как Designer держит его ИНЛАЙН, внутри
/// `<Attributes>` ([`super::super::read::read_designer_form`]). Волны 1-22 EDT-сайдкар просто НЕ ЧИТАЛИ —
/// УО терялось на edt→designer, а cf-энкодер не видел ни одной формы с УО и списывал её счётную
/// под-область root[3] на «гиперссылку над индексированным путём» (F-wave 23 опровергла это
/// абляцией — см. `form_body::ensure_no_derived_path_region`).
///
/// Второй элемент результата — флаг конверта `envelope_without_lf_pal` (см.
/// [`write_conditional_appearance_dcssca`]): ERP-flavored сайдкары (ценз 506/506 ERP) идут БЕЗ
/// `xmlns:lf` и `xmlns:pal`; SSL-flavored (единственный witness SSL) несут полный 13-ns блок.
/// Присутствие переносится в re-emit byte-exact, отсутствующие НЕ выдумываем (класс W12).
pub fn read_conditional_appearance_dcssca(
    bytes: &[u8],
) -> Result<(Vec<DcsItem>, bool), FormError> {
    let descriptor = parse(bytes)?;
    let env = descriptor.bytes_env;
    if env.bom {
        return Err(FormError::Envelope("dcssca: unexpected BOM".into()));
    }
    if env.eol != EolStyle::Crlf {
        return Err(FormError::Envelope(format!(
            "dcssca: must be CRLF, found {:?}",
            env.eol
        )));
    }
    match descriptor.decl.as_deref() {
        Some(d) if d == DCSS_DECL => {}
        other => {
            return Err(FormError::Envelope(format!(
                "dcssca: unexpected decl: {other:?}"
            )))
        }
    }
    let mut root = descriptor.root;
    if !root.prefix.is_empty() || root.local != "ConditionalAppearance" {
        return Err(FormError::Envelope(format!(
            "dcssca: unexpected root <{}>",
            root.local
        )));
    }
    root.claim();
    // `xmlns:lf` и `xmlns:pal` — ОПЦИОНАЛЬНАЯ ПАРА: ERP-flavored сайдкары (506/506 по цензу
    // ERP-корпуса) не несут ОБА, SSL-flavored — несут ОБА; смешанных флаворов в корпусах нет,
    // поэтому один presence-бит. Половинчатый флавор (ровно одно из двух) — не витнесснут, отказ.
    let required: Vec<(&str, &str)> = DCSSCA_ROOT_NS
        .iter()
        .copied()
        .filter(|(name, _)| !DCSSCA_OPTIONAL_NS.contains(name))
        .collect();
    claim_root_ns(&root, &required)?;
    let mut present = 0usize;
    for opt in DCSSCA_OPTIONAL_NS {
        let uri = DCSSCA_ROOT_NS
            .iter()
            .find(|(name, _)| name == opt)
            .expect("DCSSCA_ROOT_NS carries every optional ns")
            .1;
        if let Some(a) = root.attr(opt) {
            if a.value != uri {
                return Err(FormError::Envelope(format!(
                    "dcssca: root {opt}={:?} want {uri:?}",
                    a.value
                )));
            }
            a.claimed.set(true);
            present += 1;
        }
    }
    let envelope_without_lf_pal = match present {
        0 => true,
        n if n == DCSSCA_OPTIONAL_NS.len() => false,
        n => {
            return Err(FormError::Envelope(format!(
                "dcssca: root несёт {n} из {} опциональных ns ({DCSSCA_OPTIONAL_NS:?}) — \
                 половинчатый флавор не витнесснут (§1.0)",
                DCSSCA_OPTIONAL_NS.len()
            )))
        }
    };
    default_ns_to_dcsset(&mut root);
    let mut out = Vec::new();
    for it in root
        .children
        .iter()
        .filter(|c| c.local == "item" && c.prefix == DCSSET)
    {
        out.push(read_dcs_item(it)?);
    }
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{leftover} unconsumed node(s) in ConditionalAppearance.dcssca (§1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    super::super::read::mark_edt_dcs_items(&mut out);
    Ok((out, envelope_without_lf_pal))
}

/// Записать форм-уровневые `<item>`-ы УО в байты `ConditionalAppearance.dcssca` (byte-exact к
/// платформенному экспорту — единственный сайдкар SSL регенерируется в точности; тест ниже).
///
/// ПУСТОЕ УО сайдкара НЕ ИМЕЕТ (платформа файл не создаёт); вызывающая сторона
/// (`pipeline::form_write`) обязана проверить непустоту ДО вызова.
pub fn write_conditional_appearance_dcssca(items: &[DcsItem], without_lf_pal: bool) -> Vec<u8> {
    let mut root = OutElement::branch("", "ConditionalAppearance");
    for (name, uri) in DCSSCA_ROOT_NS {
        // presence-точный re-emit флавора конверта (см. ридер): ERP-flavor — без lf/pal.
        if without_lf_pal && DCSSCA_OPTIONAL_NS.contains(name) {
            continue;
        }
        root = root.attr(*name, *uri);
    }
    for it in items {
        let mut el = designer_dcs_item(it);
        dcsset_to_default_ns(&mut el);
        root.push(el);
    }
    super::super::write::bind_dcs_type_qname_depth(&mut root, false);
    render(&dcss_envelope(), &root)
}
