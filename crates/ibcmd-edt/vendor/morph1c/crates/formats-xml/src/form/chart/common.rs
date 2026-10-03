//! CHART · общие хелперы ОБОИХ диалектов (§1.0 frame-ошибка, bool/decimal-лексика,
//! claim/attr/leaf-аксессоры, канон цвета, AutoFont, политика EDT-омиссии, rect-кодек).

use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────────
// ОБЩИЕ ХЕЛПЕРЫ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Типизированная §1.0-ошибка кодека.
pub(crate) fn frame(msg: String) -> FormError {
    FormError::Frame(msg)
}

/// bool из текста `true`/`false` — иное значение ⇒ отказ.
pub(crate) fn parse_bool(t: &str, path: &str) -> Result<bool, FormError> {
    match t {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(frame(format!(
            "chart {path}: не-bool текст {other:?} (§1.0)"
        ))),
    }
}

/// EDT-лексика decimal-поля: designer `10` → `10.0` (без точки/экспоненты — дописать `.0`).
pub(crate) fn edt_decimal(s: &str) -> String {
    if matches!(s, "NaN" | "Infinity" | "-Infinity") || s.contains(['.', 'e', 'E']) {
        s.to_string()
    } else {
        format!("{s}.0")
    }
}

/// Designer-лексика decimal-поля: EDT `10.0` → `10` (снимается РОВНО суффикс `.0`).
pub(crate) fn designer_decimal(s: &str) -> String {
    s.strip_suffix(".0").unwrap_or(s).to_string()
}

/// Сверить, что ВСЕ атрибуты элемента востребованы (незнакомый атрибут ⇒ отказ).
pub(crate) fn expect_attrs_claimed(el: &Element, path: &str) -> Result<(), FormError> {
    for a in &el.attrs {
        if !a.claimed.get() {
            return Err(frame(format!(
                "chart {path}: незнакомый атрибут {:?}={:?} на <{}> (§1.0)",
                a.name, a.value, el.local
            )));
        }
    }
    Ok(())
}

/// Claim атрибута `xsi:type` с ожидаемым значением.
pub(crate) fn claim_xsi(el: &Element, want: &str, path: &str) -> Result<(), FormError> {
    let a = el
        .attr("xsi:type")
        .ok_or_else(|| frame(format!("chart {path}: <{}> без xsi:type (§1.0)", el.local)))?;
    if a.value != want {
        return Err(frame(format!(
            "chart {path}: <{}> xsi:type={:?}, ожидался {want:?} (§1.0)",
            el.local, a.value
        )));
    }
    a.claimed.set(true);
    Ok(())
}

/// Обязательный атрибут: claim + значение.
pub(crate) fn attr_req(el: &Element, name: &str, path: &str) -> Result<String, FormError> {
    let a = el.attr(name).ok_or_else(|| {
        frame(format!(
            "chart {path}: <{}> без атрибута {name:?} (§1.0)",
            el.local
        ))
    })?;
    a.claimed.set(true);
    Ok(a.value.clone())
}

/// Текст листа: claim элемента и текста; дети/атрибуты запрещены.
pub(crate) fn leaf_text(el: &Element, path: &str) -> Result<String, FormError> {
    el.claim_with_text();
    if !el.children.is_empty() {
        return Err(frame(format!(
            "chart {path}: <{}> — ожидался лист, найдены дети (§1.0)",
            el.local
        )));
    }
    expect_attrs_claimed(el, path)?;
    Ok(el.text.clone())
}

/// Chart colors use the same symbolic namespaces and RGB codec as form colors.
pub(crate) fn check_color_canon(c: &str, path: &str) -> Result<(), FormError> {
    if c == "auto" {
        return Ok(());
    }
    if ["style:", "web:", "win:", "pal:"].contains(&c) {
        return Err(frame(format!("chart {path}: empty color reference")));
    }
    super::super::fields::color_from_designer(c, path).map(|_| ())
}

/// Full shared FontRef decoder; chart-specific outer names do not change its grammar.
pub(crate) fn chart_read_edt_font(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    let font = super::super::read::read_edt_font(el)?;
    check_chart_font(&font, path)?;
    expect_attrs_claimed(el, path)?;
    Ok(ChartValue::Font(font))
}

pub(crate) fn chart_read_designer_font(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    let font = super::super::read::read_designer_font(el)?;
    check_chart_font(&font, path)?;
    expect_attrs_claimed(el, path)?;
    Ok(ChartValue::Font(font))
}

fn check_chart_font(font: &FontRef, path: &str) -> Result<(), FormError> {
    if let Some(reference) = &font.font_ref {
        if font.auto || ["Style.", "System."].contains(&reference.as_str()) {
            return Err(frame(format!("chart {path}: invalid font reference")));
        }
        super::super::read::font_kind_for(reference)?;
    }
    Ok(())
}

/// FontDef has ordinary defaults; AutoFont and FontRef overrides are unsettable
/// and retain explicit false/zero values. No current nondefault value is removed.
pub(crate) fn normalize_chart_font(mut font: FontRef) -> FontRef {
    if !font.auto && font.font_ref.is_none() {
        font.face_name = font.face_name.filter(|name| !name.is_empty());
        font.height = font
            .height
            .filter(|height| height != "0" && height != "0.0");
        font.bold = font.bold.filter(|value| *value);
        font.italic = font.italic.filter(|value| *value);
        font.underline = font.underline.filter(|value| *value);
        font.strikeout = font.strikeout.filter(|value| *value);
        font.scale = font.scale.filter(|scale| scale != "100");
    }
    font
}

pub(crate) fn chart_edt_font_out(
    name: &str,
    font: &FontRef,
    path: &str,
) -> Result<OutElement, FormError> {
    check_chart_font(font, path)?;
    let mut out = super::super::write::edt_font_named(name, font);
    out.self_closing = out.children.is_empty();
    Ok(out)
}

pub(crate) fn chart_designer_font_out(
    name: &str,
    font: &FontRef,
    path: &str,
) -> Result<OutElement, FormError> {
    check_chart_font(font, path)?;
    // FontDef.height is non-unsettable EFloat with default zero. The native
    // writer emits it densely, while EDT may omit the default.
    let mut current = font.clone();
    if !current.auto && current.font_ref.is_none() && current.height.is_none() {
        current.height = Some("0.0".into());
    }
    let mut out = super::super::write::designer_font_named(name, &current);
    out.prefix = "d4p1".into();
    Ok(out)
}

pub(crate) fn chart_read_edt_color(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    let value = super::super::fields::decode_edt_color(el, path)?;
    let morph1c_core::ir::PropertyValue::Ref(reference) = value else {
        return Err(frame(format!("chart {path}: expected color reference")));
    };
    let color = super::super::fields::color_to_designer(&reference, path)?;
    check_color_canon(&color, path)?;
    expect_attrs_claimed(el, path)?;
    Ok(ChartValue::Color(color))
}

pub(crate) fn chart_edt_color_out(
    name: &str,
    color: &str,
    path: &str,
) -> Result<OutElement, FormError> {
    check_color_canon(color, path)?;
    let reference = super::super::fields::color_from_designer(color, path)?;
    super::super::fields::render_edt_color(name, &reference)
}

/// Current AbstractLine.gap is an ordinary typed bool; sparse EDT absence means false.
pub(crate) fn chart_read_edt_line(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut width = None;
    let mut style = None;
    let mut gap = None;
    for child in &el.children {
        if !child.prefix.is_empty() {
            return Err(frame(format!("chart {path}: unexpected line namespace")));
        }
        match child.local.as_str() {
            "width" if width.is_none() => width = Some(leaf_text(child, path)?),
            "style" if style.is_none() => style = Some(leaf_text(child, path)?),
            "gap" if gap.is_none() => gap = Some(parse_bool(&leaf_text(child, path)?, path)?),
            _ => {
                return Err(frame(format!(
                    "chart {path}: unknown or duplicate line child"
                )));
            }
        }
    }
    Ok(ChartValue::Line {
        style: style.unwrap_or_else(|| "None".into()),
        width: width.unwrap_or_else(|| "0".into()),
        gap: gap.unwrap_or(false),
    })
}

pub(crate) fn chart_edt_line_out(
    name: &str,
    style: &str,
    width: &str,
    gap: bool,
    _path: &str,
) -> Result<OutElement, FormError> {
    let mut out = OutElement::branch("", name);
    out.push(OutElement::leaf("", "width", width));
    if gap {
        out.push(OutElement::leaf("", "gap", "true"));
    }
    out.push(OutElement::leaf("", "style", style));
    Ok(out)
}
/// `#RRGGBB` → (r, g, b).
pub(crate) fn parse_hex_color(c: &str, path: &str) -> Result<(u8, u8, u8), FormError> {
    let bad = || frame(format!("chart {path}: не-hex цвет {c:?} (§1.0)"));
    let h = c.strip_prefix('#').ok_or_else(bad)?;
    if h.len() != 6 || !h.bytes().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
    Ok((p(0)?, p(2)?, p(4)?))
}

/// An unmodified AutoFont; absolute and referenced fonts use the shared full codec.
pub(crate) fn auto_font() -> FontRef {
    FontRef {
        auto: true,
        font_ref: None,
        face_name: None,
        height: None,
        bold: None,
        italic: None,
        underline: None,
        strikeout: None,
        scale: None,
    }
}

/// Канонический токен значения для сверки с литералом омиссии.
fn lit_eq(v: &ChartValue, lit: &str) -> bool {
    match v {
        ChartValue::Bool(b) => (*b && lit == "true") || (!*b && lit == "false"),
        ChartValue::Int(s) | ChartValue::Str(s) | ChartValue::Enum(s) | ChartValue::Color(s) => {
            s == lit
        }
        _ => false,
    }
}

/// Стандартная (шейповая) EDT-омиссия значения.
pub(crate) fn std_omitted(v: &ChartValue) -> bool {
    match v {
        ChartValue::Bool(b) => !*b,
        // «0.0» — канон Dec-полей (designer «0» хранится в EDT-лексике, см. [`edt_decimal`]).
        ChartValue::Int(s) => s == "0" || s == "0.0",
        ChartValue::Str(s) => s.is_empty(),
        ChartValue::Color(c) => c == "auto",
        ChartValue::Localized(pairs) => pairs.is_empty(),
        _ => false,
    }
}

/// Полная политика EDT-омиссии строки.
pub(crate) fn edt_omitted(row: &Row, v: &ChartValue) -> bool {
    match row.edt {
        Edt::AlwaysEmit => false,
        Edt::OmitAlways => true,
        Edt::OmitIf(lit) => lit_eq(v, lit),
        Edt::Std | Edt::Unwitnessed => std_omitted(v),
    }
}

/// Прямоугольник (оба диалекта: дети left/right/top/bottom с ОДНИМ префиксом).
pub(crate) fn read_rect(el: &Element, prefix: &str, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut left = None;
    let mut right = None;
    let mut top = None;
    let mut bottom = None;
    for c in &el.children {
        if c.prefix != prefix {
            return Err(frame(format!(
                "chart {path}: незнакомый ребёнок <{}:{}> прямоугольника (§1.0)",
                c.prefix, c.local
            )));
        }
        let slot = match c.local.as_str() {
            "left" => &mut left,
            "right" => &mut right,
            "top" => &mut top,
            "bottom" => &mut bottom,
            other => {
                return Err(frame(format!(
                    "chart {path}: незнакомая сторона прямоугольника <{other}> (§1.0)"
                )));
            }
        };
        if slot.is_some() {
            return Err(frame(format!(
                "chart {path}: сторона <{}> повторяется (§1.0)",
                c.local
            )));
        }
        c.claim_with_text();
        expect_attrs_claimed(c, path)?;
        *slot = Some(c.text.clone());
    }
    let need = |o: Option<String>, n: &str| {
        o.ok_or_else(|| frame(format!("chart {path}: прямоугольник без <{n}> (§1.0)")))
    };
    Ok(ChartValue::Rect {
        left: need(left, "left")?,
        right: need(right, "right")?,
        top: need(top, "top")?,
        bottom: need(bottom, "bottom")?,
    })
}
