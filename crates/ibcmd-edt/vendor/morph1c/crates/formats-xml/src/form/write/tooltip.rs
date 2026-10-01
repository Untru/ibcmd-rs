//! WRITE · extended tooltip / context menu / mobile command bar (both dialects) and the
//! shared control `<handlers>` emitter.

use super::*;

pub(crate) fn edt_handlers_ctrl(ev: &morph1c_core::ir::FormEvent) -> OutElement {
    let mut h = OutElement::branch("", "handlers");
    h.push(OutElement::leaf("", "event", ev.name.clone()));
    h.push(OutElement::leaf("", "name", ev.handler.clone()));
    h
}

/// EDT инлайн-стаб расширенной подсказки — реконструируется ИЗ ДАННЫХ ([`TooltipBody`]),
/// а НЕ из констант (тело варьируется по корпусу; см. `DecoratorRef`). Порядок тегов = порядок
/// EDT-эмиссии LabelDecoration: title, formatted, type, maxWidth, autoMaxWidth, autoMaxHeight,
/// horizontalStretch, extInfo(handlers, horizontalAlign).
pub(crate) fn edt_extended_tooltip(t: &DecoratorRef) -> Result<OutElement, FormError> {
    let body = match &t.body {
        DecoratorBody::Tooltip(b) => b,
        DecoratorBody::ContextMenu(_) => {
            return Err(FormError::Frame(
                "extendedTooltip has ContextMenu body".into(),
            ))
        }
    };
    let mut el = OutElement::branch("", "extendedTooltip");
    el.push(OutElement::leaf("", "name", t.name.clone()));
    el.push(OutElement::leaf("", "id", t.id.to_string()));
    // displayImportance — ЧИЛД после id (⟺ Designer атрибут; witness кнопка СоздатьДокументы `High`).
    if let Some(di) = &body.display_importance {
        el.push(OutElement::leaf("", "displayImportance", di.clone()));
    }
    push_tooltip_body(&mut el, body)?;
    Ok(el)
}

/// Найти строку `TOOLTIP_BODY` по каноническому id (интерливинг таблицы с каркасом на write).
pub(crate) fn tt_field(id: morph1c_core::ir::FieldId) -> &'static FieldProj {
    tables::TOOLTIP_BODY
        .iter()
        .find(|e| e.id == id)
        .expect("TOOLTIP_BODY has field")
}

/// Дописать тело подсказки (общее для extendedTooltip): плоские свойства LabelDecoration — через
/// табличный движок (`emit_field_edt` по `TOOLTIP_BODY`, в порядке EDT-эмиссии). title/formatted/
/// type/extInfo — каркас; `maxWidth==0` суппрессится каркасом (таблица его не несёт).
///
/// БАГФИКС: движок эмитит `horizontalStretch=false` (Symmetric), тогда как прежний ручной писатель
/// эмитил ЛИШЬ `true` — терял `<horizontalStretch>false>` из источника (см. ПанельАдминистрированияБСП).
pub(crate) fn push_tooltip_body(el: &mut OutElement, body: &TooltipBody) -> Result<(), FormError> {
    let get = |id: morph1c_core::ir::FieldId| {
        body.properties
            .iter()
            .find(|(k, _)| *k == id)
            .map(|(_, v)| v)
    };
    if let Some(PropertyValue::Localized(pairs)) = get(ld::F_TITLE) {
        push_edt_title_multi(el, "title", pairs);
    }
    if body.formatted {
        el.push(OutElement::leaf("", "formatted", "true"));
    }
    el.push(OutElement::leaf("", "type", "Label"));
    // Геометрия = метамодель LabelDecoration (width → autoMaxWidth → maxWidth → height →
    // autoMaxHeight; SSL-tooltip: width<autoMaxWidth×5, autoMaxWidth<height×2 —
    // ПрограммыЭлектроннойПодписиИШифрования, height<autoMaxHeight×3, maxWidth<autoMaxHeight×24;
    // 0 контрпримеров — прежний порядок height-до-autoMaxWidth был tie-break-артефактом;
    // autoMaxWidth (OppositeBool, эмит только true) и maxWidth не ко-встречаются).
    emit_field_edt(el, tt_field(ld::F_WIDTH), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_AUTO_MAX_WIDTH), &body.properties)?;
    // maxWidth — каркас (`0`-суппресс; таблица его не эмитит).
    if let Some(PropertyValue::Int(n)) = get(ld::F_MAX_WIDTH) {
        if *n != 0 {
            el.push(OutElement::leaf("", "maxWidth", n.to_string()));
        }
    }
    emit_field_edt(el, tt_field(ld::F_HEIGHT), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_AUTO_MAX_HEIGHT), &body.properties)?;
    // maxHeight/verticalStretch — ERP-волна (метамодель геометрии; SSL их не несёт).
    emit_field_edt(el, tt_field(ld::F_MAX_HEIGHT), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_HORIZONTAL_STRETCH), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_VERTICAL_STRETCH), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_GROUP_HORIZONTAL_ALIGN), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_GROUP_VERTICAL_ALIGN), &body.properties)?;
    emit_field_edt(el, tt_field(ld::F_TEXT_COLOR), &body.properties)?;
    // font — на теле стаба, после textColor (как у декораций; ERP ×26).
    if let Some(f) = &body.font {
        el.push(edt_font(f));
    }
    let mut ext = OutElement::branch("", "extInfo").attr("xsi:type", "form:LabelDecorationExtInfo");
    for ev in &body.events {
        ext.push(edt_handlers_ctrl(ev));
    }
    // hyperlink — ПЕРЕД horizontalAlign (метамодель LabelDecorationExtInfo#1; ERP ×13).
    if let Some((_, PropertyValue::Bool(b))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_HYPERLINK)
    {
        ext.push(OutElement::leaf(
            "",
            "hyperlink",
            if *b { "true" } else { "false" },
        ));
    }
    if let Some((_, PropertyValue::Enum(tok))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_HORIZONTAL_ALIGN)
    {
        ext.push(OutElement::leaf(
            "",
            "horizontalAlign",
            tok.as_str().to_string(),
        ));
    }
    // verticalAlign — ПОСЛЕ horizontalAlign в extInfo (LabelDecorationExtInfo);
    // titleHeight/backColor/borderColor — за ним (метамодель #4/#5/#6; ERP-волна) — см. ниже.
    if let Some((_, PropertyValue::Enum(tok))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_VERTICAL_ALIGN)
    {
        ext.push(OutElement::leaf(
            "",
            "verticalAlign",
            tok.as_str().to_string(),
        ));
    }
    // titleHeight/backColor/borderColor — ERP-волна (метамодель LabelDecorationExtInfo#4/#5/#6).
    if let Some((_, PropertyValue::Int(n))) = body
        .ext_info
        .iter()
        .find(|(k, _)| *k == ld::F_EXT_TITLE_HEIGHT)
    {
        ext.push(OutElement::leaf("", "titleHeight", n.to_string()));
    }
    for (fid, tag) in [
        (ld::F_EXT_BACK_COLOR, "backColor"),
        (tables::F_LD_BORDER_COLOR, "borderColor"),
    ] {
        if let Some((_, PropertyValue::Ref(c))) =
            body.ext_info.iter().find(|(k, _)| *k == fid)
        {
            // Общий рендер (ColorRef / hex→ColorDef) — симметрично decode_edt_color.
            ext.push(super::super::fields::render_edt_color(tag, c)?);
        }
    }
    el.push(ext);
    Ok(())
}

/// EDT инлайн-стаб контекстного меню — реконструируется ИЗ ДАННЫХ ([`ContextMenuBody`]).
pub(crate) fn edt_context_menu(c: &DecoratorRef) -> Result<OutElement, FormError> {
    let body = match &c.body {
        DecoratorBody::ContextMenu(b) => b,
        DecoratorBody::Tooltip(_) => {
            return Err(FormError::Frame("contextMenu has Tooltip body".into()))
        }
    };
    let mut el = OutElement::branch("", "contextMenu");
    el.push(OutElement::leaf("", "name", c.name.clone()));
    el.push(OutElement::leaf("", "id", c.id.to_string()));
    // Пункты меню (`<items>`) — ДО `<autoFill>` (corpus fact).
    for item in &body.items {
        el.push(edt_item(item)?);
    }
    // EDT эмитит `<autoFill>true>` для авто-меню; `false` ОПУСКАЕТ (в корпусе `<autoFill>
    // false>` 0/0) — реконструкция симметрична Designer-стороне (`<Autofill>false>` у не-авто).
    if body.auto_fill == Some(true) {
        el.push(OutElement::leaf("", "autoFill", "true"));
    }
    Ok(el)
}

/// EDT состав мобильной командной панели — ПОВТОРЯЕМЫЙ корневой `<mobileDeviceCommandBarContent>`
/// (ПО ОДНОМУ на пункт; witness ВариантыОтчетов ×9): каждый несёт `<value xsi:type=
/// "core:StringValue"><value>X</value></value>` через общий value-codec.
pub(crate) fn push_edt_mobile_command_bar(
    root: &mut OutElement,
    values: &[PropertyValue],
) -> Result<(), FormError> {
    for v in values {
        match v {
            PropertyValue::Value(spec) => {
                let mut el = OutElement::branch("", "mobileDeviceCommandBarContent");
                el.push(
                    value_codec::encode(ValueDialect::Edt, "", "value", spec)
                        .map_err(FormError::Frame)?,
                );
                root.push(el);
            }
            other => {
                return Err(FormError::Frame(format!(
                    "mobileDeviceCommandBarContent item must be Value, got {other:?} (§1.6)"
                )))
            }
        }
    }
    Ok(())
}

/// Designer `<MobileDeviceCommandBarContent>` (корневой список): каждый пункт — `<xr:Item>
/// <xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type="xs:string">X</xr:Value>
/// </xr:Item>` (значение — через общий value-codec с префиксом `xr`).
pub(crate) fn designer_mobile_command_bar(values: &[PropertyValue]) -> Result<OutElement, FormError> {
    let mut el = OutElement::branch("", "MobileDeviceCommandBarContent");
    for v in values {
        match v {
            PropertyValue::Value(spec) => {
                let mut item = OutElement::branch("xr", "Item");
                item.push(OutElement::self_closing("xr", "Presentation"));
                item.push(OutElement::leaf("xr", "CheckState", "0"));
                item.push(
                    value_codec::encode(ValueDialect::Designer, "xr", "Value", spec)
                        .map_err(FormError::Frame)?,
                );
                el.push(item);
            }
            other => {
                return Err(FormError::Frame(format!(
                    "MobileDeviceCommandBarContent item must be Value, got {other:?} (§1.6)"
                )))
            }
        }
    }
    Ok(el)
}
