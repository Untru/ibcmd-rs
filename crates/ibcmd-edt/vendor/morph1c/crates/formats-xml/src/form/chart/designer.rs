//! CHART · DESIGNER-диалект: чтение инлайн `<Settings xsi:type="d4p1:Chart|GanttChart">`
//! и эхо-запись (X-байт-точность designer→designer). Отдельно от EDT-кодека (`edt.rs`) —
//! два диалектных кодека НЕ сливать (см. модульный док `mod.rs`).

use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────────
// DESIGNER: ЧТЕНИЕ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Прочитать designer-инлайн `<Settings xmlns:d4p1=… xsi:type="d4p1:Chart|GanttChart">` →
/// [`ChartSettings`]. Клеймит ВСЁ поддерево (тотальность §1.0 сверяет вызывающий по корню
/// формы). Порядок полей файла СОХРАНЯЕТСЯ в `fields` (эхо-запись byte-exact).
pub(crate) fn read_designer_chart_settings(
    kind: &str,
    s: &Element,
) -> Result<ChartSettings, FormError> {
    let t = top_tbl(kind)?;
    s.claim();
    if let Some(a) = s.attr("xmlns:d4p1") {
        if a.value != CHART_D4P1_NS_URI {
            return Err(frame(format!(
                "chart <Settings>: xmlns:d4p1={:?}, ожидался {CHART_D4P1_NS_URI:?} (§1.0)",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    if let Some(a) = s.attr("xsi:type") {
        let want = format!("d4p1:{kind}");
        if a.value != want {
            return Err(frame(format!(
                "chart <Settings>: xsi:type={:?}, ожидался {want:?} (§1.0)",
                a.value
            )));
        }
        a.claimed.set(true);
    }
    expect_attrs_claimed(s, "<Settings>")?;
    let fields = d_read_children(&s.children, t, "<Settings>")?;
    Ok(ChartSettings {
        kind: kind.to_string(),
        fields,
    })
}

/// Прочитать designer-детей по таблице `t` (префикс `d4p1`), сохранив порядок файла.
fn d_read_children(
    children: &[Element],
    t: Tbl,
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    let mut out: Vec<(String, ChartValue)> = Vec::new();
    let mut seen: Vec<&'static str> = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let el = &children[i];
        if el.prefix != "d4p1" {
            return Err(frame(format!(
                "chart {path}: незнакомое поле <{}:{}> (§1.0)",
                el.prefix, el.local
            )));
        }
        let row = row_by_designer_name(t, &el.local).ok_or_else(|| {
            frame(format!(
                "chart {path}: незнакомое поле <{}> (§1.0)",
                el.local
            ))
        })?;
        if seen.contains(&row.name) {
            return Err(frame(format!(
                "chart {path}: поле <{}> повторяется не подряд (§1.0)",
                el.local
            )));
        }
        seen.push(row.name);
        match row.shape {
            Shape::SeriesItems | Shape::PointItems | Shape::Items(_) => {
                let sub = match row.shape {
                    Shape::SeriesItems => Tbl::SeriesItem,
                    Shape::PointItems => Tbl::PointItem,
                    Shape::Items(t2) => t2,
                    _ => unreachable!(),
                };
                let mut items = Vec::new();
                while i < children.len()
                    && children[i].prefix == "d4p1"
                    && children[i].local == el.local
                {
                    let host = &children[i];
                    host.claim();
                    expect_attrs_claimed(host, path)?;
                    items.push(d_read_children(
                        &host.children,
                        sub,
                        &format!("{path}/{}", row.name),
                    )?);
                    i += 1;
                }
                out.push((row.name.to_string(), ChartValue::Items(items)));
            }
            _ => {
                out.push((
                    row.name.to_string(),
                    d_read_value(el, row, &format!("{path}/{}", row.name))?,
                ));
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Прочитать ОДНО designer-значение по шейпу строки.
fn d_read_value(el: &Element, row: &Row, path: &str) -> Result<ChartValue, FormError> {
    match row.shape {
        Shape::Bool => Ok(ChartValue::Bool(parse_bool(&leaf_text(el, path)?, path)?)),
        Shape::Int => Ok(ChartValue::Int(leaf_text(el, path)?)),
        // Канон decimal — EDT-лексика (`10.0`), designer эмитит без `.0` (ср. FontRef::height).
        Shape::Dec => Ok(ChartValue::Int(edt_decimal(&leaf_text(el, path)?))),
        Shape::Str | Shape::DateTime => Ok(ChartValue::Str(leaf_text(el, path)?)),
        Shape::Enum => Ok(ChartValue::Enum(leaf_text(el, path)?)),
        Shape::Color => {
            let c = leaf_text(el, path)?;
            check_color_canon(&c, path)?;
            Ok(ChartValue::Color(c))
        }
        Shape::Font => d_read_font(el, path),
        Shape::Line => d_read_line(el, path),
        Shape::Border => d_read_border(el, path),
        Shape::Loc => d_read_loc(el, path),
        Shape::Rect => read_rect(el, "d4p1", path),
        Shape::Nested(t2) => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(d_read_children(&el.children, t2, path)?))
        }
        Shape::ChartTable => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(d_read_children(
                &el.children,
                Tbl::Chart,
                path,
            )?))
        }
        Shape::SeriesItem => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(d_read_children(
                &el.children,
                Tbl::SeriesItem,
                path,
            )?))
        }
        Shape::DataItems => d_read_data_items(el, path),
        Shape::QBands => {
            el.claim();
            if !el.children.is_empty() {
                return Err(frame(format!(
                    "chart {path}: непустые gaugeQualityBands не витнесснуты (§1.0)"
                )));
            }
            let text = parse_bool(&attr_req(el, "useTextStr", path)?, path)?;
            let tip = parse_bool(&attr_req(el, "useTooltipStr", path)?, path)?;
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(vec![
                ("useTextStr".to_string(), ChartValue::Bool(text)),
                ("useTooltipStr".to_string(), ChartValue::Bool(tip)),
            ]))
        }
        Shape::SeriesItems | Shape::PointItems | Shape::Items(_) => Err(frame(format!(
            "chart {path}: повторяемый шейп в одиночном контексте (внутренняя ошибка кодека)"
        ))),
    }
}

/// Designer-шрифт: `<f kind="AutoFont"/>` | `<f ref="style:X" [height="6"] kind="StyleItem"/>`.
fn d_read_font(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    if !el.children.is_empty() {
        return Err(frame(format!(
            "chart {path}: шрифт с детьми не витнесснут (§1.0)"
        )));
    }
    let kind = attr_req(el, "kind", path)?;
    let font = match kind.as_str() {
        "AutoFont" => auto_font(),
        "StyleItem" => {
            let r = attr_req(el, "ref", path)?;
            let name = r.strip_prefix("style:").ok_or_else(|| {
                frame(format!(
                    "chart {path}: ref={r:?} — не style:-ссылка шрифта (§1.0)"
                ))
            })?;
            let height = match el.attr("height") {
                Some(a) => {
                    a.claimed.set(true);
                    Some(edt_decimal(&a.value))
                }
                None => None,
            };
            FontRef {
                auto: false,
                font_ref: Some(format!("Style.{name}")),
                height,
                bold: d_font_bool(el, "bold", path)?,
                italic: d_font_bool(el, "italic", path)?,
                underline: d_font_bool(el, "underline", path)?,
                strikeout: d_font_bool(el, "strikeout", path)?,
                ..auto_font()
            }
        }
        other => {
            return Err(frame(format!(
                "chart {path}: шрифт kind={other:?} не витнесснут (§1.0)"
            )));
        }
    };
    expect_attrs_claimed(el, path)?;
    Ok(ChartValue::Font(font))
}

fn d_font_bool(el: &Element, name: &str, path: &str) -> Result<Option<bool>, FormError> {
    match el.attr(name) {
        None => Ok(None),
        Some(a) => {
            a.claimed.set(true);
            match a.value.as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(frame(format!("chart {path}: @{name} must be bool"))),
            }
        }
    }
}

/// Designer-линия: `<l width="2" gap="false"><v8ui:style xsi:type="v8ui:ChartLineType">…`.
fn d_read_line(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    let width = attr_req(el, "width", path)?;
    let gap = parse_bool(&attr_req(el, "gap", path)?, path)?;
    expect_attrs_claimed(el, path)?;
    let style = d_read_v8ui_style(el, "v8ui:ChartLineType", path)?;
    Ok(ChartValue::Line { style, width, gap })
}

/// Designer-рамка: `<b width="1"><v8ui:style xsi:type="v8ui:ControlBorderType">…`.
fn d_read_border(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    let width = attr_req(el, "width", path)?;
    expect_attrs_claimed(el, path)?;
    let style = d_read_v8ui_style(el, "v8ui:ControlBorderType", path)?;
    Ok(ChartValue::Border { style, width })
}

/// Единственный ребёнок `<v8ui:style xsi:type="…">литерал</v8ui:style>`.
fn d_read_v8ui_style(el: &Element, xsi: &str, path: &str) -> Result<String, FormError> {
    let mut style = None;
    for c in &el.children {
        if c.prefix == "v8ui" && c.local == "style" && style.is_none() {
            claim_xsi(c, xsi, path)?;
            c.claim_with_text();
            expect_attrs_claimed(c, path)?;
            style = Some(c.text.clone());
        } else {
            return Err(frame(format!(
                "chart {path}: незнакомый ребёнок <{}:{}> (§1.0)",
                c.prefix, c.local
            )));
        }
    }
    style.ok_or_else(|| frame(format!("chart {path}: нет <v8ui:style> (§1.0)")))
}

/// Designer-локализация: `<t><v8:item><v8:lang>…<v8:content>…</v8:item>…</t>`; пустой ⇒ [].
fn d_read_loc(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut pairs = Vec::new();
    for item in &el.children {
        if item.prefix != "v8" || item.local != "item" {
            return Err(frame(format!(
                "chart {path}: незнакомый ребёнок <{}:{}> в локализации (§1.0)",
                item.prefix, item.local
            )));
        }
        item.claim();
        expect_attrs_claimed(item, path)?;
        let mut lang = None;
        let mut content = None;
        for c in &item.children {
            match (c.prefix.as_str(), c.local.as_str()) {
                ("v8", "lang") if lang.is_none() => {
                    c.claim_with_text();
                    expect_attrs_claimed(c, path)?;
                    lang = Some(c.text.clone());
                }
                ("v8", "content") if content.is_none() => {
                    c.claim_with_text();
                    expect_attrs_claimed(c, path)?;
                    content = Some(c.text.clone());
                }
                _ => {
                    return Err(frame(format!(
                        "chart {path}: незнакомый ребёнок <{}:{}> в v8:item (§1.0)",
                        c.prefix, c.local
                    )));
                }
            }
        }
        let lang =
            lang.ok_or_else(|| frame(format!("chart {path}: v8:item без v8:lang (§1.0)")))?;
        let content =
            content.ok_or_else(|| frame(format!("chart {path}: v8:item без v8:content (§1.0)")))?;
        pairs.push((Lang(lang), content));
    }
    Ok(ChartValue::Localized(pairs))
}

/// Designer realDataItems: `<item><valData xsi:type="xs:decimal">…</valData>
/// <valInfo xsi:nil="true"/><toolTip/></item>…` (valInfo/toolTip — фрейм-константы).
fn d_read_data_items(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut items = Vec::new();
    for item in &el.children {
        if item.prefix != "d4p1" || item.local != "item" {
            return Err(frame(format!(
                "chart {path}: незнакомый ребёнок <{}:{}> в realDataItems (§1.0)",
                item.prefix, item.local
            )));
        }
        item.claim();
        expect_attrs_claimed(item, path)?;
        let mut val = None;
        for c in &item.children {
            if c.prefix != "d4p1" {
                return Err(frame(format!(
                    "chart {path}: незнакомый ребёнок <{}:{}> в item (§1.0)",
                    c.prefix, c.local
                )));
            }
            match c.local.as_str() {
                "valData" if val.is_none() => {
                    claim_xsi(c, "xs:decimal", path)?;
                    c.claim_with_text();
                    expect_attrs_claimed(c, path)?;
                    val = Some(c.text.clone());
                }
                "valInfo" => {
                    let nil = attr_req(c, "xsi:nil", path)?;
                    if nil != "true" || !c.children.is_empty() || !c.text.is_empty() {
                        return Err(frame(format!(
                            "chart {path}: valInfo item'а — не nil-константа (§1.0)"
                        )));
                    }
                    c.claim();
                    expect_attrs_claimed(c, path)?;
                }
                "toolTip" => {
                    if !c.children.is_empty() || !c.text.is_empty() {
                        return Err(frame(format!(
                            "chart {path}: непустой toolTip item'а не витнесснут (§1.0)"
                        )));
                    }
                    c.claim();
                    expect_attrs_claimed(c, path)?;
                }
                other => {
                    return Err(frame(format!(
                        "chart {path}: незнакомое поле <{other}> в item (§1.0)"
                    )));
                }
            }
        }
        let val = val.ok_or_else(|| frame(format!("chart {path}: item без valData (§1.0)")))?;
        items.push(vec![("valData".to_string(), ChartValue::Int(val))]);
    }
    Ok(ChartValue::Items(items))
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// DESIGNER: ЗАПИСЬ (эхо cs.fields — X-байт-точность designer→designer)
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Собрать designer-инлайн `<Settings xmlns:d4p1=… xsi:type="d4p1:Chart|GanttChart">` из
/// [`ChartSettings`]. Дети — в порядке `cs.fields` (эхо исходного порядка). EDT-only поле
/// в источнике (EDT-чтение) ⇒ типизированный отказ: designer-плотная материализация из
/// EDT-источника не витнесснута.
pub(crate) fn designer_chart_settings(cs: &ChartSettings) -> Result<OutElement, FormError> {
    let t = top_tbl(&cs.kind)?;
    let mut s = OutElement::branch("", "Settings")
        .attr("xmlns:d4p1", CHART_D4P1_NS_URI)
        .attr("xsi:type", format!("d4p1:{}", cs.kind));
    for out in d_children_out(&cs.fields, t, &cs.kind)? {
        s.push(out);
    }
    Ok(s)
}

/// Designer-эмиссия детей по таблице (эхо порядка `fields`).
fn d_children_out(
    fields: &[(String, ChartValue)],
    t: Tbl,
    path: &str,
) -> Result<Vec<OutElement>, FormError> {
    let mut outs = Vec::new();
    for (name, v) in fields {
        let row = row_by_name(t, name)
            .ok_or_else(|| frame(format!("chart {path}: незнакомое поле {name:?} (§1.0)")))?;
        if !row.designer {
            return Err(frame(format!(
                "chart {path}: designer-форма поля {name:?} не витнесснута (EDT-only) (§1.0)"
            )));
        }
        outs.extend(d_value_out(row, v, &format!("{path}/{name}"))?);
    }
    Ok(outs)
}

/// Designer-эмиссия ОДНОГО поля (повторяемые шейпы дают несколько элементов).
fn d_value_out(row: &Row, v: &ChartValue, path: &str) -> Result<Vec<OutElement>, FormError> {
    let name = row.d_name();
    let one = |el: OutElement| Ok(vec![el]);
    match (row.shape, v) {
        (Shape::Bool, ChartValue::Bool(b)) => one(OutElement::leaf(
            "d4p1",
            name,
            if *b { "true" } else { "false" },
        )),
        (Shape::Int, ChartValue::Int(s)) => one(OutElement::leaf("d4p1", name, s.clone())),
        (Shape::Dec, ChartValue::Int(s)) => {
            one(OutElement::leaf("d4p1", name, designer_decimal(s)))
        }
        (Shape::Str | Shape::DateTime, ChartValue::Str(s)) => one(if s.is_empty() {
            OutElement::self_closing("d4p1", name)
        } else {
            OutElement::leaf("d4p1", name, s.clone())
        }),
        (Shape::Enum, ChartValue::Enum(s)) => one(OutElement::leaf("d4p1", name, s.clone())),
        (Shape::Color, ChartValue::Color(c)) => {
            check_color_canon(c, path)?;
            one(OutElement::leaf("d4p1", name, c.clone()))
        }
        (Shape::Font, ChartValue::Font(f)) => one(d_font_out(name, f, path)?),
        (Shape::Line, ChartValue::Line { style, width, gap }) => {
            let mut l = OutElement::branch("d4p1", name)
                .attr("width", width.clone())
                .attr("gap", if *gap { "true" } else { "false" });
            l.push(
                OutElement::leaf("v8ui", "style", style.clone())
                    .attr("xsi:type", "v8ui:ChartLineType"),
            );
            one(l)
        }
        (Shape::Border, ChartValue::Border { style, width }) => {
            let mut b = OutElement::branch("d4p1", name).attr("width", width.clone());
            b.push(
                OutElement::leaf("v8ui", "style", style.clone())
                    .attr("xsi:type", "v8ui:ControlBorderType"),
            );
            one(b)
        }
        (Shape::Loc, ChartValue::Localized(pairs)) => {
            if pairs.is_empty() {
                return one(OutElement::self_closing("d4p1", name));
            }
            let mut host = OutElement::branch("d4p1", name);
            for (lang, content) in pairs {
                let mut item = OutElement::branch("v8", "item");
                item.push(OutElement::leaf("v8", "lang", lang.as_str()));
                item.push(OutElement::leaf("v8", "content", content.clone()));
                host.push(item);
            }
            one(host)
        }
        (
            Shape::Rect,
            ChartValue::Rect {
                left,
                right,
                top,
                bottom,
            },
        ) => {
            let mut r = OutElement::branch("d4p1", name);
            r.push(OutElement::leaf("d4p1", "left", left.clone()));
            r.push(OutElement::leaf("d4p1", "right", right.clone()));
            r.push(OutElement::leaf("d4p1", "top", top.clone()));
            r.push(OutElement::leaf("d4p1", "bottom", bottom.clone()));
            one(r)
        }
        (Shape::Nested(t2), ChartValue::Nested(f)) => one(d_composite_out(name, f, t2, path)?),
        (Shape::ChartTable, ChartValue::Nested(f)) => {
            one(d_composite_out(name, f, Tbl::Chart, path)?)
        }
        (Shape::SeriesItem, ChartValue::Nested(f)) => {
            one(d_composite_out(name, f, Tbl::SeriesItem, path)?)
        }
        (Shape::SeriesItems | Shape::PointItems | Shape::Items(_), ChartValue::Items(items)) => {
            let sub = match row.shape {
                Shape::SeriesItems => Tbl::SeriesItem,
                Shape::PointItems => Tbl::PointItem,
                Shape::Items(t2) => t2,
                _ => unreachable!(),
            };
            let mut outs = Vec::new();
            for item in items {
                outs.push(d_composite_out(name, item, sub, path)?);
            }
            Ok(outs)
        }
        (Shape::DataItems, ChartValue::Items(items)) => {
            let mut host = OutElement::branch("d4p1", name);
            for item in items {
                let [(vn, vv)] = item.as_slice() else {
                    return Err(frame(format!(
                        "chart {path}: item realDataItems несёт не ровно valData (§1.0)"
                    )));
                };
                let ChartValue::Int(text) = vv else {
                    return Err(frame(format!(
                        "chart {path}: valData — не лексическое число (§1.0)"
                    )));
                };
                if vn != "valData" {
                    return Err(frame(format!(
                        "chart {path}: незнакомое поле {vn:?} в item realDataItems (§1.0)"
                    )));
                }
                let mut it = OutElement::branch("d4p1", "item");
                it.push(
                    OutElement::leaf("d4p1", "valData", text.clone())
                        .attr("xsi:type", "xs:decimal"),
                );
                it.push(OutElement::self_closing("d4p1", "valInfo").attr("xsi:nil", "true"));
                it.push(OutElement::self_closing("d4p1", "toolTip"));
                host.push(it);
            }
            if host.children.is_empty() {
                host.self_closing = true;
            }
            one(host)
        }
        (Shape::QBands, ChartValue::Nested(f)) => {
            let get = |n: &str| -> Result<bool, FormError> {
                match f.iter().find(|(fname, _)| fname == n) {
                    Some((_, ChartValue::Bool(b))) => Ok(*b),
                    _ => Err(frame(format!(
                        "chart {path}: gaugeQualityBands без bool-поля {n:?} (§1.0)"
                    ))),
                }
            };
            let text = get("useTextStr")?;
            let tip = get("useTooltipStr")?;
            one(OutElement::self_closing("d4p1", name)
                .attr("useTextStr", if text { "true" } else { "false" })
                .attr("useTooltipStr", if tip { "true" } else { "false" }))
        }
        (shape, other) => Err(frame(format!(
            "chart {path}: значение {other:?} не соответствует шейпу {shape:?} (§1.0)"
        ))),
    }
}

/// Designer-композит: `<d4p1:name>` + эхо-дети; пустой ⇒ самозакрытый.
fn d_composite_out(
    name: &str,
    fields: &[(String, ChartValue)],
    t: Tbl,
    path: &str,
) -> Result<OutElement, FormError> {
    let mut host = OutElement::branch("d4p1", name);
    for out in d_children_out(fields, t, path)? {
        host.push(out);
    }
    if host.children.is_empty() {
        host.self_closing = true;
    }
    Ok(host)
}

/// Designer-шрифт из канона [`FontRef`].
fn d_font_out(name: &str, f: &FontRef, path: &str) -> Result<OutElement, FormError> {
    if f.face_name.is_some() || f.scale.is_some() {
        return Err(frame(format!(
            "chart {path}: шрифт с переопределениями (не AutoFont/StyleItem) не витнесснут (§1.0)"
        )));
    }
    match &f.font_ref {
        None => {
            if f.height.is_some()
                || f.bold.is_some()
                || f.italic.is_some()
                || f.underline.is_some()
                || f.strikeout.is_some()
            {
                return Err(frame(format!(
                    "chart {path}: AutoFont с height не витнесснут (§1.0)"
                )));
            }
            Ok(OutElement::self_closing("d4p1", name).attr("kind", "AutoFont"))
        }
        Some(r) => {
            let style_name = r.strip_prefix("Style.").ok_or_else(|| {
                frame(format!(
                    "chart {path}: шрифт-ссылка {r:?} — не Style.-ссылка (§1.0)"
                ))
            })?;
            let mut el =
                OutElement::self_closing("d4p1", name).attr("ref", format!("style:{style_name}"));
            if let Some(h) = &f.height {
                el = el.attr("height", designer_decimal(h));
            }
            for (name, value) in [
                ("bold", f.bold),
                ("italic", f.italic),
                ("underline", f.underline),
                ("strikeout", f.strikeout),
            ] {
                if let Some(value) = value {
                    el = el.attr(name, value.to_string());
                }
            }
            Ok(el.attr("kind", "StyleItem"))
        }
    }
}
