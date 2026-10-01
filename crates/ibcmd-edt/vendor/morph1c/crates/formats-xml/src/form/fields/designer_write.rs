//! FIELDS · Designer dialect WRITE codec — render canonical field values to Designer form XML (attr values, edit-mode pair), plus the out-of-band structured emitters (choiceList/choiceParameters/typeLink/choiceParameterLinks).

use super::*;

/// Эмитить одно поле в Designer-выход по политике (или ничего). Поля-атрибуты эмитятся
/// НЕ здесь (см. [`designer_attr_value`]); [`Policy::EditMode`] эмитит тег `EditMode`
/// (пару `AutoEditMode` — отдельный слот, [`emit_designer_auto_edit_mode`]).
pub(crate) fn emit_field_designer(
    out: &mut OutElement,
    entry: &FieldProj,
    bag: &[(FieldId, PropertyValue)],
) -> Result<(), FormError> {
    if entry.des_attr {
        return Ok(());
    }
    if let Codec::ChoiceList = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            out.push(emit_choice_list_designer(items)?);
        }
        return Ok(());
    }
    if let Codec::ChoiceParameters = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            out.push(emit_choice_parameters_designer(items)?);
        }
        return Ok(());
    }
    if let Codec::ChoiceParameterLinks = entry.codec {
        if let Some(PropertyValue::List(items)) = bag_get(bag, entry.id) {
            out.push(emit_choice_parameter_links_designer(items)?);
        }
        return Ok(());
    }
    if let Codec::TypeLink = entry.codec {
        if let Some(v) = bag_get(bag, entry.id) {
            out.push(emit_type_link_designer(v)?);
        }
        return Ok(());
    }
    let value = bag_get(bag, entry.id);
    let emit: Option<PropertyValue> = match entry.policy {
        Policy::Symmetric => value.cloned(),
        Policy::OppositeBool => match value {
            Some(PropertyValue::Bool(true)) => None, // Designer-дефолт true ⇒ опускаем.
            Some(other) => Some(other.clone()),
            None => Some(PropertyValue::Bool(false)), // канон-false ⇒ эмитим false.
        },
        // PlatformDefault на Designer-стороне тождествен Keep: absent в каноне = платформенный
        // дефолт = ровно то, что Designer ОПУСКАЕТ (`des_omit`), значение — эмитится.
        Policy::Keep(k) | Policy::PlatformDefault(k) => match value {
            None => None,
            Some(v) => match k.des_omit {
                DesOmit::Never => Some(v.clone()),
                DesOmit::Always => None,
                DesOmit::Eq(l) if *v == parse_lit(entry.codec, l) => None,
                DesOmit::Eq(_) => Some(v.clone()),
            },
        },
        Policy::EditMode => match value {
            Some(PropertyValue::Enum(t)) => match t.as_str() {
                l if l == ff::EDIT_MODE_DESIGNER_DEFAULT => None,
                l if l == ff::EDIT_MODE_AUTO => {
                    Some(PropertyValue::Enum(Token::new(ff::EDIT_MODE_AUTO_RESOLVED)))
                }
                _ => Some(PropertyValue::Enum(t.clone())),
            },
            _ => None,
        },
        // Designer НИ РАЗУ не эмитит `Normal` (дефолт ПЛАТФОРМЫ) ни в одном из трёх корпусов —
        // 0 вхождений при 3 554+60 носителях, где EDT говорит `Normal` явно, а Designer молчит.
        // Значение из EDT-плеча ⇒ опускаем; отсутствие в bag ⇒ тоже опускаем; иначе эмитим
        // (`Main`/`Supplementary`). См. `Policy::ButtonImportance`.
        Policy::ButtonImportance => match value {
            Some(v)
                if *v
                    == PropertyValue::Enum(Token::new(bt::BUTTON_IMPORTANCE_DESIGNER_DEFAULT)) =>
            {
                None
            }
            Some(v) => Some(v.clone()),
            None => None,
        },
    };
    if let Some(v) = emit {
        out.push(render_designer(entry, &v)?);
    }
    Ok(())
}

/// Пара `AutoEditMode` (отдельный Designer-слот ПОСЛЕ `ShowInHeader`): эмитится
/// `<AutoEditMode>true` ⟺ канонический `editMode == Auto`.
pub(crate) fn emit_designer_auto_edit_mode(out: &mut OutElement, bag: &[(FieldId, PropertyValue)]) {
    if let Some(PropertyValue::Enum(t)) = bag_get(bag, ff::F_EDIT_MODE) {
        if t.as_str() == ff::EDIT_MODE_AUTO {
            out.push(OutElement::leaf("", "AutoEditMode", "true"));
        }
    }
}

/// Значение Designer-АТРИБУТА поля (`des_attr`), если оно должно эмититься.
pub(crate) fn designer_attr_value(
    entry: &FieldProj,
    bag: &[(FieldId, PropertyValue)],
) -> Option<String> {
    if !entry.des_attr {
        return None;
    }
    match bag_get(bag, entry.id) {
        Some(PropertyValue::Enum(t)) => Some(t.as_str().to_string()),
        _ => None,
    }
}

/// Срендерить Designer-узел поля по кодеку.
pub(crate) fn render_designer(
    entry: &FieldProj,
    value: &PropertyValue,
) -> Result<OutElement, FormError> {
    let tag = entry.des;
    Ok(match (entry.codec, value) {
        (Codec::Bool, PropertyValue::Bool(b)) => bool_leaf("", tag, *b),
        (Codec::Int, PropertyValue::Int(n)) => OutElement::leaf("", tag, n.to_string()),
        (Codec::EnumTok, PropertyValue::Enum(t)) => {
            OutElement::leaf("", tag, t.as_str().to_string())
        }
        (Codec::EnumMap(map), PropertyValue::Enum(t)) => {
            let des = map
                .iter()
                .find(|(canon, _)| *canon == t.as_str())
                .map(|(_, d)| *d)
                .ok_or_else(|| {
                    FormError::Frame(format!(
                        "Designer <{tag}>: unmapped canon literal {:?} (§1.0)",
                        t.as_str()
                    ))
                })?;
            OutElement::leaf("", tag, des.to_string())
        }
        (Codec::Text, PropertyValue::Str(s)) => OutElement::leaf("", tag, s.clone()),
        (Codec::RefText, PropertyValue::Ref(s)) => OutElement::leaf("", tag, s.clone()),
        (Codec::Localized, PropertyValue::Localized(pairs)) => designer_localized(tag, pairs),
        (Codec::Color, PropertyValue::Ref(r)) => {
            OutElement::leaf("", tag, color_to_designer(r, tag)?)
        }
        (Codec::PictureRef, v) => {
            // Канон `List([Ref, Bool(lt)[, Pixel]])` — `lt` эмитится КАК ЕСТЬ (независимый флаг,
            // не дерив): ERP несёт CommonPicture/Abs c LoadTransparent="true".
            let (r, lt) = picture_ref_lt(v)?;
            let mut el = OutElement::branch("", tag);
            if let Some(ext) = r.strip_prefix("abs:") {
                // Абсолютная (сайдкар) картинка: `<xr:Abs>{Tag}.{ext}</xr:Abs>`.
                el.push(OutElement::leaf("xr", "Abs", format!("{tag}.{ext}")));
            } else {
                el.push(OutElement::leaf("xr", "Ref", r.to_string()));
            }
            el.push(OutElement::leaf("xr", "LoadTransparent", bool_lit(lt)));
            // ВСТРОЕННЫЙ прозрачный пиксель: `<xr:TransparentPixel x=".." y=".."/>` (DENSE-атрибуты,
            // оба всегда — census ERP форм; `x="0"` явно). LT уже эмитнут `true` (денормализация).
            if let Some((x, y)) = picture_pixel(v) {
                el.push(
                    OutElement::self_closing("xr", "TransparentPixel")
                        .attr("x", x.to_string())
                        .attr("y", y.to_string()),
                );
            }
            el
        }
        (Codec::Value, PropertyValue::Value(spec)) => {
            value_codec::encode(ValueDialect::Designer, "", tag, spec).map_err(FormError::Frame)?
        }
        (Codec::DataPath, PropertyValue::Ref(p)) => OutElement::leaf("", tag, p.clone()),
        (Codec::MdObjectRef, PropertyValue::Ref(r)) => {
            OutElement::leaf("", tag, r.clone()).attr("xsi:type", "xr:MDObjectRef")
        }
        (Codec::CommonBool, PropertyValue::Bool(b)) => {
            if *b {
                // Designer опускает true — политика Keep не должна была дать сюда true.
                return Err(FormError::Frame(format!(
                    "Designer <{tag}>: CommonBool true must be omitted (policy bug)"
                )));
            }
            let mut el = OutElement::branch("", tag);
            el.push(OutElement::leaf("xr", "Common", "false"));
            el
        }
        (Codec::CommonBool, PropertyValue::List(entries)) => {
            // РОЛЕВОЙ userVisible: `<xr:Common>B</xr:Common><xr:Value name="R">B</xr:Value>×N`
            // (общий флаг + пер-ролевые исключения; зеркало `F_CMD_USE` designer-write). Роли
            // ПОСЛЕ `<xr:Common>` (порядок witness). Пустой `entries` невозможен (кодек кладёт
            // List лишь при непустых ролях; безролевой канон — `Bool`).
            let common = matches!(entries.first(), Some(PropertyValue::Bool(true)));
            let mut el = OutElement::branch("", tag);
            el.push(OutElement::leaf(
                "xr",
                "Common",
                if common { "true" } else { "false" },
            ));
            for e in entries.iter().skip(1) {
                if let PropertyValue::List(pair) = e {
                    if let (Some(PropertyValue::Str(role)), Some(PropertyValue::Bool(v))) =
                        (pair.first(), pair.get(1))
                    {
                        el.push(
                            OutElement::leaf("xr", "Value", if *v { "true" } else { "false" })
                                .attr("name", role.clone()),
                        );
                    }
                }
            }
            el
        }
        (Codec::UndefinedValue, PropertyValue::Bool(true)) => {
            OutElement::self_closing("", tag).attr("xsi:nil", "true")
        }
        (Codec::Border, PropertyValue::Ref(reference)) => {
            let Some(name) = reference.strip_prefix("Style.").filter(|n| !n.is_empty()) else {
                return Err(FormError::Frame(
                    "Border ref requires a named Style reference".into(),
                ));
            };
            OutElement::self_closing("", tag).attr("ref", format!("style:{name}"))
        }
        (Codec::Border, PropertyValue::Enum(t)) => {
            // Ширина — из канона (`@W`-суффикс, дефолт 1); keep-омиссии политик работают по
            // ПОЛНОМУ канону (`WithoutBorder@3` ≠ `WithoutBorder` ⇒ эмитится).
            let (style, width) = border_split(t.as_str());
            let mut el = OutElement::branch("", tag).attr("width", width.to_string());
            el.push(
                OutElement::leaf("v8ui", "style", style.to_string())
                    .attr("xsi:type", "v8ui:ControlBorderType"),
            );
            el
        }
        (Codec::ScrollBar, PropertyValue::Enum(t)) => {
            // Designer BOOL: ScrollAlways→true, ScrollNever→false (ScrollAuto опущен keep'ом).
            let b = match t.as_str() {
                s if s == ff::SCROLL_BAR_ALWAYS => "true",
                s if s == ff::SCROLL_BAR_NEVER => "false",
                other => {
                    return Err(FormError::Frame(format!(
                        "Designer <{tag}>: scrollbar {other:?} not bool-encodable (policy bug — §1.6)"
                    )));
                }
            };
            OutElement::leaf("", tag, b)
        }
        (Codec::Period, v @ PropertyValue::List(_)) => {
            let (start, end) = period_pair(v, tag)?;
            let mut el = OutElement::branch("", tag);
            el.push(
                OutElement::leaf("v8", "variant", PERIOD_VARIANT_VALUE.to_string())
                    .attr("xsi:type", PERIOD_VARIANT_XSI),
            );
            el.push(OutElement::leaf("v8", "startDate", start.to_string()));
            el.push(OutElement::leaf("v8", "endDate", end.to_string()));
            el
        }
        (Codec::Type, PropertyValue::Type(ts)) => {
            // Envelope Designer-ФОРМЫ объявляет dcsset и пр. ⇒ scoped-эмиссия (без
            // дублирующего инлайн-ns на `<v8:Type>`; см. `type_codec::encode_scoped`).
            type_codec::encode_scoped(TypeDialect::Designer, "", tag, ts, DESIGNER_FORM_NS)
                .map_err(FormError::Frame)?
        }
        (_, other) => {
            return Err(FormError::Frame(format!(
                "Designer <{tag}>: value {other:?} does not match codec (§1.6)"
            )));
        }
    })
}

/// Designer локализованный узел (`<v8:item>`-пары, БЕЗ formatted-атрибута).
pub(crate) fn designer_localized(tag: &str, pairs: &[(Lang, String)]) -> OutElement {
    let mut t = OutElement::branch("", tag);
    for (lang, content) in pairs {
        let mut it = OutElement::branch("v8", "item");
        it.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        it.push(OutElement::leaf("v8", "content", content.clone()));
        t.push(it);
    }
    t
}

/// Эмитить Designer `<ChoiceList>` (c `<xr:Item>`-детьми) из списка пар.
pub(crate) fn emit_choice_list_designer(items: &[PropertyValue]) -> Result<OutElement, FormError> {
    let mut cl = OutElement::branch("", "ChoiceList");
    for item in items {
        let (pres, val, pic) = choice_item(item)?;
        let mut it = OutElement::branch("xr", "Item");
        it.push(OutElement::self_closing("xr", "Presentation"));
        it.push(OutElement::leaf("xr", "CheckState", "0"));
        let mut xv =
            OutElement::branch("xr", "Value").attr("xsi:type", "FormChoiceListDesTimeValue");
        match pres {
            // Пустой канон `Localized([])` ⟺ Designer `<Presentation/>` (self-close; X с EDT
            // отсутствием `<presentation>`). Непустой — обычный `<Presentation>` с v8:item.
            PropertyValue::Localized(pairs) if pairs.is_empty() => {
                xv.push(OutElement::self_closing("", "Presentation"));
            }
            PropertyValue::Localized(pairs) => xv.push(designer_localized("Presentation", pairs)),
            _ => {
                return Err(FormError::Frame(
                    "choiceList presentation not Localized".into(),
                ));
            }
        }
        match val {
            PropertyValue::Value(spec) => xv.push(
                value_codec::encode(ValueDialect::Designer, "", "Value", spec)
                    .map_err(FormError::Frame)?,
            ),
            _ => return Err(FormError::Frame("choiceList value not Value".into())),
        }
        if let Some(p) = pic {
            // Канон `List([Ref, Bool(lt)])`; Designer эмитит независимый LoadTransparent.
            let (r, lt) = picture_ref_lt(p)?;
            let mut pe = OutElement::branch("", "Picture");
            pe.push(OutElement::leaf("xr", "Ref", r.to_string()));
            pe.push(OutElement::leaf("xr", "LoadTransparent", bool_lit(lt)));
            xv.push(pe);
        }
        it.push(xv);
        cl.push(it);
    }
    Ok(cl)
}

/// Эмитить Designer-контейнер `<ChoiceParameters>` (c `<app:item>`-детьми) из списка пар.
pub(crate) fn emit_choice_parameters_designer(
    items: &[PropertyValue],
) -> Result<OutElement, FormError> {
    let mut container = OutElement::branch("", "ChoiceParameters");
    for item in items {
        let (name, value) = choice_param_item(item)?;
        let mut it = OutElement::branch("app", "item").attr("name", name.to_string());
        // `<app:value>`: FormChoiceListDesTimeValue-обёртка (скаляр/массив) ЛИБО прямой
        // `xsi:nil="true"` (без-обёрточный пустой выбор; твин EDT `core:UndefinedValue`).
        let app_value = match value {
            ChoiceParamValue::Scalar(spec) => {
                let mut wrap = OutElement::branch("app", "value").attr("xsi:type", FCLDTV_DES);
                wrap.push(OutElement::self_closing("", "Presentation"));
                wrap.push(
                    value_codec::encode(ValueDialect::Designer, "", "Value", spec)
                        .map_err(FormError::Frame)?,
                );
                wrap
            }
            ChoiceParamValue::Array(arr) => {
                let mut wrap = OutElement::branch("app", "value").attr("xsi:type", FCLDTV_DES);
                wrap.push(OutElement::self_closing("", "Presentation"));
                // `<Value xsi:type="v8:FixedArray">` c `<v8:Value xsi:type=FCLDTV>`-детьми
                // (пустой Presentation + скаляр; ERP-witness).
                let mut fa = OutElement::branch("", "Value").attr("xsi:type", "v8:FixedArray");
                for v in arr {
                    let PropertyValue::Value(spec) = v else {
                        return Err(FormError::Frame(
                            "choiceParameters array element must be Value (§1.6)".into(),
                        ));
                    };
                    let mut vw = OutElement::branch("v8", "Value").attr("xsi:type", FCLDTV_DES);
                    vw.push(OutElement::self_closing("", "Presentation"));
                    vw.push(
                        value_codec::encode(ValueDialect::Designer, "", "Value", spec)
                            .map_err(FormError::Frame)?,
                    );
                    fa.push(vw);
                }
                wrap.push(fa);
                wrap
            }
            // Прямой `<app:value xsi:nil="true"/>` (общий value_codec, без обёртки).
            ChoiceParamValue::BareUndefined => value_codec::encode(
                ValueDialect::Designer,
                "app",
                "value",
                &bare_undefined_spec(),
            )
            .map_err(FormError::Frame)?,
        };
        it.push(app_value);
        container.push(it);
    }
    Ok(container)
}

/// Эмитить Designer `<TypeLink>` (оба `xr:`-листа всегда; порядок DataPath→LinkItem).
pub(crate) fn emit_type_link_designer(v: &PropertyValue) -> Result<OutElement, FormError> {
    let (path, n) = type_link_parts(v)?;
    let mut el = OutElement::branch("", "TypeLink");
    el.push(OutElement::leaf("xr", "DataPath", path.to_string()));
    el.push(OutElement::leaf("xr", "LinkItem", n.to_string()));
    Ok(el)
}

/// Эмитить Designer-контейнер `<ChoiceParameterLinks>` (`<xr:Link>`-дети) из списка пунктов.
fn emit_choice_parameter_links_designer(items: &[PropertyValue]) -> Result<OutElement, FormError> {
    let mut container = OutElement::branch("", "ChoiceParameterLinks");
    for item in items {
        let (name, path, mode) = choice_parameter_link_parts(item)?;
        let mut link = OutElement::branch("xr", "Link");
        link.push(OutElement::leaf("xr", "Name", name.to_string()));
        link.push(
            OutElement::leaf("xr", "DataPath", path.to_string()).attr("xsi:type", "xs:string"),
        );
        link.push(OutElement::leaf(
            "xr",
            "ValueChange",
            mode.unwrap_or("Clear").to_string(),
        ));
        container.push(link);
    }
    Ok(container)
}
