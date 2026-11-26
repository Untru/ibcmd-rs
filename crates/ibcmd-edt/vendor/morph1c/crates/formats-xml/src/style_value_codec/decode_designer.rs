//! Designer decode: `<Value xsi:type="v8ui:Font" …/>` / `<Value xsi:type="v8ui:Color">…</Value>`.

use super::*;

pub(crate) fn decode_designer(host: &Element) -> Result<StyleValueSpec, String> {
    let xsi = require_attr(host, XSI_TYPE)?;
    match xsi.as_str() {
        DES_FONT => {
            ensure_no_children_des(host)?;
            Ok(StyleValueSpec::Font(decode_des_font(host)?))
        }
        DES_COLOR => {
            ensure_no_children_des(host)?;
            host.claim_text();
            let text = host.text.clone();
            Ok(StyleValueSpec::Color(decode_des_color(&text)?))
        }
        DES_BORDER => Ok(StyleValueSpec::Border(decode_des_border(host)?)),
        other => Err(format!(
            "StyleValue: unknown Designer xsi:type {other:?} (witnessed: v8ui:Font/v8ui:Color/v8ui:Border — §1.0)"
        )),
    }
}

/// Designer `<Value xsi:type="v8ui:Border" width="0"><v8ui:style
/// xsi:type="v8ui:ControlBorderType">WithoutBorder</v8ui:style></Value>` — атрибут `width`
/// + единственный лист `<v8ui:style>` (тип рамки). Оба present DENSE (§1.0).
fn decode_des_border(host: &Element) -> Result<BorderStyle, String> {
    // Атрибуты host: xsi:type (уже прочитан вызывающим) + width.
    let mut attrs = DesAttrs::new(host);
    attrs.claim(XSI_TYPE);
    let width = attrs.require_u32("width")?;
    attrs.finish()?;
    // Ровно один ребёнок — `<v8ui:style xsi:type="v8ui:ControlBorderType">TOKEN</v8ui:style>`.
    let style_el = single_child(host, "style", "v8ui")?;
    let xt = style_el.attr(XSI_TYPE).ok_or_else(|| {
        "StyleValue: Designer Border <v8ui:style> missing xsi:type (§1.0)".to_string()
    })?;
    if xt.value != DES_BORDER_STYLE_XSI {
        return Err(format!(
            "StyleValue: Designer Border <v8ui:style> xsi:type must be {DES_BORDER_STYLE_XSI:?}, got {:?} (§1.0)",
            xt.value
        ));
    }
    xt.claimed.set(true);
    if let Some(extra) = style_el.attrs.iter().find(|a| !a.claimed.get()) {
        return Err(format!(
            "StyleValue: Designer Border <v8ui:style> has unexpected attribute {:?} (§1.0)",
            extra.name
        ));
    }
    if !style_el.children.is_empty() {
        return Err(
            "StyleValue: Designer Border <v8ui:style> must be a plain-text leaf (§1.0)".into(),
        );
    }
    style_el.claim_with_text();
    Ok(BorderStyle::Def {
        style: style_el.text.clone(),
        width,
    })
}

fn decode_des_font(host: &Element) -> Result<FontStyle, String> {
    // Клеймим все атрибуты; читаем по имени. `kind` различает Ref/Def.
    let mut attrs = DesAttrs::new(host);
    attrs.claim(XSI_TYPE); // уже прочитан вызывающим, но пометить нужно.
    let kind = attrs.require("kind")?;
    let out = match kind.as_str() {
        // Ref-семейство: kind=StyleItem (стиль-ссылка) ЛИБО kind=WindowsFont (системный
        // шрифт) — одна структура атрибутов, различие несут @kind + префикс @ref
        // (кросс-сверяются по [`FONT_REF_FAMILIES`], §1.0).
        DES_KIND_REF | DES_KIND_WINDOWS => {
            let refv = attrs.require("ref")?;
            let font = font_ref_canon_from_designer(&refv, &kind)?;
            let height = attrs.opt_u32("height")?;
            let flags = decode_des_font_flags(&mut attrs)?;
            let scale = attrs.opt_u32("scale")?;
            FontStyle::Ref {
                font,
                height,
                flags,
                scale,
            }
        }
        DES_KIND_DEF => {
            // Def DENSE: все четыре флага всегда присутствуют, значения ВАРЬИРУЮТСЯ
            // (witnessed ERP 51/51 Absolute: bold/italic/underline/strikeout true/false);
            // scale всегда 100 (witnessed).
            let face_name = attrs.require("faceName")?;
            let height = attrs.require_u32("height")?;
            let face = FontFace {
                bold: attrs.require_bool("bold")?,
                italic: attrs.require_bool("italic")?,
                underline: attrs.require_bool("underline")?,
                strikeout: attrs.require_bool("strikeout")?,
            };
            let scale = attrs.require_u32("scale")?;
            if scale != 100 {
                return Err(format!(
                    "StyleValue: Designer FontDef scale must be 100 (default), got {scale} (§1.0)"
                ));
            }
            FontStyle::Def { face_name, height, face }
        }
        other => {
            return Err(format!(
                "StyleValue: Designer Font kind must be StyleItem/WindowsFont/Absolute, \
                 got {other:?} (§1.0)"
            ))
        }
    };
    attrs.finish()?;
    Ok(out)
}

fn decode_des_font_flags(attrs: &mut DesAttrs) -> Result<FontFlags, String> {
    // Тристейт: каждый атрибут-флаг независимо опционален (отсутствие = не переопределён).
    // Witnessed ERP `ЖирныйПодчеркнутыйШрифт.xml`: ref,bold,underline,kind — italic/strikeout
    // опущены (ранее кодек требовал «все-или-ни-одного» и падал на таких наборах).
    Ok(FontFlags {
        bold: attrs.opt_bool("bold")?,
        italic: attrs.opt_bool("italic")?,
        underline: attrs.opt_bool("underline")?,
        strikeout: attrs.opt_bool("strikeout")?,
    })
}

/// Designer Color: hex `#RRGGBB` (Def) либо `prefix:Имя` (Ref).
fn decode_des_color(text: &str) -> Result<ColorStyle, String> {
    if let Some(hex) = text.strip_prefix('#') {
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!(
                "StyleValue: Designer Color hex must be #RRGGBB (6 hex digits), got {text:?} (§1.0)"
            ));
        }
        let red = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let green = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let blue = u8::from_str_radix(&hex[4..6], 16).unwrap();
        Ok(ColorStyle::Def { red, green, blue })
    } else {
        let canon = color_ref_canon_from_designer(text)?;
        Ok(ColorStyle::Ref(canon))
    }
}
