//! Designer encode: восстановить плоский `<Value xsi:type="v8ui:…" …/>` (byte-exact).

use super::*;

pub(crate) fn encode_designer(prefix: &str, local: &str, spec: &StyleValueSpec) -> OutElement {
    match spec {
        StyleValueSpec::Font(f) => encode_des_font(prefix, local, f),
        StyleValueSpec::Color(c) => {
            OutElement::leaf(prefix, local, encode_des_color(c)).attr(XSI_TYPE, DES_COLOR)
        }
        StyleValueSpec::Border(b) => encode_des_border(prefix, local, b),
    }
}

/// Designer-рамка: `<Value xsi:type="v8ui:Border" width="W"><v8ui:style
/// xsi:type="v8ui:ControlBorderType">STYLE</v8ui:style></Value>` — DENSE (всегда эмитит
/// width + лист стиля).
fn encode_des_border(prefix: &str, local: &str, b: &BorderStyle) -> OutElement {
    let BorderStyle::Def { style, width } = b;
    let mut h = OutElement::branch(prefix, local)
        .attr(XSI_TYPE, DES_BORDER)
        .attr("width", width.to_string());
    h.push(OutElement::leaf("v8ui", "style", style.clone()).attr(XSI_TYPE, DES_BORDER_STYLE_XSI));
    h
}

fn encode_des_font(prefix: &str, local: &str, f: &FontStyle) -> OutElement {
    // Порядок атрибутов = эталон: xsi:type, ref|faceName, [height], [flags], kind, [scale].
    let mut h = OutElement::self_closing(prefix, local).attr(XSI_TYPE, DES_FONT);
    match f {
        FontStyle::Ref {
            font,
            height,
            flags,
            scale,
        } => {
            // @kind — из семейства канон-префикса: `Style.`→StyleItem, `System.`→WindowsFont
            // (одна и та же остальная атрибут-структура; witnessed ERP 9/9 системных).
            let (refv, kind) = font_ref_des_from_canon(font);
            h = h.attr("ref", refv);
            if let Some(ht) = height {
                h = h.attr("height", ht.to_string());
            }
            // Тристейт: эмитим РОВНО переопределённые флаги в фикс-порядке (witnessed ERP
            // `ЖирныйПодчеркнутыйШрифт.xml`: ref,bold,underline,kind).
            for (name, v) in [
                ("bold", flags.bold),
                ("italic", flags.italic),
                ("underline", flags.underline),
                ("strikeout", flags.strikeout),
            ] {
                if let Some(b) = v {
                    h = h.attr(name, bool_text(b));
                }
            }
            h = h.attr("kind", kind);
            if let Some(s) = scale {
                h = h.attr("scale", s.to_string());
            }
        }
        FontStyle::Def { face_name, height, face } => {
            // Designer FontDef ВСЕГДА денсовый: faceName, height, фактические флаги,
            // kind=Absolute, scale=100 (witnessed ERP 51/51 Absolute).
            h = h
                .attr("faceName", face_name.clone())
                .attr("height", height.to_string())
                .attr("bold", bool_text(face.bold))
                .attr("italic", bool_text(face.italic))
                .attr("underline", bool_text(face.underline))
                .attr("strikeout", bool_text(face.strikeout))
                .attr("kind", DES_KIND_DEF)
                .attr("scale", "100");
        }
    }
    h
}

fn encode_des_color(c: &ColorStyle) -> String {
    match c {
        ColorStyle::Ref(name) => color_ref_des_from_canon(name),
        ColorStyle::Def { red, green, blue } => format!("#{red:02X}{green:02X}{blue:02X}"),
    }
}
