//! FIELDS · EDT dialect READ codec — decode leaf values from EDT form XML (period/border/picture/color/common-bool/localized), plus the out-of-band structured readers (choiceList/choiceParameters/typeLink/choiceParameterLinks). §1.0 totality.

use super::*;

/// Прочитать поля таблицы данного региона с EDT-хоста (`None` = регион отсутствует —
/// применяются только fill-политики). Пуш в bag в порядке таблицы (= канонический порядок).
///
/// `owner` — вид-владелец полей таблицы (`"RadioButtonField"`, `"Form"`, …); он ключует
/// витнессенную таблицу версий форм, по которой [`apply_read_policy`] решает, законна ли
/// реконструкция ОТСУТСТВУЮЩЕГО свойства в дампе версии источника.
pub(crate) fn read_fields_edt(
    owner: &str,
    host: Option<&Element>,
    table: &[FieldProj],
    region: Region,
    bag: &mut Vec<(FieldId, PropertyValue)>,
) -> Result<(), FormError> {
    for entry in table.iter().filter(|e| e.region == region) {
        // choiceList — repeatable/structured, вне табличного дефолт-движка.
        if let Codec::ChoiceList = entry.codec {
            if let Some(h) = host {
                let items = read_choice_list_edt(h, entry.edt)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        // choiceParameters — repeatable/structured, вне табличного дефолт-движка.
        if let Codec::ChoiceParameters = entry.codec {
            if let Some(h) = host {
                let items = read_choice_parameters_edt(h, entry.edt)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        // choiceParameterLinks — repeatable/structured, вне табличного дефолт-движка.
        if let Codec::ChoiceParameterLinks = entry.codec {
            if let Some(h) = host {
                let items = read_choice_parameter_links_edt(h, entry.edt)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        // typeLink — structured, вне табличного дефолт-движка.
        if let Codec::TypeLink = entry.codec {
            if let Some(h) = host {
                if let Some(tl) = h
                    .children
                    .iter()
                    .find(|c| c.local == entry.edt && c.prefix.is_empty())
                {
                    bag.push((entry.id, read_type_link_edt(tl)?));
                }
            }
            continue;
        }
        // Localized — EDT кодирует МНОГОЯЗЫЧНОЕ значение ПОВТОРЕНИЕМ элемента (по одному
        // `<toolTip><key>hu</key><value>…` на язык; witness ВводКонтактнойИнформации.ВводАдреса
        // toolTip×6/inputHint×6 — multi-pair-в-одном-элементе в корпусе НЕ встречается 0/2745).
        // Сливаем пары в порядке документа (= Designer-порядок `<v8:item>`).
        if let Codec::Localized = entry.codec {
            let nodes: Vec<&Element> = host
                .map(|h| {
                    h.children
                        .iter()
                        .filter(|c| c.local == entry.edt && c.prefix.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let present = if nodes.is_empty() {
                None
            } else {
                let mut pairs = Vec::new();
                for n in &nodes {
                    match decode_edt_localized(n)? {
                        PropertyValue::Localized(p) => pairs.extend(p),
                        other => {
                            return Err(FormError::Frame(format!(
                                "<{}>: localized decode gave {other:?} (bug)",
                                entry.edt
                            )));
                        }
                    }
                }
                Some(PropertyValue::Localized(pairs))
            };
            apply_read_policy(owner, entry, present, FormDialect::Edt, bag)?;
            continue;
        }
        let node = host.and_then(|h| {
            h.children
                .iter()
                .find(|c| c.local == entry.edt && c.prefix.is_empty())
        });
        let present = match node {
            Some(el) => Some(decode_edt(entry, el)?),
            None => None,
        };
        apply_read_policy(owner, entry, present, FormDialect::Edt, bag)?;
    }
    Ok(())
}

/// Декодировать EDT-узел поля по кодеку (claim'ит узел).
pub(crate) fn decode_edt(entry: &FieldProj, el: &Element) -> Result<PropertyValue, FormError> {
    let tag = entry.edt;
    match entry.codec {
        Codec::Bool => decode_bool_text(el, tag),
        Codec::Int => {
            el.claim_with_text();
            Ok(PropertyValue::Int(parse_int(&el.text, tag)?))
        }
        Codec::EnumTok => {
            el.claim_with_text();
            Ok(PropertyValue::Enum(Token::new(el.text.clone())))
        }
        Codec::EnumMap(map) => {
            el.claim_with_text();
            if !map.iter().any(|(canon, _)| *canon == el.text) {
                return Err(FormError::Frame(format!(
                    "<{tag}>={:?}: unmapped enum literal (§1.0)",
                    el.text
                )));
            }
            Ok(PropertyValue::Enum(Token::new(el.text.clone())))
        }
        Codec::Text => {
            el.claim_with_text();
            Ok(PropertyValue::Str(el.text.clone()))
        }
        Codec::RefText => {
            el.claim_with_text();
            Ok(PropertyValue::Ref(el.text.clone()))
        }
        Codec::Localized => decode_edt_localized(el),
        Codec::Color => decode_edt_color(el, tag),
        Codec::PictureRef => decode_edt_picture(el, tag),
        Codec::Value => {
            // Nullable-скаляр xsi-вида через ОБЩИЙ value_codec (Number-decimal/String/Undefined/…).
            // Хост-узел клеймится здесь; xsi-атрибут и `<value>`-лист — внутри value_codec.
            el.claim();
            value_codec::decode(ValueDialect::Edt, el).map_err(FormError::Frame)
        }
        Codec::DataPath => {
            claim_xsi(el, tag, "form:DataPath")?;
            let v = require_single_leaf(el, tag, "segments")?;
            Ok(PropertyValue::Ref(v))
        }
        Codec::MdObjectRef => {
            claim_xsi(el, tag, "core:ReferenceValue")?;
            let v = require_single_leaf(el, tag, "value")?;
            Ok(PropertyValue::Ref(v))
        }
        Codec::CommonBool => decode_edt_common_bool(el, tag),
        Codec::UndefinedValue => {
            claim_xsi(el, tag, "core:UndefinedValue")?;
            el.claim();
            if !el.children.is_empty() || !el.text.is_empty() {
                return Err(FormError::Frame(format!(
                    "<{tag}>: UndefinedValue must be empty (§1.0)"
                )));
            }
            Ok(PropertyValue::Bool(true))
        }
        Codec::Border => decode_edt_border(el, tag),
        Codec::ScrollBar => {
            // EDT-сторона скроллбара — обычный enum-текст (омиссия `ScrollNever` — политикой).
            el.claim_with_text();
            Ok(PropertyValue::Enum(Token::new(el.text.clone())))
        }
        Codec::Period => decode_edt_period(el, tag),
        Codec::Type => {
            el.claim();
            type_codec::decode(TypeDialect::Edt, el).map_err(FormError::Frame)
        }
        Codec::ChoiceList => Err(FormError::Frame(
            "choiceList decoded via table engine (bug — handled out-of-band)".into(),
        )),
        Codec::ChoiceParameters => Err(FormError::Frame(
            "choiceParameters decoded via table engine (bug — handled out-of-band)".into(),
        )),
        Codec::ChoiceParameterLinks | Codec::TypeLink => Err(FormError::Frame(
            "typeLink/choiceParameterLinks decoded via table engine (bug — handled out-of-band)"
                .into(),
        )),
    }
}

/// EDT `<period><startDate>X</startDate><endDate>Y</endDate></period>` → `List([Str,Str])`.
fn decode_edt_period(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim();
    let start = child_leaf_text(el, "", "startDate", tag)?;
    let end = child_leaf_text(el, "", "endDate", tag)?;
    if el.children.len() != 2 {
        return Err(FormError::Frame(format!(
            "<{tag}>: expected exactly <startDate>+<endDate> (§1.0)"
        )));
    }
    Ok(PropertyValue::List(vec![
        PropertyValue::Str(start),
        PropertyValue::Str(end),
    ]))
}

/// EDT `<border xsi:type="core:BorderDef">[<style>Single</style>]<width>W</width></border>`.
/// `<style>` присутствует ⟺ канон-стиль его текст; отсутствует ⟺ `WithoutBorder`. Ширина —
/// часть канона (`@W`-суффикс при W≠1, см. [`border_canon`]).
fn decode_edt_border(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    if el
        .attr("xsi:type")
        .is_some_and(|a| a.value == "core:BorderRef")
    {
        claim_xsi(el, tag, "core:BorderRef")?;
        let reference = require_single_leaf(el, tag, "border")?;
        if !reference.starts_with("Style.") || reference.len() <= 6 {
            return Err(FormError::Frame(
                "BorderRef requires a named Style reference".into(),
            ));
        }
        return Ok(PropertyValue::Ref(reference));
    }
    claim_xsi(el, tag, "core:BorderDef")?;
    el.claim();
    let style_el = el.child("style").filter(|c| c.prefix.is_empty());
    let style = match &style_el {
        Some(s) => {
            s.claim_with_text();
            s.text.clone()
        }
        None => ff::BORDER_STYLE_WITHOUT.to_string(),
    };
    // `<width>` ОПУСКАЕТСЯ EDT при ширине 0 (⟺ Designer `<Border width="0">`; witness
    // МашиночитаемыеДоверенностиОрганизаций.ФормаЭлемента `<style>Single</style>` без width).
    // EDT НИКОГДА не эмитит явный `<width>0` (census ERP: 1/3/5 present, 0 всегда опущен) ⇒
    // отсутствие ⟺ 0 без неоднозначности. Ширина 0 → канон-суффикс `@0` (≠ keep-литерал), эмитят оба.
    let width_el = el.child("width").filter(|c| c.prefix.is_empty());
    let width = match &width_el {
        Some(w) => {
            w.claim_with_text();
            w.text.clone()
        }
        None => "0".to_string(),
    };
    let expected = style_el.is_some() as usize + width_el.is_some() as usize;
    if el.children.len() != expected {
        return Err(FormError::Frame(format!(
            "<{tag}>: BorderDef unexpected children (§1.0)"
        )));
    }
    Ok(border_canon(&style, &width))
}

/// EDT картинка — ДВЕ кодировки (обе → канон `Ref`):
/// * `<tag xsi:type="core:PictureRef"><picture>Ref</picture></tag>` → `Ref("Ref")`.
/// * `<tag xsi:type="form:FormPicture"/>` (ПУСТОЙ инлайн-маркер) → `Ref("")`: «картинка формы»
///   без ссылки на метаданные (сам бинарь — в сайдкаре, не в дескрипторе; EDT несёт лишь пустой
///   маркер). Witness — DataProcessor РегистрацияИзмененийДляОбменаДанными rowsPicture/valuesPicture,
///   InputField `picture`. Непустой FormPicture (с телом) — §1.0-НЕ смоделирован (ошибка).
pub(crate) fn decode_edt_picture(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: missing xsi:type")))?;
    match xt.value.as_str() {
        "core:PictureRef" => {
            xt.claimed.set(true);
            let v = require_single_leaf(el, tag, "picture")?;
            // EDT picture-узел НЕ несёт LoadTransparent (census: 0 из всего ERP-корпуса) ⇒
            // флаг ДЕРИВИТСЯ ([`picture_lt_default`]); Designer читает его независимо.
            let lt = picture_lt_default(&v);
            Ok(picture_canon(v, lt))
        }
        "form:FormPicture" => {
            xt.claimed.set(true);
            el.claim();
            // ПУСТОЙ маркер: картинка-СПУТНИК без прозрачного пикселя (LT деривится → false).
            if el.children.is_empty() {
                return Ok(picture_canon(String::new(), false));
            }
            // НЕПУСТОЙ FormPicture несёт ВСТРОЕННУЮ картинку с ПРОЗРАЧНЫМ ПИКСЕЛЕМ: РОВНО один
            // ребёнок `<transparentPixel><x>N</x><y>M</y></transparentPixel>` (sparse-листья,
            // census ERP 100/100 — общий кодек [`crate::transparent_pixel`]). Пиксель
            // денормализует LT=true. Иная структура — §1.0-отказ (не догадка).
            let only = match el.children.as_slice() {
                [c] if c.local == "transparentPixel" && c.prefix.is_empty() => c,
                _ => {
                    return Err(FormError::Frame(format!(
                        "<{tag}>: non-empty form:FormPicture must carry exactly one \
                         <transparentPixel> (§1.0)"
                    )));
                }
            };
            let pixel_ir = match crate::transparent_pixel::decode(only) {
                morph1c_core::engine::Decoded::Present(v) => v,
                morph1c_core::engine::Decoded::Error(e) => {
                    return Err(FormError::Frame(format!("<{tag}>: {e}")));
                }
                morph1c_core::engine::Decoded::Absent => {
                    return Err(FormError::Frame(format!(
                        "<{tag}>: empty <transparentPixel> (§1.0)"
                    )));
                }
            };
            let pixel = crate::transparent_pixel::pixel_of(&pixel_ir).map_err(FormError::Frame)?;
            Ok(picture_canon_px(String::new(), pixel))
        }
        other => Err(FormError::Frame(format!(
            "<{tag}> xsi:type={other:?}, want core:PictureRef|form:FormPicture (§1.0)"
        ))),
    }
}

/// EDT цвет — ДВЕ кодировки (обе → канон `Ref`):
/// * `<tag xsi:type="core:ColorRef"><color>Style.X</color></tag>` → `Ref("Style.X")`.
/// * `<tag xsi:type="core:ColorDef"><red>255</red><green>255</green><blue>153</blue></tag>` →
///   `Ref("#FFFF99")` (RGB-триплет → `#RRGGBB` uppercase; X с Designer `#FFFF99`).
///   EDT ОПУСКАЕТ нулевые компоненты (census ERP 1829 ColorDef: `<blue>`-only ×60, red+green ×49,
///   …, 0 explicit `<c>0</c>`) — отсутствие компоненты → 0; пустой самозакрытый
///   `<tag xsi:type="core:ColorDef"/>` = все нули = `#000000` (×649). Зеркало
///   `style_value_codec::opt_component`. Witness — DataProcessor ПомощникСозданияОбменаДанными.
pub(crate) fn decode_edt_color(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: missing xsi:type")))?;
    match xt.value.as_str() {
        "core:ColorRef" => {
            xt.claimed.set(true);
            let v = require_single_leaf(el, tag, "color")?;
            Ok(PropertyValue::Ref(v))
        }
        "core:ColorDef" => {
            xt.claimed.set(true);
            el.claim();
            // §1.0 ГРОМКИЙ отказ: непустой ColorDef с ребёнком НЕ из {red,green,blue}.
            for c in &el.children {
                if !c.prefix.is_empty() || !matches!(c.local.as_str(), "red" | "green" | "blue") {
                    return Err(FormError::Frame(format!(
                        "<{tag}>: ColorDef carries unmodelled child <{}:{}> \
                         (only red/green/blue — §1.0)",
                        c.prefix, c.local
                    )));
                }
            }
            let r = opt_color_component(el, "red", tag)?;
            let g = opt_color_component(el, "green", tag)?;
            let b = opt_color_component(el, "blue", tag)?;
            Ok(PropertyValue::Ref(format!("#{r:02X}{g:02X}{b:02X}")))
        }
        other => Err(FormError::Frame(format!(
            "<{tag}> xsi:type={other:?}, want core:ColorRef|core:ColorDef (§1.0)"
        ))),
    }
}

/// ОПЦИОНАЛЬНЫЙ компонент цвета `<red>|<green>|<blue>` (EDT опускает нулевые): отсутствие → 0,
/// присутствие → целое 0..=255 (claim). Иной текст — §1.0-ошибка. Зеркало
/// [`crate::style_value_codec`]`::opt_component`.
fn opt_color_component(el: &Element, child: &str, tag: &str) -> Result<u8, FormError> {
    match el.child(child).filter(|c| c.prefix.is_empty()) {
        Some(c) => {
            c.claim_with_text();
            c.text.parse::<u8>().map_err(|e| {
                FormError::Frame(format!("<{tag}>: ColorDef <{child}>={:?}: {e}", c.text))
            })
        }
        None => Ok(0),
    }
}

/// EDT `<tag><common>true</common></tag>` = true; `<tag/>` = false; иное — §1.0-ошибка.
///
/// РОЛЕВОЙ вариант (userVisible-права): `<tag>[<common>true</common>]<for><value>B</value>
/// <role>R</role></for>×N</tag>` ⟺ Designer `<Tag><xr:Common>B</xr:Common>
/// <xr:Value name="R">B</xr:Value>×N`. Канон — `List([Bool(common), List([Str(role), Bool(v)])…])`
/// (зеркало `F_CMD_USE`); безролевой — прежний `Bool` (байт-идентичный hot-path, 372k носителей).
/// EDT эмитит `<common>` лишь при `true`; его отсутствие рядом с `<for>` ⇒ common=false. Витнессы
/// РесурсныеСпецификации (common=false×3), мирСкважины (common=true×1).
fn decode_edt_common_bool(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim();
    if el
        .children
        .iter()
        .any(|c| c.local == "for" && c.prefix.is_empty())
    {
        let common = match el.child("common").filter(|c| c.prefix.is_empty()) {
            None => false,
            Some(c) => {
                c.claim_with_text();
                if c.text != "true" {
                    return Err(FormError::Frame(format!(
                        "<{tag}><common>={:?} alongside <for>-roles, want \"true\" (§1.0)",
                        c.text
                    )));
                }
                true
            }
        };
        let mut entries = vec![PropertyValue::Bool(common)];
        for f in el
            .children
            .iter()
            .filter(|c| c.local == "for" && c.prefix.is_empty())
        {
            f.claim();
            let val = match f.child("value").filter(|c| c.prefix.is_empty()) {
                None => false,
                Some(ve) => {
                    ve.claim_with_text();
                    match ve.text.as_str() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(FormError::Frame(format!(
                                "<{tag}><for><value>={other:?}, want bool (§1.0)"
                            )));
                        }
                    }
                }
            };
            let re = f
                .child("role")
                .filter(|c| c.prefix.is_empty())
                .ok_or_else(|| FormError::Frame(format!("<{tag}><for>: no <role> (§1.0)")))?;
            re.claim_with_text();
            entries.push(PropertyValue::List(vec![
                PropertyValue::Str(re.text.clone()),
                PropertyValue::Bool(val),
            ]));
        }
        return Ok(PropertyValue::List(entries));
    }
    if el.children.is_empty() {
        return Ok(PropertyValue::Bool(false));
    }
    let v = require_single_leaf(el, tag, "common")?;
    if v != "true" {
        return Err(FormError::Frame(format!(
            "<{tag}><common>={v:?}, want true (§1.0)"
        )));
    }
    Ok(PropertyValue::Bool(true))
}

/// EDT локализованные `<key>/<value>`-пары.
fn decode_edt_localized(t: &Element) -> Result<PropertyValue, FormError> {
    t.claim();
    let mut pairs = Vec::new();
    let mut it = t.children.iter();
    while let Some(k) = it.next() {
        if k.local != "key" || !k.prefix.is_empty() {
            return Err(FormError::Frame(format!("<{}>: expected <key>", t.local)));
        }
        k.claim_with_text();
        let v = it
            .next()
            .ok_or_else(|| FormError::Frame(format!("<{}>: <key> without <value>", t.local)))?;
        if v.local != "value" || !v.prefix.is_empty() {
            return Err(FormError::Frame(format!("<{}>: expected <value>", t.local)));
        }
        v.claim_with_text();
        pairs.push((Lang::new(k.text.clone()), v.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// Прочитать повторяемые EDT `<choiceList>` хоста в список пар. Иные дети — §1.0-ошибка.
pub(crate) fn read_choice_list_edt(
    host: &Element,
    tag: &str,
) -> Result<Vec<PropertyValue>, FormError> {
    let mut items = Vec::new();
    for c in host
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        c.claim();
        // `<presentation>` ОПЦИОНАЛЕН: отсутствует ⟺ пустой канон `Localized([])` (X с Designer
        // `<Presentation/>`, который тоже декодится в `Localized([])`); эмитится тем же условием
        // «непустой» на write. Witness — RadioButtonField ВнешниеКомпоненты (choiceList без
        // presentation). Корпус EDT НЕ несёт present-but-empty `<presentation/>` (0 вхождений),
        // поэтому absent↔пусто однозначно round-trip'ится.
        let mut expected = 1usize; // <value> — обязателен всегда.
        // `<presentation>` МОЖЕТ ПОВТОРЯТЬСЯ — по одному на язык (как engine-Localized-кодек;
        // witness ERP РНПТМатериаловВПроизводстве.РабочееМесто ru+en). Сливаем пары в порядке
        // документа. Отсутствие ⇒ пустой канон `Localized([])` (X с Designer `<Presentation/>`).
        let pres_els: Vec<&Element> = c
            .children
            .iter()
            .filter(|c| c.local == "presentation" && c.prefix.is_empty())
            .collect();
        let presentation = if pres_els.is_empty() {
            PropertyValue::Localized(Vec::new())
        } else {
            let mut pairs = Vec::new();
            for pe in &pres_els {
                match decode_edt_localized(pe)? {
                    PropertyValue::Localized(p) => pairs.extend(p),
                    _ => {
                        return Err(FormError::Frame(
                            "choiceList <presentation>: localized decode bug (§1.0)".into(),
                        ));
                    }
                }
            }
            expected += pres_els.len();
            PropertyValue::Localized(pairs)
        };
        let val_el = c
            .child("value")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("choiceList: no <value> (§1.0)".into()))?;
        val_el.claim();
        let value = value_codec::decode(ValueDialect::Edt, val_el).map_err(FormError::Frame)?;
        // Опциональная `<picture xsi:type="core:PictureRef"><picture>Ref</picture></picture>` (3-й
        // ребёнок; RadioButtonField СохранениеПечатнойФормы).
        let mut parts = vec![presentation, value];
        if let Some(pe) = c.child("picture").filter(|c| c.prefix.is_empty()) {
            claim_xsi(pe, "picture", "core:PictureRef")?;
            let r = require_single_leaf(pe, "picture", "picture")?;
            // EDT choiceList-picture LoadTransparent НЕ несёт ⇒ дерив (симметрично полю-picture).
            let lt = picture_lt_default(&r);
            parts.push(picture_canon(r, lt));
            expected += 1;
        }
        if c.children.len() != expected {
            return Err(FormError::Frame(
                "choiceList: expected [<presentation>]+<value>[+<picture>] (§1.0)".into(),
            ));
        }
        items.push(PropertyValue::List(parts));
    }
    Ok(items)
}

/// Current localized presentation, required value and optional typed picture.
fn decode_fcldtv_edt(host: &Element) -> Result<PropertyValue, FormError> {
    claim_xsi(host, "value", FCLDTV_EDT)?;
    host.claim();
    let mut presentation = Vec::new();
    let mut picture = None;
    let mut value = None;
    for child in &host.children {
        if !child.prefix.is_empty() { return Err(FormError::Frame("choice wrapper child namespace".into())); }
        match child.local.as_str() {
            "presentation" => {
                let PropertyValue::Localized(pairs) = decode_edt_localized(child)? else { unreachable!() };
                presentation.extend(pairs);
            }
            "picture" if picture.is_none() => { picture = Some(decode_choice_picture_edt(child)?); }
            "value" if value.is_none() => { value = Some(child); }
            _ => return Err(FormError::Frame("choice wrapper unknown or duplicate child".into())),
        }
    }
    let value = value.ok_or_else(|| FormError::Frame("choice wrapper requires value".into()))?;
    let current = if matches!(value.attr("xsi:type"), Some(a) if a.value == "core:FixedArrayValue") {
        claim_xsi(value, "value", "core:FixedArrayValue")?;
        value.claim();
        let mut values = Vec::new();
        for child in &value.children {
            if child.local != "values" || !child.prefix.is_empty() { return Err(FormError::Frame("choice FixedArray member namespace".into())); }
            values.push(decode_fcldtv_edt(child)?);
        }
        PropertyValue::List(values)
    } else {
        value.claim();
        value_codec::decode(ValueDialect::Edt, value).map_err(FormError::Frame)?
    };
    Ok(choice_wrapper_value(current, presentation, picture))
}

/// Прочитать повторяемые EDT `<choiceParameters>` хоста в список пар `[name, value]`.
pub(crate) fn read_choice_parameters_edt(
    host: &Element,
    tag: &str,
) -> Result<Vec<PropertyValue>, FormError> {
    let mut items = Vec::new();
    for c in host
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        c.claim();
        let name_el = c
            .child("name")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("choiceParameters: no <name> (§1.0)".into()))?;
        name_el.claim_with_text();
        let val_el = c
            .child("value")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("choiceParameters: no <value> (§1.0)".into()))?;
        if c.children.len() != 2 {
            return Err(FormError::Frame(
                "choiceParameters: expected <name>+<value> (§1.0)".into(),
            ));
        }
        // БЕЗ-ОБЁРТОЧНЫЙ Undefined (пустой выбор): прямой `<value xsi:type="core:UndefinedValue"/>`
        // (НЕ обёртка FormChoiceListDesTimeValue). Валидируем+клеймим общим value_codec (Undefined ⇒
        // пусто), но в IR держим арность-1 маркер (отличен от завёрнутого-Undefined — §1.0/round-trip).
        let is_bare_undefined =
            matches!(val_el.attr("xsi:type"), Some(a) if a.value == "core:UndefinedValue");
        let item = if is_bare_undefined {
            val_el.claim();
            value_codec::decode(ValueDialect::Edt, val_el).map_err(FormError::Frame)?;
            PropertyValue::List(vec![PropertyValue::Str(name_el.text.clone())])
        } else {
            let value = decode_fcldtv_edt(val_el)?;
            PropertyValue::List(vec![PropertyValue::Str(name_el.text.clone()), value])
        };
        items.push(item);
    }
    Ok(items)
}

/// EDT-разбор вложенного `<datapath xsi:type="form:DataPath"><segments>Путь</segments>`.
fn decode_edt_nested_datapath(host: &Element, ctx: &str) -> Result<String, FormError> {
    let dp = host
        .child("datapath")
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("{ctx}: no <datapath> (§1.0)")))?;
    claim_xsi(dp, "datapath", "form:DataPath")?;
    require_single_leaf(dp, ctx, "segments")
}

/// EDT `<typeLink>` → канон `List([Ref(путь), Int(linkItem)])` (linkItem absent ⇒ 0).
pub(crate) fn read_type_link_edt(el: &Element) -> Result<PropertyValue, FormError> {
    el.claim();
    let li = el.child("linkItem").filter(|c| c.prefix.is_empty());
    let link_item = match &li {
        Some(li) => {
            li.claim_with_text();
            parse_int(&li.text, "typeLink linkItem")?
        }
        None => 0,
    };
    let path = decode_edt_nested_datapath(el, "typeLink")?;
    if el.children.len() != if li.is_some() { 2 } else { 1 } {
        return Err(FormError::Frame(
            "typeLink: unexpected children (§1.0)".into(),
        ));
    }
    Ok(PropertyValue::List(vec![
        PropertyValue::Ref(path),
        PropertyValue::Int(link_item),
    ]))
}

/// Прочитать повторяемые EDT `<choiceParameterLinks>` хоста в список пунктов.
fn read_choice_parameter_links_edt(
    host: &Element,
    tag: &str,
) -> Result<Vec<PropertyValue>, FormError> {
    let mut items = Vec::new();
    for c in host
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        c.claim();
        let name_el = c
            .child("name")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("choiceParameterLinks: no <name> (§1.0)".into()))?;
        name_el.claim_with_text();
        // changeMode — опц. (ERP-witness ЧОА Международный: `<changeMode>DontChange` между
        // name и datapath; Clear EDT ОПУСКАЕТ — SSL 0 вхождений; явный Clear канонизуем
        // омиссией — sparse, X-равно Designer-стороне).
        let change_mode = c
            .child("changeMode")
            .filter(|c| c.prefix.is_empty())
            .map(|m| {
                m.claim_with_text();
                m.text.clone()
            });
        let path = decode_edt_nested_datapath(c, "choiceParameterLinks")?;
        let expect_n = 2 + usize::from(change_mode.is_some());
        if c.children.len() != expect_n {
            return Err(FormError::Frame(
                "choiceParameterLinks: expected <name>[+<changeMode>]+<datapath> (§1.0)".into(),
            ));
        }
        let mut entry = vec![
            PropertyValue::Str(name_el.text.clone()),
            PropertyValue::Ref(path),
        ];
        if let Some(m) = change_mode.filter(|m| m != "Clear") {
            entry.push(PropertyValue::Enum(Token::new(m)));
        }
        items.push(PropertyValue::List(entry));
    }
    Ok(items)
}

fn decode_choice_picture_edt(el:&Element)->Result<PropertyValue,FormError>{
    if el.attr("xsi:type").map(|a|a.value.as_str())!=Some("form:FormPicture"){return decode_edt_picture(el,"picture");}
    claim_xsi(el,"picture","form:FormPicture")?;el.claim();
    let(mut pixel,mut glyph)=(None,None);
    for child in &el.children {
        let slot=match(child.prefix.as_str(),child.local.as_str()){("","transparentPixel")=>&mut pixel,("","glyph")=>&mut glyph,_=>return Err(FormError::Frame("unknown FormPicture field".into()))};
        if slot.is_some(){return Err(FormError::Frame("duplicate FormPicture Point".into()));}
        let value=match crate::md_picture::decode_point(child){
            morph1c_core::engine::Decoded::Present(value)=>value,
            morph1c_core::engine::Decoded::Error(e)=>return Err(FormError::Frame(e)),
            _=>return Err(FormError::Frame("missing FormPicture Point".into())),
        };
        *slot=Some(crate::transparent_pixel::pixel_of(&value).map_err(FormError::Frame)?);
    }
    let value=match pixel{Some(point)=>picture_canon_px(String::new(),point),None=>picture_canon(String::new(),false)};
    picture_with_glyph(value,glyph)
}
