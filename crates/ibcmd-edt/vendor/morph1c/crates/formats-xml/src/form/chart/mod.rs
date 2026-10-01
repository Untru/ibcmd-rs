//! XML-кодеки НАСТРОЕК ДИАГРАММЫ форм-реквизита (Chart / GanttChart) — два диалекта:
//! Designer-ИНЛАЙН `<Settings xmlns:d4p1="http://v8.1c.ru/8.2/data/chart"
//! xsi:type="d4p1:Chart|GanttChart">` внутри `Ext/Form.xml` и EDT-САЙДКАР
//! `Attributes/<attr>/ExtInfo/Chart.chart|GanttChart.chart` (`chart:Chart` /
//! `ganttchart:GanttChart`, xcore-разрежённый).
//!
//! # Витнессы (9 носителей ERP, 5 форм; W17-линия, scratchpad chart_line)
//! * nastr_demo — `Catalogs/ВариантыАнализаЦелевыхПоказателей/Forms/НастройкаДемоДанных/Диаграмма`;
//! * proverka ×3 — `DataProcessors/ПроверкаКонтрагента/Forms/Форма/Диаграмма{Показателей,
//!   РентабельностьАктивов,РентабельностьПродаж}` (мультиязычные text/vsFormat, splineMode,
//!   placement-энумы, scale c gridLine/labelColor/labelFormat);
//! * vypoln — `DataProcessors/ВыполнениеОпераций2_2/…/Диаграмма` (Pie: realSeriesData ×2 c
//!   явными цветами, realPointData, realDataItems, StyleItem-шрифт с height, titleText серии);
//! * planir ×3 — `DataProcessors/ПланированиеГрафикаПроизводства2_2/…` (GanttChart-обёртка +
//!   2 Chart с Palette8-парой paletteKind/colorPaletteDescription);
//! * sezon — `InformationRegisters/СезонныеКоэффициенты/…/Диаграмма` (опорная пара для таблицы
//!   EDT-омиссий: поле есть в designer и нет в сайдкаре ⇔ xcore-дефолт).
//!
//! # Модель
//! ЕДИНАЯ таблица знаний [`CHART_FIELDS`] (+под-таблицы композитов): имя поля, шейп значения,
//! диалект-присутствие и политика EDT-омиссии. Порядок строк = DESIGNER-порядок 8.3.27 (он же
//! порядок клеток cf-таблицы {74,…} — прототип verify2.py; клеточные константы `_NNN` и
//! placement-блоки живут в cf-энкодере, НЕ здесь). Отдельно каждая строка несёт `edt_rank` —
//! позицию в EDT-модели (`models/charts/charts.jsonl`, authoritative порядок xcore-эмиссии):
//! порядки РАСХОДЯТСЯ (isPointsDesign/realDataItems/isTransposition/animation/rebuildTime/
//! placement-энумы против tooltip/droplines-режимов), поэтому один порядок из другого не
//! выводится — несём оба.
//!
//! Значения храним ЛЕКСИЧЕСКИ ([`ChartValue`]): числа/decimal/даты как текст, енум-литералы
//! как есть. Валидация ЗНАЧЕНИЙ (витнесс-мапы енумов, валидируемые константы) — забота
//! cf-энкодера; XML-сторона только транскодирует диалект-формы.
//!
//! # Два режима записи сайдкара ([`write_chart_sidecar`])
//! * ЭХО (источник = EDT-чтение; детект — поле `translucenceMode`, EDT-модель несёт его
//!   всегда): поля пишутся как прочитаны, без синтеза — byte-exact по построению (порядок
//!   сверен с моделью ещё на чтении).
//! * ТРАНСКОД (источник = Designer-чтение): поля сортируются по `edt_rank`, xcore-дефолты
//!   ОПУСКАЮТСЯ (таблица омиссий снята сверкой sezon/proverka/vypoln/planir пар:
//!   bool=false, int=0, строка/локализация пустые, цвет auto — плюс пер-полевые литералы
//!   `chartType=Line`, `labelsLocation=Edge`, `paletteKind=Palette8`, `isRandomizedNewValues=true`
//!   и т.д.), а EDT-ONLY константы модели СИНТЕЗИРУЮТСЯ (`translucenceMode=Auto`, bubble*-тройка,
//!   colorPaletteDescription/referenceBands…, четыре пустых reference-блока, оси с
//!   interval{leftIsNum,rightIsNum}, шкалы с материализованным titleArea/titlePlacement/
//!   labelOrientation). Транскод сверен с живыми сайдкарами симулятором таблицы
//!   (scratchpad chart_line/chart_sim.py): 8/9 витнессов БАЙТ-ИДЕНТИЧНЫ; девятый (vypoln)
//!   несёт РОВНО ОДНУ дельту — EDT-экспортёр платформы пишет `isShowPointsScale=true` там,
//!   где designer И cf несут `false` (квирк EDT-выгрузки; определитель не установлен —
//!   1 витнесс, у остальных `isShowScale=true`). Значение НЕ подделываем: несём
//!   designer/cf-истину (verify2: cf-клетка сходится с designer-значением 8/8).
//!
//! # §1.0
//! Незнакомое имя поля/атрибута, невитнесснутая диалект-форма (цвет `web:`/`win:`, абсолютный
//! шрифт, `gap=true` в EDT-линии, ненулевые isExpand/isIndicator/colorPriority точек и серий в
//! EDT, непустая designer-ось, непустые gauge-бенды) — ГРОМКАЯ типизированная ошибка с именем
//! свойства. Никаких Raw/passthrough. `realDataItems` — designer-only (витнесс vypoln: непустой
//! в designer, отсутствует в сайдкаре) — в EDT НЕ эмитится никогда.
//!
//! # Плотнение EDT-бэга (`densify.rs`)
//! [`designer_dense_chart_settings`] — АЛГЕБРАИЧЕСКАЯ ИНВЕРСИЯ транскода: EDT-бэг (xcore-
//! разрежённый, с EDT-only константами) → designer-плотная форма, которую требует cf-энкодер.
//! Ценз по ВСЕМУ ERP (25 сайдкаров): 25/25 без нарушений инверсии, cf-клетка совпадает с
//! designer-путём на 24/25 (25-й — потеря EDT-выгрузки, см. док-коммент модуля).
//!
//! # Риски/границы
//! * Плейсмент-ректы cf (`@PLACEMENT_BLOCK`, runtime-значения) — НЕ здесь; риск задокументирован
//!   в cf-энкодере (SPEC_NOTES.md п.3).
//! * Designer-ЗАПИСЬ — ЭХО cs.fields (X-байт-точность designer→designer); материализация
//!   designer-плотной формы ИЗ EDT-источника для designer-ЗАПИСИ (не для cf) по-прежнему
//!   не витнесснута: `designer_chart_settings` отказывает на первом EDT-only поле. Для cf
//!   плотнение делает [`designer_dense_chart_settings`] (см. выше).

// До подключения pipeline-волной модуль не имеет внешних вызовов — глушим dead_code,
// чтобы централизованная сборка не шумела (тесты внизу держат кодеки живыми).
#![allow(dead_code)]

use super::read::{claim_root_ns, unclaimed_labels};
use super::{FormError, CORE_NS_URI, FORM_DECL, XSI_NS_URI};
use crate::descriptor::Element;
use crate::emit::{render, OutElement};
use crate::{parse, EolStyle};
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
    /// Имя поля в designer-диалекте.
    fn d_name(&self) -> &'static str {
        self.d_alias.unwrap_or(self.name)
    }
}

mod tables;
mod common;
mod densify;
mod designer;
mod edt;
#[cfg(any())]
mod tests;

pub use densify::designer_dense_chart_settings;
pub use edt::{read_chart_sidecar, write_chart_sidecar};
pub(crate) use tables::*;
pub(crate) use common::*;
pub(crate) use designer::*;
