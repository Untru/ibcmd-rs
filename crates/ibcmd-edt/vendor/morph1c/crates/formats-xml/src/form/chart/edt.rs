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
            )));
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
                )));
            }
        };
    root.claim();
    claim_root_ns(&root, ns_block)?;
    super::data_items::validate_value_namespaces(&root, false)?;
    if let Some(namespace) = root.attr("xmlns:common") {
        let mut pending = vec![&root];
        let mut used = false;
        while let Some(element) = pending.pop() {
            used |= element
                .resolved_type
                .borrow()
                .as_ref()
                .is_some_and(|(uri, local)| {
                    uri == "http://g5.1c.ru/v8/dt/metadata/common" && local == "ChartLineTypeValue"
                });
            pending.extend(&element.children);
        }
        if namespace.value != "http://g5.1c.ru/v8/dt/metadata/common" || !used {
            return Err(frame(
                "chart: root common namespace must bind a current ChartLineTypeValue".into(),
            ));
        }
        namespace.claimed.set(true);
    }

    let fields = e_read_children(&root.children, t, Frame::None, &format!("{kind}.chart"))?;
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(frame(format!(
            "chart sidecar: {leftover} unconsumed node(s) (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    super::semantic::from_source(
        kind,
        fields,
        morph1c_core::ir::form::ChartSourceFormat::Edt,
        &root,
    )
}

/// Прочитать EDT-детей по таблице `t` (без префикса), сохранив порядок и СВЕРИВ его с
/// модельным (иначе эхо-запись не была бы byte-exact — отказ громче тихой перестановки).
fn e_read_children(
    children: &[Element],
    t: Tbl,
    _fr: Frame,
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    let mut out: Vec<(String, ChartValue)> = Vec::new();
    let mut seen: Vec<&'static str> = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let el = &children[i];
        if !el.prefix.is_empty() {
            return Err(frame(format!(
                "chart {path}: незнакомое поле <{}:{}> (§1.0)",
                el.prefix, el.local
            )));
        }
        let row = rows(t)
            .iter()
            .find(|r| r.e_name() == el.local)
            .ok_or_else(|| {
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
        let sub_path = format!("{path}/{}", row.name);
        match row.shape {
            Shape::Colors => {
                let mut items = Vec::new();
                while i < children.len()
                    && children[i].prefix.is_empty()
                    && children[i].local == el.local
                {
                    items.push(vec![(
                        "value".into(),
                        e_read_color(&children[i], &sub_path)?,
                    )]);
                    i += 1;
                }
                out.push((row.name.to_string(), ChartValue::Items(items)));
            }
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
            Shape::SeriesItems | Shape::PointItems | Shape::Items(_) | Shape::DataItems => {
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
        Shape::DataItems => super::data_items::read_edt(host, path),
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
                )));
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
        Shape::Picture => super::picture::read(el, path, false),
        Shape::Colors => unreachable!("colors grouped by parent"),
        Shape::Value => super::data_items::read_value(el, path, false),
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
    // Actual ChartResource leaves an untyped empty Color reference NULL; xsi:nil=true
    // has the same model effect. Claim only these closed, value-free source shapes.
    if el.attr("xsi:type").is_none() && el.children.is_empty() && el.text.is_empty() {
        if let Some(nil) = el.attr("xsi:nil") {
            if nil.value != "true" {
                return Err(frame(format!("chart {path}: invalid untyped NULL Color")));
            }
            nil.claimed.set(true);
        }
        el.claim();
        expect_attrs_claimed(el, path)?;
        return Ok(ChartValue::Absent);
    }
    chart_read_edt_color(el, path)
}

/// EDT-шрифт: `core:AutoFont` (пустой) | `core:FontRef` (`<font>Style.X</font>` [+`<height>`]).
fn e_read_font(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    chart_read_edt_font(el, path)
}

/// EDT-линия: `<l><width>2</width><style>Solid</style></l>` (gap отсутствует = false).
fn e_read_line(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    chart_read_edt_line(el, path)
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
                )));
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
            )));
        }
    };
    let mode = Mode::Echo;
    let fields =
        super::semantic::emission_fields(cs, morph1c_core::ir::form::ChartSourceFormat::Edt)?;
    let mut outputs = e_children_out(&fields, t, mode, &cs.kind)?;
    super::semantic::restore_edt_primitive_layout(&mut outputs, cs)?;
    let mut root = OutElement::branch(prefix, cs.kind.as_str());
    if let Some(layout) = cs.source_layout.as_ref().filter(|l| {
        l.format == morph1c_core::ir::form::ChartSourceFormat::Edt && !l.root_namespaces.is_empty()
    }) {
        let mut seen = std::collections::HashSet::new();
        for (name, uri) in &layout.root_namespaces {
            if !seen.insert(name) {
                return Err(frame("chart: duplicate source root namespace facet".into()));
            }
            if name == "xmlns:common" {
                if uri != "http://g5.1c.ru/v8/dt/metadata/common" {
                    return Err(frame("chart: invalid source common namespace facet".into()));
                }
                if has_current_common_type(&outputs) {
                    root = root.attr(name, uri);
                    restore_common_bindings(
                        &mut outputs,
                        &layout.common_inline_paths,
                        &mut Vec::new(),
                    );
                }
            } else {
                if !ns_block
                    .iter()
                    .any(|(key, value)| name == key && uri == value)
                {
                    return Err(frame(
                        "chart: unexpected source root namespace facet".into(),
                    ));
                }
                root = root.attr(name, uri);
            }
        }
        for (name, uri) in ns_block {
            if !seen.contains(&name.to_string()) {
                root = root.attr(*name, *uri);
            }
        }
    } else {
        for (name, uri) in ns_block {
            root = root.attr(*name, *uri);
        }
    }
    for out in outputs {
        root.push(out);
    }
    Ok(render(&super::super::edt_envelope(), &root))
}

/// Источник — EDT-чтение? Детект по материализованной константе модели `translucenceMode`
/// (EDT-модель несёт её в КАЖДОМ витнессе; designer её не сериализует вовсе).
pub(super) fn is_edt_sourced(cs: &ChartSettings) -> bool {
    cs.source_layout
        .as_ref()
        .is_some_and(|layout| layout.format == morph1c_core::ir::form::ChartSourceFormat::Edt)
}

/// EDT-эмиссия детей таблицы `t`: модельный порядок (`edt_rank`), в транскоде — омиссии,
/// синтез Chart-констант и трансформация шкал/осей.
fn e_children_out(
    fields: &[(String, ChartValue)],
    t: Tbl,
    mode: Mode,
    path: &str,
) -> Result<Vec<OutElement>, FormError> {
    let mut seen = std::collections::HashSet::new();
    let mut outs = Vec::new();
    for (name, value) in fields {
        if !seen.insert(name) {
            return Err(frame(format!("chart {path}: duplicate current field")));
        }
        let row = row_by_name(t, name)
            .ok_or_else(|| frame(format!("chart {path}: unknown current field {name}")))?;
        outs.extend(e_value_outs(row, value, mode, &format!("{path}/{name}"))?);
    }
    Ok(outs)
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
    let name = row.e_name();
    let one = |el: OutElement| Ok(vec![el]);
    match (row.shape, v) {
        (Shape::Colors, ChartValue::Items(items)) => {
            let mut out = Vec::new();
            for fields in items {
                let [(key, ChartValue::Color(color))] = fields.as_slice() else {
                    return Err(frame(format!(
                        "chart {path}: Color list item must have one typed value"
                    )));
                };
                if key != "value" {
                    return Err(frame(format!("chart {path}: unknown Color item field")));
                }
                out.push(e_color_out(name, color, path)?);
            }
            Ok(out)
        }
        (Shape::Picture, value) => one(super::picture::write(name, value, path, false)?),
        (Shape::Value, value) => one(super::data_items::write_value(name, value, path, false)?),
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
        (Shape::Color, ChartValue::Absent) => {
            one(OutElement::self_closing("", name).attr("xsi:nil", "true"))
        }
        (Shape::Color, ChartValue::Color(c)) => one(e_color_out(name, c, path)?),
        (Shape::Font, ChartValue::Font(f)) => one(e_font_out(name, f, path)?),
        (Shape::Line, ChartValue::Line { style, width, gap }) => {
            one(chart_edt_line_out(name, style, width, *gap, path)?)
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
        (Shape::DataItems, ChartValue::Items(items)) => items
            .iter()
            .map(|f| super::data_items::write_edt(name, f, path))
            .collect(),
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
    chart_edt_color_out(name, canon, path)
}

/// EDT-шрифт из канона [`FontRef`].
fn e_font_out(name: &str, f: &FontRef, path: &str) -> Result<OutElement, FormError> {
    chart_edt_font_out(name, f, path)
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

fn has_current_common_type(elements: &[OutElement]) -> bool {
    elements.iter().any(|element| {
        element
            .attrs
            .iter()
            .any(|(name, value)| name == "xsi:type" && value == "common:ChartLineTypeValue")
            || has_current_common_type(&element.children)
    })
}
fn restore_common_bindings(
    elements: &mut [OutElement],
    paths: &[Vec<(String, usize)>],
    path: &mut Vec<(String, usize)>,
) {
    let mut counts = std::collections::HashMap::new();
    for element in elements {
        let index = counts.entry(element.local.clone()).or_insert(0);
        path.push((element.local.clone(), *index));
        *index += 1;
        if !paths.contains(path) {
            element.attrs.retain(|(name, value)| {
                !(name == "xmlns:common" && value == "http://g5.1c.ru/v8/dt/metadata/common")
            });
        }
        restore_common_bindings(&mut element.children, paths, path);
        path.pop();
    }
}
