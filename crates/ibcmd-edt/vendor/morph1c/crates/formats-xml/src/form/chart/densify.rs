//! CHART · ПЛОТНЕНИЕ EDT-бэга до DESIGNER-плотной формы — АЛГЕБРАИЧЕСКАЯ ИНВЕРСИЯ транскода
//! (`edt.rs`, `Mode::Transcode`). Нужна cf-энкодеру: он требует полу-плотный designer-бэг
//! (`Bag::req`: «a sparse EDT bag must be densified BEFORE the cf encoder») и НЕ ЗНАЕТ
//! EDT-only имён — на первом же (`translucenceMode`) отказывал §1.0, из-за чего лейн
//! `edt→cf` на ERP не доходил до конца.
//!
//! # Три правила инверсии (каждое — обращение конкретного шага транскода)
//! 1. **Омиссия → материализация.** Строка таблицы, ОТСУТСТВУЮЩАЯ в EDT-бэге, получает
//!    витнесснутый дефолт: `Edt::OmitIf(lit)` → сам литерал; иначе шейповый дефолт
//!    (bool=false, int=`0`, dec=`0.0`, строка="", цвет=auto, локализация пустая,
//!    шрифт=AutoFont — ровно множество, которое [`std_omitted`] считает опускаемым, плюс
//!    Font: `edt_title_area` опускает шрифт синтезированной области заголовка). Шейпы БЕЗ
//!    витнесснутого дефолта (енум без литерала омиссии, линия, рамка, рект, композиты,
//!    `realDataItems`) остаются ОТСУТСТВУЮЩИМИ: транскод эмитит ТОЛЬКО то, что несёт источник,
//!    значит их отсутствие в EDT ⇒ отсутствие и в designer, а cf-энкодер такое отсутствие
//!    терпит (`Bag::get` / `Bag::lit_or_dash` / `row_get` дают ту же клетку, что и
//!    материализованный дефолт — сверено по всем 25 носителям ERP).
//! 2. **Синтез → сверка-и-снятие.** EDT-only строка (материализованная xcore-константа,
//!    [`chart_synth`]) СВЕРЯЕТСЯ с витнесс-константой и СНИМАЕТСЯ; чужое значение — §1.0-отказ
//!    (designer-проекции у него нет). ИСКЛЮЧЕНИЕ — `additionalValuesScale`: cf её ЧИТАЕТ
//!    (`Bag::get`), поэтому она проходит инверсию шкалы и ОСТАЁТСЯ в бэге.
//! 3. **Трансформация → обратная трансформация.**
//!    * Ось (`valuesAxis`/`pointsAxis`): `{interval{leftIsNum=true,rightIsNum=true}}` → ПУСТАЯ
//!      designer-ось (обращение `e_transform`).
//!    * Шкала: EDT-only под-поля `titleArea` (location/transparent/marker/orientation)
//!      сверяются и снимаются, сама `titleArea` плотнится до designer-канона
//!      (`{font=font();textColor=auto;backColor=auto;border=border(WithoutBorder,1);
//!      borderColor=auto}` — ровно `SCALE_TITLE_AREA_CANON` cf-энкодера), транскод-дефолты
//!      `titlePlacement=SpecialArea` и `labelOrientation=Auto` СНИМАЮТСЯ: их designer-проекция —
//!      отсутствие (обращение [`edt_scale_fields`]/[`edt_title_area`]).
//!    * Палитра: `colorPaletteDescription{colorPalette=Auto}` СНИМАЕТСЯ — пара
//!      (`paletteKind=Auto`, отсутствие) единственная витнесснутая в cf `palette_pair`.
//!
//! # КРОСС-ВИТНЕСС-ЦЕНЗ (ВЕСЬ ERP: 25 сайдкаров `.chart` — 8 Chart + 17 GanttChart)
//! Симулятор инверсии (scratchpad `ag_ch_cross.py`) сверил плотнённый EDT-бэг ПОЛЕ-В-ПОЛЕ с
//! designer-`<Settings>` ТОЙ ЖЕ формы по всем 25 носителям, нормализовав только те поля, чья
//! designer-омиссия даёт cf-энкодеру ТУ ЖЕ клетку (перечислены в правиле 1):
//! * НАРУШЕНИЙ ИНВЕРСИИ — **0**: все EDT-only поля всех 25 носителей несут РОВНО
//!   витнесс-константы (`translucenceMode=Auto`, `bubbleSizeValueSource=NextSeries`,
//!   `bubbleSizeCommonSeries=-2`, `bubbleSizing=IncreaseArea`, четыре пустых reference-блока,
//!   `referenceBandsColorPaletteDescription{colorPalette=Auto}`, `additionalValuesAxis`
//!   = витнесс-интервал, titleArea-четвёрка).
//! * РАСХОЖДЕНИЙ с designer — **2, оба НЕВОСПРОИЗВОДИМЫ ИЗ EDT-ИСТОЧНИКА** (не наш дефект),
//!   оба в `DataProcessor.ВыполнениеОпераций2_2.Форма.МобильноеПриложениеНачальнаяСтраница`:
//!   `isShowPointsScale` (EDT-выгрузка платформы пишет `true`, designer И cf несут `false` —
//!   квирк экспортёра, задокументирован в `mod.rs`) и `realDataItems` (designer несёт 2 items
//!   15.99/10.64, EDT-сайдкар поля не имеет вовсе — `Edt::OmitAlways`).
//!
//! Побочно свип вскрыл ПРОБЕЛ таблицы знаний: `GanttChart.intervalDrawType` — енум БЕЗ литерала
//! омиссии, но 2 носителя (`Report.{МониторингЗаказаНормативныйГрафик,мирМониторингЗаказа}
//! .ФормаОтчета`) несут designer-`Flat` при ПОЛНОМ отсутствии поля в EDT-сайдкаре ⇒ дефолт
//! модели = `Flat`, строка получила `.omit("Flat")` (правит и транскод designer→EDT: он писал
//! поле там, где живая выгрузка его опускает).
//!
//! # §1.0
//! Незнакомое имя, EDT-only поле с невитнесснутым значением, ось с чужим интервалом,
//! композит вместо скаляра — типизированный отказ с именем свойства. Никаких догадок.

use super::edt::{chart_synth, expect_nested, is_edt_sourced};
use super::*;

/// EDT-источник → designer-плотный [`ChartSettings`]; designer-источник возвращается КАК ЕСТЬ
/// (он уже полу-плотный — cf-энкодер читает его напрямую с 2026-07).
///
/// Детект источника — тот же, что у писателя сайдкара ([`is_edt_sourced`]): материализованная
/// EDT-константа `translucenceMode`, которую designer не сериализует вовсе.
pub fn designer_dense_chart_settings(cs: &ChartSettings) -> Result<ChartSettings, FormError> {
    if !is_edt_sourced(cs) {
        return Ok(cs.clone());
    }
    let t = top_tbl(&cs.kind)?;
    Ok(ChartSettings {
        kind: cs.kind.clone(),
        fields: dense_children(&cs.fields, t, &cs.kind)?,
        source_layout: cs.source_layout.clone(),
    })
}

/// Плотнение детей таблицы `t`: обход в DESIGNER-порядке строк (он же порядок клеток cf),
/// материализация омиссий, снятие EDT-only, обратные трансформации.
fn dense_children(
    fields: &[(String, ChartValue)],
    t: Tbl,
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    // §1.0: каждое имя источника известно таблице (иначе инверсия не определена).
    for (n, _) in fields {
        if row_by_name(t, n).is_none() {
            return Err(frame(format!(
                "chart {path}: незнакомое поле {n:?} при плотнении EDT-бэга (§1.0)"
            )));
        }
    }
    let mut out: Vec<(String, ChartValue)> = Vec::with_capacity(fields.len());
    for row in rows(t) {
        let stored = fields.iter().find(|(n, _)| n == row.name).map(|(_, v)| v);
        let sub_path = format!("{path}/{}", row.name);
        // Правило 2: EDT-only — сверить с витнесс-константой и снять.
        if !row.designer && row.name != "additionalValuesScale" {
            if let Some(v) = stored {
                let want = edt_only_witness(t, row.name).ok_or_else(|| {
                    frame(format!(
                        "chart {sub_path}: EDT-only поле без витнесс-константы — инверсия \
                         транскода не определена (§1.0)"
                    ))
                })?;
                if *v != want {
                    return Err(frame(format!(
                        "chart {sub_path}: EDT-only поле несёт {v:?} вместо витнесс-константы \
                         {want:?} — designer-проекции у него нет (§1.0)"
                    )));
                }
            }
            continue;
        }
        match stored {
            Some(v) => {
                if let Some(nv) = dense_value(row, v, &sub_path)? {
                    out.push((row.name.to_string(), nv));
                }
            }
            // Правило 1: омиссия → витнесснутый дефолт (или остаётся отсутствующей).
            None => {
                if let Some(d) = dense_default(row) {
                    out.push((row.name.to_string(), d));
                }
            }
        }
    }
    Ok(out)
}

/// Витнесс-константа EDT-only строки = ровно то, что синтезирует транскод.
fn edt_only_witness(t: Tbl, name: &str) -> Option<ChartValue> {
    match (t, name) {
        // Ось синтезируется пустой и НАПОЛНЯЕТСЯ `e_transform` — витнесс = наполненная форма.
        (Tbl::Chart, "additionalValuesAxis") => Some(axis_witness()),
        (Tbl::Chart, n) => chart_synth(n),
        (Tbl::LabelArea, "location" | "marker" | "orientation") => {
            Some(ChartValue::Enum("Auto".to_string()))
        }
        (Tbl::LabelArea, "transparent") => Some(ChartValue::Bool(true)),
        (Tbl::Axis, "interval") => Some(interval_witness()),
        (Tbl::Interval, "leftIsNum" | "rightIsNum") => Some(ChartValue::Bool(true)),
        _ => None,
    }
}

/// Витнесс-форма EDT-оси: `{interval{leftIsNum=true,rightIsNum=true}}` (все оси корпуса).
fn axis_witness() -> ChartValue {
    ChartValue::Nested(vec![("interval".to_string(), interval_witness())])
}

/// Витнесс-форма EDT-интервала оси.
fn interval_witness() -> ChartValue {
    ChartValue::Nested(vec![
        ("leftIsNum".to_string(), ChartValue::Bool(true)),
        ("rightIsNum".to_string(), ChartValue::Bool(true)),
    ])
}

/// Плотнение ОДНОГО значения (правило 3 + рекурсия); `None` ⇒ поле СНИМАЕТСЯ.
fn dense_value(row: &Row, v: &ChartValue, path: &str) -> Result<Option<ChartValue>, FormError> {
    match row.shape {
        Shape::Nested(Tbl::Axis) => {
            let want = axis_witness();
            if *v != want {
                return Err(frame(format!(
                    "chart {path}: EDT-ось несёт {v:?} вместо витнесс-интервала {want:?} \
                     (designer-ось в корпусе всегда пуста) (§1.0)"
                )));
            }
            Ok(Some(ChartValue::Nested(Vec::new())))
        }
        Shape::Nested(Tbl::Scale) => Ok(Some(ChartValue::Nested(dense_scale(
            expect_nested(v, row.name, path)?,
            path,
        )?))),
        Shape::Nested(Tbl::Cpd) => {
            let dense = dense_children(expect_nested(v, row.name, path)?, Tbl::Cpd, path)?;
            // Пара (paletteKind=Auto, ОТСУТСТВИЕ описания) — единственная витнесснутая форма
            // Auto-палитры в designer/cf (`palette_pair`), значит Auto-описание СНИМАЕТСЯ.
            if dense.len() == 1
                && dense[0].0 == "colorPalette"
                && dense[0].1 == ChartValue::Enum("Auto".to_string())
            {
                return Ok(None);
            }
            Ok(Some(ChartValue::Nested(dense)))
        }
        Shape::Nested(t2) => Ok(Some(ChartValue::Nested(dense_children(
            expect_nested(v, row.name, path)?,
            t2,
            path,
        )?))),
        Shape::ChartTable => Ok(Some(ChartValue::Nested(dense_children(
            expect_nested(v, row.name, path)?,
            Tbl::Chart,
            path,
        )?))),
        Shape::SeriesItem => Ok(Some(ChartValue::Nested(dense_children(
            expect_nested(v, row.name, path)?,
            Tbl::SeriesItem,
            path,
        )?))),
        Shape::SeriesItems | Shape::PointItems | Shape::Items(_) => {
            let sub = match row.shape {
                Shape::SeriesItems => Tbl::SeriesItem,
                Shape::PointItems => Tbl::PointItem,
                Shape::Items(t2) => t2,
                _ => unreachable!(),
            };
            let ChartValue::Items(items) = v else {
                return Err(frame(format!(
                    "chart {path}: поле {:?} несёт {v:?} вместо набора items (§1.0)",
                    row.name
                )));
            };
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(dense_children(item, sub, path)?);
            }
            Ok(Some(ChartValue::Items(out)))
        }
        _ => Ok(Some(v.clone())),
    }
}

/// Обратная [`edt_scale_fields`]: плотнение под-полей + снятие транскод-дефолтов шкалы
/// (их designer-проекция — ОТСУТСТВИЕ; cf-клетка совпадает — `TITLE_PLC` мапит и `-`, и
/// `SpecialArea` в `2`, а `LBL_ORIENT` знает только `-`/`CustomAngle`).
fn dense_scale(
    fields: &[(String, ChartValue)],
    path: &str,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    let mut out = Vec::new();
    for (n, v) in dense_children(fields, Tbl::Scale, path)? {
        if n == "titlePlacement" && v == ChartValue::Enum("SpecialArea".to_string()) {
            continue;
        }
        if n == "labelOrientation" && v == ChartValue::Enum("Auto".to_string()) {
            continue;
        }
        out.push((n, v));
    }
    Ok(out)
}

/// Витнесснутый designer-дефолт ОТСУТСТВУЮЩЕЙ строки; `None` ⇒ оставить отсутствующей.
fn dense_default(row: &Row) -> Option<ChartValue> {
    if let Edt::OmitIf(lit) = row.edt {
        return Some(match row.shape {
            Shape::Bool => ChartValue::Bool(lit == "true"),
            Shape::Int | Shape::Dec => ChartValue::Int(lit.to_string()),
            Shape::Str | Shape::DateTime => ChartValue::Str(lit.to_string()),
            Shape::Enum => ChartValue::Enum(lit.to_string()),
            Shape::Color => ChartValue::Color(lit.to_string()),
            _ => return None,
        });
    }
    match row.shape {
        Shape::Bool => Some(ChartValue::Bool(false)),
        Shape::Int => Some(ChartValue::Int("0".to_string())),
        // Канон Dec — EDT-лексика (designer «0» приезжает в IR как «0.0», см. `edt_decimal`).
        Shape::Dec => Some(ChartValue::Int("0.0".to_string())),
        Shape::Str | Shape::DateTime => Some(ChartValue::Str(String::new())),
        Shape::Color => Some(ChartValue::Color("auto".to_string())),
        Shape::Loc => Some(ChartValue::Localized(Vec::new())),
        Shape::Font => Some(ChartValue::Font(auto_font())),
        _ => None,
    }
}
