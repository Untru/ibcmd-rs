//! READ · font parsing & canonicalization for both dialects (EDT `<font>` + Designer
//! `<Font>` refs → canonical [`FontRef`]).

use super::*;

/// EDT `<font xsi:type="core:FontRef"><font>Style.X</font>[<faceName>][<height>][<bold>]
/// [<italic>][<underline>][<strikeout>][<scale>]</font>` → [`FontRef`], либо АБСОЛЮТ
/// `<font xsi:type="core:FontDef">[<faceName>][<height>]…</font>` (без `<font>`-ссылки;
/// witness УничтожениеПерсональныхДанных `<height>11.0` — EDT эмитит SPARSE, только
/// не-дефолт). `kind` НЕ хранится (выводится из префикса ссылки / `Absolute` у Def).
pub(crate) fn read_edt_font(el: &Element) -> Result<FontRef, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| FormError::Frame("font: no xsi:type".into()))?;
    // ТРИ вида (§1.0): `FontRef` (ссылка), `FontDef` (абсолют), `AutoFont` (наследуемая база +
    // пер-компонентные переопределения). AutoFont и FontDef оба БЕЗ `<font>`-ссылки — их
    // различает лишь xsi:type ⇒ храним `auto`-бит (см. `FontRef::auto`).
    let (is_ref, is_auto) = match xt.value.as_str() {
        "core:FontRef" => (true, false),
        "core:FontDef" => (false, false),
        "core:AutoFont" => (false, true),
        other => {
            return Err(FormError::Frame(format!(
                "font xsi:type={other:?}, want core:FontRef|core:FontDef|core:AutoFont (§1.0)"
            )))
        }
    };
    xt.claimed.set(true);
    let font_ref = if is_ref {
        let r = leaf_text(el, "font")?;
        // Симметрично Designer-ридеру (`read_designer_font`): префикс канон-ссылки ДОЛЖЕН быть
        // моделируемым (`Style.`→StyleItem / `System.`→WindowsFont). Немоделированный префикс —
        // типизированная §1.0-ошибка (а НЕ молчаливый fallback к «StyleItem»).
        font_kind_for(&r)?;
        Some(r)
    } else {
        None
    };
    let opt_text = |tag: &str| -> Option<String> {
        el.child(tag).filter(|c| c.prefix.is_empty()).map(|c| {
            c.claim_with_text();
            c.text.clone()
        })
    };
    let opt_bool = |tag: &str| -> Result<Option<bool>, FormError> {
        match el.child(tag).filter(|c| c.prefix.is_empty()) {
            Some(c) => Ok(Some(matches!(
                read_bool_text(c, tag)?,
                PropertyValue::Bool(true)
            ))),
            None => Ok(None),
        }
    };
    let face_name = opt_text("faceName");
    let height = opt_text("height");
    let bold = opt_bool("bold")?;
    let italic = opt_bool("italic")?;
    let underline = opt_bool("underline")?;
    let strikeout = opt_bool("strikeout")?;
    let scale = opt_text("scale");
    expect_only_children(
        el,
        &[
            "font",
            "faceName",
            "height",
            "bold",
            "italic",
            "underline",
            "strikeout",
            "scale",
        ],
    )?;
    Ok(FontRef {
        auto: is_auto,
        font_ref,
        face_name,
        height,
        bold,
        italic,
        underline,
        strikeout,
        scale,
    })
}

/// Designer `<Font ref="style:X" [faceName height bold italic underline strikeout] kind
/// [scale]/>` → [`FontRef`]. `ref` канонизуется к EDT-форме; `kind` сверяется с выводимым.
///
/// БЕЗ `@ref` — АБСОЛЮТНЫЙ шрифт (`kind="Absolute"`, DENSE-эмиссия: witness
/// `faceName="" height="11" bold="false" … scale="100"`): дефолты сводятся к sparse-канону
/// (`faceName=""`→None, флаг `false`→None, `scale="100"`→None) — зеркало EDT-омиссии.
pub(crate) fn read_designer_font(el: &Element) -> Result<FontRef, FormError> {
    el.claim();
    let opt_attr = |name: &str| -> Option<String> {
        el.attr(name).map(|a| {
            a.claimed.set(true);
            a.value.clone()
        })
    };
    let opt_bool_attr = |name: &str| -> Result<Option<bool>, FormError> {
        match el.attr(name) {
            Some(a) => {
                a.claimed.set(true);
                match a.value.as_str() {
                    "true" => Ok(Some(true)),
                    "false" => Ok(Some(false)),
                    other => Err(FormError::Frame(format!(
                        "Font @{name}={other:?}, want bool"
                    ))),
                }
            }
            None => Ok(None),
        }
    };
    let font_ref = match opt_attr("ref") {
        Some(r) => Some(designer_font_ref_to_canon(&r)?),
        None => None,
    };
    let mut face_name = opt_attr("faceName");
    let mut height = opt_attr("height").map(|h| designer_height_to_canon(&h));
    let mut bold = opt_bool_attr("bold")?;
    let mut italic = opt_bool_attr("italic")?;
    let mut underline = opt_bool_attr("underline")?;
    let mut strikeout = opt_bool_attr("strikeout")?;
    // kind — денормализация вида: ссылка (Ref) / `Absolute` (Def, DENSE) / `AutoFont` (наследуемая
    // база + переопределения, SPARSE как ref). Absolute и AutoFont оба без @ref ⇒ kind различает.
    // Сверяем с выводимым, храним лишь `auto`-бит.
    let kind = attr_value(el, "kind")?;
    let is_auto = kind == "AutoFont";
    let want_kind = match (&font_ref, is_auto) {
        (Some(r), false) => font_kind_for(r)?,
        (None, true) => "AutoFont",
        (None, false) => "Absolute",
        (Some(_), true) => {
            return Err(FormError::Frame(format!(
                "Font kind=\"AutoFont\" carries @ref {font_ref:?} (unmodeled — §1.0)"
            )))
        }
    };
    if kind != want_kind {
        return Err(FormError::Frame(format!(
            "Font kind={kind:?} for ref {font_ref:?}, derived {want_kind:?} (§1.0)"
        )));
    }
    // DENSE-редукция — ТОЛЬКО у Absolute (Def); AutoFont эмитит sparse (лишь не-дефолт), как ref.
    let is_def = font_ref.is_none() && !is_auto;
    let mut scale = opt_attr("scale");
    if is_def {
        // DENSE→sparse: Designer эмитит дефолты явно — сводим их к отсутствию (канон = EDT-омиссия).
        face_name = face_name.filter(|f| !f.is_empty());
        height = match height {
            Some(h) => Some(h),
            None => {
                return Err(FormError::Frame(
                    "Font kind=Absolute: no @height (§1.0)".into(),
                ))
            }
        };
        bold = bold.filter(|b| *b);
        italic = italic.filter(|b| *b);
        underline = underline.filter(|b| *b);
        strikeout = strikeout.filter(|b| *b);
        scale = scale.filter(|s| s != "100");
    }
    if !el.children.is_empty() {
        return Err(FormError::Frame("Font: unexpected children (§1.0)".into()));
    }
    Ok(FontRef {
        auto: is_auto,
        font_ref,
        face_name,
        height,
        bold,
        italic,
        underline,
        strikeout,
        scale,
    })
}

/// Канон-ссылка шрифта (EDT-форма) ← Designer (`style:X`→`Style.X`, `sys:X`→`System.X`).
pub(crate) fn designer_font_ref_to_canon(text: &str) -> Result<String, FormError> {
    if let Some(rest) = text.strip_prefix("style:") {
        Ok(format!("Style.{rest}"))
    } else if let Some(rest) = text.strip_prefix("sys:") {
        Ok(format!("System.{rest}"))
    } else {
        Err(FormError::Frame(format!(
            "Font ref={text:?}: unmodeled prefix (§1.0 — witnessed style:/sys:)"
        )))
    }
}

/// Выводимый `kind` Designer-шрифта по префиксу канон-ссылки (`Style.`→`StyleItem`,
/// `System.`→`WindowsFont`).
pub(crate) fn font_kind_for(canon_ref: &str) -> Result<&'static str, FormError> {
    if canon_ref.starts_with("Style.") {
        Ok("StyleItem")
    } else if canon_ref.starts_with("System.") {
        Ok("WindowsFont")
    } else {
        Err(FormError::Frame(format!(
            "Font ref={canon_ref:?}: no derivable kind (§1.0)"
        )))
    }
}

/// Канон-высота (EDT-форма `11.0`) ← Designer (`11`): добавить `.0`, если нет дробной точки.
pub(crate) fn designer_height_to_canon(h: &str) -> String {
    if h.contains('.') {
        h.to_string()
    } else {
        format!("{h}.0")
    }
}

