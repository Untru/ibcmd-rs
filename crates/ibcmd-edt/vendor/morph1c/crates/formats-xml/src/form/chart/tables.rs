//! CHART · таблица знаний (spec-data): const `*_FIELDS` в DESIGNER-порядке 8.3.27 + `edt_rank`,
//! плюс резолверы `rows`/`top_tbl`/`row_by_name`/`row_by_designer_name`. Только ДАННЫЕ — без кодека.

use super::*;

/// Chart-таблица в DESIGNER-порядке 8.3.27 (= порядок cf-клеток головы {74,…}); EDT-only
/// строки — В ХВОСТЕ (designer их не сериализует, их designer-позиция не существует).
/// `edt_rank` = позиция поля в xcore-модели `Chart` (168 полей; невитнесснутые в таблицу
/// НЕ входят — незнакомое имя ⇒ отказ §1.0).
const CHART_FIELDS: &[Row] = &[
    Row::new("seriesCurId", Shape::Int, 0),
    Row::new("pointsCurId", Shape::Int, 1),
    Row::new("isSeriesDesign", Shape::Bool, 2),
    Row::new("realSeriesCount", Shape::Int, 3),
    Row::new("realSeriesData", Shape::SeriesItems, 4),
    Row::new("realExSeriesData", Shape::SeriesItem, 5),
    Row::new("isPointsDesign", Shape::Bool, 9),
    Row::new("realPointCount", Shape::Int, 6),
    Row::new("realPointData", Shape::PointItems, 7),
    Row::new("curSeries", Shape::Int, 10),
    Row::new("curPoint", Shape::Int, 11),
    Row::new("chartType", Shape::Enum, 12).omit("Line"),
    Row::new("circleLabelType", Shape::Enum, 13).omit("None"),
    Row::new("labelsDelimiter", Shape::Str, 14),
    // Edge → опущено (sezon/nastr/vypoln); Auto → эмитится (planir) ⇒ дефолт модели = Edge.
    Row::new("labelsLocation", Shape::Enum, 15).omit("Edge"),
    Row::new("lbFormat", Shape::Loc, 16),
    Row::new("lbpFormat", Shape::Loc, 17),
    Row::new("labelsColor", Shape::Color, 18),
    Row::new("labelsFont", Shape::Font, 19),
    Row::new("transparentLabelsBkg", Shape::Bool, 20),
    Row::new("labelsBkgColor", Shape::Color, 21),
    Row::new("labelsBorder", Shape::Border, 22),
    Row::new("labelsBorderColor", Shape::Color, 23),
    Row::new("circleExpandMode", Shape::Enum, 24).omit("None"),
    Row::new("chart3Dcrd", Shape::Enum, 25).omit("SouthWest"),
    Row::new("title", Shape::Loc, 26),
    Row::new("isShowTitle", Shape::Bool, 27),
    Row::new("isShowLegend", Shape::Bool, 28),
    Row::new("ttlBorder", Shape::Border, 29),
    Row::new("ttlBorderColor", Shape::Color, 30),
    Row::new("lgBorder", Shape::Border, 31),
    Row::new("lgBorderColor", Shape::Color, 32),
    Row::new("chBorder", Shape::Border, 33),
    Row::new("chBorderColor", Shape::Color, 34),
    Row::new("transparent", Shape::Bool, 35),
    Row::new("bkgColor", Shape::Color, 36),
    Row::new("isTrnspTtl", Shape::Bool, 37),
    Row::new("ttlColor", Shape::Color, 38),
    Row::new("isTrnspLeg", Shape::Bool, 39),
    Row::new("legColor", Shape::Color, 40),
    Row::new("isTrnspCh", Shape::Bool, 41),
    Row::new("chColor", Shape::Color, 42),
    Row::new("ttlTxtColor", Shape::Color, 43),
    Row::new("legTxtColor", Shape::Color, 44),
    Row::new("chTxtColor", Shape::Color, 45),
    Row::new("ttlFont", Shape::Font, 46),
    Row::new("legFont", Shape::Font, 47),
    Row::new("chFont", Shape::Font, 48),
    Row::new("isShowScale", Shape::Bool, 49),
    Row::new("isShowScaleVL", Shape::Bool, 50),
    Row::new("isShowSeriesScale", Shape::Bool, 51),
    Row::new("isShowPointsScale", Shape::Bool, 52),
    Row::new("isShowValuesScale", Shape::Bool, 53),
    Row::new("vsFormat", Shape::Loc, 54),
    // Auto эмитится ВСЕГДА (4/4 витнесса) ⇒ дефолт модели ≠ Auto, литерала омиссии нет.
    Row::new("xLabelsOrientation", Shape::Enum, 55),
    Row::new("scaleLine", Shape::Line, 56),
    Row::new("scaleColor", Shape::Color, 57),
    Row::new("isAutoSeriesName", Shape::Bool, 58),
    Row::new("isAutoPointName", Shape::Bool, 59),
    Row::new("maxMode", Shape::Enum, 60).omit("NotDefined"),
    Row::new("maxSeries", Shape::Int, 61),
    Row::new("maxSeriesPrc", Shape::Int, 62),
    Row::new("spaceMode", Shape::Enum, 63),
    // BigDecimal: xcore эмитит и ноль (витнесс `<baseVal>0</baseVal>` во всех сайдкарах).
    Row::new("baseVal", Shape::Int, 64).always(),
    Row::new("isOutline", Shape::Bool, 65),
    Row::new("realPiePoint", Shape::Int, 66),
    Row::new("realStockSeries", Shape::Int, 67),
    Row::new("isLight", Shape::Bool, 68),
    Row::new("isGradient", Shape::Bool, 69),
    // designer держит между isGradient и hideBaseVal; EDT-модель — после dataSourceDescription.
    Row::new("isTransposition", Shape::Bool, 104),
    Row::new("hideBaseVal", Shape::Bool, 70),
    Row::new("dataTable", Shape::Bool, 71),
    Row::new("dtVerLines", Shape::Bool, 72),
    Row::new("dtHorLines", Shape::Bool, 73),
    Row::new("dtHAlign", Shape::Enum, 74),
    Row::new("dtFormat", Shape::Loc, 75),
    Row::new("dtKeys", Shape::Bool, 76),
    // Palette8 → опущено (planir ×2), Auto/Palette32 → эмитится ⇒ дефолт модели = Palette8.
    Row::new("paletteKind", Shape::Enum, 77).omit("Palette8"),
    Row::new("animation", Shape::Enum, 119).omit("Auto"),
    Row::new("rebuildTime", Shape::Int, 120),
    Row::new("isTransposed", Shape::Bool, 105),
    Row::new("autoTransposition", Shape::Bool, 107),
    Row::new("legendScrollEnable", Shape::Bool, 78),
    Row::new("surfaceColor", Shape::Color, 79),
    Row::new("radarScaleType", Shape::Enum, 80).omit("Circle"),
    Row::new("gaugeValuesPresentation", Shape::Enum, 81).omit("Needle"),
    Row::new("gaugeQualityBands", Shape::Nested(Tbl::GaugeBands), 82),
    Row::new("beginGaugeAngle", Shape::Int, 83),
    Row::new("endGaugeAngle", Shape::Int, 84),
    Row::new("gaugeThickness", Shape::Int, 85),
    Row::new("gaugeLabelsLocation", Shape::Enum, 86),
    Row::new("gaugeLabelsArcDirection", Shape::Bool, 87),
    Row::new("gaugeBushThickness", Shape::Int, 88),
    Row::new("gaugeBushColor", Shape::Color, 89),
    Row::new("autoMaxValue", Shape::Bool, 90),
    Row::new("userMaxValue", Shape::Dec, 91),
    Row::new("autoMinValue", Shape::Bool, 92),
    Row::new("userMinValue", Shape::Dec, 93),
    Row::new("elementsIsInit", Shape::Bool, 94),
    Row::new("titleIsInit", Shape::Bool, 95),
    Row::new("legendIsInit", Shape::Bool, 96),
    Row::new("chartIsInit", Shape::Bool, 97),
    Row::new("elementsChart", Shape::Rect, 98),
    Row::new("elementsLegend", Shape::Rect, 99),
    Row::new("elementsTitle", Shape::Rect, 100),
    Row::new("borderColor", Shape::Color, 101),
    Row::new("border", Shape::Border, 102),
    Row::new("dataSourceDescription", Shape::Str, 103),
    Row::new("isDataSourceMode", Shape::Bool, 106),
    // Дефолт модели = true (charts.jsonl: default='true'; sezon/vypoln: true → опущено).
    Row::new("isRandomizedNewValues", Shape::Bool, 108).omit("true"),
    Row::new("realDataItems", Shape::DataItems, 8),
    Row::new("splineMode", Shape::Enum, 109),
    Row::new("splineStrain", Shape::Int, 110),
    Row::new("translucencePercent", Shape::Int, 112),
    Row::new("funnelNeckHeightPercent", Shape::Dec, 113),
    Row::new("funnelNeckWidthPercent", Shape::Dec, 114),
    Row::new("funnelGapSumPercent", Shape::Dec, 115),
    Row::new("multiStageLinkLine", Shape::Line, 117),
    Row::new("multiStageLinkColor", Shape::Color, 118),
    Row::new("valuesAxis", Shape::Nested(Tbl::Axis), 123),
    Row::new("pointsAxis", Shape::Nested(Tbl::Axis), 124),
    Row::new("pointsScale", Shape::Nested(Tbl::Scale), 135),
    Row::new("valuesScale", Shape::Nested(Tbl::Scale), 136),
    Row::new("seriesScale", Shape::Nested(Tbl::Scale), 137),
    Row::new("legendPlacement", Shape::Enum, 159),
    Row::new("plotAreaPlacement", Shape::Enum, 160),
    Row::new("titleAreaPlacement", Shape::Enum, 161),
    Row::new("valuesToolTipShowMode", Shape::Enum, 152),
    Row::new("pointsDropLinesShowMode", Shape::Enum, 155),
    Row::new("valuesDropLinesShowMode", Shape::Enum, 156),
    Row::new("colorPaletteDescription", Shape::Nested(Tbl::Cpd), 144),
    // ── EDT-only (материализованные xcore-константы; синтез в транскоде) ──
    Row::new("translucenceMode", Shape::Enum, 111).edt_only(),
    Row::new("bubbleSizeValueSource", Shape::Enum, 127).edt_only(),
    Row::new("bubbleSizeCommonSeries", Shape::Int, 128).edt_only(),
    Row::new("bubbleSizing", Shape::Enum, 129).edt_only(),
    Row::new(
        "referenceBandsColorPaletteDescription",
        Shape::Nested(Tbl::Cpd),
        145,
    )
    .edt_only(),
    Row::new(
        "valuesReferenceLines",
        Shape::Nested(Tbl::ReferenceLines),
        146,
    )
    .edt_only(),
    Row::new(
        "pointsReferenceLines",
        Shape::Nested(Tbl::ReferenceLines),
        147,
    )
    .edt_only(),
    Row::new(
        "valuesReferenceBands",
        Shape::Nested(Tbl::ReferenceBands),
        148,
    )
    .edt_only(),
    Row::new(
        "pointsReferenceBands",
        Shape::Nested(Tbl::ReferenceBands),
        149,
    )
    .edt_only(),
    Row::new("additionalValuesScale", Shape::Nested(Tbl::Scale), 162).edt_only(),
    Row::new("additionalValuesAxis", Shape::Nested(Tbl::Axis), 163).edt_only(),
    Row::new("multiStageLinkMode", Shape::Enum, 116),
    Row::new("seriesOrderInLegend", Shape::Enum, 122),
    Row::new("pointsAxisValuesSource", Shape::Enum, 125),
    Row::new("pointsAxisSeries", Shape::Int, 126),
    Row::new("bubbleChartNegativeValuesShowMode", Shape::Enum, 130),
    Row::new("defaultBubbleSize", Shape::Int, 131),
    Row::new("minBubbleSize", Shape::Int, 132),
    Row::new("maxBubbleSize", Shape::Int, 133),
    Row::new("pointsConnection", Shape::Enum, 134),
    Row::new("barChartPointsOrder", Shape::Enum, 138),
    Row::new("gradientPaletteMaxColors", Shape::Int, 139),
    Row::new("gradientPaletteStartColor", Shape::Color, 140),
    Row::new("gradientPaletteEndColor", Shape::Color, 141),
    Row::new("customPalette", Shape::Colors, 142),
    Row::new("nonnumericValuesUse", Shape::Enum, 150),
    Row::new("pointConnectionAcrossSkippedValues", Shape::Enum, 151),
    Row::new("valuesToolTipFillType", Shape::Enum, 153),
    Row::new("selectionMode", Shape::Enum, 154),
    Row::new("distributedKey", Shape::Str, 157),
    Row::new("innerRadiusDonutChart", Shape::Int, 158),
    Row::new("valuesEditMode", Shape::Enum, 164),
    Row::new("startAnglePieChart", Shape::Int, 165),
    Row::new("finishAnglePieChart", Shape::Int, 166),
    Row::new("isometricDepth", Shape::Int, 167),
    Row::new("trendLinesArray", Shape::Items(Tbl::TrendArray), 142),
];

/// GanttChart-обёртка: designer-порядок == относительному порядку EDT-модели (сверено по
/// planir-витнессу; невитнесснутые поля модели — interval/value/link — в таблицу не входят).
const GANTT_FIELDS: &[Row] = &[
    Row::new("chart", Shape::ChartTable, 0),
    Row::new("points", Shape::Nested(Tbl::GPoints), 1),
    Row::new("series", Shape::Nested(Tbl::GSeries), 2),
    Row::new("drawEmpty", Shape::Bool, 5),
    Row::new("timeScale", Shape::Nested(Tbl::TimeScale), 6),
    Row::new("keepScaleVariant", Shape::Enum, 7),
    Row::new("fixedVariantMeasure", Shape::Enum, 8),
    Row::new("fixedVariantInterval", Shape::Int, 9),
    Row::new("autoFullInterval", Shape::Bool, 10),
    Row::new("fullIntervalBegin", Shape::DateTime, 11),
    Row::new("fullIntervalEnd", Shape::DateTime, 12),
    Row::new("visualBegin", Shape::DateTime, 13),
    // Дефолт модели = Flat: 2 носителя ERP (Report.{МониторингЗаказаНормативныйГрафик,
    // мирМониторингЗаказа}.ФормаОтчета) несут designer-`<intervalDrawType>Flat` при ПОЛНОМ
    // отсутствии поля в EDT-сайдкаре; `Gradient` (Report.МониторингЗаказа) эмитится.
    Row::new("intervalDrawType", Shape::Enum, 14).omit("Flat"),
    Row::new("noneVariantChars", Shape::Dec, 15),
    Row::new("noneVariantMeasure", Shape::Enum, 16),
    Row::new("verticalStretch", Shape::Enum, 17).omit("None"),
    Row::new("verticalScrollEnable", Shape::Bool, 18),
    Row::new("showValueText", Shape::Enum, 19).omit("None"),
    Row::new("extTitle", Shape::Loc, 20),
    Row::new("outboundColor", Shape::Color, 21),
    Row::new("backIntervals", Shape::Nested(Tbl::BackIntervals), 22),
    Row::new("linksColor", Shape::Color, 24),
    Row::new("linksLine", Shape::Line, 25),
    Row::new("showPointsText", Shape::Enum, 26).omit("Auto"),
    Row::new("showData", Shape::Enum, 27).omit("Auto"),
    Row::new("textPlacement", Shape::Enum, 28).omit("Auto"),
    Row::new("intervalTextRepresentation", Shape::Enum, 29).omit("Auto"),
    Row::new("interval", Shape::Items(Tbl::GInterval), 3),
    Row::new("value", Shape::Items(Tbl::GValue), 4),
    Row::new("link", Shape::Items(Tbl::GLink), 23),
];

/// ChartScale (порядок диалектов СОВПАДАЕТ; сверено по 6 scale-витнессам).
const SCALE_FIELDS: &[Row] = &[
    Row::new("showTitle", Shape::Enum, 0),
    Row::new("titleTextSource", Shape::Enum, 1),
    Row::new("titleText", Shape::Loc, 2),
    Row::new("titleArea", Shape::Nested(Tbl::LabelArea), 3),
    Row::new("titlePlacement", Shape::Enum, 4),
    Row::new("gridLinesShowMode", Shape::Enum, 5),
    Row::new("scaleLabelLocation", Shape::Enum, 6),
    Row::new("gridLine", Shape::Line, 7),
    Row::new("labelFont", Shape::Font, 9),
    Row::new("labelColor", Shape::Color, 10),
    Row::new("labelOrientation", Shape::Enum, 11),
    Row::new("labelFormat", Shape::Loc, 12),
    Row::new("maxLabelRows", Shape::Int, 15),
    Row::new("labelAngle", Shape::Int, 16),
    Row::new("showInChart", Shape::Enum, 20),
    Row::new("gridLineColor", Shape::Color, 8),
    Row::new("scaleLine", Shape::Line, 13),
    Row::new("scaleLineColor", Shape::Color, 14),
    Row::new("scaleLocation", Shape::Enum, 17),
    Row::new("scaleStep", Shape::Int, 18),
    Row::new("scaleMarkLocation", Shape::Enum, 19),
];

/// LabelArea (titleArea шкал): designer несёт font/textColor/backColor/border/borderColor,
/// EDT добавляет материализованные location/transparent/marker/orientation.
const LABEL_AREA_FIELDS: &[Row] = &[
    Row::new("font", Shape::Font, 3),
    Row::new("textColor", Shape::Color, 4),
    Row::new("backColor", Shape::Color, 5),
    Row::new("border", Shape::Border, 7),
    Row::new("borderColor", Shape::Color, 8),
    Row::new("location", Shape::Enum, 0).edt_only(),
    Row::new("transparent", Shape::Bool, 6).edt_only(),
    Row::new("marker", Shape::Enum, 9).edt_only(),
    Row::new("orientation", Shape::Enum, 10).edt_only(),
    Row::new("left", Shape::Dec, 1),
    Row::new("top", Shape::Dec, 2),
    Row::new("angle", Shape::Dec, 11),
];

/// ChartColorPaletteDescription: Palette8 → опущено (planir ×2), Auto/Palette32 → эмитится.
const CPD_FIELDS: &[Row] = &[
    Row::new("colorPalette", Shape::Enum, 0).omit("Palette8"),
    Row::new("gradientPaletteStartColor", Shape::Color, 1),
    Row::new("gradientPaletteEndColor", Shape::Color, 2),
    Row::new("gradientPaletteMaxColors", Shape::Int, 4),
    Row::new("customPalette", Shape::Colors, 3),
];

/// Пустой композит (reference lines/bands — в корпусе всегда пустые).
const EMPTY_FIELDS: &[Row] = &[];

/// ChartAxis: designer — всегда пустой элемент; EDT материализует interval.
const AXIS_FIELDS: &[Row] = &[
    Row::new("baseValue", Shape::Dec, 0),
    Row::new("interval", Shape::Nested(Tbl::Interval), 1),
    Row::new("minValueDetectionMethod", Shape::Enum, 2),
    Row::new("maxValueDetectionMethod", Shape::Enum, 3),
];

/// AxisInterval (EDT-only; в корпусе всегда {leftIsNum=true, rightIsNum=true}).
const INTERVAL_FIELDS: &[Row] = &[
    Row::new("leftIsNum", Shape::Bool, 0).edt_only(),
    Row::new("leftNum", Shape::Dec, 1),
    Row::new("leftDate", Shape::DateTime, 2),
    Row::new("rightIsNum", Shape::Bool, 3).edt_only(),
    Row::new("rightNum", Shape::Dec, 4),
    Row::new("rightDate", Shape::DateTime, 5),
];

/// SeriesProperties (designer-порядок; edt_rank — позиции модели: text(5) < expand(6) <
/// indicator(7) < strIsChanged(8) < key(10) < colorPriority(11); valInfo=1/key=10 — фрейм).
const SERIES_ITEM_FIELDS: &[Row] = &[
    Row::new("valInfo", Shape::Value, 1),
    Row::new("key", Shape::Value, 10),
    Row::new("id", Shape::Int, 0),
    Row::new("color", Shape::Color, 2),
    Row::new("line", Shape::Line, 3),
    Row::new("marker", Shape::Enum, 4),
    Row::new("text", Shape::Loc, 5),
    Row::new("strIsChanged", Shape::Bool, 8),
    Row::new("isExpand", Shape::Bool, 6),
    Row::new("isIndicator", Shape::Bool, 7),
    Row::new("colorPriority", Shape::Bool, 11),
    Row::new(
        "showGraphicalRepresentationOfDataInChartLegend",
        Shape::Enum,
        12,
    )
    .alias("showGraphicalDataRepresentationInChartLegend"),
    Row::new("showGraphicalRepresentationOfDataOnChart", Shape::Enum, 13)
        .alias("showGraphicalDataRepresentationInChart"),
    Row::new("visualType", Shape::Enum, 14),
    Row::new("addType", Shape::Enum, 15),
    Row::new("valuesAxisUsage", Shape::Enum, 16),
    Row::new("stackGroup", Shape::Str, 17),
    Row::new("valueLabelFormat", Shape::Loc, 18),
    Row::new("percentLabelFormat", Shape::Loc, 19),
    Row::new("dataTableLabelFormat", Shape::Loc, 20),
    Row::new("valuesEditMode", Shape::Enum, 21),
    Row::new("info", Shape::Nested(Tbl::SeriesCalc), 9),
];

/// PointProperties: EDT-порядок ИНОЙ, чем у серий (marker/text ПЕРЕД color/line — модель;
/// сверено по vypoln realPointData).
const POINT_ITEM_FIELDS: &[Row] = &[
    Row::new("valInfo", Shape::Value, 1),
    Row::new("key", Shape::Value, 12),
    Row::new("id", Shape::Int, 0),
    Row::new("color", Shape::Color, 5),
    Row::new("line", Shape::Line, 6),
    Row::new("marker", Shape::Enum, 2),
    Row::new("text", Shape::Loc, 3),
    Row::new("strIsChanged", Shape::Bool, 4),
    Row::new("isExpand", Shape::Bool, 7),
    Row::new("isIndicator", Shape::Bool, 8),
    Row::new("colorPriority", Shape::Bool, 13),
    Row::new("intAdd", Shape::Int, 9),
    Row::new("doubleAdd", Shape::Dec, 10),
    Row::new("endX", Shape::Int, 11),
];

/// Gantt ChartPoints.
const GPOINTS_FIELDS: &[Row] = &[
    Row::new("testMode", Shape::Bool, 0),
    Row::new("value", Shape::Nested(Tbl::GDimPoint), 1),
    Row::new("contentCacheItem", Shape::Nested(Tbl::GContentPoint), 2),
    Row::new("autoText", Shape::Bool, 3),
    Row::new("useValuesReverseBehavior", Shape::Bool, 4),
];

/// Gantt ChartSeries.
const GSERIES_FIELDS: &[Row] = &[
    Row::new("testMode", Shape::Bool, 0),
    Row::new("value", Shape::Nested(Tbl::GDimSeries), 1),
    Row::new("contentCacheItem", Shape::Nested(Tbl::GContentSeries), 2),
    Row::new("autoText", Shape::Bool, 3),
    Row::new("useValuesReverseBehavior", Shape::Bool, 4),
];

/// Gantt SeriesDimensionValue.
const GDIM_SERIES_FIELDS: &[Row] = &[
    Row::new("itemKey", Shape::Int, 0),
    Row::new("key", Shape::Int, 1),
    Row::new("parentKey", Shape::Int, 2),
    Row::new("leftKey", Shape::Int, 3),
    Row::new("rightKey", Shape::Int, 4),
    Row::new("extKey", Shape::Int, 5),
    Row::new("title", Shape::Loc, 6),
    Row::new("cacheKey", Shape::Int, 7),
    Row::new("baseData", Shape::Int, 8),
];

/// Gantt PointDimensionValue (+= font/picture; picture — пустой маркер, лексика).
const GDIM_POINT_FIELDS: &[Row] = &[
    Row::new("itemKey", Shape::Int, 0),
    Row::new("key", Shape::Int, 1),
    Row::new("parentKey", Shape::Int, 2),
    Row::new("leftKey", Shape::Int, 3),
    Row::new("rightKey", Shape::Int, 4),
    Row::new("extKey", Shape::Int, 5),
    Row::new("title", Shape::Loc, 6),
    Row::new("cacheKey", Shape::Int, 7),
    Row::new("baseData", Shape::Int, 8),
    Row::new("font", Shape::Font, 9),
    Row::new("picture", Shape::Picture, 10),
];

/// Gantt GanttChartPointValueContent.
const GCONTENT_POINT_FIELDS: &[Row] = &[
    Row::new("mainColor", Shape::Color, 0),
    Row::new("secondColor", Shape::Color, 1),
    Row::new("backColor", Shape::Color, 2),
    Row::new("textColor", Shape::Color, 3),
];

/// Gantt GanttChartSeriesValueContent.
const GCONTENT_SERIES_FIELDS: &[Row] = &[
    Row::new("mainColor", Shape::Color, 0),
    Row::new("secondColor", Shape::Color, 1),
    Row::new("hatchBetweenIntervalsColor", Shape::Color, 2),
];

/// Gantt TimeScale (designer `<level>` ⟷ EDT `<levels>` — единственное имя-расхождение).
const TIMESCALE_FIELDS: &[Row] = &[
    Row::new("placement", Shape::Enum, 0).omit("Top"),
    Row::new("levels", Shape::Items(Tbl::TsLevel), 1).alias("level"),
    Row::new("transparent", Shape::Bool, 2),
    Row::new("backColor", Shape::Color, 3),
    Row::new("textColor", Shape::Color, 4),
    Row::new("currentLevel", Shape::Int, 5),
];

/// Gantt TimeScaleLevel.
const TS_LEVEL_FIELDS: &[Row] = &[
    Row::new("measure", Shape::Enum, 0),
    Row::new("interval", Shape::Int, 1),
    Row::new("show", Shape::Bool, 2),
    Row::new("line", Shape::Line, 3),
    Row::new("scaleColor", Shape::Color, 4),
    Row::new("dayFormatRule", Shape::Enum, 5),
    Row::new("format", Shape::Loc, 6),
    Row::new("labels", Shape::Nested(Tbl::TsLabels), 7),
    Row::new("backColor", Shape::Color, 8),
    Row::new("textColor", Shape::Color, 9),
    Row::new("showPereodicalLabels", Shape::Bool, 10),
];

/// Gantt TimeScaleLabels (в корпусе — только счётчик `ticks`).
const TS_LABELS_FIELDS: &[Row] = &[
    Row::new("ticks", Shape::Int, 1),
    Row::new("labels", Shape::Items(Tbl::TimeLabel), 0).alias("label"),
];

/// Gantt GanttChartBackgroundIntervals.
const BACK_INTERVALS_FIELDS: &[Row] = &[
    Row::new("collection", Shape::Nested(Tbl::Collect), 0),
    Row::new("contentCacheItem", Shape::Colors, 1),
    Row::new("ticks", Shape::Int, 2),
];

/// Gantt Collect (в корпусе — только `ticks`).
const COLLECT_FIELDS: &[Row] = &[
    Row::new("ticks", Shape::Int, 1),
    Row::new("items", Shape::Items(Tbl::CollectItem), 0).alias("item"),
];

const GAUGE_BANDS_FIELDS: &[Row] = &[
    Row::new("items", Shape::Items(Tbl::GaugeBand), 0).alias("item"),
    Row::new("useTextStr", Shape::Bool, 1),
    Row::new("useTooltipStr", Shape::Bool, 2),
];
const GAUGE_BAND_FIELDS: &[Row] = &[
    Row::new("begin", Shape::Int, 0),
    Row::new("end", Shape::Int, 1),
    Row::new("backColor", Shape::Color, 2),
    Row::new("text", Shape::Loc, 3),
    Row::new("tooltip", Shape::Loc, 4),
    Row::new("textString", Shape::Str, 5).alias("textStr"),
    Row::new("tooltipString", Shape::Str, 6).alias("tooltipStr"),
    Row::new("useTextString", Shape::Bool, 7),
    Row::new("useToolTipString", Shape::Bool, 8),
];
const REFERENCE_LINES_FIELDS: &[Row] =
    &[Row::new("chartReferenceLine", Shape::Items(Tbl::ReferenceLine), 0).alias("referenceLine")];
const REFERENCE_BANDS_FIELDS: &[Row] =
    &[Row::new("chartReferenceBand", Shape::Items(Tbl::ReferenceBand), 0).alias("referenceBand")];
const REFERENCE_LINE_FIELDS: &[Row] = &[
    Row::new("value", Shape::Value, 0),
    Row::new("labelText", Shape::Loc, 1),
    Row::new("tooltip", Shape::Loc, 2).alias("toolTip"),
    Row::new("labelArea", Shape::Nested(Tbl::LabelArea), 3),
    Row::new("line", Shape::Line, 4),
    Row::new("color", Shape::Color, 5),
    Row::new("semitransparencyPercent", Shape::Value, 6),
    Row::new("details", Shape::Value, 7),
    Row::new("position", Shape::Enum, 8),
    Row::new("valueIsPointNumber", Shape::Bool, 9),
];
const REFERENCE_BAND_FIELDS: &[Row] = &[
    Row::new("begin", Shape::Value, 0),
    Row::new("end", Shape::Value, 1),
    Row::new("labelText", Shape::Loc, 2),
    Row::new("tooltip", Shape::Loc, 3).alias("toolTip"),
    Row::new("labelArea", Shape::Nested(Tbl::LabelArea), 4),
    Row::new("color", Shape::Color, 5),
    Row::new("semitransparencyPercent", Shape::Value, 6),
    Row::new("border", Shape::Line, 7),
    Row::new("borderColor", Shape::Color, 8),
    Row::new("borderSemitransparencyPercent", Shape::Value, 9),
    Row::new("details", Shape::Value, 10),
    Row::new("position", Shape::Enum, 11),
    Row::new("displayAreaBegin", Shape::Dec, 12),
    Row::new("displayAreaEnd", Shape::Dec, 13),
    Row::new("beginIsPointNumber", Shape::Bool, 14),
    Row::new("endIsPointNumber", Shape::Bool, 15),
];

const TRENDARRAY_FIELDS: &[Row] = &[
    Row::new("seriesId", Shape::Int, 0),
    Row::new("line", Shape::Items(Tbl::Trend), 1).alias("trendline"),
];
const TREND_FIELDS: &[Row] = &[
    Row::new("approximationType", Shape::Enum, 0),
    Row::new("approximationDegree", Shape::Int, 1),
    Row::new("line", Shape::Line, 2),
    Row::new("factor", Shape::Enum, 3),
    Row::new("color", Shape::Color, 4),
    Row::new("text", Shape::Loc, 5),
    Row::new("showInLegend", Shape::Bool, 6),
    Row::new("showEquation", Shape::Bool, 7),
    Row::new("showDeterminationFactor", Shape::Bool, 8),
    Row::new("marker", Shape::Enum, 9),
    Row::new("stockChartUsedPointValue", Shape::Enum, 10),
    Row::new("equationArea", Shape::Nested(Tbl::LabelArea), 11),
];
const SERIESCALC_FIELDS: &[Row] = &[
    Row::new("enabled", Shape::Bool, 0),
    Row::new("min", Shape::Dec, 1),
    Row::new("max", Shape::Dec, 2),
    Row::new("absMin", Shape::Dec, 3),
    Row::new("absMax", Shape::Dec, 4),
    Row::new("startAngle", Shape::Dec, 5),
    Row::new("stopAngle", Shape::Dec, 6),
    Row::new("percent", Shape::Dec, 7),
    Row::new("str", Shape::Str, 8),
    Row::new("m_isExpand", Shape::Bool, 9),
    Row::new("m_centerPoint", Shape::Nested(Tbl::Point), 10),
    Row::new("endX", Shape::Int, 11),
];
const GINTERVAL_FIELDS: &[Row] = &[
    Row::new("itemKey", Shape::Int, 0),
    Row::new("key", Shape::Int, 1),
    Row::new("begin", Shape::DateTime, 2),
    Row::new("end", Shape::DateTime, 3),
    Row::new("text", Shape::Loc, 4),
    Row::new("color", Shape::Color, 5),
    Row::new("intervalKey", Shape::Int, 6),
    Row::new("textColor", Shape::Color, 7),
];
const GVALUE_FIELDS: &[Row] = &[
    Row::new("itemKey", Shape::Int, 0),
    Row::new("key", Shape::Int, 1),
    Row::new("text", Shape::Loc, 2),
    Row::new("editFlag", Shape::Bool, 3),
    Row::new("backColor", Shape::Color, 4),
    Row::new("textColor", Shape::Color, 5),
    Row::new("mainColor", Shape::Color, 6),
    Row::new("secondColor", Shape::Color, 7),
];
const GLINK_FIELDS: &[Row] = &[
    Row::new("curBeginKey", Shape::Int, 0),
    Row::new("curEndKey", Shape::Int, 1),
    Row::new("beginKey", Shape::Int, 2),
    Row::new("endKey", Shape::Int, 3),
    Row::new("linkType", Shape::Enum, 4),
    Row::new("color", Shape::Color, 5),
];
const TIMELABEL_FIELDS: &[Row] = &[
    Row::new("key", Shape::DateTime, 0),
    Row::new("text", Shape::Loc, 1),
    Row::new("lineColor", Shape::Color, 2),
    Row::new("textColor", Shape::Color, 3),
    Row::new("textFormatted", Shape::Bool, 4),
];
const COLLECTITEM_FIELDS: &[Row] = &[
    Row::new("key", Shape::DateTime, 0),
    Row::new("secondaryDate", Shape::DateTime, 1),
    Row::new("cacheKey", Shape::Int, 2),
];
const POINT_FIELDS: &[Row] = &[Row::new("x", Shape::Int, 0), Row::new("y", Shape::Int, 1)];

/// Строки под-таблицы по идентификатору.
pub(crate) fn rows(t: Tbl) -> &'static [Row] {
    match t {
        Tbl::TrendArray => TRENDARRAY_FIELDS,
        Tbl::Trend => TREND_FIELDS,
        Tbl::SeriesCalc => SERIESCALC_FIELDS,
        Tbl::GInterval => GINTERVAL_FIELDS,
        Tbl::GValue => GVALUE_FIELDS,
        Tbl::GLink => GLINK_FIELDS,
        Tbl::TimeLabel => TIMELABEL_FIELDS,
        Tbl::CollectItem => COLLECTITEM_FIELDS,
        Tbl::Point => POINT_FIELDS,
        Tbl::Chart => CHART_FIELDS,
        Tbl::Gantt => GANTT_FIELDS,
        Tbl::Scale => SCALE_FIELDS,
        Tbl::LabelArea => LABEL_AREA_FIELDS,
        Tbl::Cpd => CPD_FIELDS,
        Tbl::Empty => EMPTY_FIELDS,
        Tbl::Axis => AXIS_FIELDS,
        Tbl::Interval => INTERVAL_FIELDS,
        Tbl::SeriesItem => SERIES_ITEM_FIELDS,
        Tbl::PointItem => POINT_ITEM_FIELDS,
        Tbl::GPoints => GPOINTS_FIELDS,
        Tbl::GSeries => GSERIES_FIELDS,
        Tbl::GDimSeries => GDIM_SERIES_FIELDS,
        Tbl::GDimPoint => GDIM_POINT_FIELDS,
        Tbl::GContentPoint => GCONTENT_POINT_FIELDS,
        Tbl::GContentSeries => GCONTENT_SERIES_FIELDS,
        Tbl::TimeScale => TIMESCALE_FIELDS,
        Tbl::TsLevel => TS_LEVEL_FIELDS,
        Tbl::TsLabels => TS_LABELS_FIELDS,
        Tbl::BackIntervals => BACK_INTERVALS_FIELDS,
        Tbl::Collect => COLLECT_FIELDS,
        Tbl::GaugeBands => GAUGE_BANDS_FIELDS,
        Tbl::GaugeBand => GAUGE_BAND_FIELDS,
        Tbl::ReferenceLines => REFERENCE_LINES_FIELDS,
        Tbl::ReferenceLine => REFERENCE_LINE_FIELDS,
        Tbl::ReferenceBands => REFERENCE_BANDS_FIELDS,
        Tbl::ReferenceBand => REFERENCE_BAND_FIELDS,
    }
}

/// Верхнеуровневая таблица по виду настроек.
pub(crate) fn top_tbl(kind: &str) -> Result<Tbl, FormError> {
    match kind {
        "Chart" => Ok(Tbl::Chart),
        "GanttChart" => Ok(Tbl::Gantt),
        other => Err(frame(format!(
            "chart: незнакомый вид настроек диаграммы {other:?} (§1.0)"
        ))),
    }
}

/// Поиск строки по каноническому имени.
pub(crate) fn row_by_name(t: Tbl, name: &str) -> Option<&'static Row> {
    rows(t).iter().find(|r| r.name == name)
}

/// Поиск строки по designer-имени (алиас учитывается; EDT-only строки не находятся).
pub(crate) fn row_by_designer_name(t: Tbl, local: &str) -> Option<&'static Row> {
    rows(t).iter().find(|r| r.d_name() == local)
}
