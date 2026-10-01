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
    if s.contains(['.', 'e', 'E']) {
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

/// Канон цвета валиден? (`auto` | `#RRGGBB` | `style:Имя`; web:/win:-цвета не витнесснуты.)
pub(crate) fn check_color_canon(c: &str, path: &str) -> Result<(), FormError> {
    let hex_ok =
        c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if c == "auto" || hex_ok || c.strip_prefix("style:").is_some_and(|n| !n.is_empty()) {
        Ok(())
    } else {
        Err(frame(format!(
            "chart {path}: цвет {c:?} — не витнесснутая форма (ожидались auto/#RRGGBB/style:Имя) (§1.0)"
        )))
    }
}

/// `#RRGGBB` → (r, g, b).
pub(crate) fn parse_hex_color(c: &str, path: &str) -> Result<(u8, u8, u8), FormError> {
    let bad = || frame(format!("chart {path}: не-hex цвет {c:?} (§1.0)"));
    let h = c.strip_prefix('#').ok_or_else(bad)?;
    if h.len() != 6 {
        return Err(bad());
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
    Ok((p(0)?, p(2)?, p(4)?))
}

/// АВТО-шрифт (в chart-контексте FontRef со всеми None ≡ designer `kind="AutoFont"` ≡ EDT
/// `core:AutoFont`; абсолютные шрифты в диаграммах не витнесснуты и отказывают на чтении).
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
                )))
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
