//! FIELDS · Designer dialect READ codec — decode leaf values from Designer form XML (edit-mode pair, period/border/picture/localized), plus the out-of-band structured readers (choiceList/choiceParameters/typeLink/choiceParameterLinks). §1.0 totality.

use super::*;

/// Прочитать поля таблицы данного региона с Designer-элемента контрола. Поля-атрибуты
/// (`des_attr`) читаются из атрибутов `el`. [`Policy::EditMode`] читает ПАРУ тегов
/// (`EditMode` + `AutoEditMode`).
///
/// `owner` — вид-владелец полей таблицы (см. [`read_fields_edt`]): по нему и каноническому
/// имени поля гейтится РЕКОНСТРУКЦИЯ отсутствующего свойства версией источника.
pub(crate) fn read_fields_designer(
    owner: &str,
    el: &Element,
    table: &[FieldProj],
    region: Region,
    bag: &mut Vec<(FieldId, PropertyValue)>,
) -> Result<(), FormError> {
    for entry in table.iter().filter(|e| e.region == region) {
        if let Codec::ChoiceList = entry.codec {
            if let Some(cl) = el
                .children
                .iter()
                .find(|c| c.local == entry.des && c.prefix.is_empty())
            {
                let items = read_choice_list_designer(cl)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        if let Codec::ChoiceParameters = entry.codec {
            if let Some(cl) = el
                .children
                .iter()
                .find(|c| c.local == entry.des && c.prefix.is_empty())
            {
                let items = read_choice_parameters_designer(cl)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        if let Codec::ChoiceParameterLinks = entry.codec {
            if let Some(cl) = el
                .children
                .iter()
                .find(|c| c.local == entry.des && c.prefix.is_empty())
            {
                let items = read_choice_parameter_links_designer(cl)?;
                if !items.is_empty() {
                    bag.push((entry.id, PropertyValue::List(items)));
                }
            }
            continue;
        }
        if let Codec::TypeLink = entry.codec {
            if let Some(tl) = el
                .children
                .iter()
                .find(|c| c.local == entry.des && c.prefix.is_empty())
            {
                bag.push((entry.id, read_type_link_designer(tl)?));
            }
            continue;
        }
        if let Policy::EditMode = entry.policy {
            apply_read_policy(
                owner,
                entry,
                read_designer_edit_mode(el, entry)?,
                FormDialect::Designer,
                bag,
            )?;
            continue;
        }
        let present = if entry.des_attr {
            match el.attr(entry.des) {
                Some(a) => {
                    a.claimed.set(true);
                    Some(PropertyValue::Enum(Token::new(a.value.clone())))
                }
                None => None,
            }
        } else {
            let node = el
                .children
                .iter()
                .find(|c| c.local == entry.des && c.prefix.is_empty());
            match node {
                Some(n) if matches!(entry.codec, Codec::DataPath) && super::super::tables::field_kind(owner).is_some() => {
                    n.claim_with_text();
                    if !n.children.is_empty() { return Err(FormError::Frame("DataPath must be a text leaf".into())); }
                    Some(super::super::data_path::read_native_field(&n.text)?)
                }
                Some(n) => Some(decode_designer(entry, n)?),
                None => None,
            }
        };
        apply_read_policy(owner, entry, present, FormDialect::Designer, bag)?;
    }
    Ok(())
}

/// Прочитать Designer-пару `EditMode`/`AutoEditMode` в канонический литерал.
fn read_designer_edit_mode(
    el: &Element,
    entry: &FieldProj,
) -> Result<Option<PropertyValue>, FormError> {
    let em = el
        .children
        .iter()
        .find(|c| c.local == entry.des && c.prefix.is_empty());
    let auto = el
        .children
        .iter()
        .find(|c| c.local == "AutoEditMode" && c.prefix.is_empty());
    let canon = match (em, auto) {
        (None, None) => return Ok(None), // absent ⇒ Designer-дефолт (fill политики).
        (Some(e), Some(a)) => {
            e.claim_with_text();
            a.claim_with_text();
            if a.text != "true" {
                return Err(FormError::Frame(format!(
                    "<AutoEditMode>={:?}, want true (§1.0)",
                    a.text
                )));
            }
            if e.text != ff::EDIT_MODE_AUTO_RESOLVED {
                return Err(FormError::Frame(format!(
                    "<EditMode>={:?} with AutoEditMode=true: unmodeled resolution (§1.0)",
                    e.text
                )));
            }
            ff::EDIT_MODE_AUTO.to_string()
        }
        (Some(e), None) => {
            e.claim_with_text();
            e.text.clone()
        }
        (None, Some(_)) => {
            return Err(FormError::Frame(
                "<AutoEditMode> without <EditMode>: unmodeled (§1.0)".into(),
            ));
        }
    };
    Ok(Some(PropertyValue::Enum(Token::new(canon))))
}

/// Декодировать Designer-узел поля по кодеку (claim'ит узел).
pub(crate) fn decode_designer(entry: &FieldProj, el: &Element) -> Result<PropertyValue, FormError> {
    let tag = entry.des;
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
            match map.iter().find(|(_, des)| *des == el.text) {
                Some((canon, _)) => Ok(PropertyValue::Enum(Token::new(*canon))),
                None => Err(FormError::Frame(format!(
                    "<{tag}>={:?}: unmapped enum literal (§1.0)",
                    el.text
                ))),
            }
        }
        Codec::Text => {
            el.claim_with_text();
            Ok(PropertyValue::Str(el.text.clone()))
        }
        Codec::RefText => {
            el.claim_with_text();
            Ok(PropertyValue::Ref(el.text.clone()))
        }
        Codec::Localized => decode_designer_localized(el),
        Codec::Color => {
            el.claim_with_text();
            Ok(PropertyValue::Ref(color_from_designer(&el.text, tag)?))
        }
        Codec::PictureRef => decode_designer_picture(el, tag),
        Codec::Value => {
            // Designer-скаляр xsi-вида через ОБЩИЙ value_codec (xs:decimal/xs:string/xsi:nil/…).
            el.claim();
            value_codec::decode(ValueDialect::Designer, el).map_err(FormError::Frame)
        }
        Codec::DataPath => {
            el.claim_with_text();
            if !el.children.is_empty() {
                return Err(FormError::Frame(format!("<{tag}>: must be a text leaf")));
            }
            super::super::data_path::read_native(&el.text)
        }
        Codec::MdObjectRef => {
            claim_xsi(el, tag, "xr:MDObjectRef")?;
            el.claim_with_text();
            if !el.children.is_empty() {
                return Err(FormError::Frame(format!("<{tag}>: must be a text leaf")));
            }
            Ok(PropertyValue::Ref(el.text.clone()))
        }
        Codec::CommonBool => {
            // Безролевой: `<Tag><xr:Common>false</xr:Common></Tag>` (только не-дефолт `false` —
            // Designer опускает `true` → absent-дефолт). РОЛЕВОЙ (userVisible-права): `<xr:Common>B`
            // + `<xr:Value name="R">B</xr:Value>×N` ⟺ EDT `<for>`; канон — `List([Bool(common),
            // List([Str(role), Bool(v)])…])` (зеркало `F_CMD_USE`/`read_designer_common_flag`).
            // Witness РесурсныеСпецификации (common=false), мирСкважины (common=true).
            el.claim();
            let c = el
                .child("Common")
                .filter(|c| c.prefix == "xr")
                .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <xr:Common>")))?;
            c.claim_with_text();
            let common = match c.text.as_str() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(FormError::Frame(format!(
                        "<{tag}><xr:Common>={other:?}, want bool (§1.0)"
                    )));
                }
            };
            let mut roles = Vec::new();
            for v in &el.children {
                if v.prefix == "xr" && v.local == "Common" {
                    continue;
                }
                if !(v.prefix == "xr" && v.local == "Value") {
                    return Err(FormError::Frame(format!(
                        "<{tag}>: unexpected extra children (§1.0)"
                    )));
                }
                let name = v.attr("name").ok_or_else(|| {
                    FormError::Frame(format!("<{tag}><xr:Value>: no name (§1.0)"))
                })?;
                name.claimed.set(true);
                v.claim_with_text();
                let val = match v.text.as_str() {
                    "true" => true,
                    "false" => false,
                    other => {
                        return Err(FormError::Frame(format!(
                            "<{tag}><xr:Value name={:?}>={other:?}, want bool (§1.0)",
                            name.value
                        )));
                    }
                };
                roles.push(PropertyValue::List(vec![
                    PropertyValue::Str(name.value.clone()),
                    PropertyValue::Bool(val),
                ]));
            }
            if roles.is_empty() {
                // Безролевой: только `false` витнессирован (Designer опускает `true`).
                if common {
                    Err(FormError::Frame(format!(
                        "<{tag}><xr:Common>true</xr:Common> without <xr:Value>-roles is unwitnessed \
                         (Designer omits true → absent-default; §1.0)"
                    )))
                } else {
                    Ok(PropertyValue::Bool(false))
                }
            } else {
                let mut entries = vec![PropertyValue::Bool(common)];
                entries.extend(roles);
                Ok(PropertyValue::List(entries))
            }
        }
        Codec::UndefinedValue => {
            // `<Tag xsi:nil="true"/>` — самозакрытый nil-маркер.
            let nil = el
                .attr("xsi:nil")
                .ok_or_else(|| FormError::Frame(format!("<{tag}>: no xsi:nil (§1.0)")))?;
            if nil.value != "true" {
                return Err(FormError::Frame(format!(
                    "<{tag}> xsi:nil={:?}, want true",
                    nil.value
                )));
            }
            nil.claimed.set(true);
            el.claim();
            if !el.children.is_empty() || !el.text.is_empty() {
                return Err(FormError::Frame(format!(
                    "<{tag}>: nil must be empty (§1.0)"
                )));
            }
            Ok(PropertyValue::Bool(true))
        }
        Codec::Border => decode_designer_border(el, tag),
        Codec::ScrollBar => {
            // Designer-сторона — BOOL: `true`=ScrollAlways, `false`=ScrollNever.
            el.claim_with_text();
            let canon = match el.text.as_str() {
                "true" => ff::SCROLL_BAR_ALWAYS,
                "false" => ff::SCROLL_BAR_NEVER,
                other => {
                    return Err(FormError::Frame(format!(
                        "<{tag}>={other:?}: scrollbar wants bool (§1.0)"
                    )));
                }
            };
            if !el.children.is_empty() {
                return Err(FormError::Frame(format!(
                    "<{tag}>: must be a bool leaf (§1.0)"
                )));
            }
            Ok(PropertyValue::Enum(Token::new(canon)))
        }
        Codec::Period => decode_designer_period(el, tag),
        Codec::Type => {
            el.claim();
            type_codec::decode(TypeDialect::Designer, el).map_err(FormError::Frame)
        }
        Codec::ChoiceList => Err(FormError::Frame(
            "ChoiceList decoded via table engine (bug — handled out-of-band)".into(),
        )),
        Codec::ChoiceParameters => Err(FormError::Frame(
            "ChoiceParameters decoded via table engine (bug — handled out-of-band)".into(),
        )),
        Codec::ChoiceParameterLinks | Codec::TypeLink => Err(FormError::Frame(
            "TypeLink/ChoiceParameterLinks decoded via table engine (bug — handled out-of-band)"
                .into(),
        )),
    }
}

/// Designer `<Period><v8:variant xsi:type="v8:StandardPeriodVariant">Custom</v8:variant>
/// <v8:startDate>X</v8:startDate><v8:endDate>Y</v8:endDate></Period>` → `List([Str,Str])`.
/// `variant`=`Custom` (каркас) сверяется; иное — §1.0-ошибка.
fn decode_designer_period(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim();
    let variant = el
        .children
        .iter()
        .find(|c| c.local == "variant" && c.prefix == "v8")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <v8:variant> (§1.0)")))?;
    let xt = variant
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: <v8:variant> no xsi:type (§1.0)")))?;
    if xt.value != PERIOD_VARIANT_XSI {
        return Err(FormError::Frame(format!(
            "<{tag}>: <v8:variant> xsi:type={:?}, want {PERIOD_VARIANT_XSI:?} (§1.0)",
            xt.value
        )));
    }
    xt.claimed.set(true);
    variant.claim_with_text();
    if variant.text != PERIOD_VARIANT_VALUE {
        return Err(FormError::Frame(format!(
            "<{tag}>: <v8:variant>={:?}, only {PERIOD_VARIANT_VALUE:?} modeled (§1.0)",
            variant.text
        )));
    }
    let start = child_leaf_text(el, "v8", "startDate", tag)?;
    let end = child_leaf_text(el, "v8", "endDate", tag)?;
    if el.children.len() != 3 {
        return Err(FormError::Frame(format!(
            "<{tag}>: expected <v8:variant>+<v8:startDate>+<v8:endDate> (§1.0)"
        )));
    }
    Ok(PropertyValue::List(vec![
        PropertyValue::Str(start),
        PropertyValue::Str(end),
    ]))
}

/// Designer `<Border width="1"><v8ui:style xsi:type="v8ui:ControlBorderType">WithoutBorder
/// </v8ui:style></Border>` — эмитится ТОЛЬКО не-`Single` (Single Designer опускает через KEEP).
fn decode_designer_border(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    if let Some(reference) = el.attr("ref") {
        let Some(name) = reference
            .value
            .strip_prefix("style:")
            .filter(|n| !n.is_empty())
        else {
            return Err(FormError::Frame(
                "Border ref requires a named style reference".into(),
            ));
        };
        if !el.children.is_empty() || !el.text.is_empty() {
            return Err(FormError::Frame(
                "Border ref must have no children or text".into(),
            ));
        }
        reference.claimed.set(true);
        el.claim();
        return Ok(PropertyValue::Ref(format!("Style.{name}")));
    }
    el.claim();
    let w = el
        .attr("width")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: Border missing width attr (§1.0)")))?;
    w.claimed.set(true);
    let s = el
        .child("style")
        .filter(|c| c.prefix == "v8ui")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <v8ui:style> (§1.0)")))?;
    let xt = s
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: <v8ui:style> no xsi:type (§1.0)")))?;
    if xt.value != "v8ui:ControlBorderType" {
        return Err(FormError::Frame(format!(
            "<{tag}>: <v8ui:style> xsi:type={:?}, want v8ui:ControlBorderType (§1.0)",
            xt.value
        )));
    }
    xt.claimed.set(true);
    s.claim_with_text();
    if el.children.len() != 1 {
        return Err(FormError::Frame(format!(
            "<{tag}>: Border unexpected children (§1.0)"
        )));
    }
    Ok(border_canon(&s.text, &w.value))
}

/// Designer `<Tag><xr:Ref>R</xr:Ref><xr:LoadTransparent>b</…>`: `LoadTransparent` — НЕЗАВИСИМЫЙ
/// витнессированный флаг (ERP census: `RowsPicture`/`Picture`/`ValuesPicture`/`HeaderPicture`
/// несут `CommonPicture.*`/`Abs` c `LoadTransparent="true"` ПРОТИВ префикс-правила
/// StdPicture⇒true — витнесс Catalog.ГруппыСотрудников.ФормаСписка). Хранится в каноне
/// `List([Ref, Bool])` ([`picture_canon`]), НЕ деривится/отвергается.
///
/// БЕЗ `<xr:Ref>` — АБСОЛЮТНАЯ (инлайн-сайдкар) картинка `<xr:Abs>{Tag}.{ext}</xr:Abs>`
/// (базовое имя ВСЕГДА = имени тега — сверено 19/19 SSL; ext ∈ zip/png/svg). Канон —
/// `List([Ref("abs:{ext}"), Bool(lt)])` (EDT-аналог — пустой маркер `form:FormPicture`,
/// LoadTransparent EDT НЕ несёт ⇒ на edt-read деривится).
pub(crate) fn decode_designer_picture(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim();
    let r = match el.child("Ref").filter(|c| c.prefix == "xr") {
        Some(r) => r,
        None => return decode_designer_picture_abs(el, tag),
    };
    r.claim_with_text();
    let lt = el
        .child("LoadTransparent")
        .filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <xr:LoadTransparent>")))?;
    lt.claim_with_text();
    let load_transparent = picture_lt_value(&lt.text, tag)?;
    let pixel = el.child("TransparentPixel").filter(|c| c.prefix == "xr");
    if el.children.len() != 2 + usize::from(pixel.is_some()) {
        return Err(FormError::Frame(format!(
            "<{tag}>: unexpected picture children"
        )));
    }
    if let Some(pixel) = pixel {
        if !load_transparent {
            return Err(FormError::Frame(
                "Picture TransparentPixel requires LoadTransparent=true".into(),
            ));
        }
        return Ok(picture_canon_px(
            r.text.clone(),
            decode_designer_transparent_pixel(pixel, tag)?,
        ));
    }
    Ok(picture_canon(r.text.clone(), load_transparent))
}

/// Разобрать Abs-вариант Designer-картинки (см. [`decode_designer_picture`]). `LoadTransparent` —
/// НЕЗАВИСИМЫЙ флаг (ERP census: `ValuesPicture`/`Picture`/`HeaderPicture` Abs c `true` — витнесс
/// Catalog.ИсточникиДанныхПланирования.ФормаЗаполнения).
fn decode_designer_picture_abs(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    let a = el
        .child("Abs")
        .filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <xr:Ref>/<xr:Abs> (§1.0)")))?;
    a.claim_with_text();
    let ext = a
        .text
        .strip_prefix(tag)
        .and_then(|rest| rest.strip_prefix('.'))
        .ok_or_else(|| {
            FormError::Frame(format!(
                "<{tag}>: <xr:Abs>={:?}, want \"{tag}.<ext>\" (§1.0 — witnessed base=tag)",
                a.text
            ))
        })?;
    let lt = el
        .child("LoadTransparent")
        .filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <xr:LoadTransparent>")))?;
    lt.claim_with_text();
    let load_transparent = picture_lt_value(&lt.text, tag)?;
    // Опциональный ТРЕТИЙ ребёнок `<xr:TransparentPixel x=".." y=".."/>` (DENSE-атрибуты) —
    // ВСТРОЕННАЯ картинка с прозрачным пикселем. LT его ДЕНОРМАЛИЗУЕТ (census ERP форм 100/100:
    // Abs+LT=true ⟺ TransparentPixel). §1.0: пиксель при LT=false / LT=true без пикселя —
    // невитнессированное противоречие → отказ (не догадка).
    let pixel_el = el.child("TransparentPixel").filter(|c| c.prefix == "xr");
    let has_pixel = pixel_el.is_some();
    let canon = match (load_transparent, pixel_el) {
        (false, None) => picture_canon(format!("abs:{ext}"), false),
        (true, Some(px)) => {
            let pixel = decode_designer_transparent_pixel(px, tag)?;
            picture_canon_px(format!("abs:{ext}"), pixel)
        }
        (true, None) => {
            return Err(FormError::Frame(format!(
                "<{tag}>: <xr:LoadTransparent>true</…> without <xr:TransparentPixel> — \
                 unwitnessed (LT denormalizes pixel presence, §1.0)"
            )));
        }
        (false, Some(_)) => {
            return Err(FormError::Frame(format!(
                "<{tag}>: <xr:TransparentPixel> with <xr:LoadTransparent>false</…> — \
                 unwitnessed (LT denormalizes pixel presence, §1.0)"
            )));
        }
    };
    if el.children.len() != if has_pixel { 3 } else { 2 } {
        return Err(FormError::Frame(format!(
            "<{tag}>: unexpected extra children (§1.0)"
        )));
    }
    Ok(canon)
}

/// Разобрать Designer `<xr:TransparentPixel x="N" y="M"/>` → `(x, y)`. DENSE-атрибуты (оба
/// обязательны — census ERP форм 100/100; `x="0"` пишется явно, в отличие от sparse EDT-листьев),
/// строгий int-парс, без детей/текста (§1.0). Зеркало `morph1c_pipeline::picture_read::parse_pixel`
/// (config/CommonPicture-обёртка несёт ту же форму).
fn decode_designer_transparent_pixel(el: &Element, tag: &str) -> Result<(i64, i64), FormError> {
    el.claim();
    if !el.children.is_empty() || !el.text.is_empty() {
        return Err(FormError::Frame(format!(
            "<{tag}>: <xr:TransparentPixel> must be an empty element (§1.0)"
        )));
    }
    let coord = |name: &str| -> Result<i64, FormError> {
        let a = el.attr(name).ok_or_else(|| {
            FormError::Frame(format!(
                "<{tag}>: <xr:TransparentPixel> has no @{name} (DENSE attrs witnessed, §1.0)"
            ))
        })?;
        a.claimed.set(true);
        a.value.parse().map_err(|e| {
            FormError::Frame(format!(
                "<{tag}>: <xr:TransparentPixel> @{name}={:?} is not an integer: {e} (§1.0)",
                a.value
            ))
        })
    };
    Ok((coord("x")?, coord("y")?))
}

/// Designer локализованные `<v8:item><v8:lang>/<v8:content>`-пары (БЕЗ formatted-атрибута —
/// заголовки ПОЛЕЙ его не несут, в отличие от `LabelDecoration`).
fn decode_designer_localized(t: &Element) -> Result<PropertyValue, FormError> {
    t.claim();
    let mut pairs = Vec::new();
    for item in &t.children {
        if item.local != "item" || item.prefix != "v8" {
            return Err(FormError::Frame(format!(
                "<{}>: expected <v8:item>",
                t.local
            )));
        }
        item.claim();
        let lang = item
            .child("lang")
            .filter(|c| c.prefix == "v8")
            .ok_or_else(|| FormError::Frame(format!("<{}>: no v8:lang", t.local)))?;
        let content = item
            .child("content")
            .filter(|c| c.prefix == "v8")
            .ok_or_else(|| FormError::Frame(format!("<{}>: no v8:content", t.local)))?;
        lang.claim_with_text();
        content.claim_with_text();
        if item.children.len() != 2 {
            return Err(FormError::Frame(format!(
                "<{}>: <v8:item> must have exactly lang+content",
                t.local
            )));
        }
        pairs.push((Lang::new(lang.text.clone()), content.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// Прочитать Designer `<ChoiceList>` (c `<xr:Item>`-детьми) в список пар.
pub(crate) fn read_choice_list_designer(cl: &Element) -> Result<Vec<PropertyValue>, FormError> {
    cl.claim();
    let mut items = Vec::new();
    for item in cl.children.iter() {
        if item.local != "Item" || item.prefix != "xr" {
            return Err(FormError::Frame(format!(
                "ChoiceList: unexpected child <{}:{}> (§1.0)",
                item.prefix, item.local
            )));
        }
        item.claim();
        // Каркас: пустой `<xr:Presentation/>` + константа `<xr:CheckState>0`.
        let pres = item
            .child("Presentation")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("ChoiceList item: no <xr:Presentation>".into()))?;
        pres.claim();
        if !pres.children.is_empty() || !pres.text.is_empty() {
            return Err(FormError::Frame(
                "ChoiceList: <xr:Presentation> must be empty (§1.0)".into(),
            ));
        }
        let cs = item
            .child("CheckState")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("ChoiceList item: no <xr:CheckState>".into()))?;
        cs.claim_with_text();
        if cs.text != "0" {
            return Err(FormError::Frame(format!(
                "ChoiceList: <xr:CheckState>={:?}, want 0 (§1.0)",
                cs.text
            )));
        }
        // `<xr:Value xsi:type="FormChoiceListDesTimeValue">` — вложенные Presentation + Value.
        let xv = item
            .child("Value")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| FormError::Frame("ChoiceList item: no <xr:Value>".into()))?;
        xv.claim();
        let xt = xv
            .attr("xsi:type")
            .ok_or_else(|| FormError::Frame("ChoiceList xr:Value: no xsi:type".into()))?;
        if xt.value != "FormChoiceListDesTimeValue" {
            return Err(FormError::Frame(format!(
                "ChoiceList xr:Value xsi:type={:?}: unmodeled (§1.0)",
                xt.value
            )));
        }
        xt.claimed.set(true);
        let pres_in = xv
            .child("Presentation")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("ChoiceList xr:Value: no <Presentation>".into()))?;
        let presentation = decode_designer_localized(pres_in)?;
        let val_el = xv
            .child("Value")
            .filter(|c| c.prefix.is_empty())
            .ok_or_else(|| FormError::Frame("ChoiceList xr:Value: no <Value>".into()))?;
        val_el.claim();
        let value =
            value_codec::decode(ValueDialect::Designer, val_el).map_err(FormError::Frame)?;
        // Опциональная `<Picture><xr:Ref>Ref</xr:Ref><xr:LoadTransparent>b</xr:LoadTransparent>
        // </Picture>` внутри FormChoiceListDesTimeValue (RadioButtonField СохранениеПечатнойФормы).
        let mut parts = vec![presentation, value];
        let expected_xv = match xv.child("Picture").filter(|c| c.prefix.is_empty()) {
            Some(pe) => {
                parts.push(decode_designer_picture(pe, "Picture")?);
                3
            }
            None => 2,
        };
        if xv.children.len() != expected_xv {
            return Err(FormError::Frame(
                "ChoiceList xr:Value: expected Presentation+Value[+Picture] (§1.0)".into(),
            ));
        }
        if item.children.len() != 3 {
            return Err(FormError::Frame(
                "ChoiceList xr:Item: expected Presentation+CheckState+Value (§1.0)".into(),
            ));
        }
        items.push(PropertyValue::List(parts));
    }
    Ok(items)
}

/// Designer: разобрать обёртку `<app:value xsi:type="FormChoiceListDesTimeValue">` → скаляр `ValueSpec`.
/// Дети: пустой `<Presentation/>` + скаляр `<Value xsi:type="xs:…">`.
// Native empty Picture is a null containment; nonempty values use the same
// fully typed picture codec as other form fields.
fn decode_fcldtv_designer(host: &Element) -> Result<PropertyValue, FormError> {
    claim_xsi(host, "value", FCLDTV_DES)?;
    host.claim();
    let mut presentation = Vec::new();
    let mut picture = None;
    let mut value = None;
    let mut saw_picture = false;
    let mut saw_presentation = false;
    for child in &host.children {
        if !child.prefix.is_empty() { return Err(FormError::Frame("choice wrapper child namespace".into())); }
        match child.local.as_str() {
            "Presentation" if !saw_presentation => {
                saw_presentation = true;
                let PropertyValue::Localized(pairs) = decode_designer_localized(child)? else { unreachable!() };
                presentation = pairs;
            }
            "Picture" if !saw_picture => {
                saw_picture = true;
                if child.children.is_empty() && child.attrs.is_empty() && child.text.is_empty() { child.claim(); }
                else { picture = Some(decode_choice_picture(child)?); }
            }
            "Value" if value.is_none() => { value = Some(child); }
            _ => return Err(FormError::Frame("choice wrapper unknown or duplicate child".into())),
        }
    }
    let val = value.ok_or_else(|| FormError::Frame("choice wrapper requires Value".into()))?;
    let current = if matches!(val.attr("xsi:type"), Some(a) if a.value == "v8:FixedArray") {
        claim_xsi(val, "Value", "v8:FixedArray")?;
        val.claim();
        let mut values = Vec::new();
        for child in &val.children {
            if child.local != "Value" || child.prefix != "v8" { return Err(FormError::Frame("choice FixedArray member namespace".into())); }
            values.push(decode_fcldtv_designer(child)?);
        }
        PropertyValue::List(values)
    } else {
        val.claim();
        value_codec::decode(ValueDialect::Designer, val).map_err(FormError::Frame)?
    };
    Ok(choice_wrapper_value(current, presentation, picture))
}

/// Прочитать Designer-контейнер `<ChoiceParameters>` (c `<app:item>`-детьми) в список пар.
pub(crate) fn read_choice_parameters_designer(
    cl: &Element,
) -> Result<Vec<PropertyValue>, FormError> {
    cl.claim();
    if !cl.text.is_empty() {
        return Err(FormError::Frame(
            "ChoiceParameters container must have no text (§1.0)".into(),
        ));
    }
    let mut items = Vec::new();
    for item in cl.children.iter() {
        if item.local != "item" || item.prefix != "app" {
            return Err(FormError::Frame(format!(
                "ChoiceParameters: expected <app:item>, got <{}:{}> (§1.0)",
                item.prefix, item.local
            )));
        }
        let name = item.attr("name").ok_or_else(|| {
            FormError::Frame("ChoiceParameters <app:item>: no @name (§1.0)".into())
        })?;
        name.claimed.set(true);
        item.claim();
        if item.children.len() != 1 {
            return Err(FormError::Frame(
                "ChoiceParameters <app:item>: expected single <app:value> (§1.0)".into(),
            ));
        }
        let value_el = &item.children[0];
        if value_el.local != "value" || value_el.prefix != "app" {
            return Err(FormError::Frame(format!(
                "ChoiceParameters: expected <app:value>, got <{}:{}> (§1.0)",
                value_el.prefix, value_el.local
            )));
        }
        // БЕЗ-ОБЁРТОЧНЫЙ Undefined (пустой выбор): `<app:value xsi:nil="true"/>` (НЕ обёртка
        // FormChoiceListDesTimeValue). Твин EDT `<value xsi:type="core:UndefinedValue"/>`; канон —
        // арность-1 маркер (отличен от завёрнутого-Undefined `FCLDTV`+внутренний nil).
        let is_bare_undefined = value_el.attr("xsi:nil").is_some();
        let entry = if is_bare_undefined {
            value_el.claim();
            value_codec::decode(ValueDialect::Designer, value_el).map_err(FormError::Frame)?;
            PropertyValue::List(vec![PropertyValue::Str(name.value.clone())])
        } else {
            let value = decode_fcldtv_designer(value_el)?;
            PropertyValue::List(vec![PropertyValue::Str(name.value.clone()), value])
        };
        items.push(entry);
    }
    Ok(items)
}

/// Designer `<TypeLink><xr:DataPath>путь</xr:DataPath><xr:LinkItem>N</xr:LinkItem></TypeLink>`
/// → канон (вложенные листы — в `xr:`-неймспейсе, witness ШаблоныАнкет).
pub(crate) fn read_type_link_designer(el: &Element) -> Result<PropertyValue, FormError> {
    el.claim();
    let dp = el
        .child("DataPath")
        .filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame("TypeLink: no <xr:DataPath> (§1.0)".into()))?;
    dp.claim_with_text();
    let li = el
        .child("LinkItem")
        .filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame("TypeLink: no <xr:LinkItem> (§1.0)".into()))?;
    li.claim_with_text();
    if el.children.len() != 2 {
        return Err(FormError::Frame(
            "TypeLink: unexpected children (§1.0)".into(),
        ));
    }
    Ok(PropertyValue::List(vec![
        super::super::data_path::read_native(&dp.text)?,
        PropertyValue::Int(parse_int(&li.text, "TypeLink LinkItem")?),
    ]))
}

/// Прочитать Designer-контейнер `<ChoiceParameterLinks>` (`<xr:Link>`-дети; вложенные
/// листы — в `xr:`-неймспейсе, witness ЗначенияСвойствОбъектов) в список пунктов.
fn read_choice_parameter_links_designer(cl: &Element) -> Result<Vec<PropertyValue>, FormError> {
    cl.claim();
    let mut items = Vec::new();
    for link in cl.children.iter() {
        if link.local != "Link" || link.prefix != "xr" {
            return Err(FormError::Frame(format!(
                "ChoiceParameterLinks: expected <xr:Link>, got <{}:{}> (§1.0)",
                link.prefix, link.local
            )));
        }
        link.claim();
        let name = link
            .child("Name")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("ChoiceParameterLinks Link: no <xr:Name> (§1.0)".into())
            })?;
        name.claim_with_text();
        let dp = link
            .child("DataPath")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("ChoiceParameterLinks Link: no <xr:DataPath> (§1.0)".into())
            })?;
        claim_xsi(dp, "DataPath", "xs:string")?;
        dp.claim_with_text();
        // ValueChange: Designer эмитит ВСЕГДА; канон — SPARSE третий элемент (только
        // НЕ-Clear; ERP-witness ЧОА Международный.ФормаСчета `DontChange` ⟷ EDT
        // `<changeMode>DontChange`; дескрипторный кодек `choice_param_links` — та же модель).
        let vc = link
            .child("ValueChange")
            .filter(|c| c.prefix == "xr")
            .ok_or_else(|| {
                FormError::Frame("ChoiceParameterLinks Link: no <xr:ValueChange> (§1.0)".into())
            })?;
        vc.claim_with_text();
        if link.children.len() != 3 {
            return Err(FormError::Frame(
                "ChoiceParameterLinks Link: expected Name+DataPath+ValueChange (§1.0)".into(),
            ));
        }
        let mut entry = vec![
            PropertyValue::Str(name.text.clone()),
            super::super::data_path::read_native(&dp.text)?,
        ];
        if vc.text != "Clear" {
            entry.push(PropertyValue::Enum(Token::new(vc.text.clone())));
        }
        items.push(PropertyValue::List(entry));
    }
    Ok(items)
}

fn decode_choice_picture(el: &Element) -> Result<PropertyValue, FormError> {
    let Some(abs) = el.child("Abs").filter(|c| c.prefix == "xr") else {
        return decode_designer_picture(el, "Picture");
    };
    super::super::picture_semantics::validate_asset_path(&abs.text)?;
    el.claim(); abs.claim_with_text();
    let lt = el.child("LoadTransparent").filter(|c| c.prefix == "xr")
        .ok_or_else(|| FormError::Frame("choice Picture: missing LoadTransparent".into()))?;
    lt.claim_with_text();
    let flag = picture_lt_value(&lt.text, "Picture")?;
    let pixel = el.child("TransparentPixel").filter(|c| c.prefix == "xr");
    if el.children.len() != if pixel.is_some() { 3 } else { 2 } {
        return Err(FormError::Frame("choice Picture: duplicate/unknown child".into()));
    }
    let reference = format!("abs-file:{}", abs.text);
    match pixel {
        Some(pixel) if flag => Ok(picture_canon_px(reference, decode_designer_transparent_pixel(pixel,"Picture")?)),
        Some(_) => Err(FormError::Frame("choice Picture: pixel requires LoadTransparent=true".into())),
        None => Ok(picture_canon(reference, flag)),
    }
}
