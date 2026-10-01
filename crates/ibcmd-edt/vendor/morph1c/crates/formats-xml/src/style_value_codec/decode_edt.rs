//! EDT decode: `<value xsi:type="core:FontValue"><value xsi:type="core:…Ref|…Def">…`.

use super::*;

pub(crate) fn decode_edt(host: &Element) -> Result<StyleValueSpec, String> {
    let outer_xsi = claim_only_xsi(host)?;
    // Внешний host несёт РОВНО один ребёнок — внутренний `<value xsi:type="core:…Ref|…Def">`.
    let inner = single_child(host, "value", "")?;
    let inner_xsi = claim_only_xsi(inner)?;
    match outer_xsi.as_str() {
        EDT_FONT_VALUE => Ok(StyleValueSpec::Font(decode_edt_font(inner, &inner_xsi)?)),
        EDT_COLOR_VALUE => Ok(StyleValueSpec::Color(decode_edt_color(inner, &inner_xsi)?)),
        EDT_BORDER_VALUE => Ok(StyleValueSpec::Border(decode_edt_border(inner, &inner_xsi)?)),
        other => Err(format!(
            "StyleValue: unknown EDT outer xsi:type {other:?} (witnessed: FontValue/ColorValue/BorderValue — §1.0)"
        )),
    }
}

/// EDT `BorderDef` — поля `[style] [width]` (порядок фикс, оба опциональны; омиссия →
/// дефолт `WithoutBorder`/`0`). Пустой `<value xsi:type="core:BorderDef"/>` ⇒ дефолты.
fn decode_edt_border(inner: &Element, inner_xsi: &str) -> Result<BorderStyle, String> {
    match inner_xsi {
        EDT_BORDER_DEF => {
            let mut it = EdtChildren::new(inner);
            let style = it
                .opt_text("style")?
                .unwrap_or_else(|| BORDER_STYLE_DEFAULT.to_string());
            let width = it.opt_u32("width")?.unwrap_or(0);
            it.finish()?;
            Ok(BorderStyle::Def { style, width })
        }
        other => Err(format!(
            "StyleValue: unknown EDT inner Border xsi:type {other:?} (witnessed: BorderDef — §1.0)"
        )),
    }
}

fn decode_edt_font(inner: &Element, inner_xsi: &str) -> Result<FontStyle, String> {
    match inner_xsi {
        EDT_FONT_REF => {
            // Порядок полей фиксирован: font [height] [bold] [italic] [underline] [strikeout]
            // [scale] — КАЖДЫЙ флаг независимо опционален (тристейт: опущен = не
            // переопределён; witnessed ERP `ЖирныйПодчеркнутыйШрифт` — только
            // bold+underline, `ПодчеркнутыйШрифт` — только underline).
            let mut it = EdtChildren::new(inner);
            let font = it.require_text("font")?;
            let font = font_ref_canon_from_edt(&font)?;
            let height = it.opt_edt_height("height")?;
            let flags = decode_edt_font_flags(&mut it)?;
            let scale = it.opt_u32("scale")?;
            it.finish()?;
            Ok(FontStyle::Ref { font, height, flags, scale })
        }
        EDT_FONT_DEF => {
            // faceName + height + опциональные true-флаги (EDT опускает false; witnessed
            // ERP: `ЖирныйШрифтEDI` bold / `МелкийНаклонныйШрифтБЭД` italic /
            // `ПодчеркнутыйТекстСервисДоставки` underline / `ЗачеркнутыйШрифтБЭД` strikeout).
            let mut it = EdtChildren::new(inner);
            let face_name = it.require_text("faceName")?;
            let height = it.require_edt_height("height")?;
            let face = FontFace {
                bold: it.opt_bool("bold")?.unwrap_or(false),
                italic: it.opt_bool("italic")?.unwrap_or(false),
                underline: it.opt_bool("underline")?.unwrap_or(false),
                strikeout: it.opt_bool("strikeout")?.unwrap_or(false),
            };
            it.finish()?;
            Ok(FontStyle::Def { face_name, height, face })
        }
        other => Err(format!(
            "StyleValue: unknown EDT inner Font xsi:type {other:?} (witnessed: FontRef/FontDef — §1.0)"
        )),
    }
}

/// Разобрать тристейт-флаги начертания (`bold`/`italic`/`underline`/`strikeout`):
/// каждый флаг НЕЗАВИСИМО опционален (отсутствие = не переопределён), порядок фиксирован —
/// пропуски не меняют порядок остальных (witnessed ERP: bold+underline без italic/strikeout).
fn decode_edt_font_flags(it: &mut EdtChildren) -> Result<FontFlags, String> {
    Ok(FontFlags {
        bold: it.opt_bool("bold")?,
        italic: it.opt_bool("italic")?,
        underline: it.opt_bool("underline")?,
        strikeout: it.opt_bool("strikeout")?,
    })
}

fn decode_edt_color(inner: &Element, inner_xsi: &str) -> Result<ColorStyle, String> {
    match inner_xsi {
        EDT_COLOR_REF => {
            let mut it = EdtChildren::new(inner);
            let color = it.require_text("color")?;
            it.finish()?;
            color_ref_validate(&color)?;
            Ok(ColorStyle::Ref(color))
        }
        EDT_COLOR_DEF => {
            // Каждая компонента ОПЦИОНАЛЬНА (0 опускается), порядок фикс red/green/blue.
            let mut it = EdtChildren::new(inner);
            let red = it.opt_component("red")?;
            let green = it.opt_component("green")?;
            let blue = it.opt_component("blue")?;
            it.finish()?;
            Ok(ColorStyle::Def { red, green, blue })
        }
        other => Err(format!(
            "StyleValue: unknown EDT inner Color xsi:type {other:?} (witnessed: ColorRef/ColorDef — §1.0)"
        )),
    }
}
