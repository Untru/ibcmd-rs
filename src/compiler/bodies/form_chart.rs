//! The embedded chart a `Chart`- or `GanttChart`-typed form attribute stores.
//!
//! A form attribute whose `<Settings xsi:type="d4p1:Chart">` (or
//! `d4p1:GanttChart`) carries a design stores it in member 14 of its record
//! (`NativeFormAttribute.trailing[0]`) as the chart's own serialization:
//!
//! ```text
//! {0,1,"Chart",{"#",3543ef08-…,{11},{74,<data>}}}
//! {0,1,"GanttChart",{"#",3a6e63bf-…,{19,{0,{11},{74,<data>}},<Gantt members>}}}
//! ```
//!
//! This module is the exact inverse of the exporter's decoder
//! (`parse_form_chart_settings_xml`, `format_form_chart_settings_body_xml` and
//! `parse_form_gantt_chart_settings_xml` in `mssql_dump::form_body`): every
//! member that decoder publishes is read from the element it publishes, every
//! member it validates against a literal is written as that literal, and the
//! XML is consumed in the order the decoder writes it, so an element the
//! decoder would not have written -- or would have written elsewhere -- is
//! refused rather than guessed. `findings/rt-embedded.md` §1.2.
//!
//! Members no XML carries (the decoder reads none of them) are written as an
//! XML load writes them. The platform-loaded seeds under
//! `tests/fixtures/native-evidence/8.3.27.2214/form-chart-*` are that
//! evidence where they have it; the ERP УХ corpus (27 attributes, all saved by
//! Designer) is the rest:
//!
//! - the record version is `74` and the scale-id list holds the ex-series id
//!   and every real series id (`1+N`), a repeated id once;
//! - a border's member 5 is `0`, and its style uuid is the palette uuid,
//!   except a form's chart border (`chBorder`) that is not `WithoutBorder`,
//!   which stores the nil uuid (10 of the 14 ERP УХ Gantt charts that have
//!   one); a spreadsheet template's stores the palette uuid there as well
//!   (all 5 template charts of both corpora that have one, the two БСП rows
//!   an XML load wrote among them);
//! - `userMaxValue` and `userMinValue` are doubles: a template stores them in
//!   the platform's double spelling (`2e3`, `5e2`, `3e2` -- every non-zero
//!   one of ERP УХ, one of them in a row an XML load wrote), a form the XML's
//!   own (its decoder publishes the stored text and refuses an exponent);
//! - member 100 repeats the palette code of member 179;
//! - a series' cached colour and marker are its legend entry's when that
//!   entry names one, else the automatic palette's for the chart type;
//! - the two twelve-member layout runs (members 82..93 and 148..159) hold the
//!   untouched layout, `0,0,0,0,0,0,1,1,0,0,1,1` -- what the zero-series load
//!   and 29 of the 46 records of both corpora store. The rest is a design-time
//!   legend layout the platform computes on its own.
//!
//! The round trip is indifferent to all of them, and every value this writer
//! returns is checked by running it back through the exporter's own decoder:
//! a chart that does not export back to the XML it was built from is refused.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail, ensure};
use quick_xml::Reader;
use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesStart, Event};

use super::form_native::{format_native_color, format_native_font};
use crate::form_schema::FormControlBorderStyle;

/// The namespace the `d4p1:` prefix of a chart's `<Settings>` names.
const CHART_NAMESPACE: &str = "http://v8.1c.ru/8.2/data/chart";
/// The value type ids a chart and a Gantt chart store ahead of their record.
const CHART_VALUE_TYPE_UUID: &str = "3543ef08-3316-4f7e-9447-0cd0a1cbf1d5";
const GANTT_CHART_VALUE_TYPE_UUID: &str = "3a6e63bf-16aa-42eb-b48c-2fff9670ad2f";
/// The style uuid a border record carries, and the nil uuid a non-empty chart
/// border stores instead.
const BORDER_UUID: &str = "48312c09-257f-4b29-b280-284dd89efc1e";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
/// The style uuid every chart line record carries.
const LINE_UUID: &str = "e5cabe59-d992-4d31-8086-3116931aff81";
/// `auto`, and a font that names nothing.
const AUTO_COLOR: &str = "{3,4,{0}}";
const AUTO_FONT: &str = "{7,3,0,1,100}";
/// The bounds the exporters' decoders read under: the form exporter stops at
/// 64 series and refuses a longer chart through the round-trip check, the
/// template exporter reads ERP УХ's 75-series `ФинансовыйАнализ` charts.
const MAX_SERIES: usize = 1024;
const MAX_POINTS: usize = 4096;
/// The untouched layout both twelve-member layout runs hold.
const UNTOUCHED_LAYOUT: [&str; 12] = ["0", "0", "0", "0", "0", "0", "1", "1", "0", "0", "1", "1"];

/// The row a chart value is written into. A spreadsheet template's chart
/// drawing stores the same serialization as a form attribute, except for the
/// members the module docs name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChartHost {
    FormAttribute,
    SpreadsheetTemplate,
}

/// `<Settings xsi:type="d4p1:Chart">`, the whole element as Form.xml spells
/// it, to the member-14 value `{0,1,"Chart",{…}}`.
pub(crate) fn format_form_embedded_chart(settings_xml: &str) -> Result<String> {
    let value = chart_value(settings_xml, ChartHost::FormAttribute)?;
    verify_round_trip(
        settings_xml,
        &value,
        crate::mssql_dump::render_form_chart_settings_value,
    )?;
    Ok(value)
}

/// `<Settings xsi:type="d4p1:GanttChart">` to `{0,1,"GanttChart",{…}}`.
pub(crate) fn format_form_embedded_gantt_chart(settings_xml: &str) -> Result<String> {
    let value = gantt_chart_value(settings_xml, ChartHost::FormAttribute)?;
    verify_round_trip(
        settings_xml,
        &value,
        crate::mssql_dump::render_form_gantt_chart_settings_value,
    )?;
    Ok(value)
}

/// The chart value `format_form_embedded_chart` returns, before the form
/// exporter's check: a spreadsheet template carries the same serialization
/// and checks it against its own exporter instead.
pub(crate) fn chart_value(settings_xml: &str, host: ChartHost) -> Result<String> {
    let root = parse_settings(settings_xml, "d4p1:Chart")?;
    let record = chart_record(&root, host)?;
    Ok(format!(
        "{{0,1,\"Chart\",{{\"#\",{CHART_VALUE_TYPE_UUID},{{11}},{record}}}}}"
    ))
}

/// The Gantt chart's counterpart to `chart_value`.
pub(crate) fn gantt_chart_value(settings_xml: &str, host: ChartHost) -> Result<String> {
    let root = parse_settings(settings_xml, "d4p1:GanttChart")?;
    let wrapper = gantt_wrapper(&root, host)?;
    Ok(format!(
        "{{0,1,\"GanttChart\",{{\"#\",{GANTT_CHART_VALUE_TYPE_UUID},{wrapper}}}}}"
    ))
}

/// The exporter renders the value at the indent `<Settings>` sits at inside
/// `<Attribute>`; the source is the element as the file spells it. Both are
/// compared line by line with the indentation set aside, so the check does
/// not depend on where the caller cut the element out.
fn verify_round_trip(
    source: &str,
    value: &str,
    render: fn(&str) -> Option<String>,
) -> Result<()> {
    let rendered = render(value)
        .ok_or_else(|| anyhow!("the exporter cannot read back the chart the writer built"))?;
    let expected = significant_lines(source);
    let actual = significant_lines(&rendered);
    if expected != actual {
        let at = expected
            .iter()
            .zip(&actual)
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| expected.len().min(actual.len()));
        bail!(
            "the chart does not export back to its own XML: line {} would read {:?} where the source reads {:?}",
            at + 1,
            actual.get(at).copied().unwrap_or("<end>"),
            expected.get(at).copied().unwrap_or("<end>"),
        );
    }
    Ok(())
}

fn significant_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|line| line.trim_start_matches([' ', '\t']))
        .filter(|line| !line.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// The chart record, `{74,…}`.
// ---------------------------------------------------------------------------

/// One `realSeriesData` or the `realExSeriesData` placeholder.
struct Series {
    id: String,
    color: String,
    line: String,
    marker: &'static str,
    text: String,
    str_is_changed: &'static str,
    is_expand: &'static str,
    is_indicator: &'static str,
}

/// One `realPointData`.
struct Point {
    id: String,
    color: String,
    line: String,
    marker: &'static str,
    text: String,
    str_is_changed: &'static str,
    is_expand: &'static str,
    is_indicator: &'static str,
    color_priority: &'static str,
}

const MARKERS: &[(&str, &str)] = &[
    ("None", "0"),
    ("Rect", "1"),
    ("Circle", "2"),
    ("Rhomb", "3"),
    ("Auto", "4"),
];

const CHART_TYPES: &[(&str, &str)] = &[
    ("Line", "0"),
    ("StackedColumn", "5"),
    ("Column3D", "6"),
    ("StackedBar", "9"),
    ("Pie", "12"),
    ("Gauge", "38"),
    ("Bubble", "44"),
];

/// `{74,seriesCurId,pointsCurId,isSeriesDesign,N,<series 11×N>,<ex-series
/// 11>,isPointsDesign,P,<points 11×P>,<tail>}`, read from the chart's own
/// elements (a `Chart` attribute's `<Settings>`, a Gantt chart's
/// `<d4p1:chart>`).
#[allow(clippy::too_many_lines)]
fn chart_record(node: &XmlNode, host: ChartHost) -> Result<String> {
    let mut c = Children::chart(node)?;
    let series_cur_id = integer(c.required("seriesCurId")?)?;
    let points_cur_id = integer(c.required("pointsCurId")?)?;
    let is_series_design = boolean(c.required("isSeriesDesign")?)?;
    let series_count = count(c.required("realSeriesCount")?)?;
    ensure!(
        series_count <= MAX_SERIES,
        "the chart declares {series_count} series, more than the exporter reads"
    );
    let mut series = Vec::new();
    while let Some(item) = c.optional("realSeriesData") {
        series.push(series_of(item)?);
    }
    ensure!(
        series.len() == series_count,
        "the chart declares {series_count} series and spells {}",
        series.len()
    );
    let ex_series = series_of(c.required("realExSeriesData")?)?;
    let is_points_design = boolean(c.required("isPointsDesign")?)?;
    let point_count = count(c.required("realPointCount")?)?;
    ensure!(
        point_count <= MAX_POINTS,
        "the chart declares {point_count} points, more than the exporter reads"
    );
    let mut points = Vec::new();
    while let Some(item) = c.optional("realPointData") {
        points.push(point_of(item)?);
    }
    ensure!(
        points.len() == point_count,
        "the chart declares {point_count} points and spells {}",
        points.len()
    );

    // Tail members `t[0..100)`, then the rest, keyed by the fixed offset the
    // decoder names them by (`tidx`); `fixed[i]` is that offset's value.
    let mut fixed: Vec<Option<String>> = vec![None; 197];
    let mut set = |slot: usize, value: String| fixed[slot] = Some(value);

    set(0, integer(c.required("curSeries")?)?);
    set(1, integer(c.required("curPoint")?)?);
    let chart_type = code(c.required("chartType")?, CHART_TYPES)?;
    set(2, chart_type.to_string());
    set(
        3,
        code(
            c.required("circleLabelType")?,
            &[("None", "0"), ("Series", "1"), ("ValuePercent", "7")],
        )?
        .to_string(),
    );
    set(4, string(c.required("labelsDelimiter")?)?);
    set(
        5,
        code(
            c.required("labelsLocation")?,
            &[("Edge", "0"), ("EdgeAuto", "3"), ("Auto", "4")],
        )?
        .to_string(),
    );
    set(6, localized(c.required("lbFormat")?)?);
    set(7, localized(c.required("lbpFormat")?)?);
    set(8, color(c.required("labelsColor")?)?);
    // The labels font is published as a literal: members 9 and 10 read `0`
    // and member 102 holds the empty font on every record.
    ensure!(
        font(c.required("labelsFont")?)? == AUTO_FONT,
        "<d4p1:labelsFont> names a font the chart writer has not measured"
    );
    set(9, "0".into());
    set(10, "0".into());
    exact(c.required("transparentLabelsBkg")?, "true")?;
    set(104, color(c.required("labelsBkgColor")?)?);
    set(105, border(c.required("labelsBorder")?, false)?);
    set(106, color(c.required("labelsBorderColor")?)?);
    exact(c.required("circleExpandMode")?, "None")?;
    exact(c.required("chart3Dcrd")?, "SouthWest")?;
    set(11, localized(c.required("title")?)?);
    set(12, boolean(c.required("isShowTitle")?)?.into());
    set(13, boolean(c.required("isShowLegend")?)?.into());
    set(14, border(c.required("ttlBorder")?, false)?);
    set(15, color(c.required("ttlBorderColor")?)?);
    set(16, border(c.required("lgBorder")?, false)?);
    set(17, color(c.required("lgBorderColor")?)?);
    set(
        18,
        border(c.required("chBorder")?, host == ChartHost::FormAttribute)?,
    );
    set(19, color(c.required("chBorderColor")?)?);
    set(20, boolean(c.required("transparent")?)?.into());
    set(21, color(c.required("bkgColor")?)?);
    set(22, boolean(c.required("isTrnspTtl")?)?.into());
    set(23, color(c.required("ttlColor")?)?);
    set(24, boolean(c.required("isTrnspLeg")?)?.into());
    set(25, color(c.required("legColor")?)?);
    set(26, boolean(c.required("isTrnspCh")?)?.into());
    set(27, color(c.required("chColor")?)?);
    set(28, color(c.required("ttlTxtColor")?)?);
    set(29, color(c.required("legTxtColor")?)?);
    set(30, color(c.required("chTxtColor")?)?);
    set(31, font(c.required("ttlFont")?)?);
    set(32, font(c.required("legFont")?)?);
    set(33, font(c.required("chFont")?)?);
    for (slot, name) in [
        (34, "isShowScale"),
        (35, "isShowScaleVL"),
        (36, "isShowSeriesScale"),
        (37, "isShowPointsScale"),
        (38, "isShowValuesScale"),
    ] {
        set(slot, boolean(c.required(name)?)?.into());
    }
    set(39, localized(c.required("vsFormat")?)?);
    // `xLabelsOrientation` is `post[1]`'s code (`Horizontal` 0, `Vertical` 1,
    // `Auto` 2), which the points scale's own orientation mirrors; `tail[40]`
    // reads 1 on the one `Vertical` record of the stand and 0 on every other.
    let (orientation_40, orientation_code) =
        match leaf(c.required("xLabelsOrientation")?)?.trim() {
            "Auto" => ("0", "2"),
            "Horizontal" => ("0", "0"),
            "Vertical" => ("1", "1"),
            other => bail!(
                "<d4p1:xLabelsOrientation> spells {other}, which the chart writer has not measured"
            ),
        };
    set(40, orientation_40.into());
    set(41, line(c.required("scaleLine")?)?);
    set(42, color(c.required("scaleColor")?)?);
    set(43, boolean(c.required("isAutoSeriesName")?)?.into());
    set(44, boolean(c.required("isAutoPointName")?)?.into());
    set(45, code(c.required("maxMode")?, &[("NotDefined", "0")])?.into());
    set(46, integer(c.required("maxSeries")?)?);
    set(47, integer(c.required("maxSeriesPrc")?)?);
    set(48, code(c.required("spaceMode")?, &[("Half", "1")])?.into());
    set(49, integer(c.required("baseVal")?)?);
    set(50, boolean(c.required("isOutline")?)?.into());
    set(51, integer(c.required("realPiePoint")?)?);
    set(52, integer(c.required("realStockSeries")?)?);
    for (slot, name) in [
        (53, "isLight"),
        (54, "isGradient"),
        (55, "isTransposition"),
        (56, "hideBaseVal"),
        (57, "dataTable"),
        (58, "dtVerLines"),
        (59, "dtHorLines"),
    ] {
        set(slot, boolean(c.required(name)?)?.into());
    }
    set(60, code(c.required("dtHAlign")?, &[("Right", "2")])?.into());
    set(61, localized(c.required("dtFormat")?)?);
    set(62, boolean(c.required("dtKeys")?)?.into());
    // `paletteKind` names the palette record at 179, whose code member 100
    // repeats; member 63 reads `0` on every record.
    let palette_kind = leaf(c.required("paletteKind")?)?.trim().to_string();
    let palette_code = match palette_kind.as_str() {
        "Auto" => "14",
        "Palette8" => "0",
        "Palette32" => "1",
        "Gradient" => "13",
        other => bail!("<d4p1:paletteKind> spells {other}, which the chart writer has not measured"),
    };
    set(63, "0".into());
    set(64, "0".into());
    set(
        120,
        code(
            c.required("animation")?,
            &[("Auto", "0"), ("Use", "1"), ("DontUse", "2")],
        )?
        .into(),
    );
    set(121, integer(c.required("rebuildTime")?)?);
    exact(c.required("isTransposed")?, "false")?;
    exact(c.required("autoTransposition")?, "false")?;
    set(65, boolean(c.required("legendScrollEnable")?)?.into());
    set(66, color(c.required("surfaceColor")?)?);
    set(67, code(c.required("radarScaleType")?, &[("Circle", "0")])?.into());
    set(
        68,
        code(c.required("gaugeValuesPresentation")?, &[("Needle", "0")])?.into(),
    );
    set(69, gauge_quality_bands(c.required("gaugeQualityBands")?)?);
    set(70, integer(c.required("beginGaugeAngle")?)?);
    set(71, integer(c.required("endGaugeAngle")?)?);
    set(72, integer(c.required("gaugeThickness")?)?);
    set(
        73,
        code(c.required("gaugeLabelsLocation")?, &[("InsideScale", "1")])?.into(),
    );
    set(74, boolean(c.required("gaugeLabelsArcDirection")?)?.into());
    set(75, integer(c.required("gaugeBushThickness")?)?);
    set(76, color(c.required("gaugeBushColor")?)?);
    set(77, boolean(c.required("autoMaxValue")?)?.into());
    let bound = |node: &XmlNode| match host {
        ChartHost::FormAttribute => decimal(node),
        ChartHost::SpreadsheetTemplate => double(node),
    };
    set(78, bound(c.required("userMaxValue")?)?);
    set(79, boolean(c.required("autoMinValue")?)?.into());
    set(80, bound(c.required("userMinValue")?)?);
    set(81, boolean(c.required("elementsIsInit")?)?.into());
    // `titleIsInit`, `legendIsInit` and `chartIsInit` are `post[7..10)`, which
    // the template exporter reads as one flag.
    let title_is_init = boolean(c.required("titleIsInit")?)?;
    let legend_is_init = boolean(c.required("legendIsInit")?)?;
    let chart_is_init = boolean(c.required("chartIsInit")?)?;
    ensure!(
        title_is_init == legend_is_init && legend_is_init == chart_is_init,
        "the chart's titleIsInit, legendIsInit and chartIsInit disagree"
    );
    for (start, name) in [
        (163, "elementsChart"),
        (167, "elementsLegend"),
        (171, "elementsTitle"),
    ] {
        for (offset, value) in rectangle(c.required(name)?)?.into_iter().enumerate() {
            set(start + offset, value);
        }
    }
    set(95, color(c.required("borderColor")?)?);
    set(96, border(c.required("border")?, false)?);
    set(97, string(c.required("dataSourceDescription")?)?);
    set(98, boolean(c.required("isDataSourceMode")?)?.into());
    set(99, boolean(c.required("isRandomizedNewValues")?)?.into());
    let data_count = series_count * point_count;
    let data_items = if data_count > 0 {
        real_data_items(c.required("realDataItems")?, data_count)?
    } else {
        Vec::new()
    };
    set(
        110,
        match c.optional("splineMode") {
            Some(mode) => code(mode, &[("SmoothCurve", "1")])?.into(),
            None => "0".into(),
        },
    );
    set(112, integer(c.required("splineStrain")?)?);
    set(111, decimal(c.required("translucencePercent")?)?);
    set(113, percent(c.required("funnelNeckHeightPercent")?)?);
    set(114, percent(c.required("funnelNeckWidthPercent")?)?);
    set(115, percent(c.required("funnelGapSumPercent")?)?);
    set(116, line(c.required("multiStageLinkLine")?)?);
    set(117, color(c.required("multiStageLinkColor")?)?);
    set(127, axis(c.required("valuesAxis")?)?);
    set(128, axis(c.required("pointsAxis")?)?);
    for (slot, name) in [(139, "pointsScale"), (140, "valuesScale"), (141, "seriesScale")] {
        let block = match c.optional(name) {
            Some(scale) => scale_of(scale)?,
            None => Scale::default(),
        };
        set(slot, block.encode());
    }
    set(
        161,
        match c.optional("legendPlacement") {
            Some(placement) => code(
                placement,
                &[
                    ("Top", "3"),
                    ("Bottom", "4"),
                    ("UseCoordinates", "5"),
                    ("None", "6"),
                ],
            )?
            .into(),
            None => "0".into(),
        },
    );
    set(
        160,
        match c.optional("plotAreaPlacement") {
            Some(placement) => code(placement, &[("UseCoordinates", "1")])?.into(),
            None => "0".into(),
        },
    );
    set(
        162,
        match c.optional("titleAreaPlacement") {
            Some(placement) => code(
                placement,
                &[("UseCoordinates", "1"), ("Top", "3"), ("None", "8")],
            )?
            .into(),
            None => "0".into(),
        },
    );
    let tooltip_mode = match c.optional("valuesToolTipShowMode") {
        Some(mode) => code(mode, &[("ShowOnHover", "2")])?,
        None => "0",
    };
    let show_modes: &[(&str, &str)] = &[("Show", "1"), ("DontShow", "2")];
    let points_drop_lines = match c.optional("pointsDropLinesShowMode") {
        Some(mode) => code(mode, show_modes)?,
        None => "0",
    };
    let values_drop_lines = match c.optional("valuesDropLinesShowMode") {
        Some(mode) => code(mode, show_modes)?,
        None => "0",
    };
    set(
        183,
        format!("{{0,{tooltip_mode},0,{points_drop_lines},{values_drop_lines}}}"),
    );
    // The palette record is `{0,<code>,<gradient start colour>,auto,0,0}`; the
    // start colour is repeated at 144 (`valuesAxis` + 17).
    let gradient_start = match (palette_code, c.optional("colorPaletteDescription")) {
        ("14", None) => AUTO_COLOR.to_string(),
        (_, Some(description)) if palette_code != "14" => {
            let mut d = Children::chart(description)?;
            exact(d.required("colorPalette")?, &palette_kind)?;
            let start = match d.optional("gradientPaletteStartColor") {
                Some(start) => color(start)?,
                None => AUTO_COLOR.to_string(),
            };
            d.finish()?;
            start
        }
        _ => bail!("<d4p1:paletteKind> {palette_kind} disagrees with <d4p1:colorPaletteDescription>"),
    };
    let reference_bands_palette = match c.optional("referenceBandsColorPaletteDescription") {
        None => "{0,14,{3,4,{0}},{3,4,{0}},0,0}".to_string(),
        Some(description) => {
            let mut d = Children::chart(description)?;
            let code = match leaf(d.required("colorPalette")?)?.trim() {
                "Palette8" => "0",
                "Palette32" => "1",
                "Gradient" => "13",
                other => bail!(
                    "<d4p1:colorPalette> spells {other}, which the chart writer has not measured"
                ),
            };
            let start = match d.optional("gradientPaletteStartColor") {
                Some(start) => color(start)?,
                None => AUTO_COLOR.to_string(),
            };
            d.finish()?;
            format!("{{0,{code},{start},{AUTO_COLOR},0,0}}")
        }
    };
    c.finish()?;

    // The members no element carries.
    set(100, palette_code.into());
    set(101, orientation_code.into());
    for slot in 107..110 {
        set(slot, title_is_init.into());
    }
    for (slot, value) in [
        (102, AUTO_FONT),
        (103, "1"),
        (118, "2"),
        (119, "255"),
        (122, NIL_UUID),
        (126, "0"),
        (129, "0"),
        (130, "0"),
        (131, "2"),
        (132, "-2"),
        (133, "1"),
        (134, "10"),
        (135, "1"),
        (136, "20"),
        (137, "0"),
        (138, "0"),
        (142, "0"),
        (143, "0"),
        (145, AUTO_COLOR),
        (146, "0"),
        (94, "0"),
        (175, "{0,0}"),
        (176, "{0,0}"),
        (177, "{0,0}"),
        (178, "{0,0}"),
        (181, "0"),
        (182, "0"),
        (184, "{0,0,0,0}"),
        (185, "0"),
        (186, ""),
        (187, "60"),
        (189, "{0,0,{0,1,0,1,0},0,0}"),
    ] {
        set(slot, value.into());
    }
    set(
        179,
        format!("{{0,{palette_code},{gradient_start},{{3,4,{{0}}}},0,0}}"),
    );
    set(144, gradient_start);
    set(180, reference_bands_palette);
    set(188, Scale::default().encode());
    for slot in 190..197 {
        set(slot, "0".into());
    }
    for (offset, value) in UNTOUCHED_LAYOUT.iter().enumerate() {
        set(82 + offset, (*value).into());
        set(148 + offset, (*value).into());
    }
    // 123 is the scale-id count, 124/125 the two lists it heads, 147 the
    // legend list: all four are written in place below.
    let fixed_value = |slot: usize| -> Result<&str> {
        fixed[slot]
            .as_deref()
            .ok_or_else(|| anyhow!("chart tail member {slot} was never written"))
    };

    // The scale-id list: the ex-series id, then every real series id, each
    // once. Where an id repeats, the list the platform stores is the real
    // series ids alone (ERP УХ `ВыполнениеОпераций2_2`), which the decoder
    // reads only when the ex-series id is the first of them.
    let real_ids: Vec<&str> = series.iter().map(|item| item.id.as_str()).collect();
    let mut all_ids = vec![ex_series.id.as_str()];
    all_ids.extend(real_ids.iter().copied());
    let mut scale_ids: Vec<&str> = Vec::new();
    for id in &all_ids {
        if !scale_ids.contains(id) {
            scale_ids.push(id);
        }
    }
    ensure!(
        scale_ids.len() == all_ids.len() || scale_ids == real_ids,
        "the chart's series ids repeat in an order the scale-id list has not been measured for"
    );

    let mut tail: Vec<String> = Vec::with_capacity(260);
    for slot in 0..100 {
        tail.push(fixed_value(slot)?.to_string());
    }
    tail.extend(data_items);
    for slot in 100..123 {
        tail.push(fixed_value(slot)?.to_string());
    }
    tail.push(scale_ids.len().to_string());
    for id in &scale_ids {
        tail.push(format!("{{0,{id},0}}"));
    }
    for _ in 0..=series.len() {
        tail.push("{0,0}".to_string());
    }
    for slot in 126..147 {
        tail.push(fixed_value(slot)?.to_string());
    }
    for point in &points {
        tail.push(format!("{{{}}}", point.color));
    }
    for item in series.iter().chain(std::iter::once(&ex_series)) {
        tail.push(format!(
            "{{{},{},0,0,0,\"\",{{1,0}},{{1,0}},{{1,0}},0}}",
            item.color, item.marker
        ));
    }
    for slot in 148..186 {
        tail.push(fixed_value(slot)?.to_string());
    }
    for _ in 0..data_count {
        tail.push("{{1,{1,0},0},0}".to_string());
    }
    for slot in 186..197 {
        tail.push(fixed_value(slot)?.to_string());
    }

    let mut data: Vec<String> = vec![
        "74".to_string(),
        series_cur_id,
        points_cur_id,
        is_series_design.to_string(),
        series_count.to_string(),
    ];
    for (index, item) in series.iter().enumerate() {
        data.push(series_record(item, Some(index), chart_type, palette_code));
    }
    data.push(series_record(&ex_series, None, chart_type, palette_code));
    data.push(is_points_design.to_string());
    data.push(point_count.to_string());
    for point in &points {
        data.push(format!(
            "{},{},{},{},{},{},{},{},{{\"U\"}},{{\"U\"}},{}",
            point.text,
            point.str_is_changed,
            point.id,
            point.color,
            point.line,
            point.marker,
            point.is_expand,
            point.is_indicator,
            point.color_priority
        ));
    }
    data.extend(tail);
    Ok(format!("{{{}}}", data.join(",")))
}

/// The eleven members of a series record: its cached colour, line, cached
/// marker, text, three flags, id, two undefined placeholders and the colour
/// priority (`0` on every record, the only value the decoder reads).
///
/// The colour and the marker the platform publishes are the legend entry's;
/// the record's own two are what it last drew with. A legend entry that names
/// a colour or a marker is drawn with it (every such entry of both corpora).
/// An automatic one is drawn from the automatic palette by the series'
/// position -- ex-series last -- and that palette depends on the chart type:
/// the platform-loaded `form-chart-*` seeds and ERP УХ agree on the four
/// real-series colours of a `Column3D`/`Gauge` chart and of a `Line` chart,
/// and on the ex-series colour of each; a `Palette8` chart's ex-series is
/// drawn silver. The markers cycle `Rhomb`, `Rect`, `Circle` by position and
/// the ex-series draws `Rect`, on every automatic entry of both corpora.
fn series_record(
    series: &Series,
    index: Option<usize>,
    chart_type: &str,
    palette_code: &str,
) -> String {
    const COLUMN_COLORS: [&str; 4] = ["16762956", "7964671", "49407", "7258719"];
    const LINE_COLORS: [&str; 4] = ["16757805", "7105526", "2866687", "5090622"];
    let line_like = matches!(chart_type, "0" | "44");
    let cached_color = if series.color != AUTO_COLOR {
        series.color.clone()
    } else {
        let code = match (index, palette_code) {
            (None, "0") => "12632256",
            (None, _) if line_like => "8874375",
            (None, _) => "11837108",
            (Some(position), _) if line_like => LINE_COLORS[position % 4],
            (Some(position), _) => COLUMN_COLORS[position % 4],
        };
        format!("{{3,0,{{{code}}}}}")
    };
    let cached_marker = if series.marker != "4" {
        series.marker
    } else {
        match index {
            Some(position) => ["3", "1", "2"][position % 3],
            None => "1",
        }
    };
    format!(
        "{cached_color},{},{cached_marker},{},{},{},{},{},{{\"U\"}},{{\"U\"}},0",
        series.line,
        series.text,
        series.str_is_changed,
        series.is_expand,
        series.is_indicator,
        series.id
    )
}

fn series_of(node: &XmlNode) -> Result<Series> {
    let mut c = Children::chart(node)?;
    let series = Series {
        id: integer(c.required("id")?)?,
        color: color(c.required("color")?)?,
        line: line(c.required("line")?)?,
        marker: code(c.required("marker")?, MARKERS)?,
        text: localized(c.required("text")?)?,
        str_is_changed: boolean(c.required("strIsChanged")?)?,
        is_expand: boolean(c.required("isExpand")?)?,
        is_indicator: boolean(c.required("isIndicator")?)?,
    };
    exact(c.required("colorPriority")?, "false")?;
    c.finish()?;
    Ok(series)
}

fn point_of(node: &XmlNode) -> Result<Point> {
    let mut c = Children::chart(node)?;
    let point = Point {
        id: integer(c.required("id")?)?,
        color: color(c.required("color")?)?,
        line: line(c.required("line")?)?,
        marker: code(c.required("marker")?, MARKERS)?,
        text: localized(c.required("text")?)?,
        str_is_changed: boolean(c.required("strIsChanged")?)?,
        is_expand: boolean(c.required("isExpand")?)?,
        is_indicator: boolean(c.required("isIndicator")?)?,
        color_priority: boolean(c.required("colorPriority")?)?,
    };
    c.finish()?;
    Ok(point)
}

/// `<d4p1:realDataItems>`: three members per item -- the typed value, the
/// undefined `valInfo`, the tooltip.
fn real_data_items(node: &XmlNode, expected: usize) -> Result<Vec<String>> {
    let mut c = Children::chart(node)?;
    let mut members = Vec::with_capacity(expected * 3);
    while let Some(item) = c.optional("item") {
        let mut i = Children::chart(item)?;
        let value = i.required("valData")?;
        ensure!(
            value.children.is_empty() && value.attributes.len() == 1,
            "<d4p1:valData> carries markup the chart writer does not place"
        );
        let typed = match value.attribute("xsi:type") {
            Some("xs:decimal") => format!("{{\"N\",{}}}", decimal_text(&value.text)?),
            Some("xs:string") => format!("{{\"S\",{}}}", quote(&value.text)),
            _ => bail!("<d4p1:valData> names a value type the chart writer has not measured"),
        };
        let info = i.required("valInfo")?;
        ensure!(
            info.children.is_empty()
                && info.text.trim().is_empty()
                && attributes_are(info, &[("xsi:nil", "true")]),
            "<d4p1:valInfo> names a value the chart writer has not measured"
        );
        let tooltip = string(i.required("toolTip")?)?;
        i.finish()?;
        members.push(typed);
        members.push("{\"U\"}".to_string());
        members.push(tooltip);
    }
    c.finish()?;
    ensure!(
        members.len() == expected * 3,
        "the chart spells {} data items where its series and points make {expected}",
        members.len() / 3
    );
    Ok(members)
}

/// A scale block (`pointsScale`, `valuesScale`, `seriesScale`, and the
/// fourth, unnamed one at 188): 22 members, 23 when a grid line record is
/// inserted after member 8.
struct Scale {
    show_title: &'static str,
    title_text_source: &'static str,
    title_placement: &'static str,
    title_text: String,
    title_area: String,
    grid_lines_show_mode: &'static str,
    label_location: &'static str,
    grid_line: Option<String>,
    label_font: String,
    label_color: String,
    label_orientation: &'static str,
    label_format: String,
    label_angle: String,
    max_label_rows: String,
    show_in_chart: &'static str,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            show_title: "0",
            title_text_source: "0",
            title_placement: "2",
            title_text: "{1,0}".to_string(),
            title_area: format!(
                "{{1,4,0.5,0.5,{AUTO_FONT},{AUTO_COLOR},{AUTO_COLOR},1,{{3,0,{{0}},0,1,0,{BORDER_UUID}}},{AUTO_COLOR},4,2,0}}"
            ),
            grid_lines_show_mode: "2",
            label_location: "0",
            grid_line: None,
            label_font: AUTO_FONT.to_string(),
            label_color: AUTO_COLOR.to_string(),
            label_orientation: "2",
            label_format: "{1,0}".to_string(),
            label_angle: "0".to_string(),
            max_label_rows: "0".to_string(),
            show_in_chart: "0",
        }
    }
}

impl Scale {
    fn encode(&self) -> String {
        let mut members: Vec<&str> = vec![
            "2",
            self.show_title,
            self.title_text_source,
            self.title_placement,
            &self.title_text,
            &self.title_area,
            self.grid_lines_show_mode,
            self.label_location,
            if self.grid_line.is_some() { "1" } else { "0" },
        ];
        if let Some(line) = self.grid_line.as_deref() {
            members.push(line);
        }
        members.extend([
            AUTO_COLOR,
            &self.label_font,
            &self.label_color,
            self.label_orientation,
            &self.label_format,
            "0",
            AUTO_COLOR,
            "0",
            &self.label_angle,
            "0",
            &self.max_label_rows,
            "0",
            self.show_in_chart,
        ]);
        format!("{{{}}}", members.join(","))
    }
}

/// A scale element. The form exporter and the template exporter write the
/// members in different orders (the template one puts `labelFormat` ahead of
/// the grid and `labelFont` ahead of `labelColor`), so each member is taken
/// wherever it stands, once; the caller's round-trip check holds the order.
fn scale_of(node: &XmlNode) -> Result<Scale> {
    let mut scale = Scale::default();
    ensure!(
        node.text.trim().is_empty() && node.attributes.is_empty(),
        "<{}> carries markup the chart writer does not place",
        node.name
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut title_area_seen = false;
    let mut title_text_source = false;
    let mut title_text = false;
    for child in &node.children {
        let name = child
            .name
            .strip_prefix("d4p1:")
            .ok_or_else(|| anyhow!("<{}> names <{}>, which the chart writer cannot place", node.name, child.name))?;
        ensure!(
            seen.insert(name.to_string()),
            "<{}> names <d4p1:{name}> twice",
            node.name
        );
        match name {
            "showTitle" => {
                scale.show_title = code(child, &[("DontShow", "1"), ("Show", "2")])?;
            }
            "titleTextSource" => {
                scale.title_text_source = code(child, &[("UseText", "1")])?;
                title_text_source = true;
            }
            "titleText" => {
                scale.title_text = localized(child)?;
                title_text = true;
            }
            "titleArea" => {
                scale.title_area = title_area(child)?;
                title_area_seen = true;
            }
            "titlePlacement" => {
                scale.title_placement = code(child, &[("PlotArea", "1")])?;
            }
            "gridLinesShowMode" => {
                scale.grid_lines_show_mode = code(child, &[("Show", "0"), ("DontShow", "1")])?;
            }
            "gridLine" => scale.grid_line = Some(line(child)?),
            "labelColor" => scale.label_color = color(child)?,
            "scaleLabelLocation" => {
                scale.label_location = code(child, &[("None", "1")])?;
            }
            "labelFont" => scale.label_font = font(child)?,
            "labelFormat" => scale.label_format = localized(child)?,
            "labelOrientation" => {
                scale.label_orientation = code(
                    child,
                    &[("Horizontal", "0"), ("Vertical", "1"), ("CustomAngle", "3")],
                )?;
            }
            "maxLabelRows" => scale.max_label_rows = integer(child)?,
            "labelAngle" => scale.label_angle = integer(child)?,
            "showInChart" => {
                scale.show_in_chart = code(child, &[("DontShow", "2")])?;
            }
            other => bail!(
                "<{}> names <d4p1:{other}>, which the chart writer cannot place",
                node.name
            ),
        }
    }
    ensure!(title_area_seen, "<{}> has no <d4p1:titleArea>", node.name);
    ensure!(
        title_text_source == title_text,
        "<{}> spells a title text without its source",
        node.name
    );
    Ok(scale)
}

/// `<d4p1:valuesAxis>`/`<d4p1:pointsAxis>`: `{0,<baseValue>,{0,1,<minValue>,
/// 1,<maxValue>},<maxDetection>,<minDetection>}`, an absent value 0 and a
/// detection flag 2 where the element publishes `UseValueWithLimitations`.
fn axis(node: &XmlNode) -> Result<String> {
    let mut c = Children::chart(node)?;
    let base = match c.optional("baseValue") {
        Some(value) => decimal(value)?,
        None => "0".to_string(),
    };
    let mut bound = |name: &str| -> Result<String> {
        match c.optional(name) {
            Some(value) => {
                ensure!(
                    value.children.is_empty()
                        && attributes_are(value, &[("xsi:type", "xs:decimal")]),
                    "<d4p1:{name}> is not an xs:decimal"
                );
                decimal_text(&value.text)
            }
            None => Ok("0".to_string()),
        }
    };
    let min = bound("minValue")?;
    let max = bound("maxValue")?;
    let mut detection = |name: &str| -> Result<&'static str> {
        match c.optional(name) {
            Some(value) => {
                exact(value, "UseValueWithLimitations")?;
                Ok("2")
            }
            None => Ok("0"),
        }
    };
    let min_detection = detection("minValueDetectionMethod")?;
    let max_detection = detection("maxValueDetectionMethod")?;
    c.finish()?;
    Ok(format!(
        "{{0,{base},{{0,1,{min},1,{max}}},{max_detection},{min_detection}}}"
    ))
}

/// `{1,4,0.5,0.5,<font>,<textColor>,<backColor>,1,<border>,<borderColor>,4,2,0}`.
fn title_area(node: &XmlNode) -> Result<String> {
    let mut c = Children::chart(node)?;
    let font = font(c.required("font")?)?;
    let text_color = color(c.required("textColor")?)?;
    let back_color = color(c.required("backColor")?)?;
    let border = border(c.required("border")?, false)?;
    let border_color = color(c.required("borderColor")?)?;
    c.finish()?;
    Ok(format!(
        "{{1,4,0.5,0.5,{font},{text_color},{back_color},1,{border},{border_color},4,2,0}}"
    ))
}

/// A placement rectangle, spelled left, right, top, bottom and stored left,
/// top, right, bottom.
fn rectangle(node: &XmlNode) -> Result<[String; 4]> {
    let mut c = Children::chart(node)?;
    let left = decimal(c.required("left")?)?;
    let right = decimal(c.required("right")?)?;
    let top = decimal(c.required("top")?)?;
    let bottom = decimal(c.required("bottom")?)?;
    c.finish()?;
    Ok([left, top, right, bottom])
}

// ---------------------------------------------------------------------------
// The Gantt chart wrapper, `{19,…}`.
// ---------------------------------------------------------------------------

const TIME_MEASURES: &[(&str, &str)] = &[
    ("Minute", "10"),
    ("Hour", "20"),
    ("Day", "30"),
    ("Month", "50"),
];

/// `{19,{0,{11},{74,…}},<points>,<series>,0,0,drawEmpty,<timeScale>,
/// keepScaleVariant,fixedVariantMeasure,fixedVariantInterval,
/// autoFullInterval,fullIntervalBegin,fullIntervalEnd,visualBegin,
/// intervalDrawType,noneVariantChars,noneVariantMeasure,verticalStretch,
/// verticalScrollEnable,showValueText,{1,0},outboundColor,
/// {3,{0,{1,0,0},0},{0,0}},0,linksColor,<linksLine>,{0,0,0},showPointsText,
/// showData,1,textPlacement,0}` -- revision 19, the one that stores
/// `textPlacement` (18 of the 18 ERP УХ records).
fn gantt_wrapper(node: &XmlNode, host: ChartHost) -> Result<String> {
    let mut c = Children::chart(node)?;
    let chart = chart_record(c.required("chart")?, host)?;
    let points = gantt_series_like(c.required("points")?, true)?;
    let series = gantt_series_like(c.required("series")?, false)?;
    let draw_empty = boolean(c.required("drawEmpty")?)?;
    let time_scale = gantt_time_scale(c.required("timeScale")?)?;
    let keep_scale_variant = code(
        c.required("keepScaleVariant")?,
        &[("AllData", "2"), ("Auto", "3")],
    )?;
    let fixed_variant_measure = code(c.required("fixedVariantMeasure")?, TIME_MEASURES)?;
    let fixed_variant_interval = integer(c.required("fixedVariantInterval")?)?;
    let auto_full_interval = boolean(c.required("autoFullInterval")?)?;
    let full_interval_begin = gantt_date(c.required("fullIntervalBegin")?)?;
    let full_interval_end = gantt_date(c.required("fullIntervalEnd")?)?;
    let visual_begin = gantt_date(c.required("visualBegin")?)?;
    let interval_draw_type = code(
        c.required("intervalDrawType")?,
        &[("Flat", "0"), ("Gradient", "3")],
    )?;
    let none_variant_chars = integer(c.required("noneVariantChars")?)?;
    let none_variant_measure = code(c.required("noneVariantMeasure")?, TIME_MEASURES)?;
    let vertical_stretch = code(c.required("verticalStretch")?, &[("None", "0")])?;
    let vertical_scroll_enable = boolean(c.required("verticalScrollEnable")?)?;
    let show_value_text = code(
        c.required("showValueText")?,
        &[("None", "0"), ("Right", "1")],
    )?;
    empty(c.required("extTitle")?)?;
    let outbound_color = color(c.required("outboundColor")?)?;
    {
        let intervals = c.required("backIntervals")?;
        let mut b = Children::chart(intervals)?;
        let collection = b.required("collection")?;
        let mut inner = Children::chart(collection)?;
        exact(inner.required("ticks")?, "0")?;
        inner.finish()?;
        exact(b.required("ticks")?, "0")?;
        b.finish()?;
    }
    let links_color = color(c.required("linksColor")?)?;
    let links_line = line(c.required("linksLine")?)?;
    let show_points_text = code(
        c.required("showPointsText")?,
        &[("Auto", "0"), ("Show", "1")],
    )?;
    let show_data = code(c.required("showData")?, &[("Auto", "0")])?;
    let text_placement = code(c.required("textPlacement")?, &[("Auto", "0"), ("Cut", "1")])?;
    exact(c.required("intervalTextRepresentation")?, "Auto")?;
    c.finish()?;
    let members = [
        "19".to_string(),
        format!("{{0,{{11}},{chart}}}"),
        points,
        series,
        "0".into(),
        "0".into(),
        draw_empty.into(),
        time_scale,
        keep_scale_variant.into(),
        fixed_variant_measure.into(),
        fixed_variant_interval,
        auto_full_interval.into(),
        full_interval_begin,
        full_interval_end,
        visual_begin,
        interval_draw_type.into(),
        none_variant_chars,
        none_variant_measure.into(),
        vertical_stretch.into(),
        vertical_scroll_enable.into(),
        show_value_text.into(),
        "{1,0}".into(),
        outbound_color,
        "{3,{0,{1,0,0},0},{0,0}}".into(),
        "0".into(),
        links_color,
        links_line,
        "{0,0,0}".into(),
        show_points_text.into(),
        show_data.into(),
        "1".into(),
        text_placement.into(),
        "0".into(),
    ];
    Ok(format!("{{{}}}", members.join(",")))
}

/// `<d4p1:points>` and `<d4p1:series>`: `{1|0,{3,0,1,0,<value>,<cache>,
/// <autoText>,0}}`. The points' value carries an empty picture and an
/// automatic font behind its header and their cache four colours; the
/// series' value carries neither and their cache three.
fn gantt_series_like(node: &XmlNode, is_points: bool) -> Result<String> {
    let mut c = Children::chart(node)?;
    exact(c.required("testMode")?, "false")?;
    let mut v = Children::chart(c.required("value")?)?;
    for name in ["itemKey", "key", "parentKey", "leftKey", "rightKey", "extKey"] {
        exact(v.required(name)?, "0")?;
    }
    empty(v.required("title")?)?;
    exact(v.required("cacheKey")?, "0")?;
    let base_data = integer(v.required("baseData")?)?;
    if is_points {
        ensure!(
            font(v.required("font")?)? == AUTO_FONT,
            "a Gantt chart's points name a font the chart writer has not measured"
        );
        empty(v.required("picture")?)?;
    }
    v.finish()?;
    let mut k = Children::chart(c.required("contentCacheItem")?)?;
    let main = color(k.required("mainColor")?)?;
    let second = color(k.required("secondColor")?)?;
    let rest = if is_points {
        let back = color(k.required("backColor")?)?;
        let text = color(k.required("textColor")?)?;
        format!("{back},{text}")
    } else {
        color(k.required("hatchBetweenIntervalsColor")?)?
    };
    k.finish()?;
    let auto_text = boolean(c.required("autoText")?)?;
    exact(c.required("useValuesReverseBehavior")?, "false")?;
    c.finish()?;
    let header = format!("{{8,0,0,0,0,0,{{\"U\"}},{{1,0}},{{\"U\"}},0,{base_data}}}");
    let cache = format!("{{0,1,{{0,{{0,{main},{second}}},{rest}}}}}");
    Ok(if is_points {
        format!(
            "{{1,{{3,0,1,0,{{2,{header},{{4,0,{{0}},\"\",-1,-1,1,0,\"\"}},{AUTO_FONT}}},{cache},{auto_text},0}}}}"
        )
    } else {
        format!("{{0,{{3,0,1,0,{{3,{header}}},{cache},{auto_text},0}}}}")
    })
}

/// `<d4p1:timeScale>`: `{3,0,<level count>,<level>…,0,<backColor>,
/// <textColor>,0}`, a level `{8,<measure>,1,1,<line>,{3,0,{12632256}},3,
/// {1,0},{0,{1,0,0}},{3,4,{0}},{3,4,{0}},1}`. Only the measure and the line
/// vary over the corpus; every other member is the literal the decoder
/// validates, and member 3 -- which it does not read -- is `1` on all 18.
fn gantt_time_scale(node: &XmlNode) -> Result<String> {
    let mut c = Children::chart(node)?;
    exact(c.required("placement")?, "Top")?;
    let mut levels = Vec::new();
    while let Some(level) = c.optional("level") {
        let mut l = Children::chart(level)?;
        let measure = code(l.required("measure")?, TIME_MEASURES)?;
        exact(l.required("interval")?, "1")?;
        // The form exporter publishes only `true`/`MonthDayWeekDay` here and
        // refuses anything else through the round-trip check; a template's
        // exporter reads both members (`show`, and `dayFormatRule` `WeekDay`
        // 2 / `MonthDayWeekDay` 3).
        let show = boolean(l.required("show")?)?;
        let level_line = line(l.required("line")?)?;
        exact(l.required("scaleColor")?, "#C0C0C0")?;
        let day_format_rule = match leaf(l.required("dayFormatRule")?)?.trim() {
            "WeekDay" => "2",
            "MonthDayWeekDay" => "3",
            other => bail!("<d4p1:dayFormatRule> spells {other}, which the chart writer has not measured"),
        };
        empty(l.required("format")?)?;
        let mut labels = Children::chart(l.required("labels")?)?;
        exact(labels.required("ticks")?, "0")?;
        labels.finish()?;
        exact(l.required("backColor")?, "auto")?;
        exact(l.required("textColor")?, "auto")?;
        exact(l.required("showPereodicalLabels")?, "true")?;
        l.finish()?;
        levels.push(format!(
            "{{8,{measure},1,{show},{level_line},{{3,0,{{12632256}}}},{day_format_rule},{{1,0}},{{0,{{1,0,0}}}},{AUTO_COLOR},{AUTO_COLOR},1}}"
        ));
    }
    ensure!(!levels.is_empty(), "a Gantt chart's time scale names no level");
    exact(c.required("transparent")?, "false")?;
    let back_color = color(c.required("backColor")?)?;
    let text_color = color(c.required("textColor")?)?;
    exact(c.required("currentLevel")?, "0")?;
    c.finish()?;
    Ok(format!(
        "{{3,0,{},{},0,{back_color},{text_color},0}}",
        levels.len(),
        levels.join(",")
    ))
}

/// `<d4p1:gaugeQualityBands>`: `{1,N,<band>×N,<useTextStr>,<useTooltipStr>}`,
/// a band `{3,<begin>,<end>,<backColor>,<text>,<tooltip>,"",0,"",0,<begin>,
/// <end>}`. The two string members stay empty while both flags are `false`,
/// which is all the stand spells (ERP УХ `ФинансовыйАнализ` gauges, three
/// bands each); a flag set to `true` is refused.
fn gauge_quality_bands(node: &XmlNode) -> Result<String> {
    ensure!(
        node.text.trim().is_empty()
            && attributes_are(node, &[("useTextStr", "false"), ("useTooltipStr", "false")]),
        "<d4p1:gaugeQualityBands> spells flags the chart writer has not measured"
    );
    let mut bands = String::new();
    for item in &node.children {
        let names = item
            .children
            .iter()
            .map(|child| child.name.as_str())
            .collect::<Vec<_>>();
        ensure!(
            item.name == "v8ui:item"
                && item.attributes.is_empty()
                && item.text.trim().is_empty()
                && names
                    == [
                        "v8ui:begin",
                        "v8ui:end",
                        "v8ui:backColor",
                        "v8ui:text",
                        "v8ui:tooltip"
                    ],
            "a gauge quality band names members the chart writer has not measured"
        );
        let begin = decimal(&item.children[0])?;
        let end = decimal(&item.children[1])?;
        let back_color = color(&item.children[2])?;
        let text = localized(&item.children[3])?;
        let tooltip = localized(&item.children[4])?;
        bands.push_str(&format!(
            ",{{3,{begin},{end},{back_color},{text},{tooltip},\"\",0,\"\",0,{begin},{end}}}"
        ));
    }
    Ok(format!("{{1,{}{bands},0,0}}", node.children.len()))
}

/// `YYYY-MM-DDTHH:MM:SS` to the bare fourteen digits the record stores.
fn gantt_date(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    let bytes = text.as_bytes();
    let well_formed = bytes.len() == 19
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            _ => byte.is_ascii_digit(),
        });
    ensure!(
        well_formed,
        "<{}> spells {text}, which is not a date the chart writer places",
        node.name
    );
    Ok(format!(
        "{}{}{}{}{}{}",
        &text[0..4],
        &text[5..7],
        &text[8..10],
        &text[11..13],
        &text[14..16],
        &text[17..19]
    ))
}

// ---------------------------------------------------------------------------
// Scalar members.
// ---------------------------------------------------------------------------

fn boolean(node: &XmlNode) -> Result<&'static str> {
    match leaf(node)?.trim() {
        "true" => Ok("1"),
        "false" => Ok("0"),
        other => bail!("<{}> spells {other}, which is not a boolean", node.name),
    }
}

fn integer(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    let digits = text.strip_prefix('-').unwrap_or(text);
    ensure!(
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) && text.parse::<i64>().is_ok(),
        "<{}> spells {text}, which is not an integer",
        node.name
    );
    Ok(text.to_string())
}

fn count(node: &XmlNode) -> Result<usize> {
    let text = leaf(node)?.trim();
    ensure!(
        !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()),
        "<{}> spells {text}, which is not a count",
        node.name
    );
    text.parse::<usize>()
        .map_err(|_| anyhow!("<{}> spells {text}, which is not a count", node.name))
}

/// A plain decimal, stored as the XML spells it: the decoder publishes the
/// stored text verbatim and refuses an exponent.
fn decimal(node: &XmlNode) -> Result<String> {
    decimal_text(leaf(node)?).map_err(|error| anyhow!("<{}>: {error}", node.name))
}

fn decimal_text(text: &str) -> Result<String> {
    let text = text.trim();
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, "0"));
    ensure!(
        !whole.is_empty()
            && !fraction.is_empty()
            && whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit()),
        "{text} is not a decimal the chart writer places"
    );
    Ok(text.to_string())
}

/// A decimal stored as a double, in the platform's spelling. A value the
/// sixteen digits do not hold exactly is refused by the caller's round trip.
fn double(node: &XmlNode) -> Result<String> {
    let text = decimal(node)?;
    let value: f64 = text
        .parse()
        .map_err(|_| anyhow!("<{}> spells {text}, which is not a double", node.name))?;
    Ok(platform_double(value))
}

/// A whole percentage, stored as the fraction it is in the platform's
/// shortest exponent spelling: `10` is `1e-1`, `3` is `3e-2`, `0` is `0`
/// (every funnel member of both corpora).
fn percent(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    let value: u32 = text
        .parse()
        .ok()
        .filter(|value| *value <= 100 && text.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| anyhow!("<{}> spells {text}, which is not a percentage the chart writer places", node.name))?;
    Ok(platform_double(f64::from(value) / 100.0))
}

/// A double as the platform writes one into a chart record: sixteen
/// significant digits, trailing zeros dropped, the exponent written only when
/// it is not zero.
fn platform_double(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let formatted = format!("{value:.15e}");
    let (mantissa, exponent) = formatted
        .split_once('e')
        .unwrap_or((formatted.as_str(), "0"));
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    if exponent == "0" {
        mantissa.to_string()
    } else {
        format!("{mantissa}e{exponent}")
    }
}

fn code(node: &XmlNode, table: &[(&str, &'static str)]) -> Result<&'static str> {
    let text = leaf(node)?.trim();
    table
        .iter()
        .find_map(|(name, code)| (*name == text).then_some(*code))
        .ok_or_else(|| {
            anyhow!(
                "<{}> spells {text}, which the chart writer has not measured",
                node.name
            )
        })
}

fn exact(node: &XmlNode, expected: &str) -> Result<()> {
    let text = leaf(node)?.trim();
    ensure!(
        text == expected,
        "<{}> spells {text} where the chart writer has only measured {expected}",
        node.name
    );
    Ok(())
}

fn empty(node: &XmlNode) -> Result<()> {
    ensure!(
        leaf(node)?.trim().is_empty(),
        "<{}> carries a value the chart writer has not measured",
        node.name
    );
    Ok(())
}

/// A 1C string literal of an element's text, its own line breaks included.
fn string(node: &XmlNode) -> Result<String> {
    Ok(quote(leaf(node)?))
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// A colour: `auto` is the chart's unset colour, `<kind>:<uuid>` a style item
/// the configuration no longer has (published verbatim by the decoder), and
/// every other spelling the form items' own colour table.
fn color(node: &XmlNode) -> Result<String> {
    let text = leaf(node)?.trim();
    if text == "auto" {
        return Ok(AUTO_COLOR.to_string());
    }
    if let Some((kind, uuid)) = text.split_once(':')
        && !kind.is_empty()
        && kind.bytes().all(|byte| byte.is_ascii_digit())
        && is_uuid(uuid)
        && uuid != NIL_UUID
    {
        return Ok(format!("{{3,3,{{{kind},{uuid}}}}}"));
    }
    format_native_color(Some(text), |_| None).ok_or_else(|| {
        anyhow!(
            "<{}> names the colour {text}, which the chart writer cannot place",
            node.name
        )
    })
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

/// A font element: its attributes are the whole value.
fn font(node: &XmlNode) -> Result<String> {
    ensure!(
        node.children.is_empty() && node.text.trim().is_empty() && !node.attributes.is_empty(),
        "<{}> carries markup the chart writer does not place",
        node.name
    );
    let attributes = node
        .attributes
        .iter()
        .cloned()
        .collect::<BTreeMap<String, String>>();
    if attributes.len() == 1 && attributes.get("kind").map(String::as_str) == Some("AutoFont") {
        return Ok(AUTO_FONT.to_string());
    }
    format_native_font(&attributes, |_| None).ok_or_else(|| {
        anyhow!(
            "<{}> names a font the chart writer cannot place",
            node.name
        )
    })
}

/// `<… width="W" gap="false"><v8ui:style xsi:type="v8ui:ChartLineType">…`
/// to `{4,0,{0},<style>,<width>,0,<line uuid>,0}`.
fn line(node: &XmlNode) -> Result<String> {
    ensure!(
        node.attributes.len() == 2 && node.attribute("gap") == Some("false"),
        "<{}> names line attributes the chart writer has not measured",
        node.name
    );
    let width = width_of(node)?;
    let style = match style_of(node, "v8ui:ChartLineType")? {
        "Solid" => "1",
        "Dotted" => "2",
        other => bail!(
            "<{}> names the line style {other}, which the chart writer has not measured",
            node.name
        ),
    };
    Ok(format!("{{4,0,{{0}},{style},{width},0,{LINE_UUID},0}}"))
}

/// `<… width="W"><v8ui:style xsi:type="v8ui:ControlBorderType">…` to
/// `{3,0,{0},<style>,<width>,0,<uuid>}`.
fn border(node: &XmlNode, chart_border: bool) -> Result<String> {
    ensure!(
        node.attributes.len() == 1,
        "<{}> names border attributes the chart writer has not measured",
        node.name
    );
    let width = width_of(node)?;
    let spelled = style_of(node, "v8ui:ControlBorderType")?;
    let style = FormControlBorderStyle::from_xml_value(spelled).ok_or_else(|| {
        anyhow!(
            "<{}> names the border style {spelled}, which the chart writer has not measured",
            node.name
        )
    })?;
    let uuid = if chart_border && style != FormControlBorderStyle::WithoutBorder {
        NIL_UUID
    } else {
        BORDER_UUID
    };
    Ok(format!("{{3,0,{{0}},{},{width},0,{uuid}}}", style.raw_code()))
}

fn width_of(node: &XmlNode) -> Result<&str> {
    let width = node
        .attribute("width")
        .ok_or_else(|| anyhow!("<{}> names no width", node.name))?;
    ensure!(
        !width.is_empty() && width.bytes().all(|byte| byte.is_ascii_digit()),
        "<{}> names the width {width}, which is not a count",
        node.name
    );
    Ok(width)
}

/// The text of the single `<v8ui:style xsi:type="…">` a line or a border
/// holds.
fn style_of<'a>(node: &'a XmlNode, style_type: &str) -> Result<&'a str> {
    ensure!(
        node.text.trim().is_empty() && node.children.len() == 1,
        "<{}> carries markup the chart writer does not place",
        node.name
    );
    let style = &node.children[0];
    ensure!(
        style.name == "v8ui:style"
            && style.children.is_empty()
            && attributes_are(style, &[("xsi:type", style_type)]),
        "<{}> names a style the chart writer has not measured",
        node.name
    );
    Ok(style.text.trim())
}

/// `<v8:item><v8:lang>…</v8:lang><v8:content>…</v8:content></v8:item>…` to
/// `{1,<count>,{"<lang>","<content>"}…}`, and no item to `{1,0}`.
fn localized(node: &XmlNode) -> Result<String> {
    ensure!(
        node.attributes.is_empty() && node.text.trim().is_empty(),
        "<{}> carries markup the chart writer does not place",
        node.name
    );
    let mut items = Vec::new();
    for item in &node.children {
        ensure!(
            item.name == "v8:item" && item.attributes.is_empty() && item.text.trim().is_empty(),
            "<{}> names <{}>, which the chart writer cannot place",
            node.name,
            item.name
        );
        let [lang, content] = item.children.as_slice() else {
            bail!("<{}> names an item that is not a language and a text", node.name);
        };
        ensure!(
            lang.name == "v8:lang" && content.name == "v8:content",
            "<{}> names an item that is not a language and a text",
            node.name
        );
        items.push(format!(
            "{{{},{}}}",
            quote(leaf(lang)?),
            quote(leaf(content)?)
        ));
    }
    Ok(if items.is_empty() {
        "{1,0}".to_string()
    } else {
        format!("{{1,{},{}}}", items.len(), items.join(","))
    })
}

// ---------------------------------------------------------------------------
// The XML the writer reads.
// ---------------------------------------------------------------------------

/// One element: its qualified name, its attributes in document order, its
/// character data, its child elements.
#[derive(Debug, Default)]
struct XmlNode {
    name: String,
    attributes: Vec<(String, String)>,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find_map(|(name, value)| (name == key).then_some(value.as_str()))
    }
}

fn attributes_are(node: &XmlNode, expected: &[(&str, &str)]) -> bool {
    node.attributes.len() == expected.len()
        && expected
            .iter()
            .all(|(key, value)| node.attribute(key) == Some(*value))
}

/// A leaf element's text: no attributes, no children.
fn leaf(node: &XmlNode) -> Result<&str> {
    ensure!(
        node.children.is_empty() && node.attributes.is_empty(),
        "<{}> carries markup the chart writer does not place",
        node.name
    );
    Ok(node.text.as_str())
}

/// The child elements of one element, consumed in the order the decoder
/// writes them. Every name is taken under the chart namespace prefix.
struct Children<'a> {
    owner: &'a str,
    nodes: &'a [XmlNode],
    next: usize,
}

impl<'a> Children<'a> {
    fn chart(node: &'a XmlNode) -> Result<Self> {
        ensure!(
            node.text.trim().is_empty(),
            "<{}> mixes text with its elements",
            node.name
        );
        ensure!(
            node.attributes.is_empty() || node.name == "Settings",
            "<{}> names attributes the chart writer has not measured",
            node.name
        );
        Ok(Self {
            owner: &node.name,
            nodes: &node.children,
            next: 0,
        })
    }

    fn optional(&mut self, name: &str) -> Option<&'a XmlNode> {
        let node = self.nodes.get(self.next)?;
        (node.name.strip_prefix("d4p1:") == Some(name)).then(|| {
            self.next += 1;
            node
        })
    }

    fn required(&mut self, name: &str) -> Result<&'a XmlNode> {
        match self.optional(name) {
            Some(node) => Ok(node),
            None => match self.nodes.get(self.next) {
                Some(found) => Err(anyhow!(
                    "<{}> spells <{}> where the chart writer expects <d4p1:{name}>",
                    self.owner,
                    found.name
                )),
                None => Err(anyhow!("<{}> ends before <d4p1:{name}>", self.owner)),
            },
        }
    }

    fn finish(&self) -> Result<()> {
        match self.nodes.get(self.next) {
            Some(extra) => Err(anyhow!(
                "<{}> names <{}>, which the chart writer cannot place",
                self.owner,
                extra.name
            )),
            None => Ok(()),
        }
    }
}

/// The `<Settings>` element, which must declare the chart namespace under
/// `d4p1` and the expected `xsi:type`.
fn parse_settings(xml: &str, expected_type: &str) -> Result<XmlNode> {
    let root = parse_xml(xml)?;
    ensure!(
        root.name == "Settings"
            && attributes_are(
                &root,
                &[("xmlns:d4p1", CHART_NAMESPACE), ("xsi:type", expected_type)]
            ),
        "the attribute's <Settings> is not a {expected_type} the chart writer reads"
    );
    Ok(root)
}

const MAX_XML_DEPTH: usize = 32;
const MAX_XML_NODES: usize = 200_000;

fn parse_xml(xml: &str) -> Result<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut nodes = 0usize;
    loop {
        let event = reader.read_event()?;
        let is_eof = matches!(event, Event::Eof);
        let mut text = None;
        match event {
            Event::Start(start) => {
                nodes += 1;
                ensure!(
                    root.is_none() && stack.len() < MAX_XML_DEPTH && nodes <= MAX_XML_NODES,
                    "the chart XML is not one bounded element"
                );
                stack.push(node_from_start(&start)?);
            }
            Event::Empty(start) => {
                nodes += 1;
                ensure!(
                    root.is_none() && nodes <= MAX_XML_NODES,
                    "the chart XML is not one bounded element"
                );
                let node = node_from_start(&start)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| anyhow!("the chart XML closes an element it never opened"))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Text(event) => {
                let raw = event.xml_content()?;
                text = Some(unescape(raw.as_ref())?.into_owned());
            }
            Event::CData(event) => text = Some(event.xml_content()?.into_owned()),
            Event::GeneralRef(reference) => {
                text = Some(match reference.resolve_char_ref()? {
                    Some(character) => character.to_string(),
                    None => {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("the chart XML names the entity &{entity};"))?
                            .to_string()
                    }
                });
            }
            Event::Eof => {}
            _ => bail!("the chart XML carries markup other than elements and text"),
        }
        if let Some(text) = text {
            match stack.last_mut() {
                Some(node) => node.text.push_str(&text),
                None => ensure!(
                    text.trim().is_empty(),
                    "the chart XML carries text outside its element"
                ),
            }
        }
        if is_eof {
            break;
        }
    }
    ensure!(stack.is_empty(), "the chart XML leaves an element open");
    root.ok_or_else(|| anyhow!("the chart XML holds no element"))
}

fn node_from_start(start: &BytesStart<'_>) -> Result<XmlNode> {
    let name = std::str::from_utf8(start.name().as_ref())?.to_string();
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute?;
        let key = std::str::from_utf8(attribute.key.as_ref())?.to_string();
        attributes.push((key, attribute.unescape_value()?.into_owned()));
    }
    Ok(XmlNode {
        name,
        attributes,
        ..XmlNode::default()
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{format_form_embedded_chart, format_form_embedded_gantt_chart, platform_double};

    macro_rules! evidence {
        ($path:literal) => {
            include_str!(concat!(
                "../../../tests/fixtures/native-evidence/8.3.27.2214/",
                $path
            ))
        };
    }

    /// `(name, stored member 14, native <Settings>)`: the platform-loaded
    /// `form-chart-*` seeds -- each a load from XML through ibcmd
    /// 8.3.27.2214, so the stored record is what a load writes.
    const LOADED_CHARTS: &[(&str, &str, &str)] = &[
        (
            "zero-series",
            evidence!("form-chart-series-count/raw/zero-series.txt"),
            evidence!("form-chart-series-count/native/zero-series-settings.xml"),
        ),
        (
            "one-series",
            evidence!("form-chart-series-count/raw/one-series.txt"),
            evidence!("form-chart-series-count/native/one-series-settings.xml"),
        ),
        (
            "four-series",
            evidence!("form-chart-series-count/raw/four-series.txt"),
            evidence!("form-chart-series-count/native/four-series-settings.xml"),
        ),
        (
            "gauge-type",
            evidence!("form-chart-linetype-splinemode/raw/gauge-type.txt"),
            evidence!("form-chart-linetype-splinemode/native/gauge-type-settings.xml"),
        ),
        (
            "line-type",
            evidence!("form-chart-linetype-splinemode/raw/line-type.txt"),
            evidence!("form-chart-linetype-splinemode/native/line-type-settings.xml"),
        ),
        (
            "spline-mode",
            evidence!("form-chart-linetype-splinemode/raw/spline-mode.txt"),
            evidence!("form-chart-linetype-splinemode/native/spline-mode-settings.xml"),
        ),
        (
            "legend-bottom",
            evidence!("form-chart-placement-and-showmodes/raw/legend-bottom.txt"),
            evidence!("form-chart-placement-and-showmodes/native/legend-bottom-settings.xml"),
        ),
        (
            "points-drop-lines-show-mode",
            evidence!("form-chart-placement-and-showmodes/raw/points-drop-lines-show-mode.txt"),
            evidence!(
                "form-chart-placement-and-showmodes/native/points-drop-lines-show-mode-settings.xml"
            ),
        ),
        (
            "title-area-placement",
            evidence!("form-chart-placement-and-showmodes/raw/title-area-placement.txt"),
            evidence!(
                "form-chart-placement-and-showmodes/native/title-area-placement-settings.xml"
            ),
        ),
        (
            "values-drop-lines-show-mode",
            evidence!("form-chart-placement-and-showmodes/raw/values-drop-lines-show-mode.txt"),
            evidence!(
                "form-chart-placement-and-showmodes/native/values-drop-lines-show-mode-settings.xml"
            ),
        ),
        (
            "values-tooltip-show-mode",
            evidence!("form-chart-placement-and-showmodes/raw/values-tooltip-show-mode.txt"),
            evidence!(
                "form-chart-placement-and-showmodes/native/values-tooltip-show-mode-settings.xml"
            ),
        ),
        (
            "points-scale-full",
            evidence!("form-chart-points-scale/raw/full.txt"),
            evidence!("form-chart-points-scale/native/full-settings.xml"),
        ),
        (
            "points-scale-label-color-only",
            evidence!("form-chart-points-scale/raw/label-color-only.txt"),
            evidence!("form-chart-points-scale/native/label-color-only-settings.xml"),
        ),
        (
            "points-scale-minimal",
            evidence!("form-chart-points-scale/raw/minimal.txt"),
            evidence!("form-chart-points-scale/native/minimal-settings.xml"),
        ),
    ];

    /// Designer-saved records: UT's `ПроверкаКонтрагента` and ERP УХ's
    /// two-series, one-point mobile chart.
    const SAVED_CHARTS: &[(&str, &str, &str)] = &[
        (
            "provkontr-gauge-1",
            evidence!("form-chart-provkontr-gauge/raw/gauge-1.txt"),
            evidence!("form-chart-provkontr-gauge/native/gauge-1-settings.xml"),
        ),
        (
            "provkontr-gauge-2",
            evidence!("form-chart-provkontr-gauge/raw/gauge-2.txt"),
            evidence!("form-chart-provkontr-gauge/native/gauge-2-settings.xml"),
        ),
        (
            "provkontr-target",
            evidence!("form-chart-provkontr-target/raw/target.txt"),
            evidence!("form-chart-provkontr-target/native/target-settings.xml"),
        ),
        (
            "uh-mobile-point",
            evidence!("form-chart-uh-edge/raw/mobile-point.txt"),
            evidence!("form-chart-uh-edge/native/mobile-point-settings.xml"),
        ),
    ];

    /// A version-73 record (ERP УХ `Reports/МатрицаРисков`, Designer
    /// history): the writer stores version 74, which the exporter reads the
    /// same.
    const OLD_SHAPE_CHART: (&str, &str) = (
        evidence!("form-chart-uh-edge/raw/bubble.txt"),
        evidence!("form-chart-uh-edge/native/bubble-settings.xml"),
    );

    const SAVED_GANTT_CHARTS: &[(&str, &str, &str)] = &[
        (
            "palette32-single-border",
            evidence!("form-gantt-chart-uh/raw/palette32-single-border.txt"),
            evidence!("form-gantt-chart-uh/native/palette32-single-border-settings.xml"),
        ),
        (
            "hex-scale-colors",
            evidence!("form-gantt-chart-uh/raw/hex-scale-colors.txt"),
            evidence!("form-gantt-chart-uh/native/hex-scale-colors-settings.xml"),
        ),
    ];

    /// The `<Settings>` element as the file spells it.
    fn element(native: &str) -> &str {
        let start = native
            .find("<Settings")
            .expect("fixture holds a <Settings> element");
        let end = native
            .rfind("</Settings>")
            .expect("fixture closes its <Settings>")
            + "</Settings>".len();
        &native[start..end]
    }

    fn crlf(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    }

    fn flat(text: &str) -> String {
        text.replace(['\r', '\n'], "")
    }

    // -- A stored value as a tree, and its members labelled by slot. --------

    #[derive(Clone, Debug, PartialEq)]
    enum Member {
        Leaf(String),
        List(Vec<Member>),
    }

    impl Member {
        fn list(&self) -> &[Member] {
            match self {
                Member::List(items) => items,
                Member::Leaf(text) => panic!("{text} is not a list"),
            }
        }

        fn leaf(&self) -> &str {
            match self {
                Member::Leaf(text) => text,
                Member::List(_) => panic!("a list is not a leaf"),
            }
        }

        fn show(&self) -> String {
            match self {
                Member::Leaf(text) => text.clone(),
                Member::List(items) => format!(
                    "{{{}}}",
                    items.iter().map(Member::show).collect::<Vec<_>>().join(",")
                ),
            }
        }
    }

    fn parse(text: &str) -> Member {
        let flat = flat(text);
        let bytes = flat.trim().as_bytes();
        let mut position = 0;
        let member = parse_list(bytes, &mut position);
        assert_eq!(position, bytes.len(), "trailing text after the value");
        member
    }

    fn parse_list(bytes: &[u8], position: &mut usize) -> Member {
        assert_eq!(bytes[*position], b'{');
        *position += 1;
        let mut items = Vec::new();
        loop {
            if bytes[*position] == b'{' {
                items.push(parse_list(bytes, position));
            } else {
                let start = *position;
                while bytes[*position] != b',' && bytes[*position] != b'}' {
                    if bytes[*position] == b'"' {
                        *position += 1;
                        loop {
                            if bytes[*position] == b'"' {
                                if bytes.get(*position + 1) == Some(&b'"') {
                                    *position += 2;
                                    continue;
                                }
                                break;
                            }
                            *position += 1;
                        }
                    }
                    *position += 1;
                }
                items.push(Member::Leaf(
                    String::from_utf8(bytes[start..*position].to_vec()).expect("utf-8"),
                ));
            }
            match bytes[*position] {
                b',' => *position += 1,
                b'}' => {
                    *position += 1;
                    return Member::List(items);
                }
                other => panic!("unexpected byte {other}"),
            }
        }
    }

    /// Every member of a chart record `{74,…}`, labelled the way the decoder
    /// names it: `h<n>` the head, `s<k>.<m>`/`ex.<m>` the series records,
    /// `p<k>.<m>` the points, `t<n>` a tail member by its fixed offset, and
    /// `rd`/`id`/`sc`/`lp`/`lg`/`sp` the lists that grow the tail.
    fn chart_labels(data: &[Member], prefix: &str, out: &mut Vec<(String, String)>) {
        let push = |label: String, member: &Member, out: &mut Vec<(String, String)>| {
            out.push((format!("{prefix}{label}"), member.show()));
        };
        let series: usize = data[4].leaf().parse().expect("series count");
        let mut at = 0;
        for head in 0..5 {
            push(format!("h{head}"), &data[at], out);
            at += 1;
        }
        for index in 0..=series {
            let name = if index == series {
                "ex".to_string()
            } else {
                format!("s{index}")
            };
            for member in 0..11 {
                push(format!("{name}.{member}"), &data[at], out);
                at += 1;
            }
        }
        push("isPointsDesign".into(), &data[at], out);
        push("realPointCount".into(), &data[at + 1], out);
        let points: usize = data[at + 1].leaf().parse().expect("point count");
        at += 2;
        for index in 0..points {
            for member in 0..11 {
                push(format!("p{index}.{member}"), &data[at], out);
                at += 1;
            }
        }
        let tail = &data[at..];
        let growth = 3 * series * points;
        let ids: usize = tail[123 + growth].leaf().parse().expect("id count");
        let mut labels = Vec::new();
        labels.extend((0..100).map(|slot| format!("t{slot}")));
        labels.extend((0..growth).map(|index| format!("rd{index}")));
        labels.extend((100..124).map(|slot| format!("t{slot}")));
        labels.extend((0..ids).map(|index| format!("id{index}")));
        labels.extend((0..=series).map(|index| format!("sc{index}")));
        labels.extend((126..147).map(|slot| format!("t{slot}")));
        labels.extend((0..points).map(|index| format!("lp{index}")));
        labels.extend((0..=series).map(|index| format!("lg{index}")));
        labels.extend((148..186).map(|slot| format!("t{slot}")));
        labels.extend((0..series * points).map(|index| format!("sp{index}")));
        labels.extend((186..197).map(|slot| format!("t{slot}")));
        for (label, member) in labels.into_iter().zip(tail) {
            push(label, member, out);
        }
    }

    /// The labelled members of a whole member-14 value.
    fn labels(value: &Member) -> Vec<(String, String)> {
        let outer = value.list();
        let holder = outer[3].list();
        let mut out = vec![
            (
                "outer".to_string(),
                format!("{},{},{}", outer[0].show(), outer[1].show(), outer[2].show()),
            ),
            (
                "holder".to_string(),
                format!("{},{}", holder[0].show(), holder[1].show()),
            ),
        ];
        if outer[2].leaf() == "\"Chart\"" {
            out.push(("holder.2".to_string(), holder[2].show()));
            chart_labels(holder[3].list(), "", &mut out);
        } else {
            for (index, member) in holder[2].list().iter().enumerate() {
                if index == 1 {
                    let triple = member.list();
                    out.push((
                        "w1.head".to_string(),
                        format!("{},{}", triple[0].show(), triple[1].show()),
                    ));
                    chart_labels(triple[2].list(), "chart.", &mut out);
                } else {
                    out.push((format!("w{index}"), member.show()));
                }
            }
        }
        out
    }

    /// The members `written` holds differently from `stored`, by label.
    fn differences(stored: &Member, written: &Member) -> Vec<String> {
        let stored = labels(stored);
        let written = labels(written);
        let stored_map: BTreeMap<&str, &str> = stored
            .iter()
            .map(|(label, value)| (label.as_str(), value.as_str()))
            .collect();
        let written_map: BTreeMap<&str, &str> = written
            .iter()
            .map(|(label, value)| (label.as_str(), value.as_str()))
            .collect();
        let mut out = Vec::new();
        for (label, value) in &stored {
            match written_map.get(label.as_str()) {
                Some(other) if other == value => {}
                Some(other) => out.push(format!("{label}: stored {value} / written {other}")),
                None => out.push(format!("{label}: stored {value} / not written")),
            }
        }
        for (label, value) in &written {
            if !stored_map.contains_key(label.as_str()) {
                out.push(format!("{label}: not stored / written {value}"));
            }
        }
        out
    }

    /// The two layout runs no XML carries: the platform lays the legend out
    /// on load, and the writer stores the untouched layout.
    fn is_layout_member(difference: &str) -> bool {
        let label = difference.split(':').next().unwrap_or(difference);
        let slot = label.rsplit('.').next().unwrap_or(label);
        slot.strip_prefix('t')
            .and_then(|number| number.parse::<usize>().ok())
            .is_some_and(|slot| (82..94).contains(&slot) || (148..160).contains(&slot))
    }

    /// Everything a load stores that no XML carries -- the version, the
    /// scale-id list, the border members, the palette copy, every series'
    /// cached colour and marker -- is what the writer stores too; only the
    /// legend layout the platform computes on load is left out.
    #[test]
    fn writes_what_the_platform_stores_but_the_legend_layout() {
        for (name, raw, native) in LOADED_CHARTS.iter().chain(SAVED_CHARTS) {
            let written = format_form_embedded_chart(element(native))
                .unwrap_or_else(|error| panic!("{name}: {error:#}"));
            let rest = differences(&parse(raw), &parse(&written))
                .into_iter()
                .filter(|difference| !is_layout_member(difference))
                .collect::<Vec<_>>();
            assert!(rest.is_empty(), "{name}: {rest:#?}");
        }
    }

    #[test]
    fn writes_the_untouched_layout_records_byte_for_byte() {
        for (name, raw, native) in [&LOADED_CHARTS[0], &SAVED_CHARTS[0], &SAVED_CHARTS[1]] {
            let written = format_form_embedded_chart(element(native))
                .unwrap_or_else(|error| panic!("{name}: {error:#}"));
            assert_eq!(written, flat(raw), "{name}");
        }
    }

    #[test]
    fn writes_the_two_gantt_fixtures_byte_for_byte() {
        for (name, raw, native) in SAVED_GANTT_CHARTS {
            let written = format_form_embedded_gantt_chart(element(native))
                .unwrap_or_else(|error| panic!("{name}: {error:#}"));
            assert_eq!(
                differences(&parse(raw), &parse(&written)),
                Vec::<String>::new(),
                "{name}"
            );
            assert_eq!(written, flat(raw), "{name}");
        }
    }

    #[test]
    fn every_written_chart_exports_back_to_its_own_xml() {
        let charts = LOADED_CHARTS
            .iter()
            .chain(SAVED_CHARTS)
            .map(|(_, _, native)| *native)
            .chain([OLD_SHAPE_CHART.1]);
        for native in charts {
            let written = format_form_embedded_chart(element(native)).expect("chart is written");
            let rendered = crate::mssql_dump::render_form_chart_settings_value(&written)
                .expect("the exporter reads the written chart");
            assert_eq!(rendered, crlf(native));
        }
        for (name, _, native) in SAVED_GANTT_CHARTS {
            let written = format_form_embedded_gantt_chart(element(native))
                .unwrap_or_else(|error| panic!("{name}: {error:#}"));
            let rendered = crate::mssql_dump::render_form_gantt_chart_settings_value(&written)
                .expect("the exporter reads the written Gantt chart");
            assert_eq!(rendered, crlf(native), "{name}");
        }
    }

    #[test]
    fn writes_the_modern_shape_for_an_old_record() {
        let (raw, native) = OLD_SHAPE_CHART;
        let written = parse(&format_form_embedded_chart(element(native)).expect("chart is written"));
        assert_eq!(written.list()[3].list()[3].list()[0].leaf(), "74");
        assert_eq!(parse(raw).list()[3].list()[3].list()[0].leaf(), "73");
    }

    #[test]
    fn refuses_what_it_cannot_place() {
        let settings = element(LOADED_CHARTS[0].2);
        // An element the decoder never writes.
        let extra = settings.replace(
            "<d4p1:pointsAxis/>",
            "<d4p1:pointsAxis/>\r\n\t\t\t\t<d4p1:unknownMember>1</d4p1:unknownMember>",
        );
        assert_ne!(extra, settings);
        assert!(format_form_embedded_chart(&extra).is_err());
        // A chart type the decoder has no code for.
        let area = settings.replace(
            "<d4p1:chartType>Column3D</d4p1:chartType>",
            "<d4p1:chartType>Area</d4p1:chartType>",
        );
        assert_ne!(area, settings);
        assert!(format_form_embedded_chart(&area).is_err());
        // A style colour only the configuration could resolve.
        let styled = settings.replace(
            "<d4p1:scaleColor>#A9A9A9</d4p1:scaleColor>",
            "<d4p1:scaleColor>style:ЦветШкалыДиаграммы</d4p1:scaleColor>",
        );
        assert_ne!(styled, settings);
        assert!(format_form_embedded_chart(&styled).is_err());
        // The wrong kind.
        assert!(format_form_embedded_gantt_chart(settings).is_err());
        // A value the decoder does not write where it stands: an automatic
        // label colour is left out of the points scale.
        let scaled = element(LOADED_CHARTS[13].2);
        let auto_label = scaled.replace(
            "</d4p1:gridLine>\r\n\t\t\t\t</d4p1:pointsScale>",
            "</d4p1:gridLine>\r\n\t\t\t\t\t<d4p1:labelColor>auto</d4p1:labelColor>\r\n\t\t\t\t</d4p1:pointsScale>",
        );
        assert_ne!(auto_label, scaled);
        assert!(format_form_embedded_chart(&auto_label).is_err());
    }

    #[test]
    fn spells_a_fraction_the_way_the_platform_does() {
        assert_eq!(platform_double(0.0), "0");
        assert_eq!(platform_double(0.1), "1e-1");
        assert_eq!(platform_double(0.03), "3e-2");
        assert_eq!(platform_double(0.45), "4.5e-1");
        assert_eq!(platform_double(1.0), "1");
    }

    /// The whole ERP УХ population: every `<Settings xsi:type="d4p1:Chart">`
    /// and `d4p1:GanttChart` of the native tree beside the member 14 its
    /// stored form body holds (`findings/rt-embedded.md` §1.2), as
    /// `F:\ibcmd\lab\tools\agent-chart\extract.py` writes them out
    /// (`IBCMD_CHART_CORPUS` overrides the directory): `index.tsv` and, per
    /// attribute, `NN.settings.xml` and `NN.stored.txt`.
    ///
    /// Every attribute must be written -- which includes the exporter reading
    /// it back to its own XML. The stored comparison is reported rather than
    /// asserted: the corpus was saved by Designer, and the members it
    /// differs in are the ones the exporter never publishes.
    #[test]
    #[ignore = "reads the ERP УХ extraction under F:\\ibcmd\\lab"]
    fn rebuilds_the_erp_uh_chart_corpus() {
        let directory = std::env::var("IBCMD_CHART_CORPUS")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/corpus".to_string());
        let index = std::fs::read_to_string(format!("{directory}/index.tsv")).expect("index.tsv");
        let mut written_count = 0;
        let mut exact = 0;
        let mut refused = Vec::new();
        let mut report = String::new();
        for line in index.lines().filter(|line| !line.trim().is_empty()) {
            let fields = line.split('\t').collect::<Vec<_>>();
            let (tag, kind, path, name) = (fields[0], fields[1], fields[2], fields[3]);
            let settings = std::fs::read_to_string(format!("{directory}/{tag}.settings.xml"))
                .expect("settings");
            let stored =
                std::fs::read_to_string(format!("{directory}/{tag}.stored.txt")).expect("stored");
            let written = if kind == "gantt" {
                format_form_embedded_gantt_chart(&settings)
            } else {
                format_form_embedded_chart(&settings)
            };
            match written {
                Ok(written) => {
                    written_count += 1;
                    let differences = differences(&parse(&stored), &parse(&written));
                    if differences.is_empty() {
                        exact += 1;
                        report.push_str(&format!("{tag} {kind} {path} {name}: exact\n"));
                    } else {
                        report.push_str(&format!(
                            "{tag} {kind} {path} {name}: {} member(s) differ\n",
                            differences.len()
                        ));
                        for difference in differences {
                            report.push_str(&format!("    {difference}\n"));
                        }
                    }
                }
                Err(error) => {
                    report.push_str(&format!("{tag} {kind} {path} {name}: REFUSED {error:#}\n"));
                    refused.push(tag.to_string());
                }
            }
        }
        report.push_str(&format!(
            "written and exported back to their own XML: {written_count}; stored byte-exact: {exact}\n"
        ));
        println!("{report}");
        if let Ok(path) = std::env::var("IBCMD_CHART_REPORT") {
            std::fs::write(path, &report).expect("report");
        }
        assert!(refused.is_empty(), "refused: {refused:?}");
    }

    /// The whole form, not just its chart. Each ERP УХ form that holds a chart
    /// attribute, as the writer compiled it (`audit-native-form-writer` with
    /// `IBCMD_RS_WRITE_BODIES_DIR`; `IBCMD_CHART_BODIES` names that
    /// directory), and its stored body are both rendered by the exporter
    /// against the offline context of a saved dump (`IBCMD_CHART_RUN_ROOT`, a
    /// directory whose `candidate_dump` is that dump). A compiled body that
    /// renders what the stored body renders exports back to the native
    /// Form.xml wherever the stored body does.
    #[test]
    #[ignore = "reads the ERP УХ dump and compiled bodies under F:\\ibcmd\\lab"]
    fn chart_forms_export_as_their_stored_bodies_do() {
        let corpus = std::env::var("IBCMD_CHART_CORPUS")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/corpus".to_string());
        let bodies = std::env::var("IBCMD_CHART_BODIES")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/audit2/bodies".to_string());
        let run_root = std::env::var("IBCMD_CHART_RUN_ROOT")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/offline_root".to_string());
        let context = crate::mssql_dump::offline_context::OfflineFormContextFactory::from_run_root(
            std::path::Path::new(&run_root),
            None,
        )
        .expect("offline context");
        let render = |path: &str, uuid: &str| -> Option<String> {
            let text = std::fs::read_to_string(path).ok()?;
            let body = crate::module_blob::parse_form_body_plain(text.trim_start_matches('\u{feff}'))
                .expect("form body parses");
            crate::mssql_dump::render_form_body_xml_offline(&body, &context, uuid)
        };
        let index = std::fs::read_to_string(format!("{corpus}/index.tsv")).expect("index.tsv");
        let mut seen = Vec::new();
        let mut report = String::new();
        let mut compiled = 0;
        let mut same = 0;
        let mut different = Vec::new();
        for line in index.lines().filter(|line| !line.trim().is_empty()) {
            let fields = line.split('\t').collect::<Vec<_>>();
            let (path, uuid, stored_path) = (fields[2], fields[4], fields[5]);
            if seen.contains(&uuid) {
                continue;
            }
            seen.push(uuid);
            let stored = render(stored_path, uuid).expect("the stored body exports");
            let Some(written) = render(&format!("{bodies}/{uuid}.0.txt"), uuid) else {
                report.push_str(&format!("{path}: not compiled\n"));
                continue;
            };
            compiled += 1;
            if written == stored {
                same += 1;
                report.push_str(&format!("{path}: exports as the stored body does\n"));
            } else {
                let at = written
                    .lines()
                    .zip(stored.lines())
                    .position(|(left, right)| left != right)
                    .unwrap_or(0);
                report.push_str(&format!(
                    "{path}: differs at line {}: {:?} / stored {:?}\n",
                    at + 1,
                    written.lines().nth(at),
                    stored.lines().nth(at)
                ));
                different.push(path.to_string());
            }
        }
        report.push_str(&format!(
            "forms: {}; compiled: {compiled}; export as the stored body: {same}\n",
            seen.len()
        ));
        println!("{report}");
        if let Ok(path) = std::env::var("IBCMD_CHART_REPORT") {
            std::fs::write(path, &report).expect("report");
        }
        assert!(different.is_empty(), "differ: {different:#?}");
    }
}
