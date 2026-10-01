//! CHART · EDT-диалект (сайдкар `Chart.chart` / `GanttChart.chart`): чтение + запись
//! (эхо | транскод с синтезом xcore-констант, трансформацией шкал/осей). Отдельно от
//! Designer-кодека (`designer.rs`) — два диалектных кодека НЕ сливать.

use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────────
// EDT-САЙДКАР: ЧТЕНИЕ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Фрейм-константы item-контекста (EDT `valInfo`/`key` = `core:UndefinedValue`; в IR не
/// хранятся — клеймятся на чтении и восстанавливаются на записи по модельным рангам).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    /// Не item-контекст.
    None,
    /// SeriesProperties: valInfo — ранг 1, key — ранг 10.
    Series,
    /// PointProperties: valInfo — ранг 1, key — ранг 12.
    Point,
}

impl Frame {
    /// Ранги (valInfo, key) для инъекции на записи.
    fn ranks(self) -> Option<(u16, u16)> {
        match self {
            Frame::None => None,
            Frame::Series => Some((1, 10)),
            Frame::Point => Some((1, 12)),
        }
    }
}

/// Прочитать EDT-сайдкар `Chart.chart` / `GanttChart.chart` → [`ChartSettings`].
///
/// Envelope (hexdump-сверен по витнессам): БЕЗ BOM, CRLF, 2 пробела, decl
/// `<?xml version="1.0" encoding="UTF-8"?>`, trailing EOL. Корень `chart:Chart` (ns-блок
/// xsi/chart/core) или `ganttchart:GanttChart` (xsi/core/ganttchart). Порядок полей
/// СВЕРЯЕТСЯ с модельным (`edt_rank` неубывающий) — эхо-запись byte-exact по построению.
/// §1.0: тотальность по всему дереву (unclaimed == 0).
pub fn read_chart_sidecar(bytes: &[u8]) -> Result<ChartSettings, FormError> {
    let d = parse(bytes)?;
    if d.bytes_env.bom {
        return Err(FormError::Envelope("chart sidecar: unexpected BOM".into()));
    }
    if d.bytes_env.eol != EolStyle::Crlf {
        return Err(FormError::Envelope(format!(
            "chart sidecar: must be CRLF, found {:?}",
            d.bytes_env.eol
        )));
    }
    match d.decl.as_deref() {
        Some(dd) if dd == FORM_DECL => {}
        other => {
            return Err(FormError::Envelope(format!(
                "chart sidecar: unexpected decl: {other:?}"
            )))
        }
    }
    let root = d.root;
    let (kind, t, ns_block): (&str, Tbl, &[(&str, &str)]) =
        match (root.prefix.as_str(), root.local.as_str()) {
            ("chart", "Chart") => ("Chart", Tbl::Chart, EDT_CHART_ROOT_NS),
            ("ganttchart", "GanttChart") => ("GanttChart", Tbl::Gantt, EDT_GANTT_ROOT_NS),
            (p, l) => {
                return Err(FormError::Envelope(format!(
                    "chart sidecar: unexpected root <{p}:{l}>"
                )))
            }
        };
    root.claim();
    claim_root_ns(&root, ns_block)?;
    let fields = e_read_children(&root.children, t, Frame::None, &format!("{kind}.chart"))?;
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(frame(format!(
            "chart sidecar: {leftover} unconsumed node(s) (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    Ok(ChartSettings {
        kind: kind.to_string(),
        fields,
    })
}

/// Прочитать EDT-детей по таблице `t` (без префикса), сохранив порядок и СВЕРИВ его с
/// модельным (иначе эхо-запись не была бы byte-exact — отказ громче тихой перестановки).
fn e_read_children(
    children: &[Element],
    t: Tbl,
    fr: Frame,
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    let mut out: Vec<(String, ChartValue)> = Vec::new();
    let mut seen: Vec<&'static str> = Vec::new();
    let mut last_rank: Option<u16> = None;
    let mut i = 0;
    while i < children.len() {
        let el = &children[i];
        if !el.prefix.is_empty() {
            return Err(frame(format!(
                "chart {path}: незнакомое поле <{}:{}> (§1.0)",
                el.prefix, el.local
            )));
        }
        // Фрейм-константы item-контекста — клеймим, в IR не несём.
        if fr != Frame::None && (el.local == "valInfo" || el.local == "key") {
            claim_xsi(el, "core:UndefinedValue", path)?;
            el.claim();
            expect_attrs_claimed(el, path)?;
            if !el.children.is_empty() || !el.text.is_empty() {
                return Err(frame(format!(
                    "chart {path}: <{}> — не Undefined-константа (§1.0)",
                    el.local
                )));
            }
            i += 1;
            continue;
        }
        let row = row_by_name(t, &el.local).ok_or_else(|| {
            frame(format!(
                "chart {path}: незнакомое поле <{}> (§1.0)",
                el.local
            ))
        })?;
        match row.edt {
            Edt::OmitAlways | Edt::Unwitnessed => {
                return Err(frame(format!(
                    "chart {path}: EDT-форма поля <{}> не витнесснута (§1.0)",
                    el.local
                )))
            }
            _ => {}
        }
        if let Some(lr) = last_rank {
            if row.edt_rank < lr {
                return Err(frame(format!(
                    "chart {path}: поле <{}> нарушает модельный порядок EDT (§1.0)",
                    el.local
                )));
            }
        }
        last_rank = Some(row.edt_rank);
        if seen.contains(&row.name) {
            return Err(frame(format!(
                "chart {path}: поле <{}> повторяется не подряд (§1.0)",
                el.local
            )));
        }
        seen.push(row.name);
        let sub_path = format!("{path}/{}", row.name);
        match row.shape {
            Shape::Loc => {
                let mut pairs = Vec::new();
                while i < children.len()
                    && children[i].prefix.is_empty()
                    && children[i].local == el.local
                {
                    pairs.push(e_read_loc_pair(&children[i], &sub_path)?);
                    i += 1;
                }
                out.push((row.name.to_string(), ChartValue::Localized(pairs)));
            }
            Shape::SeriesItems | Shape::PointItems | Shape::Items(_) => {
                let mut items = Vec::new();
                while i < children.len()
                    && children[i].prefix.is_empty()
                    && children[i].local == el.local
                {
                    items.push(e_read_item(&children[i], row, &sub_path)?);
                    i += 1;
                }
                out.push((row.name.to_string(), ChartValue::Items(items)));
            }
            _ => {
                out.push((row.name.to_string(), e_read_value(el, row, &sub_path)?));
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Один item повторяемого композита (серии — через `<properties>`; точки/уровни — прямые).
fn e_read_item(
    host: &Element,
    row: &Row,
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    host.claim();
    expect_attrs_claimed(host, path)?;
    match row.shape {
        Shape::SeriesItems => e_read_series_props(host, path),
        Shape::PointItems => e_read_children(&host.children, Tbl::PointItem, Frame::Point, path),
        Shape::Items(t2) => e_read_children(&host.children, t2, Frame::None, path),
        _ => unreachable!("e_read_item только для повторяемых шейпов"),
    }
}

/// `<properties>`-обёртка серии (realSeriesData/realExSeriesData).
fn e_read_series_props(host: &Element, path: &str) -> Result<Vec<(String, ChartValue)>, FormError> {
    let [props] = host.children.as_slice() else {
        return Err(frame(format!(
            "chart {path}: у серии ожидался РОВНО один <properties> (§1.0)"
        )));
    };
    if !props.prefix.is_empty() || props.local != "properties" {
        return Err(frame(format!(
            "chart {path}: незнакомый ребёнок <{}> серии (§1.0)",
            props.local
        )));
    }
    props.claim();
    expect_attrs_claimed(props, path)?;
    e_read_children(&props.children, Tbl::SeriesItem, Frame::Series, path)
}

/// Одна пара `<key>/<value>` EDT-локализации.
fn e_read_loc_pair(el: &Element, path: &str) -> Result<(Lang, String), FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut key = None;
    let mut value = None;
    for c in &el.children {
        match (c.prefix.as_str(), c.local.as_str()) {
            ("", "key") if key.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                key = Some(c.text.clone());
            }
            ("", "value") if value.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                value = Some(c.text.clone());
            }
            _ => {
                return Err(frame(format!(
                    "chart {path}: незнакомый ребёнок <{}:{}> локализации (§1.0)",
                    c.prefix, c.local
                )))
            }
        }
    }
    let key = key.ok_or_else(|| frame(format!("chart {path}: локализация без <key> (§1.0)")))?;
    let value =
        value.ok_or_else(|| frame(format!("chart {path}: локализация без <value> (§1.0)")))?;
    Ok((Lang(key), value))
}

/// Прочитать ОДНО EDT-значение по шейпу строки.
fn e_read_value(el: &Element, row: &Row, path: &str) -> Result<ChartValue, FormError> {
    match row.shape {
        Shape::Bool => Ok(ChartValue::Bool(parse_bool(&leaf_text(el, path)?, path)?)),
        Shape::Int | Shape::Dec => Ok(ChartValue::Int(leaf_text(el, path)?)),
        Shape::Str | Shape::DateTime => Ok(ChartValue::Str(leaf_text(el, path)?)),
        Shape::Enum => Ok(ChartValue::Enum(leaf_text(el, path)?)),
        Shape::Color => e_read_color(el, path),
        Shape::Font => e_read_font(el, path),
        Shape::Line => e_read_line(el, path),
        Shape::Border => e_read_border(el, path),
        Shape::Loc => unreachable!("Loc группируется в e_read_children"),
        Shape::Rect => read_rect(el, "", path),
        Shape::Nested(t2) => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(e_read_children(
                &el.children,
                t2,
                Frame::None,
                path,
            )?))
        }
        Shape::ChartTable => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(e_read_children(
                &el.children,
                Tbl::Chart,
                Frame::None,
                path,
            )?))
        }
        Shape::SeriesItem => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Nested(e_read_series_props(el, path)?))
        }
        Shape::QBands => {
            el.claim();
            expect_attrs_claimed(el, path)?;
            if !el.children.is_empty() || !el.text.is_empty() {
                return Err(frame(format!(
                    "chart {path}: непустые gaugeQualityBands не витнесснуты (§1.0)"
                )));
            }
            // Пустой элемент ⇔ оба designer-флага false (сверено по sezon-паре).
            Ok(ChartValue::Nested(vec![
                ("useTextStr".to_string(), ChartValue::Bool(false)),
                ("useTooltipStr".to_string(), ChartValue::Bool(false)),
            ]))
        }
        Shape::SeriesItems | Shape::PointItems | Shape::Items(_) => {
            unreachable!("повторяемые шейпы группируются в e_read_children")
        }
        Shape::DataItems => unreachable!("realDataItems отвергается раньше (Edt::OmitAlways)"),
    }
}

/// EDT-цвет: `core:ColorDef` (r/g/b-дети; 0-компоненты опущены) → `#RRGGBB`;
/// `core:ColorRef`+`Style.X` → `style:X`. Иные ссылки (System./web-цвета) не витнесснуты.
fn e_read_color(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| frame(format!("chart {path}: цвет без xsi:type (§1.0)")))?;
    xt.claimed.set(true);
    match xt.value.as_str() {
        "core:ColorDef" => {
            let mut rgb: [u8; 3] = [0, 0, 0];
            for c in &el.children {
                let idx = match (c.prefix.as_str(), c.local.as_str()) {
                    ("", "red") => 0,
                    ("", "green") => 1,
                    ("", "blue") => 2,
                    _ => {
                        return Err(frame(format!(
                            "chart {path}: незнакомая компонента цвета <{}> (§1.0)",
                            c.local
                        )))
                    }
                };
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                rgb[idx] = c.text.parse::<u8>().map_err(|_| {
                    frame(format!(
                        "chart {path}: компонента цвета {:?} — не байт (§1.0)",
                        c.text
                    ))
                })?;
            }
            expect_attrs_claimed(el, path)?;
            Ok(ChartValue::Color(format!(
                "#{:02X}{:02X}{:02X}",
                rgb[0], rgb[1], rgb[2]
            )))
        }
        "core:ColorRef" => {
            expect_attrs_claimed(el, path)?;
            let [c] = el.children.as_slice() else {
                return Err(frame(format!(
                    "chart {path}: ColorRef без единственного <color> (§1.0)"
                )));
            };
            if !c.prefix.is_empty() || c.local != "color" {
                return Err(frame(format!(
                    "chart {path}: незнакомый ребёнок <{}> ColorRef (§1.0)",
                    c.local
                )));
            }
            c.claim_with_text();
            expect_attrs_claimed(c, path)?;
            let name = c.text.strip_prefix("Style.").ok_or_else(|| {
                frame(format!(
                    "chart {path}: цвет-ссылка {:?} — не Style.-ссылка (§1.0)",
                    c.text
                ))
            })?;
            Ok(ChartValue::Color(format!("style:{name}")))
        }
        other => Err(frame(format!(
            "chart {path}: цвет xsi:type={other:?} не витнесснут (§1.0)"
        ))),
    }
}

/// EDT-шрифт: `core:AutoFont` (пустой) | `core:FontRef` (`<font>Style.X</font>` [+`<height>`]).
fn e_read_font(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    let xt = el
        .attr("xsi:type")
        .ok_or_else(|| frame(format!("chart {path}: шрифт без xsi:type (§1.0)")))?;
    xt.claimed.set(true);
    expect_attrs_claimed(el, path)?;
    match xt.value.as_str() {
        "core:AutoFont" => {
            if !el.children.is_empty() {
                return Err(frame(format!(
                    "chart {path}: AutoFont с детьми не витнесснут (§1.0)"
                )));
            }
            Ok(ChartValue::Font(auto_font()))
        }
        "core:FontRef" => {
            let mut font = None;
            let mut height = None;
            for c in &el.children {
                match (c.prefix.as_str(), c.local.as_str()) {
                    ("", "font") if font.is_none() => {
                        c.claim_with_text();
                        expect_attrs_claimed(c, path)?;
                        font = Some(c.text.clone());
                    }
                    ("", "height") if height.is_none() => {
                        c.claim_with_text();
                        expect_attrs_claimed(c, path)?;
                        height = Some(c.text.clone());
                    }
                    _ => {
                        return Err(frame(format!(
                            "chart {path}: незнакомый ребёнок <{}> FontRef (§1.0)",
                            c.local
                        )))
                    }
                }
            }
            let font =
                font.ok_or_else(|| frame(format!("chart {path}: FontRef без <font> (§1.0)")))?;
            if !font.starts_with("Style.") {
                return Err(frame(format!(
                    "chart {path}: шрифт-ссылка {font:?} — не Style.-ссылка (§1.0)"
                )));
            }
            Ok(ChartValue::Font(FontRef {
                auto: false,
                font_ref: Some(font),
                height,
                ..auto_font()
            }))
        }
        other => Err(frame(format!(
            "chart {path}: шрифт xsi:type={other:?} не витнесснут (§1.0)"
        ))),
    }
}

/// EDT-линия: `<l><width>2</width><style>Solid</style></l>` (gap отсутствует = false).
fn e_read_line(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    let mut width = None;
    let mut style = None;
    for c in &el.children {
        match (c.prefix.as_str(), c.local.as_str()) {
            ("", "width") if width.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                width = Some(c.text.clone());
            }
            ("", "style") if style.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                style = Some(c.text.clone());
            }
            _ => {
                return Err(frame(format!(
                    "chart {path}: незнакомый ребёнок <{}> линии (§1.0)",
                    c.local
                )))
            }
        }
    }
    Ok(ChartValue::Line {
        style: style.ok_or_else(|| frame(format!("chart {path}: линия без <style> (§1.0)")))?,
        width: width.ok_or_else(|| frame(format!("chart {path}: линия без <width> (§1.0)")))?,
        gap: false,
    })
}

/// EDT-рамка: `core:BorderDef`; style опущен ⇔ WithoutBorder, width опущен ⇔ 0.
fn e_read_border(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    claim_xsi(el, "core:BorderDef", path)?;
    expect_attrs_claimed(el, path)?;
    let mut style = None;
    let mut width = None;
    for c in &el.children {
        match (c.prefix.as_str(), c.local.as_str()) {
            ("", "style") if style.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                style = Some(c.text.clone());
            }
            ("", "width") if width.is_none() => {
                c.claim_with_text();
                expect_attrs_claimed(c, path)?;
                width = Some(c.text.clone());
            }
            _ => {
                return Err(frame(format!(
                    "chart {path}: незнакомый ребёнок <{}> рамки (§1.0)",
                    c.local
                )))
            }
        }
    }
    Ok(ChartValue::Border {
        style: style.unwrap_or_else(|| "WithoutBorder".to_string()),
        width: width.unwrap_or_else(|| "0".to_string()),
    })
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// EDT-САЙДКАР: ЗАПИСЬ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Режим EDT-записи (см. модульный docstring).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Источник — EDT-чтение: чистое эхо (без синтеза/трансформаций/омиссий).
    Echo,
    /// Источник — Designer-чтение: xcore-омиссии + синтез EDT-only констант + трансформация
    /// шкал/осей.
    Transcode,
}

/// Записать [`ChartSettings`] в байты EDT-сайдкара.
///
/// * EDT-источник (несёт `translucenceMode`) — эхо: byte-exact к прочитанному файлу.
/// * Designer-источник — транскод в модельном порядке с омиссиями/синтезом; сверен
///   поэлементно со всеми 9 живыми сайдкарами витнесс-набора.
pub fn write_chart_sidecar(cs: &ChartSettings) -> Result<Vec<u8>, FormError> {
    let (t, prefix, ns_block): (Tbl, &str, &[(&str, &str)]) = match cs.kind.as_str() {
        "Chart" => (Tbl::Chart, "chart", EDT_CHART_ROOT_NS),
        "GanttChart" => (Tbl::Gantt, "ganttchart", EDT_GANTT_ROOT_NS),
        other => {
            return Err(frame(format!(
                "chart: незнакомый вид настроек диаграммы {other:?} (§1.0)"
            )))
        }
    };
    let mode = if is_edt_sourced(cs) {
        Mode::Echo
    } else {
        Mode::Transcode
    };
    let mut root = OutElement::branch(prefix, cs.kind.as_str());
    for (name, uri) in ns_block {
        root = root.attr(*name, *uri);
    }
    for out in e_children_out(&cs.fields, t, mode, &cs.kind)? {
        root.push(out);
    }
    Ok(render(&super::super::edt_envelope(), &root))
}

/// Источник — EDT-чтение? Детект по материализованной константе модели `translucenceMode`
/// (EDT-модель несёт её в КАЖДОМ витнессе; designer её не сериализует вовсе).
pub(super) fn is_edt_sourced(cs: &ChartSettings) -> bool {
    let chart_fields: &[(String, ChartValue)] = if cs.kind == "GanttChart" {
        match cs.fields.iter().find(|(n, _)| n == "chart") {
            Some((_, ChartValue::Nested(f))) => f,
            _ => return false,
        }
    } else {
        &cs.fields
    };
    chart_fields.iter().any(|(n, _)| n == "translucenceMode")
}

/// EDT-эмиссия детей таблицы `t`: модельный порядок (`edt_rank`), в транскоде — омиссии,
/// синтез Chart-констант и трансформация шкал/осей.
fn e_children_out(
    fields: &[(String, ChartValue)],
    t: Tbl,
    mode: Mode,
    path: &str,
) -> Result<Vec<OutElement>, FormError> {
    // §1.0: каждое имя источника известно таблице и уникально.
    let mut names: Vec<&str> = Vec::new();
    for (name, _) in fields {
        if row_by_name(t, name).is_none() {
            return Err(frame(format!(
                "chart {path}: незнакомое поле {name:?} (§1.0)"
            )));
        }
        if names.contains(&name.as_str()) {
            return Err(frame(format!("chart {path}: дубль поля {name:?} (§1.0)")));
        }
        names.push(name.as_str());
    }
    let mut ranked: Vec<(u16, OutElement)> = Vec::new();
    for row in rows(t) {
        let stored = fields.iter().find(|(n, _)| n == row.name).map(|(_, v)| v);
        let value: Option<ChartValue> = match stored {
            Some(v) => Some(v.clone()),
            None if mode == Mode::Transcode && t == Tbl::Chart => chart_synth(row.name),
            None => None,
        };
        let Some(mut value) = value else { continue };
        if mode == Mode::Transcode {
            value = e_transform(row, value, path)?;
            if edt_omitted(row, &value) {
                continue;
            }
        }
        // Гейт невитнесснутых EDT-форм (в эхо-режиме недостижим — чтение их отвергло).
        match row.edt {
            Edt::OmitAlways => continue,
            Edt::Unwitnessed => {
                if std_omitted(&value) {
                    continue;
                }
                return Err(frame(format!(
                    "chart {path}: EDT-форма поля {:?} со значением {value:?} не витнесснута (§1.0)",
                    row.name
                )));
            }
            _ => {}
        }
        for out in e_value_outs(row, &value, mode, &format!("{path}/{}", row.name))? {
            ranked.push((row.edt_rank, out));
        }
    }
    ranked.sort_by_key(|(r, _)| *r);
    Ok(ranked.into_iter().map(|(_, o)| o).collect())
}

/// Синтез EDT-only материализованных констант Chart-модели (транскод; значения — снятые
/// константы всех 9 витнессов; шкалы/оси синтезируются пустыми — их наполняет [`e_transform`]).
///
/// Та же таблица служит ВИТНЕСС-ЭТАЛОНОМ обратного направления (`densify.rs`, правило 2):
/// EDT-only поле сверяется с этой константой и снимается.
pub(super) fn chart_synth(name: &str) -> Option<ChartValue> {
    Some(match name {
        "translucenceMode" => ChartValue::Enum("Auto".to_string()),
        "bubbleSizeValueSource" => ChartValue::Enum("NextSeries".to_string()),
        "bubbleSizeCommonSeries" => ChartValue::Int("-2".to_string()),
        "bubbleSizing" => ChartValue::Enum("IncreaseArea".to_string()),
        "colorPaletteDescription" | "referenceBandsColorPaletteDescription" => {
            ChartValue::Nested(vec![(
                "colorPalette".to_string(),
                ChartValue::Enum("Auto".to_string()),
            )])
        }
        "valuesReferenceLines"
        | "pointsReferenceLines"
        | "valuesReferenceBands"
        | "pointsReferenceBands" => ChartValue::Nested(Vec::new()),
        "valuesAxis" | "pointsAxis" | "additionalValuesAxis" => ChartValue::Nested(Vec::new()),
        "pointsScale" | "valuesScale" | "seriesScale" | "additionalValuesScale" => {
            ChartValue::Nested(Vec::new())
        }
        _ => return None,
    })
}

/// Транскод-трансформации designer-значения в EDT-модельную форму: ось (пустая designer-ось →
/// материализованный interval) и шкала (материализация titleArea/titlePlacement/labelOrientation).
fn e_transform(row: &Row, value: ChartValue, path: &str) -> Result<ChartValue, FormError> {
    match row.shape {
        Shape::Nested(Tbl::Axis) => {
            let fields = expect_nested(&value, row.name, path)?;
            if fields.is_empty() {
                // Витнесс-константа всех осей корпуса: {leftIsNum=true, rightIsNum=true}.
                Ok(ChartValue::Nested(vec![(
                    "interval".to_string(),
                    ChartValue::Nested(vec![
                        ("leftIsNum".to_string(), ChartValue::Bool(true)),
                        ("rightIsNum".to_string(), ChartValue::Bool(true)),
                    ]),
                )]))
            } else {
                Ok(value)
            }
        }
        Shape::Nested(Tbl::Scale) => {
            let src = expect_nested(&value, row.name, path)?;
            Ok(ChartValue::Nested(edt_scale_fields(src)))
        }
        _ => Ok(value),
    }
}

/// Материализация EDT-шкалы из designer-полей: titleArea дополняется константами
/// location/transparent/marker/orientation, titlePlacement дефолтится в SpecialArea,
/// labelOrientation — в Auto (сверено по 6 scale-парам + 3 синтезированным дефолтам).
fn edt_scale_fields(src: &[(String, ChartValue)]) -> Vec<(String, ChartValue)> {
    let mut out: Vec<(String, ChartValue)> = Vec::new();
    let mut title_area: Option<&[(String, ChartValue)]> = None;
    let mut title_placement: Option<ChartValue> = None;
    let mut label_orientation: Option<ChartValue> = None;
    for (n, v) in src {
        match n.as_str() {
            "titleArea" => {
                if let ChartValue::Nested(f) = v {
                    title_area = Some(f.as_slice());
                }
            }
            "titlePlacement" => title_placement = Some(v.clone()),
            "labelOrientation" => label_orientation = Some(v.clone()),
            _ => out.push((n.clone(), v.clone())),
        }
    }
    out.push(("titleArea".to_string(), edt_title_area(title_area)));
    out.push((
        "titlePlacement".to_string(),
        title_placement.unwrap_or_else(|| ChartValue::Enum("SpecialArea".to_string())),
    ));
    out.push((
        "labelOrientation".to_string(),
        label_orientation.unwrap_or_else(|| ChartValue::Enum("Auto".to_string())),
    ));
    // Порядок здесь не важен: генеральный писатель сортирует по edt_rank.
    out
}

/// Материализация titleArea: designer-поля (font/цвета/border) + EDT-константы
/// location=Auto / transparent=true / marker=Auto / orientation=Auto; border при отсутствии
/// дефолтится в {WithoutBorder, 1} (витнесс дефолт-шкал: `<border …><width>1</width>`).
fn edt_title_area(src: Option<&[(String, ChartValue)]>) -> ChartValue {
    let mut f: Vec<(String, ChartValue)> =
        vec![("location".to_string(), ChartValue::Enum("Auto".to_string()))];
    let mut has_border = false;
    if let Some(src) = src {
        for (n, v) in src {
            has_border |= n == "border";
            f.push((n.clone(), v.clone()));
        }
    }
    if !has_border {
        f.push((
            "border".to_string(),
            ChartValue::Border {
                style: "WithoutBorder".to_string(),
                width: "1".to_string(),
            },
        ));
    }
    f.push(("transparent".to_string(), ChartValue::Bool(true)));
    f.push(("marker".to_string(), ChartValue::Enum("Auto".to_string())));
    f.push((
        "orientation".to_string(),
        ChartValue::Enum("Auto".to_string()),
    ));
    ChartValue::Nested(f)
}

/// EDT-эмиссия ОДНОГО поля (повторяемые шейпы дают несколько элементов подряд).
fn e_value_outs(
    row: &Row,
    v: &ChartValue,
    mode: Mode,
    path: &str,
) -> Result<Vec<OutElement>, FormError> {
    let name = row.name;
    let one = |el: OutElement| Ok(vec![el]);
    match (row.shape, v) {
        (Shape::Bool, ChartValue::Bool(b)) => one(OutElement::leaf(
            "",
            name,
            if *b { "true" } else { "false" },
        )),
        (Shape::Int, ChartValue::Int(s)) => one(OutElement::leaf("", name, s.clone())),
        (Shape::Dec, ChartValue::Int(s)) => one(OutElement::leaf("", name, edt_decimal(s))),
        (Shape::Str | Shape::DateTime, ChartValue::Str(s)) => {
            one(OutElement::leaf("", name, s.clone()))
        }
        (Shape::Enum, ChartValue::Enum(s)) => one(OutElement::leaf("", name, s.clone())),
        (Shape::Color, ChartValue::Color(c)) => one(e_color_out(name, c, path)?),
        (Shape::Font, ChartValue::Font(f)) => one(e_font_out(name, f, path)?),
        (Shape::Line, ChartValue::Line { style, width, gap }) => {
            if *gap {
                return Err(frame(format!(
                    "chart {path}: линия с gap=true в EDT не витнесснута (§1.0)"
                )));
            }
            let mut l = OutElement::branch("", name);
            l.push(OutElement::leaf("", "width", width.clone()));
            l.push(OutElement::leaf("", "style", style.clone()));
            one(l)
        }
        (Shape::Border, ChartValue::Border { style, width }) => {
            let mut b = OutElement::self_closing("", name).attr("xsi:type", "core:BorderDef");
            if style != "WithoutBorder" {
                b.self_closing = false;
                b.push(OutElement::leaf("", "style", style.clone()));
            }
            if width != "0" {
                b.self_closing = false;
                b.push(OutElement::leaf("", "width", width.clone()));
            }
            one(b)
        }
        (Shape::Loc, ChartValue::Localized(pairs)) => {
            let mut outs = Vec::new();
            for (lang, content) in pairs {
                let mut host = OutElement::branch("", name);
                host.push(OutElement::leaf("", "key", lang.as_str()));
                host.push(OutElement::leaf("", "value", content.clone()));
                outs.push(host);
            }
            Ok(outs)
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
            let mut r = OutElement::branch("", name);
            r.push(OutElement::leaf("", "left", left.clone()));
            r.push(OutElement::leaf("", "right", right.clone()));
            r.push(OutElement::leaf("", "top", top.clone()));
            r.push(OutElement::leaf("", "bottom", bottom.clone()));
            one(r)
        }
        (Shape::Nested(t2), ChartValue::Nested(f)) => {
            one(e_composite_out(name, f, t2, mode, path)?)
        }
        (Shape::ChartTable, ChartValue::Nested(f)) => {
            one(e_composite_out(name, f, Tbl::Chart, mode, path)?)
        }
        (Shape::SeriesItem, ChartValue::Nested(f)) => one(e_item_out(
            name,
            f,
            Tbl::SeriesItem,
            Frame::Series,
            mode,
            path,
        )?),
        (Shape::SeriesItems | Shape::PointItems | Shape::Items(_), ChartValue::Items(items)) => {
            let (sub, fr) = match row.shape {
                Shape::SeriesItems => (Tbl::SeriesItem, Frame::Series),
                Shape::PointItems => (Tbl::PointItem, Frame::Point),
                Shape::Items(t2) => (t2, Frame::None),
                _ => unreachable!(),
            };
            let mut outs = Vec::new();
            for item in items {
                outs.push(e_item_out(name, item, sub, fr, mode, path)?);
            }
            Ok(outs)
        }
        (Shape::QBands, ChartValue::Nested(f)) => {
            for (n, fv) in f {
                if !matches!(fv, ChartValue::Bool(false)) {
                    return Err(frame(format!(
                        "chart {path}: gaugeQualityBands.{n}={fv:?} — EDT-форма не витнесснута (§1.0)"
                    )));
                }
            }
            one(OutElement::self_closing("", name))
        }
        (Shape::DataItems, _) => unreachable!("realDataItems отфильтрован (Edt::OmitAlways)"),
        (shape, other) => Err(frame(format!(
            "chart {path}: значение {other:?} не соответствует шейпу {shape:?} (§1.0)"
        ))),
    }
}

/// EDT-композит без фрейма: `<name>` + дети в модельном порядке; пустой ⇒ самозакрытый.
fn e_composite_out(
    name: &str,
    fields: &[(String, ChartValue)],
    t: Tbl,
    mode: Mode,
    path: &str,
) -> Result<OutElement, FormError> {
    let mut host = OutElement::branch("", name);
    for out in e_children_out(fields, t, mode, path)? {
        host.push(out);
    }
    if host.children.is_empty() {
        host.self_closing = true;
    }
    Ok(host)
}

/// EDT-item (серия/точка/уровень): фрейм-константы `valInfo`/`key` инъектируются по модельным
/// рангам; серии — под обёрткой `<properties>`.
fn e_item_out(
    name: &str,
    fields: &[(String, ChartValue)],
    t: Tbl,
    fr: Frame,
    mode: Mode,
    path: &str,
) -> Result<OutElement, FormError> {
    // Ранжированная сборка (эхо-порядок = модельный: сверен на чтении).
    let mut ranked: Vec<(u16, OutElement)> = Vec::new();
    if let Some((vi_rank, key_rank)) = fr.ranks() {
        ranked.push((
            vi_rank,
            OutElement::self_closing("", "valInfo").attr("xsi:type", "core:UndefinedValue"),
        ));
        ranked.push((
            key_rank,
            OutElement::self_closing("", "key").attr("xsi:type", "core:UndefinedValue"),
        ));
    }
    let mut names: Vec<&str> = Vec::new();
    for (n, _) in fields {
        if names.contains(&n.as_str()) {
            return Err(frame(format!("chart {path}: дубль поля {n:?} (§1.0)")));
        }
        names.push(n.as_str());
    }
    for row in rows(t) {
        let Some((_, v)) = fields.iter().find(|(n, _)| n == row.name) else {
            continue;
        };
        if mode == Mode::Transcode && edt_omitted(row, v) {
            continue;
        }
        match row.edt {
            Edt::OmitAlways => continue,
            Edt::Unwitnessed => {
                if std_omitted(v) {
                    continue;
                }
                return Err(frame(format!(
                    "chart {path}: EDT-форма поля {:?} со значением {v:?} не витнесснута (§1.0)",
                    row.name
                )));
            }
            _ => {}
        }
        for out in e_value_outs(row, v, mode, &format!("{path}/{}", row.name))? {
            ranked.push((row.edt_rank, out));
        }
    }
    // §1.0-стража таблицы: незнакомое имя item'а.
    for (n, _) in fields {
        if row_by_name(t, n).is_none() {
            return Err(frame(format!("chart {path}: незнакомое поле {n:?} (§1.0)")));
        }
    }
    ranked.sort_by_key(|(r, _)| *r);
    let mut inner: Vec<OutElement> = ranked.into_iter().map(|(_, o)| o).collect();
    let mut host = OutElement::branch("", name);
    match fr {
        Frame::Series => {
            let mut props = OutElement::branch("", "properties");
            for o in inner.drain(..) {
                props.push(o);
            }
            host.push(props);
        }
        _ => {
            for o in inner.drain(..) {
                host.push(o);
            }
            if host.children.is_empty() {
                host.self_closing = true;
            }
        }
    }
    Ok(host)
}

/// EDT-цвет из канона.
fn e_color_out(name: &str, canon: &str, path: &str) -> Result<OutElement, FormError> {
    if canon == "auto" {
        // «auto» кодируется ОТСУТСТВИЕМ поля — сюда попадает лишь эхо порченого файла.
        return Err(frame(format!(
            "chart {path}: цвет auto не имеет EDT-формы (кодируется омиссией) (§1.0)"
        )));
    }
    if let Some(style) = canon.strip_prefix("style:") {
        let mut el = OutElement::branch("", name).attr("xsi:type", "core:ColorRef");
        el.push(OutElement::leaf("", "color", format!("Style.{style}")));
        return Ok(el);
    }
    let (r, g, b) = parse_hex_color(canon, path)?;
    let mut el = OutElement::self_closing("", name).attr("xsi:type", "core:ColorDef");
    for (comp, val) in [("red", r), ("green", g), ("blue", b)] {
        if val != 0 {
            el.self_closing = false;
            el.push(OutElement::leaf("", comp, val.to_string()));
        }
    }
    Ok(el)
}

/// EDT-шрифт из канона [`FontRef`].
fn e_font_out(name: &str, f: &FontRef, path: &str) -> Result<OutElement, FormError> {
    if f.face_name.is_some()
        || f.bold.is_some()
        || f.italic.is_some()
        || f.underline.is_some()
        || f.strikeout.is_some()
        || f.scale.is_some()
    {
        return Err(frame(format!(
            "chart {path}: шрифт с переопределениями (не AutoFont/StyleItem) не витнесснут (§1.0)"
        )));
    }
    match &f.font_ref {
        None => Ok(OutElement::self_closing("", name).attr("xsi:type", "core:AutoFont")),
        Some(r) => {
            if !r.starts_with("Style.") {
                return Err(frame(format!(
                    "chart {path}: шрифт-ссылка {r:?} — не Style.-ссылка (§1.0)"
                )));
            }
            let mut el = OutElement::branch("", name).attr("xsi:type", "core:FontRef");
            el.push(OutElement::leaf("", "font", r.clone()));
            if let Some(h) = &f.height {
                el.push(OutElement::leaf("", "height", edt_decimal(h)));
            }
            Ok(el)
        }
    }
}

/// Вытащить поля из [`ChartValue::Nested`], иначе — отказ с именем поля.
pub(super) fn expect_nested<'v>(
    v: &'v ChartValue,
    name: &str,
    path: &str,
) -> Result<&'v [(String, ChartValue)], FormError> {
    match v {
        ChartValue::Nested(f) => Ok(f),
        other => Err(frame(format!(
            "chart {path}: поле {name:?} несёт {other:?} вместо композита (§1.0)"
        ))),
    }
}
