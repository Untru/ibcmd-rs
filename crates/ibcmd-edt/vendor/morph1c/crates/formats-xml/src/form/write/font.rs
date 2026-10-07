//! WRITE · font writers for BOTH dialects (topic leaf): EDT `core:FontRef`/`core:FontDef`
//! and Designer `<Font>`, plus canon→designer ref/height/kind helpers.

use super::*;

/// EDT `<font xsi:type="core:FontRef"><font>ref</font>[<faceName>][<height>][<bold>]…
/// [<scale>]</font>` из [`FontRef`]; `font_ref=None` ⇒ АБСОЛЮТ `core:FontDef` (без
/// `<font>`-ссылки, SPARSE — только Some-поля; witness `<height>11.0`). Порядок детей =
/// EDT-эмиссия.
pub(crate) fn edt_font(f: &FontRef) -> OutElement {
    edt_font_named("font", f)
}

/// [`edt_font`] с явным тегом (`font`/`titleFont` — одна кодировка, разные локусы).
pub(crate) fn edt_font_named(tag: &str, f: &FontRef) -> OutElement {
    // AutoFont (наследуемая база + sparse-переопределения; witness `<bold>true`) — тот же
    // sparse-скелет, что у FontRef/FontDef, но БЕЗ `<font>`-ссылки и с иным xsi:type.
    let xsi = if f.auto {
        "core:AutoFont"
    } else if f.font_ref.is_some() {
        "core:FontRef"
    } else {
        "core:FontDef"
    };
    let mut el = OutElement::branch("", tag).attr("xsi:type", xsi);
    if let Some(r) = &f.font_ref {
        el.push(OutElement::leaf("", "font", r.clone()));
    }
    if let Some(fc) = &f.face_name {
        el.push(OutElement::leaf("", "faceName", fc.clone()));
    }
    if let Some(h) = &f.height {
        el.push(OutElement::leaf("", "height", h.clone()));
    }
    for (v, tag) in [
        (f.bold, "bold"),
        (f.italic, "italic"),
        (f.underline, "underline"),
        (f.strikeout, "strikeout"),
    ] {
        if let Some(b) = v {
            el.push(OutElement::leaf("", tag, if b { "true" } else { "false" }));
        }
    }
    if let Some(s) = &f.scale {
        el.push(OutElement::leaf("", "scale", s.clone()));
    }
    el
}

/// Designer `<Font ref="style:X" [faceName height bold italic underline strikeout] kind
/// [scale]/>` из [`FontRef`]. `ref`←канон (`Style.`→`style:`); `kind` выводится; `height`
/// без `.0`; `scale` — ПОСЛЕ `kind`.
///
/// `font_ref=None` ⇒ АБСОЛЮТ: БЕЗ `@ref`, DENSE-эмиссия дефолтов (witness `faceName=""
/// height="11" bold="false" italic="false" underline="false" strikeout="false"
/// kind="Absolute" scale="100"`).
pub(crate) fn designer_font(f: &FontRef) -> OutElement {
    designer_font_named("Font", f)
}

/// [`designer_font`] с явным тегом (`Font`/`TitleFont` — одна кодировка, разные локусы).
pub(crate) fn designer_font_named(tag: &str, f: &FontRef) -> OutElement {
    let mut el = OutElement::self_closing("", tag);
    // DENSE (эмиссия дефолтов) — ТОЛЬКО у Absolute (Def). AutoFont, как и ref, SPARSE: эмитит лишь
    // заданные переопределения (`bold="true" kind="AutoFont"`), без @ref и без dense-дефолтов.
    let dense = f.font_ref.is_none() && !f.auto;
    if let Some(r) = &f.font_ref {
        el = el.attr("ref", canon_font_ref_to_designer(r));
    }
    if dense {
        el = el.attr("faceName", f.face_name.clone().unwrap_or_default());
    } else if let Some(fc) = &f.face_name {
        el = el.attr("faceName", fc.clone());
    }
    if let Some(h) = &f.height {
        el = el.attr("height", canon_height_to_designer(h));
    }
    for (v, name) in [
        (f.bold, "bold"),
        (f.italic, "italic"),
        (f.underline, "underline"),
        (f.strikeout, "strikeout"),
    ] {
        if dense {
            el = el.attr(name, if v == Some(true) { "true" } else { "false" });
        } else if let Some(b) = v {
            el = el.attr(name, if b { "true" } else { "false" });
        }
    }
    el = el.attr(
        "kind",
        match &f.font_ref {
            Some(r) => font_kind_for(r),
            None if f.auto => "AutoFont",
            None => "Absolute",
        },
    );
    if dense {
        el = el.attr(
            "scale",
            f.scale.clone().unwrap_or_else(|| "100".to_string()),
        );
    } else if let Some(s) = &f.scale {
        el = el.attr("scale", s.clone());
    }
    el
}

/// Designer-ссылка шрифта ← канон (`Style.X`→`style:X`, `System.X`→`sys:X`).
pub(crate) fn canon_font_ref_to_designer(canon: &str) -> String {
    if let Some(rest) = canon.strip_prefix("Style.") {
        format!("style:{rest}")
    } else if let Some(rest) = canon.strip_prefix("System.") {
        format!("sys:{rest}")
    } else {
        canon.to_string()
    }
}

/// Выводимый `kind` по префиксу канон-ссылки (сверяется на read; здесь — реконструируется).
pub(crate) fn font_kind_for(canon: &str) -> &'static str {
    if canon.starts_with("System.") {
        "WindowsFont"
    } else {
        "StyleItem"
    }
}

/// Designer-высота ← канон (EDT `11.0` → `11`): снять хвостовой `.0`.
pub(crate) fn canon_height_to_designer(h: &str) -> String {
    h.strip_suffix(".0")
        .map(|s| s.to_string())
        .unwrap_or_else(|| h.to_string())
}
