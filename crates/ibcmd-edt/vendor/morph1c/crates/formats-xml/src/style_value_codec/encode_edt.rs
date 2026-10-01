//! EDT encode: восстановить вложенный `<value xsi:type="core:…Value">…` (byte-exact).

use super::*;

pub(crate) fn encode_edt(prefix: &str, local: &str, spec: &StyleValueSpec) -> OutElement {
    let (outer_xsi, inner) = match spec {
        StyleValueSpec::Font(f) => (EDT_FONT_VALUE, encode_edt_font(f)),
        StyleValueSpec::Color(c) => (EDT_COLOR_VALUE, encode_edt_color(c)),
        StyleValueSpec::Border(b) => (EDT_BORDER_VALUE, encode_edt_border(b)),
    };
    let mut outer = OutElement::branch(prefix, local).attr(XSI_TYPE, outer_xsi);
    outer.push(inner);
    outer
}

fn encode_edt_font(f: &FontStyle) -> OutElement {
    match f {
        FontStyle::Ref {
            font,
            height,
            flags,
            scale,
        } => {
            let mut inner = OutElement::branch("", "value").attr(XSI_TYPE, EDT_FONT_REF);
            inner.push(OutElement::leaf("", "font", font_ref_edt_from_canon(font)));
            if let Some(h) = height {
                inner.push(OutElement::leaf("", "height", edt_height(*h)));
            }
            // Тристейт: эмитим РОВНО переопределённые флаги в фикс-порядке (byte-exact).
            for (name, v) in [
                ("bold", flags.bold),
                ("italic", flags.italic),
                ("underline", flags.underline),
                ("strikeout", flags.strikeout),
            ] {
                if let Some(b) = v {
                    inner.push(OutElement::leaf("", name, bool_text(b)));
                }
            }
            if let Some(s) = scale {
                inner.push(OutElement::leaf("", "scale", s.to_string()));
            }
            inner
        }
        FontStyle::Def { face_name, height, face } => {
            let mut inner = OutElement::branch("", "value").attr(XSI_TYPE, EDT_FONT_DEF);
            inner.push(OutElement::leaf("", "faceName", face_name.clone()));
            inner.push(OutElement::leaf("", "height", edt_height(*height)));
            // Абсолютный шрифт: EDT эмитит только true-флаги (false = дефолт, опускается).
            for (name, b) in [
                ("bold", face.bold),
                ("italic", face.italic),
                ("underline", face.underline),
                ("strikeout", face.strikeout),
            ] {
                if b {
                    inner.push(OutElement::leaf("", name, "true"));
                }
            }
            inner
        }
    }
}

fn encode_edt_color(c: &ColorStyle) -> OutElement {
    match c {
        ColorStyle::Ref(name) => {
            let mut inner = OutElement::branch("", "value").attr(XSI_TYPE, EDT_COLOR_REF);
            inner.push(OutElement::leaf("", "color", name.clone()));
            inner
        }
        ColorStyle::Def { red, green, blue } => {
            // Все-нулевой ColorDef (чёрный `#000000`) EDT пишет пустым самозакрытым
            // `<value xsi:type="core:ColorDef"/>` (byte-exact) — иначе `></value>`.
            if *red == 0 && *green == 0 && *blue == 0 {
                return OutElement::self_closing("", "value").attr(XSI_TYPE, EDT_COLOR_DEF);
            }
            let mut inner = OutElement::branch("", "value").attr(XSI_TYPE, EDT_COLOR_DEF);
            // Нулевая компонента ОПУСКАЕТСЯ (конвенция EDT); порядок red/green/blue.
            if *red != 0 {
                inner.push(OutElement::leaf("", "red", red.to_string()));
            }
            if *green != 0 {
                inner.push(OutElement::leaf("", "green", green.to_string()));
            }
            if *blue != 0 {
                inner.push(OutElement::leaf("", "blue", blue.to_string()));
            }
            inner
        }
    }
}

/// EDT `BorderDef` — дефолты (`style=WithoutBorder`, `width=0`) ОПУСКАЮТСЯ; пустой
/// `<value xsi:type="core:BorderDef"/>` восстанавливается самозакрытым (byte-exact).
fn encode_edt_border(b: &BorderStyle) -> OutElement {
    let BorderStyle::Def { style, width } = b;
    let has_style = style != BORDER_STYLE_DEFAULT;
    let has_width = *width != 0;
    if !has_style && !has_width {
        return OutElement::self_closing("", "value").attr(XSI_TYPE, EDT_BORDER_DEF);
    }
    let mut inner = OutElement::branch("", "value").attr(XSI_TYPE, EDT_BORDER_DEF);
    if has_style {
        inner.push(OutElement::leaf("", "style", style.clone()));
    }
    if has_width {
        inner.push(OutElement::leaf("", "width", width.to_string()));
    }
    inner
}
