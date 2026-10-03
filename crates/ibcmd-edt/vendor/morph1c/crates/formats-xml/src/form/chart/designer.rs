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
    super::data_items::validate_value_namespaces(s, true)?;
    let fields = d_read_children(&s.children, t, "<Settings>")?;
    super::semantic::from_source(
        kind,
        fields,
        morph1c_core::ir::form::ChartSourceFormat::Designer,
        s,
    )
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
        if t == Tbl::SeriesItem && el.local == "trendline" {
            let mut lines = Vec::new();
            while i < children.len()
                && children[i].prefix == "d4p1"
                && children[i].local == "trendline"
            {
                let line = &children[i];
                line.claim();
                expect_attrs_claimed(line, path)?;
                lines.push(d_read_children(&line.children, Tbl::Trend, path)?);
                i += 1;
            }
            if out.iter().any(|(n, _)| n == "__associated_trendlines") {
                return Err(frame(format!(
                    "chart {path}: nonadjacent repeated trendlines"
                )));
            }
            out.push(("__associated_trendlines".into(), ChartValue::Items(lines)));
            continue;
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
            Shape::Colors if t == Tbl::BackIntervals => {
                let mut items = Vec::new();
                while i < children.len()
                    && children[i].prefix == "d4p1"
                    && children[i].local == el.local
                {
                    let child = &children[i];
                    let color = leaf_text(child, path)?;
                    check_color_canon(&color, path)?;
                    items.push(vec![("value".into(), ChartValue::Color(color))]);
                    i += 1;
                }
                out.push((row.name.into(), ChartValue::Items(items)));
            }
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
    if t == Tbl::Chart {
        collect_associated_trendlines(&mut out)?;
    }
    Ok(out)
}

fn collect_associated_trendlines(fields: &mut Vec<(String, ChartValue)>) -> Result<(), FormError> {
    let mut arrays = Vec::new();
    for (name, value) in fields.iter_mut() {
        if !matches!(name.as_str(), "realSeriesData" | "realExSeriesData") {
            continue;
        }
        let groups: Vec<&mut Vec<(String, ChartValue)>> = match value {
            ChartValue::Items(items) => items.iter_mut().collect(),
            ChartValue::Nested(f) => vec![f],
            _ => return Err(frame("chart: invalid current series shape".into())),
        };
        for f in groups {
            if let Some(i) = f.iter().position(|(n, _)| n == "__associated_trendlines") {
                let (_, lines) = f.remove(i);
                let id = f
                    .iter()
                    .find(|(n, _)| n == "id")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| ChartValue::Int("1".into()));
                arrays.push(vec![("seriesId".into(), id), ("line".into(), lines)]);
            }
        }
    }
    if !arrays.is_empty() {
        if fields.iter().any(|(n, _)| n == "trendLinesArray") {
            return Err(frame(
                "chart: ambiguous root and associated trendline declarations".into(),
            ));
        }
        fields.push(("trendLinesArray".into(), ChartValue::Items(arrays)));
    }
    Ok(())
}

/// Прочитать ОДНО designer-значение по шейпу строки.
fn d_read_value(el: &Element, row: &Row, path: &str) -> Result<ChartValue, FormError> {
    match row.shape {
        Shape::Colors => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            if !el.text.is_empty() {
                return Err(frame(format!("chart {path}: Color list has text")));
            }
            let mut items = Vec::new();
            for child in &el.children {
                if child.prefix != "v8" || child.local != "Value" || !child.children.is_empty() {
                    return Err(frame(format!(
                        "chart {path}: expected typed v8:Value Color"
                    )));
                }
                child.claim_with_text();
                let typ = child
                    .attr("xsi:type")
                    .ok_or_else(|| frame(format!("chart {path}: Color item missing type")))?;
                if typ.value != "v8ui:Color" {
                    return Err(frame(format!("chart {path}: incorrect Color item type")));
                }
                typ.claimed.set(true);
                expect_attrs_claimed(child, path)?;
                check_color_canon(&child.text, path)?;
                items.push(vec![(
                    "value".into(),
                    ChartValue::Color(child.text.clone()),
                )]);
            }
            Ok(ChartValue::Items(items))
        }
        Shape::Picture => super::picture::read(el, path, true),
        Shape::Value
            if matches!(
                row.name,
                "semitransparencyPercent" | "borderSemitransparencyPercent"
            ) && el.attr("xsi:type").is_none() =>
        {
            let number = leaf_text(el, path)?;
            if matches!(number.as_str(), "Infinity" | "-Infinity") {
                return Ok(ChartValue::Value(Box::new(
                    morph1c_core::ir::form::ChartTypedValue::Scalar(
                        morph1c_core::ir::value::PropertyValue::Value(
                            morph1c_core::ir::value::ValueSpec {
                                kind: morph1c_core::ir::value::ValueScalarKind::Undefined,
                                scalar: None,
                            },
                        ),
                    ),
                )));
            }
            if !super::semantic::valid_big_decimal(&number) {
                return Err(frame(format!(
                    "chart {path}: invalid NumberValue percentage"
                )));
            }
            Ok(ChartValue::Value(Box::new(
                morph1c_core::ir::form::ChartTypedValue::Scalar(
                    morph1c_core::ir::value::PropertyValue::Value(
                        morph1c_core::ir::value::ValueSpec {
                            kind: morph1c_core::ir::value::ValueScalarKind::Number,
                            scalar: Some(Box::new(morph1c_core::ir::value::PropertyValue::Str(
                                designer_decimal(&number),
                            ))),
                        },
                    ),
                ),
            )))
        }
        Shape::Value => super::data_items::read_value(el, path, true),
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
            if t2 == Tbl::GaugeBands {
                return d_read_gauge(el, path);
            }
            expect_attrs_claimed(el, path)?;
            if t2 == Tbl::Axis {
                return d_read_axis(el, path);
            }
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
        Shape::DataItems => super::data_items::read_native(el, path),
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
    chart_read_designer_font(el, path)
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
    let fields =
        super::semantic::emission_fields(cs, morph1c_core::ir::form::ChartSourceFormat::Designer)?;
    let mut output = d_children_out(&fields, t, &cs.kind)?;
    super::semantic::restore_native_picture_layout(&mut output, cs)?;
    for out in output {
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
        if t == Tbl::Chart && name == "trendLinesArray" {
            continue;
        }
        outs.extend(d_value_out(t, row, v, &format!("{path}/{name}"))?);
    }
    if t == Tbl::Chart {
        emit_associated_trendlines(&mut outs, fields, path)?;
    }
    Ok(outs)
}

fn emit_associated_trendlines(
    outputs: &mut [OutElement],
    fields: &[(String, ChartValue)],
    path: &str,
) -> Result<(), FormError> {
    let Some((_, value)) = fields.iter().find(|(n, _)| n == "trendLinesArray") else {
        return Ok(());
    };
    let ChartValue::Items(arrays) = value else {
        return Err(frame("chart: trendLinesArray must be typed Items".into()));
    };
    let current = ChartSettings {
        kind: "Chart".into(),
        fields: fields.to_vec(),
        source_layout: None,
    };
    let projected = super::semantic::project_native_trends(&current)?;
    let projected_arrays = projected
        .fields
        .iter()
        .find(|(n, _)| n == "trendLinesArray")
        .map(|(_, v)| v);
    if projected_arrays != Some(value) {
        return Err(frame(
            "chart: unprojected trend topology requires bound typed transport".into(),
        ));
    }
    for series in outputs
        .iter_mut()
        .filter(|e| matches!(e.local.as_str(), "realSeriesData" | "realExSeriesData"))
    {
        let id = series
            .children
            .iter()
            .find(|c| c.local == "id")
            .and_then(|c| c.text.as_deref())
            .unwrap_or("1");
        let Some(array) = arrays.iter().find(|array| {
            array
                .iter()
                .any(|(n, v)| n == "seriesId" && matches!(v,ChartValue::Int(key) if key == id))
        }) else {
            continue;
        };
        let Some(ChartValue::Items(lines)) =
            array.iter().find(|(n, _)| n == "line").map(|(_, v)| v)
        else {
            return Err(frame(
                "chart: trendline association requires current lines".into(),
            ));
        };
        let mut generated = Vec::new();
        for line in lines {
            generated.push(d_composite_out("trendline", line, Tbl::Trend, path)?);
        }
        let position = series
            .children
            .iter()
            .position(|e| e.local == "showGraphicalDataRepresentationInChartLegend")
            .unwrap_or(series.children.len());
        series.children.splice(position..position, generated);
    }
    Ok(())
}

/// Designer-эмиссия ОДНОГО поля (повторяемые шейпы дают несколько элементов).
fn d_value_out(t: Tbl, row: &Row, v: &ChartValue, path: &str) -> Result<Vec<OutElement>, FormError> {
    let name = row.d_name();
    let one = |el: OutElement| Ok(vec![el]);
    match (row.shape, v) {
        (Shape::Colors, ChartValue::Items(items)) => {
            if items.is_empty() {
                return Ok(Vec::new());
            }
            if row.name == "contentCacheItem" {
                let mut result = Vec::new();
                for fields in items {
                    let [(key, ChartValue::Color(color))] = fields.as_slice() else {
                        return Err(frame(
                            "chart: background Color collection item must be typed".into(),
                        ));
                    };
                    if key != "value" {
                        return Err(frame(
                            "chart: background Color collection item has unknown field".into(),
                        ));
                    }
                    check_color_canon(color, path)?;
                    result.push(OutElement::leaf("d4p1", name, color.clone()));
                }
                return Ok(result);
            }
            let mut host = OutElement::branch("d4p1", name);
            for fields in items {
                let [(key, ChartValue::Color(color))] = fields.as_slice() else {
                    return Err(frame(format!(
                        "chart {path}: Color list item must have one typed value"
                    )));
                };
                if key != "value" {
                    return Err(frame(format!("chart {path}: unknown Color item field")));
                }
                check_color_canon(color, path)?;
                host.push(
                    OutElement::leaf("v8", "Value", color.clone()).attr("xsi:type", "v8ui:Color"),
                );
            }
            one(host)
        }
        (Shape::Picture, value) => one(super::picture::write(name, value, path, true)?),
        (Shape::Value, ChartValue::Value(value))
            if matches!(
                row.name,
                "semitransparencyPercent" | "borderSemitransparencyPercent"
            ) =>
        {
            use morph1c_core::ir::form::ChartTypedValue;
            use morph1c_core::ir::value::{PropertyValue, ValueScalarKind};
            if let ChartTypedValue::ProjectedNumber(number) = value.as_ref() {
                if !super::semantic::valid_big_decimal(number)
                    && !matches!(number.as_str(), "Infinity" | "-Infinity")
                {
                    return Err(frame(format!("chart {path}: invalid projected percentage")));
                }
                return one(OutElement::leaf("d4p1", name, number.clone()));
            }
            let ChartTypedValue::Scalar(PropertyValue::Value(v)) = value.as_ref() else {
                return Err(frame(format!(
                    "chart {path}: nonNumber percentage requires bound typed transport"
                )));
            };
            if v.kind != ValueScalarKind::Number {
                return Err(frame(format!(
                    "chart {path}: nonNumber percentage requires bound typed transport"
                )));
            }
            let Some(PropertyValue::Str(number)) = v.scalar.as_deref() else {
                return Err(frame(format!(
                    "chart {path}: NumberValue percentage lacks string scalar"
                )));
            };
            if !super::semantic::valid_big_decimal(number) {
                return Err(frame(format!(
                    "chart {path}: invalid current NumberValue percentage"
                )));
            }
            one(OutElement::leaf(
                "d4p1",
                name,
                super::percent::native_percentage_projection(number)?.0,
            ))
        }
        (Shape::Value, value) => one(super::data_items::write_value(name, value, path, true)?),
        (Shape::Bool, ChartValue::Bool(b)) => one(OutElement::leaf(
            "d4p1",
            name,
            if *b { "true" } else { "false" },
        )),
        (Shape::Int, ChartValue::Int(s)) => one(OutElement::leaf("d4p1", name, s.clone())),
        (Shape::Dec, ChartValue::Int(s)) => {
            one(OutElement::leaf("d4p1", name, super::semantic::native_float_text(t, row.name, s)?))
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
            one(super::data_items::write_native(name, items, path)?)
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
    if t == Tbl::GaugeBands {
        let mut children = Vec::new();
        for (n, v) in fields {
            if n == "useTextStr" || n == "useTooltipStr" {
                let ChartValue::Bool(value) = v else {
                    return Err(frame(format!("chart {path}: gauge flag must be boolean")));
                };
                host = host.attr(n, value.to_string());
            } else {
                children.push((n.clone(), v.clone()));
            }
        }
        for out in d_children_out(&children, t, path)? {
            host.push(out);
        }
        host.self_closing = host.children.is_empty();
        return Ok(host);
    }
    if t == Tbl::Axis {
        let value = |n: &str| fields.iter().find(|(s, _)| s == n).map(|(_, v)| v);
        let default = value("baseValue")
            .is_none_or(|v| matches!(v,ChartValue::Int(s) if s=="0" || s=="0.0"))
            && ["minValueDetectionMethod", "maxValueDetectionMethod"]
                .iter()
                .all(|n| {
                    value(n).is_none_or(|v| matches!(v,ChartValue::Enum(s) if s=="AutoDetect"))
                });
        if let Some(ChartValue::Nested(interval)) = value("interval") {
            for side in ["left", "right"] {
                let get = |suffix: &str| {
                    interval
                        .iter()
                        .find(|(n, _)| n == &format!("{side}{suffix}"))
                        .map(|(_, v)| v)
                };
                match get("IsNum") {
                    Some(ChartValue::Bool(true))
                        if !matches!(get("Date"), None | Some(ChartValue::Absent)) =>
                    {
                        return Err(frame(
                            "chart: inactive axis date requires bound typed transport".into(),
                        ));
                    }
                    Some(ChartValue::Bool(false))
                        if !get("Num").is_none_or(
                            |v| matches!(v,ChartValue::Int(n) if designer_decimal(n) == "0"),
                        ) =>
                    {
                        return Err(frame(
                            "chart: inactive axis number requires bound typed transport".into(),
                        ));
                    }
                    Some(ChartValue::Bool(false))
                        if matches!(get("Date"), None | Some(ChartValue::Absent)) =>
                    {
                        return Err(frame(
                            "chart: nullable date-mode boundary requires bound typed transport"
                                .into(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        let interval_default = value("interval").is_some_and(|v| match v {
            ChartValue::Nested(f) => f.iter().all(|(n, v)| match (n.as_str(), v) {
                ("leftIsNum" | "rightIsNum", ChartValue::Bool(b)) => *b,
                ("leftNum" | "rightNum", ChartValue::Int(n)) => designer_decimal(n) == "0",
                ("leftDate" | "rightDate", ChartValue::Absent) => true,
                _ => false,
            }),
            _ => false,
        });
        // Preserve every current bound; never treat active nonzero values as defaults.
        let default = default && interval_default;
        if !default {
            if let Some(ChartValue::Int(s)) = value("baseValue") {
                host.push(OutElement::leaf("d4p1", "baseValue", designer_decimal(s)));
            }
            let interval = value("interval").and_then(|v| match v {
                ChartValue::Nested(f) => Some(f),
                _ => None,
            });
            for (side, tag) in [("left", "minValue"), ("right", "maxValue")] {
                let get =
                    |n: &str| interval.and_then(|f| f.iter().find(|(s, _)| s == n).map(|(_, v)| v));
                let numeric = matches!(get(&format!("{side}IsNum")), Some(ChartValue::Bool(true)));
                let data = get(&format!("{side}{}", if numeric { "Num" } else { "Date" }));
                match data {
                    Some(ChartValue::Int(s)) if numeric => host.push(
                        OutElement::leaf("d4p1", tag, designer_decimal(s))
                            .attr("xsi:type", "xs:decimal"),
                    ),
                    Some(ChartValue::Str(s)) if !numeric => host.push(
                        OutElement::leaf("d4p1", tag, s.clone()).attr("xsi:type", "xs:dateTime"),
                    ),
                    None => {}
                    _ => return Err(frame(format!("chart {path}: axis boundary type mismatch"))),
                }
            }
            for n in ["minValueDetectionMethod", "maxValueDetectionMethod"] {
                if let Some(ChartValue::Enum(s)) = value(n) {
                    host.push(OutElement::leaf("d4p1", n, s.clone()));
                }
            }
        }
        if host.children.is_empty() {
            host.self_closing = true;
        }
        return Ok(host);
    }
    for out in d_children_out(fields, t, path)? {
        host.push(out);
    }
    if host.children.is_empty() {
        host.self_closing = true;
    }
    Ok(host)
}

fn d_read_axis(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    let mut fields = Vec::new();
    let mut interval = Vec::new();
    let mut seen = Vec::new();
    for c in &el.children {
        if c.prefix != "d4p1" || seen.contains(&c.local) {
            return Err(frame(format!(
                "chart {path}: unknown or duplicate axis field"
            )));
        }
        seen.push(c.local.clone());
        match c.local.as_str() {
            "baseValue" => fields.push((
                c.local.clone(),
                ChartValue::Int(designer_decimal(&leaf_text(c, path)?)),
            )),
            "minValueDetectionMethod" | "maxValueDetectionMethod" => {
                fields.push((c.local.clone(), ChartValue::Enum(leaf_text(c, path)?)))
            }
            "minValue" | "maxValue" => {
                let side = if c.local == "minValue" {
                    "left"
                } else {
                    "right"
                };
                let xt = attr_req(c, "xsi:type", path)?;
                let numeric = match xt.as_str() {
                    "xs:decimal" => true,
                    "xs:dateTime" => false,
                    _ => {
                        return Err(frame(format!(
                            "chart {path}: unsupported axis boundary type"
                        )));
                    }
                };
                let text = leaf_text(c, path)?;
                interval.push((format!("{side}IsNum"), ChartValue::Bool(numeric)));
                interval.push((
                    format!("{side}{}", if numeric { "Num" } else { "Date" }),
                    if numeric {
                        ChartValue::Int(designer_decimal(&text))
                    } else {
                        ChartValue::Str(text)
                    },
                ));
            }
            _ => return Err(frame(format!("chart {path}: unknown axis field"))),
        }
    }
    if !interval.is_empty() {
        fields.push(("interval".into(), ChartValue::Nested(interval)));
    }
    Ok(ChartValue::Nested(fields))
}

/// Designer-шрифт из канона [`FontRef`].
fn d_font_out(name: &str, f: &FontRef, path: &str) -> Result<OutElement, FormError> {
    chart_designer_font_out(name, f, path)
}

fn d_read_gauge(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    let mut fields = Vec::new();
    for n in ["useTextStr", "useTooltipStr"] {
        if let Some(a) = el.attr(n) {
            a.claimed.set(true);
            fields.push((n.into(), ChartValue::Bool(parse_bool(&a.value, path)?)));
        }
    }
    expect_attrs_claimed(el, path)?;
    fields.extend(d_read_children(&el.children, Tbl::GaugeBands, path)?);
    Ok(ChartValue::Nested(fields))
}
