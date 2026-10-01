//! FIELDS · EDT dialect WRITE codec — render canonical field values to EDT form XML by codec/policy, plus the out-of-band structured emitters (choiceList/choiceParameters/typeLink/choiceParameterLinks).

use super::*;

/// Эмитить одно поле в EDT-выход по политике (или ничего).
pub(crate) fn emit_field_edt(
    out: &mut OutElement,
    entry: &FieldProj,
    bag: &[(FieldId, PropertyValue)],
) -> Result<(), FormError> {
    if let Codec::ChoiceList = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            emit_choice_list_edt(out, items)?;
        }
        return Ok(());
    }
    if let Codec::ChoiceParameters = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            emit_choice_parameters_edt(out, items)?;
        }
        return Ok(());
    }
    if let Codec::ChoiceParameterLinks = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            emit_choice_parameter_links_edt(out, items)?;
        }
        return Ok(());
    }
    if let Codec::TypeLink = entry.codec {
        if let Some(v) = bag_get(bag, entry.id) {
            out.push(emit_type_link_edt(v)?);
        }
        return Ok(());
    }
    let value = bag_get(bag, entry.id);
    // [`Policy::PlatformDefault`]/[`Policy::EditMode`]: отсутствие в каноне значит ПЛАТФОРМЕННЫЙ
    // дефолт, а EDT-омиссия значит ДРУГОЕ (ecore-дефолт EDT) ⇒ EDT ОБЯЗАН выписать литерал
    // платформы ЯВНО (ровно так пишет и реальная EDT-выгрузка: `<editMode>Enter` 86 533,
    // `<headerHorizontalAlign>Left` 122 142 при отсутствующем Designer-теге).
    let platform_fill: Option<PropertyValue> = match entry
        .policy_for(morph1c_core::version::current_roundtrip_target())
    {
        Policy::PlatformDefault(k) if value.is_none() => Some(parse_lit(entry.codec, k.des_fill)),
        Policy::EditMode if value.is_none() => {
            Some(parse_lit(entry.codec, ff::EDIT_MODE_DESIGNER_DEFAULT))
        }
        _ => None,
    };
    let value = platform_fill.as_ref().or(value);
    let emit = match entry.policy_for(morph1c_core::version::current_roundtrip_target()) {
        Policy::Symmetric => value,
        Policy::OppositeBool => match value {
            Some(PropertyValue::Bool(true)) => value,
            _ => None,
        },
        // Keep и PlatformDefault делят EDT-правило омиссии (`edt_omit`); отличие лишь в том, что
        // у PlatformDefault absent уже заменён на литерал платформы выше.
        Policy::Keep(k) | Policy::PlatformDefault(k) => match (value, k.edt_omit) {
            (Some(v), Some(omit)) if *v == parse_lit(entry.codec, omit) => None,
            (v, _) => v,
        },
        // Симметрично Designer-стороне (эмитим ТОЛЬКО Enum; None/иное — не эмитим):
        // прежний catch-all `v => v` пропускал бы non-Enum в render (ошибка кодека), тогда
        // как Designer его молча ронял — write-асимметрия на некорректном значении. Канон
        // никогда не несёт `Enter` (он подставлен выше при отсутствии), EDT опускает лишь
        // `Directly` ⇒ EDT-написание однозначно.
        Policy::EditMode => match value {
            Some(PropertyValue::Enum(t)) if t.as_str() == ff::EDIT_MODE_EDT_DEFAULT => None,
            Some(v @ PropertyValue::Enum(_)) => Some(v),
            _ => None,
        },
        // ТОЧНОЕ ПРИСУТСТВИЕ: fill'а нет ⇒ и омит-литерала нет. Значение в bag ⇒ эмитим как
        // есть, нет ⇒ не эмитим (зеркало рид-плеча ⇒ edt→edt байт-точен на ВСЕХ трёх корпусах:
        // ERP опускает тег 80 252/80 252, SSL/coverage эмитят `Normal` явно и опускают лишь у
        // кнопки-умолчания). См. `Policy::ButtonImportance`.
        Policy::ButtonImportance => value,
    };
    if let Some(v) = emit {
        // Localized: МНОГОЯЗЫЧНОЕ значение EDT кодирует ПОВТОРЕНИЕМ элемента — по одному
        // `<tag>` на пару (зеркало read-слияния; одноязычное — один элемент, byte-идентично
        // прежней эмиссии; пустое — один пустой элемент).
        if let (Codec::Localized, PropertyValue::Localized(pairs)) = (entry.codec, v) {
            if pairs.len() > 1 {
                for pair in pairs {
                    out.push(edt_localized(entry.edt, std::slice::from_ref(pair)));
                }
                return Ok(());
            }
        }
        out.push(render_edt(entry, v)?);
    }
    Ok(())
}

/// Срендерить EDT-узел поля по кодеку.
pub(crate) fn render_edt(
    entry: &FieldProj,
    value: &PropertyValue,
) -> Result<OutElement, FormError> {
    let tag = entry.edt;
    Ok(match (entry.codec, value) {
        (Codec::Bool, PropertyValue::Bool(b)) => bool_leaf("", tag, *b),
        (Codec::Int, PropertyValue::Int(n)) => OutElement::leaf("", tag, n.to_string()),
        (Codec::EnumTok, PropertyValue::Enum(t)) => {
            OutElement::leaf("", tag, t.as_str().to_string())
        }
        (Codec::EnumMap(map), PropertyValue::Enum(t)) => {
            if !map.iter().any(|(_, edt)| *edt == t.as_str()) {
                return Err(FormError::Frame(format!(
                    "<{tag}>: unknown typed EDT enumeration {:?}",
                    t.as_str()
                )));
            }
            OutElement::leaf("", tag, t.as_str().to_string())
        }
        (Codec::Text, PropertyValue::Str(s)) => OutElement::leaf("", tag, s.clone()),
        (Codec::RefText, PropertyValue::Ref(s)) => OutElement::leaf("", tag, s.clone()),
        (Codec::Localized, PropertyValue::Localized(pairs)) => edt_localized(tag, pairs),
        (Codec::Color, PropertyValue::Ref(r)) => render_edt_color(tag, r)?,
        (Codec::PictureRef, v) => {
            // Канон `List([Ref, Bool(lt)[, Pixel]])` (или голый `Ref` — дерив-совместимость). EDT
            // LoadTransparent НЕ несёт ⇒ `lt` тут игнорируется (edt-round-trip байт-точен).
            let (r, lt) = picture_ref_lt(v)?;
            if !r.is_empty()
                && !r.starts_with("abs:")
                && (picture_pixel(v).is_some() || lt != picture_lt_default(r))
            {
                return Err(FormError::Frame(format!(
                    "<{tag}>: EDT PictureRef cannot represent per-use transparency/pixel for {r:?}"
                )));
            }
            if r.is_empty() || r.starts_with("abs:") {
                // Пустой канон `Ref("")` И designer-абсолют `Ref("abs:ext")` ⟺ EDT инлайн-маркер
                // `form:FormPicture` (бинарь — в сайдкаре, не в дескрипторе). С прозрачным пикселем
                // маркер НЕПУСТ: несёт `<transparentPixel><x/><y/></transparentPixel>` (sparse-
                // листья, общий кодек).
                match picture_pixel(v) {
                    None => OutElement::self_closing("", tag).attr("xsi:type", "form:FormPicture"),
                    Some((x, y)) => {
                        let mut el =
                            OutElement::branch("", tag).attr("xsi:type", "form:FormPicture");
                        el.push(
                            crate::transparent_pixel::encode(
                                "",
                                "transparentPixel",
                                &PropertyValue::List(vec![
                                    PropertyValue::Int(x),
                                    PropertyValue::Int(y),
                                ]),
                            )
                            .map_err(FormError::Frame)?,
                        );
                        el
                    }
                }
            } else {
                let mut el = OutElement::branch("", tag).attr("xsi:type", "core:PictureRef");
                el.push(OutElement::leaf("", "picture", r.to_string()));
                el
            }
        }
        (Codec::Value, PropertyValue::Value(spec)) => {
            value_codec::encode(ValueDialect::Edt, "", tag, spec).map_err(FormError::Frame)?
        }
        (Codec::DataPath, PropertyValue::Ref(p)) => {
            let mut el = OutElement::branch("", tag).attr("xsi:type", "form:DataPath");
            el.push(OutElement::leaf("", "segments", p.clone()));
            el
        }
        (Codec::MdObjectRef, PropertyValue::Ref(r)) => {
            let mut el = OutElement::branch("", tag).attr("xsi:type", "core:ReferenceValue");
            el.push(OutElement::leaf("", "value", r.clone()));
            el
        }
        (Codec::CommonBool, PropertyValue::Bool(b)) => {
            if *b {
                let mut el = OutElement::branch("", tag);
                el.push(OutElement::leaf("", "common", "true"));
                el
            } else {
                OutElement::self_closing("", tag)
            }
        }
        (Codec::CommonBool, PropertyValue::List(entries)) => {
            // РОЛЕВОЙ userVisible: `[<common>true</common>]<for><value>B</value><role>R</role>
            // </for>×N` (общий флаг + пер-ролевые исключения; зеркало `F_CMD_USE` edt-write).
            let mut el = OutElement::branch("", tag);
            if matches!(entries.first(), Some(PropertyValue::Bool(true))) {
                el.push(OutElement::leaf("", "common", "true"));
            }
            for e in entries.iter().skip(1) {
                if let PropertyValue::List(pair) = e {
                    if let (Some(PropertyValue::Str(role)), Some(PropertyValue::Bool(v))) =
                        (pair.first(), pair.get(1))
                    {
                        let mut f = OutElement::branch("", "for");
                        if *v {
                            f.push(OutElement::leaf("", "value", "true"));
                        }
                        f.push(OutElement::leaf("", "role", role.clone()));
                        el.push(f);
                    }
                }
            }
            el
        }
        (Codec::UndefinedValue, PropertyValue::Bool(true)) => {
            OutElement::self_closing("", tag).attr("xsi:type", "core:UndefinedValue")
        }
        (Codec::Border, PropertyValue::Ref(reference)) => {
            if !reference.starts_with("Style.") || reference.len() <= 6 {
                return Err(FormError::Frame(
                    "BorderRef requires a named Style reference".into(),
                ));
            }
            let mut el = OutElement::branch("", tag).attr("xsi:type", "core:BorderRef");
            el.push(OutElement::leaf("", "border", reference.clone()));
            el
        }
        (Codec::Border, PropertyValue::Enum(t)) => {
            // `<style>X` эмитится ⟺ стиль НЕ `WithoutBorder` (симметрично `decode_edt_border`:
            // отсутствие `<style>` ⟺ `WithoutBorder`); ширина — из канона (`@W`-суффикс,
            // дефолт 1). Покрывает `Single` (PictureField/CalendarField), произвольный стиль
            // `Overline` (LabelDecoration) и не-1 ширину (`WithoutBorder@3`) без спец-кейса.
            let (style, width) = border_split(t.as_str());
            let mut el = OutElement::branch("", tag).attr("xsi:type", "core:BorderDef");
            if style != ff::BORDER_STYLE_WITHOUT {
                el.push(OutElement::leaf("", "style", style.to_string()));
            }
            // EDT ОПУСКАЕТ `<width>` при 0 (зеркало `decode_edt_border`; Designer эмитит `width="0"`).
            if width != "0" {
                el.push(OutElement::leaf("", "width", width.to_string()));
            }
            el
        }
        // EDT-сторона скроллбара — enum-текст (`ScrollNever` сюда не доходит: keep-омиссия).
        (Codec::ScrollBar, PropertyValue::Enum(t)) => {
            OutElement::leaf("", tag, t.as_str().to_string())
        }
        (Codec::Period, v @ PropertyValue::List(_)) => {
            let (start, end) = period_pair(v, tag)?;
            let mut el = OutElement::branch("", tag);
            el.push(OutElement::leaf("", "startDate", start.to_string()));
            el.push(OutElement::leaf("", "endDate", end.to_string()));
            el
        }
        (Codec::Type, PropertyValue::Type(ts)) => {
            type_codec::encode(TypeDialect::Edt, "", tag, ts).map_err(FormError::Frame)?
        }
        (_, other) => {
            return Err(FormError::Frame(format!(
                "EDT <{tag}>: value {other:?} does not match codec (§1.6)"
            )));
        }
    })
}

/// EDT локализованный узел (`<key>/<value>`-пары).
pub(crate) fn edt_localized(tag: &str, pairs: &[(Lang, String)]) -> OutElement {
    let mut t = OutElement::branch("", tag);
    for (lang, text) in pairs {
        t.push(OutElement::leaf("", "key", lang.as_str().to_string()));
        t.push(OutElement::leaf("", "value", text.clone()));
    }
    t
}

/// Эмитить EDT повторяемые `<choiceList>` из списка пар.
pub(crate) fn emit_choice_list_edt(
    out: &mut OutElement,
    items: &[PropertyValue],
) -> Result<(), FormError> {
    for item in items {
        let (pres, val, pic) = choice_item(item)?;
        let mut cl = OutElement::branch("", "choiceList");
        match pres {
            // Пустой канон `Localized([])` ⟺ EDT ОПУСКАЕТ `<presentation>` (симметрично read);
            // непустой — по одному `<presentation>` НА ЯЗЫК (EDT повторяет элемент, а не пары).
            PropertyValue::Localized(pairs) if pairs.is_empty() => {}
            PropertyValue::Localized(pairs) => {
                for pair in pairs {
                    cl.push(edt_localized("presentation", std::slice::from_ref(pair)));
                }
            }
            _ => {
                return Err(FormError::Frame(
                    "choiceList presentation not Localized".into(),
                ));
            }
        }
        match val {
            PropertyValue::Value(spec) => cl.push(
                value_codec::encode(ValueDialect::Edt, "", "value", spec)
                    .map_err(FormError::Frame)?,
            ),
            _ => return Err(FormError::Frame("choiceList value not Value".into())),
        }
        if let Some(p) = pic {
            // Канон `List([Ref, Bool(lt)])`; EDT LoadTransparent не несёт (lt игнор).
            let (r, lt) = picture_ref_lt(p)?;
            if picture_pixel(p).is_some() || lt != picture_lt_default(r) {
                return Err(FormError::Frame(format!(
                    "choiceList picture: EDT cannot represent per-use transparency/pixel for {r:?}"
                )));
            }
            let mut pe = OutElement::branch("", "picture").attr("xsi:type", "core:PictureRef");
            pe.push(OutElement::leaf("", "picture", r.to_string()));
            cl.push(pe);
        }
        out.push(cl);
    }
    Ok(())
}

/// Эмитить EDT повторяемые `<choiceParameters>` из списка пар.
pub(crate) fn emit_choice_parameters_edt(
    out: &mut OutElement,
    items: &[PropertyValue],
) -> Result<(), FormError> {
    for item in items {
        let (name, value) = choice_param_item(item)?;
        let mut cp = OutElement::branch("", "choiceParameters");
        cp.push(OutElement::leaf("", "name", name.to_string()));
        // `<value>`-ребёнок: FormChoiceListDesTimeValue-обёртка (скаляр/массив) ЛИБО прямой
        // `core:UndefinedValue` (без-обёрточный пустой выбор — §1.0/round-trip, отличен от
        // завёрнутого-Undefined).
        let value_child = match value {
            ChoiceParamValue::Scalar(spec) => {
                let mut wrap = OutElement::branch("", "value").attr("xsi:type", FCLDTV_EDT);
                wrap.push(
                    value_codec::encode(ValueDialect::Edt, "", "value", spec)
                        .map_err(FormError::Frame)?,
                );
                wrap
            }
            ChoiceParamValue::Array(arr) => {
                let mut wrap = OutElement::branch("", "value").attr("xsi:type", FCLDTV_EDT);
                let mut fa =
                    OutElement::branch("", "value").attr("xsi:type", "core:FixedArrayValue");
                for v in arr {
                    let PropertyValue::Value(spec) = v else {
                        return Err(FormError::Frame(
                            "choiceParameters array element must be Value (§1.6)".into(),
                        ));
                    };
                    let mut vw = OutElement::branch("", "values").attr("xsi:type", FCLDTV_EDT);
                    vw.push(
                        value_codec::encode(ValueDialect::Edt, "", "value", spec)
                            .map_err(FormError::Frame)?,
                    );
                    fa.push(vw);
                }
                wrap.push(fa);
                wrap
            }
            // Прямой `<value xsi:type="core:UndefinedValue"/>` (общий value_codec, без обёртки).
            ChoiceParamValue::BareUndefined => {
                value_codec::encode(ValueDialect::Edt, "", "value", &bare_undefined_spec())
                    .map_err(FormError::Frame)?
            }
        };
        cp.push(value_child);
        out.push(cp);
    }
    Ok(())
}

/// Эмитить EDT `<typeLink>` (linkItem ОМИТ при 0; порядок linkItem→datapath).
pub(crate) fn emit_type_link_edt(v: &PropertyValue) -> Result<OutElement, FormError> {
    let (path, n) = type_link_parts(v)?;
    let mut el = OutElement::branch("", "typeLink");
    if n != 0 {
        el.push(OutElement::leaf("", "linkItem", n.to_string()));
    }
    let mut dp = OutElement::branch("", "datapath").attr("xsi:type", "form:DataPath");
    dp.push(OutElement::leaf("", "segments", path.to_string()));
    el.push(dp);
    Ok(el)
}

/// Эмитить EDT повторяемые `<choiceParameterLinks>` из списка пунктов.
fn emit_choice_parameter_links_edt(
    out: &mut OutElement,
    items: &[PropertyValue],
) -> Result<(), FormError> {
    for item in items {
        let (name, path, mode) = choice_parameter_link_parts(item)?;
        let mut el = OutElement::branch("", "choiceParameterLinks");
        el.push(OutElement::leaf("", "name", name.to_string()));
        // changeMode — только НЕ-Clear (ERP-witness: name→changeMode→datapath).
        if let Some(m) = mode {
            el.push(OutElement::leaf("", "changeMode", m.to_string()));
        }
        let mut dp = OutElement::branch("", "datapath").attr("xsi:type", "form:DataPath");
        dp.push(OutElement::leaf("", "segments", path.to_string()));
        el.push(dp);
        out.push(el);
    }
    Ok(())
}
