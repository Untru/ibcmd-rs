//! Typed Chart/GanttChart settings in EDT sidecars and native form Attributes.
//!
//! Both codecs read the current model, validate closed field/value grammars and
//! share canonical properties. Original installed SDK model metadata provides
//! scalar defaults, enum literals and native field order. Source layout holds
//! names/order/presence only and never previous values or XML bytes.
//!
//! Nullable fields remain distinct from explicit Null/Undefined. Persisted model
//! properties that the target native writer omits require the form's separately
//! bound typed semantic resource; the descriptor codec refuses unprojected state.
//! Bitmap resources are attached by the pipeline, independently of XML parsing.

// До подключения pipeline-волной модуль не имеет внешних вызовов — глушим dead_code,
// чтобы централизованная сборка не шумела (тесты внизу держат кодеки живыми).
#![allow(dead_code)]

use super::read::{claim_root_ns, unclaimed_labels};
use super::{CORE_NS_URI, FORM_DECL, FormError, XSI_NS_URI};
use crate::descriptor::Element;
use crate::emit::{OutElement, render};
use crate::{EolStyle, parse};
use morph1c_core::ir::form::{ChartSettings, ChartValue};
use morph1c_core::ir::{FontRef, Lang};

/// URI ns designer-настроек диаграммы (инлайн-объявление `xmlns:d4p1` на `<Settings>`;
/// ОДИН URI на оба вида — сверено по ERP: и Chart, и GanttChart объявляют его же).
const CHART_D4P1_NS_URI: &str = "http://v8.1c.ru/8.2/data/chart";
/// URI ns EDT-модели диаграммы (корень `<chart:Chart>`).
const EDT_CHART_NS_URI: &str = "http://g5.1c.ru/v8/dt/chart/model";
/// URI ns EDT-модели диаграммы Ганта (корень `<ganttchart:GanttChart>`).
const EDT_GANTT_NS_URI: &str = "http://g5.1c.ru/v8/dt/ganttchart/model";

/// ns-блок корня `Chart.chart` в ТОЧНОМ порядке эталона (hexdump-сверено по всем Chart-витнессам).
const EDT_CHART_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns:xsi", XSI_NS_URI),
    ("xmlns:chart", EDT_CHART_NS_URI),
    ("xmlns:core", CORE_NS_URI),
];
/// ns-блок корня `GanttChart.chart` (порядок ИНОЙ, чем у Chart, — сверено по planir-витнессу).
const EDT_GANTT_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns:xsi", XSI_NS_URI),
    ("xmlns:core", CORE_NS_URI),
    ("xmlns:ganttchart", EDT_GANTT_NS_URI),
];

// ─────────────────────────────────────────────────────────────────────────────────────────
// ТАБЛИЦА ЗНАНИЙ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// Идентификатор под-таблицы композита (вместо `&'static [Row]` в [`Shape`], чтобы не зависеть
/// от адресной идентичности const-промоций).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tbl {
    /// Верхнеуровневая Chart-таблица ([`CHART_FIELDS`]).
    Chart,
    /// Верхнеуровневая GanttChart-обёртка ([`GANTT_FIELDS`]).
    Gantt,
    /// ChartScale (points/values/series/additionalValuesScale).
    Scale,
    /// LabelArea (titleArea шкалы).
    LabelArea,
    /// ChartColorPaletteDescription.
    Cpd,
    /// Пустой композит (reference-lines/bands: в корпусе всегда пустые; ребёнок → отказ).
    Empty,
    /// ChartAxis (в designer всегда пустая; в EDT несёт interval).
    Axis,
    /// AxisInterval (EDT-only).
    Interval,
    /// SeriesProperties (realSeriesData/realExSeriesData).
    SeriesItem,
    /// PointProperties (realPointData).
    PointItem,
    /// Gantt ChartPoints.
    GPoints,
    /// Gantt ChartSeries.
    GSeries,
    /// Gantt SeriesDimensionValue (series/value).
    GDimSeries,
    /// Gantt PointDimensionValue (points/value; += font/picture).
    GDimPoint,
    /// Gantt GanttChartPointValueContent (points/contentCacheItem).
    GContentPoint,
    /// Gantt GanttChartSeriesValueContent (series/contentCacheItem).
    GContentSeries,
    /// Gantt TimeScale.
    TimeScale,
    /// Gantt TimeScaleLevel (designer `<level>` ⟷ EDT `<levels>`).
    TsLevel,
    /// Gantt TimeScaleLabels (`labels`).
    TsLabels,
    /// Gantt GanttChartBackgroundIntervals.
    BackIntervals,
    /// Gantt Collect (`collection`).
    Collect,
    TrendArray,
    Trend,
    SeriesCalc,
    GInterval,
    GValue,
    GLink,
    TimeLabel,
    CollectItem,
    Point,
    GaugeBands,
    GaugeBand,
    ReferenceLines,
    ReferenceLine,
    ReferenceBands,
    ReferenceBand,
}

/// Шейп значения поля — как декодировать/кодировать обе диалект-формы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// Булево (`true`/`false`).
    Bool,
    /// Целое/long — лексический текст, БЕЗ переформатирования.
    Int,
    /// float/double: designer `10` ⟷ EDT `10.0` (funnel*, высота шрифта, noneVariantChars).
    Dec,
    /// Строка (пробелы значимы: labelsDelimiter=", ").
    Str,
    /// Дата Ганта — лексически (`2016-07-01T00:00:00`; форма в диалектах совпадает).
    DateTime,
    Value,
    /// Nullable current chart Picture model value.
    Picture,
    /// Ordered current Color values; native SDK wraps them in typed v8:Value.
    Colors,
    /// Enum-литерал как есть (валидация значений — cf-энкодер).
    Enum,
    /// Цвет; канон = designer-текст `auto` | `#RRGGBB` | `style:Имя`.
    Color,
    /// Шрифт: AutoFont (все поля [`FontRef`] = None) | StyleItem-ссылка (`Style.X` + height).
    Font,
    /// Линия (style+width+gap; gap в EDT отсутствует=false).
    Line,
    /// Рамка (style+width; EDT опускает style=WithoutBorder и width=0).
    Border,
    /// Локализованная строка (designer `<v8:item>` ⟷ EDT повторяемый `<key>/<value>`).
    Loc,
    /// Прямоугольник (left/right/top/bottom — лексика; порядок диалектов совпадает).
    Rect,
    /// Одиночный вложенный композит по под-таблице.
    Nested(Tbl),
    /// Повторяемый композит по под-таблице (одно IR-поле `Items`).
    Items(Tbl),
    /// realSeriesData (повторяемый; EDT — с обёрткой `<properties>` и фрейм-константами).
    SeriesItems,
    /// realExSeriesData — одиночная серия (EDT — та же `<properties>`-обёртка).
    SeriesItem,
    /// realPointData (повторяемый; EDT — БЕЗ `<properties>`, фрейм-константы прямые).
    PointItems,
    /// realDataItems (designer-only: `<item><valData xsi:type="xs:decimal">…`).
    DataItems,
    /// gaugeQualityBands: designer-атрибуты useTextStr/useTooltipStr ⟷ EDT пустой элемент.
    QBands,
    /// Вложенная Chart-таблица (Gantt `chart`) — рекурсия в [`CHART_FIELDS`].
    ChartTable,
}

/// Политика EDT-эмиссии строки таблицы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edt {
    /// Стандарт шейпа: bool=false / int=«0» / dec=«0» / строка="" / цвет=auto / локализация
    /// пустая → опустить; енумы/композиты — эмитятся всегда.
    Std,
    /// Опустить РОВНО при этом каноническом токене значения (енум-дефолты модели;
    /// `isRandomizedNewValues`: дефолт true). Иное значение — эмитить (стандарт НЕ применяется).
    OmitIf(&'static str),
    /// Эмитить всегда (baseVal: BigDecimal-поле, витнесс `<baseVal>0</baseVal>`).
    AlwaysEmit,
    /// В EDT НЕ эмитится никогда (realDataItems — витнесс vypoln: данные есть, сайдкар пуст).
    OmitAlways,
    /// EDT-форма НЕ витнесснута: чтение — отказ; запись — опустить при шейп-дефолте, иначе
    /// отказ (isExpand/isIndicator/colorPriority серий и точек, strIsChanged точек).
    Unwitnessed,
}

/// Строка таблицы знаний.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Row {
    /// Каноническое имя (= имя EDT-модели).
    name: &'static str,
    /// Designer-имя, если отличается (единственный случай: `levels` ⟷ designer `level`).
    d_alias: Option<&'static str>,
    /// Шейп значения.
    shape: Shape,
    /// Позиция в EDT-модели (models/charts/charts.jsonl) — порядок xcore-эмиссии.
    edt_rank: u16,
    /// Политика EDT-эмиссии.
    edt: Edt,
    /// Несёт ли designer-диалект это поле (false = EDT-only; designer-чтение → отказ).
    designer: bool,
}

impl Row {
    const fn new(name: &'static str, shape: Shape, edt_rank: u16) -> Self {
        Row {
            name,
            d_alias: None,
            shape,
            edt_rank,
            edt: Edt::Std,
            designer: true,
        }
    }
    const fn omit(mut self, lit: &'static str) -> Self {
        self.edt = Edt::OmitIf(lit);
        self
    }
    const fn always(mut self) -> Self {
        self.edt = Edt::AlwaysEmit;
        self
    }
    const fn omit_always(mut self) -> Self {
        self.edt = Edt::OmitAlways;
        self
    }
    const fn unwitnessed(mut self) -> Self {
        self.edt = Edt::Unwitnessed;
        self
    }
    const fn edt_only(mut self) -> Self {
        self.designer = false;
        self
    }
    const fn alias(mut self, a: &'static str) -> Self {
        self.d_alias = Some(a);
        self
    }
    fn e_name(&self) -> &'static str {
        match self.name {
            "isExpand" => "expand",
            "isIndicator" => "indicator",
            other => other,
        }
    }
    /// Имя поля в designer-диалекте.
    fn d_name(&self) -> &'static str {
        self.d_alias.unwrap_or(self.name)
    }
}

mod common;
mod data_items;
mod densify;
mod designer;
mod edt;

mod percent;
mod picture;
mod sdk_defaults;
mod semantic;
mod tables;
#[cfg(any())]
mod tests;
mod value_border;
mod value_enum;
mod value_period;

pub(crate) use common::*;
pub use densify::designer_dense_chart_settings;
pub(crate) use designer::*;
pub use edt::{read_chart_sidecar, write_chart_sidecar};
pub use percent::{
    native_percentage_integer, native_percentage_projection, percentage_numeric_equal,
};
pub use semantic::project_native_trends;
pub(crate) use tables::*;
pub(crate) fn validate_current_value(
    value: &morph1c_core::ir::form::ChartTypedValue,
) -> Result<(), FormError> {
    data_items::write_value(
        "value",
        &ChartValue::Value(Box::new(value.clone())),
        "current value resource",
        false,
    )
    .map(|_| ())
}
pub(crate) fn is_native_enum_type(name: &str) -> bool {
    value_enum::is_native_type_name(name)
}
pub(crate) fn native_enum_binding(
    name: &str,
    literal: &str,
) -> morph1c_core::ir::form::ChartTypedValue {
    value_enum::binding_value(name, literal)
}

#[doc(hidden)]
pub use semantic::normalize_big_decimal;

pub(super) use semantic::{native_design_default, suppressed_design_fields};
