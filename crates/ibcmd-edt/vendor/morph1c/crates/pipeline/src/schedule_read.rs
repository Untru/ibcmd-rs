//! Чтение/запись РАСПИСАНИЯ регламентного задания (`ScheduledJob`) как сайдкара дескриптора
//! (§1.2/§4) — сиблинг [`crate::help_read`]/[`crate::picture_read`]: «дескриптор-read метаданных
//! не трогает тело; whole-config-конвейер присоединяет его отдельным проходом».
//!
//! # Раскладка и кодирование (RE SSL: 45 из 50 ScheduledJob'ов несут сайдкар; ERP: 258 из 287 —
//! ОБА диалекта)
//! * **EDT** — `<obj-dir>/Schedule.schedule`: БЕЗ BOM, **CRLF**, отступ — 2 ПРОБЕЛА, хвостовой
//!   перевод строки ЕСТЬ. Корень `<schedule:Schedule xmlns:schedule="http://g5.1c.ru/v8/dt/schedule"
//!   …>`; поля — АТРИБУТЫ в ФИКСИРОВАННОМ порядке схемы: пять «всегда-атрибутов»
//!   (даты + времена), ЧИСЛОВЫЕ — только при значении ≠ 0 (witnessed ERP: 13 разных наборов на
//!   258 файлов — ровно эта проекция; в т.ч. 11 файлов БЕЗ `weeksPeriod` ↔ Designer
//!   `WeeksPeriod="0"`); дни недели и месяцы — ПОВТОРЯЮЩИЕСЯ элементы с ИМЕНАМИ
//!   (`<weekDays>Mon</weekDays>`, `<months>Jan</months>`); вложенные суточные —
//!   `<dailySchedules beginTime=… endTime=… completionTime=… [repeatPeriodInDay=…]/>`.
//! * **Designer** — `<dir>/<Name>/Ext/Schedule.xml`: С BOM, **CRLF**, отступ — ТАБ, хвостового
//!   перевода строки НЕТ (та же обёрточная конвенция, что `Ext/Help.xml`/`Ext/Picture.xml`).
//!   Корень `<JobSchedule … version="2.20|2.21">` (версия формата дампа — ВХОД чтения, см.
//!   [`crate::sidecar_version`]) → `<Schedule …>` со ВСЕМИ 12 атрибутами ВСЕГДА (в фиксированном
//!   порядке); дни/месяцы — ОДИН элемент со СПИСКОМ ЧИСЕЛ через пробел
//!   (`<ent:WeekDays>1 2 3 4 5 6 7</ent:WeekDays>`); вложенные — `<ent:DetailedDailySchedules …>`.
//!
//! Канон [`Schedule`] = Designer-полнота (все поля явные, дни/месяцы — ЧИСЛА): именно её требует
//! cf-тело. EDT-ридер восстанавливает опущенные значения (КОРЕНЬ: числовая омиссия == 0 —
//! [`edt_root_omission_default`]; ВЛОЖЕННЫЙ: [`Schedule::platform_default`], `weeks_period` = 1),
//! EDT-райтер их снова опускает — §1.6: оба диалекта дают РАВНЫЙ [`Schedule`].
//!
//! # §1.0-самопроверка на read
//! Пере-сериализация канона в ИСХОДНЫЙ диалект (Designer — ВЕРСИЕЙ ИСТОЧНИКА) ОБЯЗАНА
//! воспроизвести исходные байты — иначе ГРОМКИЙ отказ (а не тихая нормализация непрошенного
//! кодирования). Это и есть гарантия byte-exact round-trip'а: любой незамоделированный
//! атрибут/элемент/порядок сразу виден.
//!
//! # Вложенные (`dailySchedules` / `DetailedDailySchedules`) — witnessed SSL
//! `ОбновлениеАгрегатов` (2) + ERP (7 у 5 носителей). EDT несёт у них 3 времени (+
//! `repeatPeriodInDay` при ≠ 0 — witnessed ERP `ПолучениеДанныхСмартвей` 3600), Designer — все
//! 12 атрибутов + ПУСТЫЕ `<ent:WeekDays/>`/`<ent:Months/>`; расхождение снимается тем, что
//! EDT-канон вложенного = [`Schedule::platform_default`] + witnessed-атрибуты (`weeks_period`
//! = 1, `days_repeat_period` = 0 — ровно то, что пишет Designer, ERP 7/7 подтверждает). §1.0:
//! EDT-`dailySchedules` с ЛЮБЫМ иным атрибутом — незасвидетельствованный шейп → отказ (а не
//! молча-потерянное поле).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use formats_xml::Element;
use morph1c_core::ir::{MetadataObject, Schedule};
use morph1c_core::version::FormatVersion;

use crate::ConvertError;

/// Вид, несущий расписание (единственный — RE: сайдкар есть только у `ScheduledJobs/`).
const SCHEDULE_KIND: &str = "ScheduledJob";
/// Имя EDT-сайдкара (рядом с `.mdo`).
const EDT_FILE: &str = "Schedule.schedule";
/// Имя Designer-сайдкара (внутри `Ext/`).
const DESIGNER_FILE: &str = "Schedule.xml";
/// UTF-8 BOM — Designer-сайдкар его несёт, EDT нет.
const BOM: char = '\u{FEFF}';

/// Канонический ПОРЯДОК полей расписания == порядок атрибутов Designer `<Schedule …>` (witnessed
/// 45/45 — один и тот же) И порядок «схемы» EDT (тот же, но с опущенными дефолтами). Держим ОДИН
/// список, чтобы оба райтера не разъехались.
///
/// Пары `(designer_attr, edt_attr)`; доступ к значению — через [`field_value`]/[`set_field`].
const FIELDS: &[(&str, &str)] = &[
    ("BeginDate", "beginDate"),
    ("EndDate", "endDate"),
    ("BeginTime", "beginTime"),
    ("EndTime", "endTime"),
    ("CompletionTime", "completionTime"),
    ("CompletionInterval", "completionInterval"),
    ("RepeatPeriodInDay", "repeatPeriodInDay"),
    ("RepeatPause", "repeatPause"),
    ("WeekDayInMonth", "weekDayInMonth"),
    ("DayInMonth", "dayInMonth"),
    ("WeeksPeriod", "weeksPeriod"),
    ("DaysRepeatPeriod", "daysRepeatPeriod"),
];

/// EDT-порядок атрибутов КОРНЯ (witnessed: `beginDate endDate daysRepeatPeriod beginTime endTime
/// completionTime repeatPeriodInDay repeatPause weeksPeriod dayInMonth` — `daysRepeatPeriod`
/// стоит ТРЕТЬИМ, не последним, а `dayInMonth` — последним; отличается от Designer-порядка).
/// `weekDayInMonth` — ПОСЛЕ `weeksPeriod` (witnessed ERP `ФормированиеСегментов`:
/// `…weeksPeriod="1" weekDayInMonth="1"`; в SSL атрибут не встречался ни разу — прежняя позиция
/// «перед weeksPeriod» была невитнессированной догадкой). §1.0: порядок структурен
/// (байт-точность), поэтому — отдельный витнессированный список.
const EDT_ROOT_ORDER: &[&str] = &[
    "beginDate",
    "endDate",
    "daysRepeatPeriod",
    "beginTime",
    "endTime",
    "completionTime",
    "completionInterval",
    "repeatPeriodInDay",
    "repeatPause",
    "weeksPeriod",
    "weekDayInMonth",
    "dayInMonth",
];

/// EDT-атрибуты КОРНЯ, которые пишутся ВСЕГДА — даже со значением дефолта (witnessed SSL 45/45
/// И ERP 258/258: именно эти ПЯТЬ присутствуют во ВСЕХ файлах корпусов). Остальные (все —
/// ЧИСЛОВЫЕ) пишутся ⇔ значение ≠ 0 ([`edt_root_omission_default`]): SSL 45/45 нёс
/// `weeksPeriod="1"` и выглядел как «всегда», но ERP witnessed 11 файлов БЕЗ `weeksPeriod` ↔
/// Designer `WeeksPeriod="0"` (класс багов `const-modeled settable field`).
const EDT_ROOT_ALWAYS: &[&str] = &[
    "beginDate",
    "endDate",
    "beginTime",
    "endTime",
    "completionTime",
];

/// EDT-атрибуты ВЛОЖЕННОГО `<dailySchedules>`: три времени — ВСЕГДА (witnessed SSL 2/2 + ERP
/// 7/7), `repeatPeriodInDay` — при ≠ 0 (witnessed ERP `ПолучениеДанныхСмартвей`: 3600 ↔
/// Designer-вложенный `RepeatPeriodInDay="3600"`). Ни дат, ни `weeksPeriod` вложенный НЕ несёт:
/// это ДРУГОЙ, урезанный EDT-тип.
const EDT_NESTED_ATTRS: &[&str] = &["beginTime", "endTime", "completionTime"];

/// EDT-атрибуты ВЛОЖЕННОГО, пишущиеся ТОЛЬКО при значении ≠ 0 (порядок эмиссии — хвост после
/// [`EDT_NESTED_ATTRS`], как в [`EDT_ROOT_ORDER`]).
const EDT_NESTED_OPTIONAL_ATTRS: &[&str] = &["repeatPeriodInDay"];

/// Канон ОМИССИИ атрибута в EDT-КОРНЕ: даты/времена нулевые, ВСЕ числовые == 0 (в отличие от
/// [`Schedule::platform_default`], где `weeks_period` == 1 — то дефолт ВЛОЖЕННОГО узла).
/// Witnessed ERP: 11 корней без `weeksPeriod` ↔ Designer `WeeksPeriod="0"` (никогда `="1"`).
fn edt_root_omission_default() -> Schedule {
    Schedule {
        weeks_period: 0,
        ..Schedule::platform_default()
    }
}

/// Имена дней недели в EDT (индекс+1 == канон 1..7, Пн..Вс).
const EDT_WEEK_DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
/// Имена месяцев в EDT (индекс+1 == канон 1..12).
const EDT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Значение поля расписания по КАНОНИЧЕСКОМУ имени (Designer-атрибут) — как текст, ровно в том
/// виде, в каком оно едет в XML обоих диалектов.
fn field_value(s: &Schedule, designer_name: &str) -> String {
    match designer_name {
        "BeginDate" => s.begin_date.clone(),
        "EndDate" => s.end_date.clone(),
        "BeginTime" => s.begin_time.clone(),
        "EndTime" => s.end_time.clone(),
        "CompletionTime" => s.completion_time.clone(),
        "CompletionInterval" => s.completion_interval.to_string(),
        "RepeatPeriodInDay" => s.repeat_period_in_day.to_string(),
        "RepeatPause" => s.repeat_pause.to_string(),
        "WeekDayInMonth" => s.week_day_in_month.to_string(),
        "DayInMonth" => s.day_in_month.to_string(),
        "WeeksPeriod" => s.weeks_period.to_string(),
        "DaysRepeatPeriod" => s.days_repeat_period.to_string(),
        other => unreachable!("unknown schedule field {other}"),
    }
}

/// Записать значение поля расписания по КАНОНИЧЕСКОМУ имени. Числовое поле с не-числом →
/// типизированный отказ (§1.0).
fn set_field(s: &mut Schedule, designer_name: &str, raw: &str, ctx: &str) -> Result<(), String> {
    let num = || -> Result<i64, String> {
        raw.parse::<i64>()
            .map_err(|e| format!("{ctx}: {designer_name}={raw:?} is not an integer: {e}"))
    };
    match designer_name {
        "BeginDate" => s.begin_date = raw.to_string(),
        "EndDate" => s.end_date = raw.to_string(),
        "BeginTime" => s.begin_time = raw.to_string(),
        "EndTime" => s.end_time = raw.to_string(),
        "CompletionTime" => s.completion_time = raw.to_string(),
        "CompletionInterval" => s.completion_interval = num()?,
        "RepeatPeriodInDay" => s.repeat_period_in_day = num()?,
        "RepeatPause" => s.repeat_pause = num()?,
        "WeekDayInMonth" => s.week_day_in_month = num()?,
        "DayInMonth" => s.day_in_month = num()?,
        "WeeksPeriod" => s.weeks_period = num()?,
        "DaysRepeatPeriod" => s.days_repeat_period = num()?,
        other => return Err(format!("{ctx}: unknown schedule field {other}")),
    }
    Ok(())
}

/// EDT-имя атрибута → каноническое (Designer) имя поля.
fn canon_of_edt(edt_name: &str) -> Option<&'static str> {
    FIELDS.iter().find(|(_, e)| *e == edt_name).map(|(d, _)| *d)
}

/// Подгрузить расписание (если сайдкар существует) в `obj.schedule`. Не-`ScheduledJob` виды и
/// cf (контейнер) — no-op; ScheduledJob БЕЗ сайдкара — тоже no-op (witnessed: 5 из 50).
///
/// §1.0: сайдкар, чья пере-сериализация не воспроизводит исходные байты → ГРОМКИЙ отказ
/// (см. модульный docstring).
pub fn attach_schedule(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if kind != SCHEDULE_KIND || format == Format::Cf {
        return Ok(());
    }
    let Some(path) = sidecar_path(format, descriptor_path) else {
        return Ok(());
    };
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason,
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| read_err(format!("schedule {} is not UTF-8: {e}", path.display())))?;
    let doc = formats_xml::parse(text.as_bytes())
        .map_err(|e| read_err(format!("schedule {} XML: {e}", path.display())))?;
    let ctx = format!("schedule {}", path.display());
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (Designer —
    // ВЕРСИЕЙ ИСТОЧНИКА: версия — свойство файла, детектится из его корня, §1.6).
    let (schedule, back) = match format {
        Format::Edt => {
            let s = parse_edt(&doc.root, &ctx).map_err(read_err)?;
            let back = serialize_edt(&s);
            (s, back)
        }
        Format::Designer => {
            let (s, version) = parse_designer(&doc.root, &ctx).map_err(read_err)?;
            let back = serialize_designer(&s, version);
            (s, back)
        }
        Format::Cf => unreachable!("cf returned above"),
    };
    if back != bytes {
        return Err(read_err(format!(
            "schedule {} does not round-trip byte-exactly through the IR (the sidecar carries a \
             shape this codec does not model — refusing to silently drop it, §1.0)",
            path.display()
        )));
    }
    // §1.0: дескриптор-read расписание НЕ заполняет — только этот проход.
    if obj.schedule.is_some() {
        return Err(read_err(
            "object already carries a schedule before the sidecar attach (unexpected — the \
             descriptor projection must not populate it)"
                .into(),
        ));
    }
    obj.schedule = Some(schedule);
    Ok(())
}

/// Write-side mirror of [`attach_schedule`]: эмитить `obj.schedule` рядом с только что записанным
/// дескриптором в раскладке/кодировке ЦЕЛЕВОГО формата. cf (контейнер — тело собирает
/// cf-ассемблер) и объект без расписания — no-op. §1.0: расписание у вида БЕЗ раскладки сайдкара
/// — типизированный отказ (тихий дроп запрещён).
pub fn write_schedule(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    let Some(schedule) = obj.schedule.as_ref() else {
        return Ok(());
    };
    if format == Format::Cf {
        return Ok(());
    }
    if kind != SCHEDULE_KIND {
        return Err(ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object carries a schedule but only ScheduledJob has a witnessed schedule \
                     sidecar layout (§1.0 — silent drop forbidden)"
                .into(),
        });
    }
    let path = sidecar_path(format, descriptor_out).ok_or_else(|| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: "descriptor path has no parent/stem to anchor the schedule sidecar at".into(),
    })?;
    crate::form_write::write_file(&path, &serialize(format, schedule))
}

/// Путь сайдкара расписания относительно дескриптора: EDT `<obj-dir>/Schedule.schedule`,
/// Designer `<dir>/<Name>/Ext/Schedule.xml`.
fn sidecar_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => Some(descriptor_path.parent()?.join(EDT_FILE)),
        Format::Designer => Some(
            descriptor_path
                .parent()?
                .join(descriptor_path.file_stem()?)
                .join("Ext")
                .join(DESIGNER_FILE),
        ),
        Format::Cf => None,
    }
}

// ---------------------------------------------------------------------------------------------
// READ
// ---------------------------------------------------------------------------------------------

/// EDT: `<schedule:Schedule …>` → канон. Опущенные атрибуты КОРНЯ = канон омиссии
/// ([`edt_root_omission_default`] — числовые 0); дни/месяцы — повторяющиеся элементы с ИМЕНАМИ.
fn parse_edt(root: &Element, ctx: &str) -> Result<Schedule, String> {
    if root.local != "Schedule" {
        return Err(format!(
            "{ctx}: root is <{}>, expected <Schedule>",
            root.local
        ));
    }
    parse_edt_node(root, ctx, false)
}

/// Один EDT-узел расписания (корень либо вложенный `<dailySchedules>`). `nested` включает
/// §1.0-гейт «у вложенного witnessed только времена + `repeatPeriodInDay`» и выбирает канон
/// омиссии: корень — [`edt_root_omission_default`] (числа == 0), вложенный —
/// [`Schedule::platform_default`] (`weeks_period` == 1 — ровно то, что пишет Designer).
fn parse_edt_node(el: &Element, ctx: &str, nested: bool) -> Result<Schedule, String> {
    let mut s = if nested {
        Schedule::platform_default()
    } else {
        edt_root_omission_default()
    };
    for a in &el.attrs {
        if a.name.starts_with("xmlns") {
            continue; // объявление ns — не поле.
        }
        if nested
            && !EDT_NESTED_ATTRS.contains(&a.name.as_str())
            && !EDT_NESTED_OPTIONAL_ATTRS.contains(&a.name.as_str())
        {
            return Err(format!(
                "{ctx}: nested <dailySchedules> carries attribute {:?} — only {:?} + {:?} are \
                 witnessed (§1.0 — unwitnessed shape)",
                a.name, EDT_NESTED_ATTRS, EDT_NESTED_OPTIONAL_ATTRS
            ));
        }
        let canon = canon_of_edt(&a.name).ok_or_else(|| {
            format!(
                "{ctx}: unknown EDT schedule attribute {:?} (§1.0 — a field this codec does not \
                 model would be silently lost)",
                a.name
            )
        })?;
        set_field(&mut s, canon, &a.value, ctx)?;
    }
    for child in &el.children {
        match child.local.as_str() {
            "weekDays" => s
                .week_days
                .push(index_of(&EDT_WEEK_DAYS, &child.text, ctx, "weekDays")?),
            "months" => s
                .months
                .push(index_of(&EDT_MONTHS, &child.text, ctx, "months")?),
            "dailySchedules" => s.daily_schedules.push(parse_edt_node(child, ctx, true)?),
            other => {
                return Err(format!(
                    "{ctx}: unknown EDT schedule element <{other}> (§1.0)"
                ))
            }
        }
    }
    Ok(s)
}

/// Имя дня/месяца → 1-based индекс. §1.0: неизвестное имя — отказ.
fn index_of(table: &[&str], name: &str, ctx: &str, what: &str) -> Result<i64, String> {
    table
        .iter()
        .position(|n| *n == name)
        .map(|i| i as i64 + 1)
        .ok_or_else(|| format!("{ctx}: unknown {what} value {name:?} (known: {table:?}) — §1.0"))
}

/// Designer: `<JobSchedule version="…"><Schedule …>` → (канон, witnessed-версия формата
/// источника). Все 12 атрибутов присутствуют всегда; дни/месяцы — списки чисел через пробел.
/// §1.0: отсутствующая/невитнессированная версия корня — отказ (см. [`crate::sidecar_version`]).
fn parse_designer(root: &Element, ctx: &str) -> Result<(Schedule, FormatVersion), String> {
    if root.local != "JobSchedule" {
        return Err(format!(
            "{ctx}: root is <{}>, expected <JobSchedule>",
            root.local
        ));
    }
    let version = root
        .attr("version")
        .ok_or_else(|| format!("{ctx}: <JobSchedule> root has no version attribute (§1.0)"))?;
    let version = crate::sidecar_version::parse_witnessed(&version.value, ctx)?;
    let sched = root
        .child("Schedule")
        .ok_or_else(|| format!("{ctx}: <JobSchedule> has no <Schedule> child (§1.0)"))?;
    Ok((parse_designer_node(sched, ctx)?, version))
}

/// Один Designer-узел расписания (`<Schedule>` либо `<ent:DetailedDailySchedules>`).
fn parse_designer_node(el: &Element, ctx: &str) -> Result<Schedule, String> {
    let mut s = Schedule::platform_default();
    for a in &el.attrs {
        if a.name.starts_with("xmlns") {
            continue;
        }
        if !FIELDS.iter().any(|(d, _)| *d == a.name) {
            return Err(format!(
                "{ctx}: unknown Designer schedule attribute {:?} (§1.0 — a field this codec does \
                 not model would be silently lost)",
                a.name
            ));
        }
        set_field(&mut s, &a.name, &a.value, ctx)?;
    }
    for child in &el.children {
        match child.local.as_str() {
            "WeekDays" => s.week_days = parse_num_list(&child.text, ctx, "WeekDays")?,
            "Months" => s.months = parse_num_list(&child.text, ctx, "Months")?,
            "DetailedDailySchedules" => {
                s.daily_schedules.push(parse_designer_node(child, ctx)?);
            }
            other => {
                return Err(format!(
                    "{ctx}: unknown Designer schedule element <{other}> (§1.0)"
                ))
            }
        }
    }
    Ok(s)
}

/// `"1 2 3"` → `[1,2,3]` (пусто → `[]`). §1.0: не-число — отказ.
fn parse_num_list(text: &str, ctx: &str, what: &str) -> Result<Vec<i64>, String> {
    text.split_whitespace()
        .map(|t| {
            t.parse::<i64>()
                .map_err(|e| format!("{ctx}: {what} item {t:?} is not an integer: {e}"))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// WRITE (byte-exact — см. §1.0-самопроверку на read)
// ---------------------------------------------------------------------------------------------

/// Канон → дисковые байты целевого диалекта. Designer штампуется версией АМБЬЕНТНОГО
/// round-trip-таргета ([`crate::sidecar_version::write_target`] — формат выхода — параметр,
/// FORMATS.md §1); §1.0-самопроверка на read сериализует ВЕРСИЕЙ ИСТОЧНИКА напрямую.
pub(crate) fn serialize(format: Format, s: &Schedule) -> Vec<u8> {
    match format {
        Format::Edt => serialize_edt(s),
        Format::Designer => serialize_designer(s, crate::sidecar_version::write_target()),
        Format::Cf => Vec::new(), // контейнер — не файловый сайдкар.
    }
}

/// EDT `Schedule.schedule`: БЕЗ BOM, CRLF, отступ 2 пробела, хвостовой перевод строки ЕСТЬ.
/// Пять «всегда-атрибутов» пишутся всегда; ЧИСЛОВЫЕ — только при значении ≠ 0
/// ([`edt_root_omission_default`]; witnessed SSL 45/45 + ERP 258/258).
fn serialize_edt(s: &Schedule) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str("<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\"");
    let def = edt_root_omission_default();
    for edt in EDT_ROOT_ORDER {
        let canon = canon_of_edt(edt).expect("EDT_ROOT_ORDER names are canonical");
        let v = field_value(s, canon);
        // Пять «всегда-атрибутов» пишутся даже с дефолтом; прочие — только при ≠ омиссии.
        if !EDT_ROOT_ALWAYS.contains(edt) && v == field_value(&def, canon) {
            continue;
        }
        out.push_str(&format!(" {edt}=\"{v}\""));
    }
    // БЕЗДЕТНЫЙ корень — САМОЗАКРЫТЫЙ тег (witnessed ERP: 11 файлов `…/>` без
    // `</schedule:Schedule>`; в SSL бездетных корней нет).
    if s.week_days.is_empty() && s.months.is_empty() && s.daily_schedules.is_empty() {
        out.push_str("/>\r\n");
        return out.into_bytes();
    }
    out.push_str(">\r\n");
    for d in &s.week_days {
        out.push_str(&format!(
            "  <weekDays>{}</weekDays>\r\n",
            EDT_WEEK_DAYS[(*d - 1) as usize]
        ));
    }
    for m in &s.months {
        out.push_str(&format!(
            "  <months>{}</months>\r\n",
            EDT_MONTHS[(*m - 1) as usize]
        ));
    }
    for n in &s.daily_schedules {
        out.push_str("  <dailySchedules");
        for edt in EDT_NESTED_ATTRS {
            let canon = canon_of_edt(edt).expect("nested attrs are canonical");
            out.push_str(&format!(" {edt}=\"{}\"", field_value(n, canon)));
        }
        // Хвостовые опциональные атрибуты вложенного — только при ≠ 0 (witnessed ERP
        // `ПолучениеДанныхСмартвей`: `repeatPeriodInDay="3600"` ПОСЛЕ трёх времён).
        for edt in EDT_NESTED_OPTIONAL_ATTRS {
            let canon = canon_of_edt(edt).expect("nested optional attrs are canonical");
            let v = field_value(n, canon);
            if v != "0" {
                out.push_str(&format!(" {edt}=\"{v}\""));
            }
        }
        out.push_str("/>\r\n");
    }
    out.push_str("</schedule:Schedule>\r\n");
    out.into_bytes()
}

/// Designer `Ext/Schedule.xml`: С BOM, CRLF, ТАБ-отступ, БЕЗ хвостового перевода строки. Все 12
/// атрибутов ВСЕГДА; дни/месяцы — списки чисел (пустой список — САМОЗАКРЫТЫЙ тег, witnessed у
/// вложенных). Корень штампуется `version` ЗАДАННОЙ версии формата (2.20 ERP / 2.21 SSL — оба
/// witnessed; ns-блок побайтно ОДИН И ТОТ ЖЕ в обоих корпусах, различается ТОЛЬКО `version=`).
fn serialize_designer(s: &Schedule, version: FormatVersion) -> Vec<u8> {
    let mut out = String::new();
    out.push(BOM);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str(&format!(
        "<JobSchedule xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
         xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n",
    ));
    serialize_designer_node(&mut out, s, "Schedule", 1);
    out.push_str("</JobSchedule>");
    out.into_bytes()
}

/// Один Designer-узел (`<Schedule>` / `<ent:DetailedDailySchedules>`) с `depth` табами отступа.
fn serialize_designer_node(out: &mut String, s: &Schedule, tag: &str, depth: usize) {
    let pad = "\t".repeat(depth);
    let inner = "\t".repeat(depth + 1);
    // Вложенные узлы несут ns-префикс `ent:`, корневой `<Schedule>` — нет (witnessed).
    let qname = if depth == 1 {
        tag.to_string()
    } else {
        format!("ent:{tag}")
    };
    out.push_str(&format!("{pad}<{qname}"));
    for (d, _) in FIELDS {
        out.push_str(&format!(" {d}=\"{}\"", field_value(s, d)));
    }
    out.push_str(">\r\n");
    write_num_list(out, &inner, "WeekDays", &s.week_days);
    write_num_list(out, &inner, "Months", &s.months);
    for n in &s.daily_schedules {
        serialize_designer_node(out, n, "DetailedDailySchedules", depth + 1);
    }
    out.push_str(&format!("{pad}</{qname}>\r\n"));
}

/// `<ent:WeekDays>1 2 3</ent:WeekDays>` — либо САМОЗАКРЫТЫЙ `<ent:WeekDays/>` при пустом списке
/// (witnessed у вложенных суточных расписаний).
fn write_num_list(out: &mut String, pad: &str, tag: &str, items: &[i64]) {
    if items.is_empty() {
        out.push_str(&format!("{pad}<ent:{tag}/>\r\n"));
        return;
    }
    let list: Vec<String> = items.iter().map(|i| i.to_string()).collect();
    out.push_str(&format!(
        "{pad}<ent:{tag}>{}</ent:{tag}>\r\n",
        list.join(" ")
    ));
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Витнессированные дисковые байты ОДНОГО И ТОГО ЖЕ расписания SSL `ЗагрузкаКурсовВалют`
    /// в двух диалектах (беру дословно из корпуса — это и есть эталон byte-exact).
    const EDT_SRC: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\" ",
        "beginDate=\"0001-01-01\" endDate=\"0001-01-01\" daysRepeatPeriod=\"1\" ",
        "beginTime=\"08:15:00\" endTime=\"00:00:00\" completionTime=\"00:00:00\" ",
        "weeksPeriod=\"1\">\r\n",
        "  <weekDays>Mon</weekDays>\r\n  <weekDays>Tue</weekDays>\r\n",
        "  <weekDays>Wed</weekDays>\r\n  <weekDays>Thu</weekDays>\r\n",
        "  <weekDays>Fri</weekDays>\r\n  <weekDays>Sat</weekDays>\r\n",
        "  <weekDays>Sun</weekDays>\r\n",
        "  <months>Jan</months>\r\n  <months>Feb</months>\r\n  <months>Mar</months>\r\n",
        "  <months>Apr</months>\r\n  <months>May</months>\r\n  <months>Jun</months>\r\n",
        "  <months>Jul</months>\r\n  <months>Aug</months>\r\n  <months>Sep</months>\r\n",
        "  <months>Oct</months>\r\n  <months>Nov</months>\r\n  <months>Dec</months>\r\n",
        "</schedule:Schedule>\r\n"
    );

    const DESIGNER_SRC: &str = concat!(
        "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<JobSchedule xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" ",
        "xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
        "\t<Schedule BeginDate=\"0001-01-01\" EndDate=\"0001-01-01\" BeginTime=\"08:15:00\" ",
        "EndTime=\"00:00:00\" CompletionTime=\"00:00:00\" CompletionInterval=\"0\" ",
        "RepeatPeriodInDay=\"0\" RepeatPause=\"0\" WeekDayInMonth=\"0\" DayInMonth=\"0\" ",
        "WeeksPeriod=\"1\" DaysRepeatPeriod=\"1\">\r\n",
        "\t\t<ent:WeekDays>1 2 3 4 5 6 7</ent:WeekDays>\r\n",
        "\t\t<ent:Months>1 2 3 4 5 6 7 8 9 10 11 12</ent:Months>\r\n",
        "\t</Schedule>\r\n",
        "</JobSchedule>"
    );

    fn parse(format: Format, src: &str) -> Schedule {
        let doc = formats_xml::parse(src.as_bytes()).expect("xml");
        match format {
            Format::Edt => parse_edt(&doc.root, "t").expect("edt"),
            Format::Designer => parse_designer(&doc.root, "t").expect("designer").0,
            Format::Cf => unreachable!(),
        }
    }

    /// §1.6: ОБА диалекта дают РАВНЫЙ канон (EDT-опущенные дефолты восстановлены).
    #[test]
    fn both_dialects_yield_the_same_canonical_schedule() {
        let e = parse(Format::Edt, EDT_SRC);
        let d = parse(Format::Designer, DESIGNER_SRC);
        assert_eq!(e, d, "edt canon == designer canon (§1.6)");
        assert_eq!(e.begin_time, "08:15:00");
        assert_eq!(e.days_repeat_period, 1);
        assert_eq!(e.weeks_period, 1);
        assert_eq!(e.week_days, (1..=7).collect::<Vec<_>>());
        assert_eq!(e.months, (1..=12).collect::<Vec<_>>());
    }

    /// Byte-exact round-trip в ОБОИХ диалектах (это же и есть §1.0-самопроверка на read).
    #[test]
    fn serialize_reproduces_the_witnessed_bytes() {
        let e = parse(Format::Edt, EDT_SRC);
        assert_eq!(
            String::from_utf8(serialize(Format::Edt, &e)).unwrap(),
            EDT_SRC,
            "EDT byte-exact"
        );
        let d = parse(Format::Designer, DESIGNER_SRC);
        assert_eq!(
            String::from_utf8(serialize(Format::Designer, &d)).unwrap(),
            DESIGNER_SRC,
            "Designer byte-exact"
        );
        // …и КРОСС-диалектно: канон из EDT пишется в байт-точный Designer и наоборот (§1.6).
        assert_eq!(
            String::from_utf8(serialize(Format::Designer, &e)).unwrap(),
            DESIGNER_SRC,
            "edt canon → designer bytes"
        );
        assert_eq!(
            String::from_utf8(serialize(Format::Edt, &d)).unwrap(),
            EDT_SRC,
            "designer canon → edt bytes"
        );
    }

    /// ВЛОЖЕННЫЕ суточные расписания (SSL `ОбновлениеАгрегатов` — единственный носитель):
    /// EDT несёт у них только 3 времени, Designer — все 12 атрибутов + пустые списки.
    #[test]
    fn nested_daily_schedules_roundtrip_in_both_dialects() {
        const EDT_NESTED: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\" ",
            "beginDate=\"0001-01-01\" endDate=\"0001-01-01\" daysRepeatPeriod=\"1\" ",
            "beginTime=\"01:00:00\" endTime=\"00:00:00\" completionTime=\"00:00:00\" ",
            "weeksPeriod=\"1\">\r\n",
            "  <weekDays>Mon</weekDays>\r\n",
            "  <months>Jan</months>\r\n",
            "  <dailySchedules beginTime=\"01:00:00\" endTime=\"00:00:00\" ",
            "completionTime=\"00:00:00\"/>\r\n",
            "  <dailySchedules beginTime=\"14:00:00\" endTime=\"00:00:00\" ",
            "completionTime=\"00:00:00\"/>\r\n",
            "</schedule:Schedule>\r\n"
        );
        let s = parse(Format::Edt, EDT_NESTED);
        assert_eq!(s.daily_schedules.len(), 2);
        // Вложенный канон = платформенный дефолт + времена (weeks_period 1, days_repeat 0).
        assert_eq!(s.daily_schedules[1].begin_time, "14:00:00");
        assert_eq!(s.daily_schedules[1].weeks_period, 1);
        assert_eq!(s.daily_schedules[1].days_repeat_period, 0);
        assert!(s.daily_schedules[1].week_days.is_empty());
        assert_eq!(
            String::from_utf8(serialize(Format::Edt, &s)).unwrap(),
            EDT_NESTED,
            "EDT nested byte-exact"
        );
        // Designer-проекция того же канона → пере-читывается в РАВНЫЙ канон (§1.6).
        let d_bytes = serialize(Format::Designer, &s);
        let d_doc = formats_xml::parse(&d_bytes).expect("xml");
        assert_eq!(parse_designer(&d_doc.root, "t").expect("designer").0, s);
    }

    /// ERP-витнесс `мирОбновлениеИсторииДанных` (дословные байты корпуса): Designer 2.20 с
    /// `WeeksPeriod="0"` ↔ EDT БЕЗ `weeksPeriod` и с САМОЗАКРЫТЫМ бездетным корнем. Ловит оба
    /// ERP-отличия EDT-корня разом (омиссия числовых == 0; бездетный корень — `/>`).
    #[test]
    fn erp_2_20_witness_roundtrips_in_both_dialects() {
        const EDT_ERP: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\" ",
            "beginDate=\"0001-01-01\" endDate=\"0001-01-01\" beginTime=\"00:00:00\" ",
            "endTime=\"00:00:00\" completionTime=\"00:00:00\"/>\r\n"
        );
        const DESIGNER_ERP: &str = concat!(
            "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<JobSchedule xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" ",
            "xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" ",
            "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
            "\t<Schedule BeginDate=\"0001-01-01\" EndDate=\"0001-01-01\" BeginTime=\"00:00:00\" ",
            "EndTime=\"00:00:00\" CompletionTime=\"00:00:00\" CompletionInterval=\"0\" ",
            "RepeatPeriodInDay=\"0\" RepeatPause=\"0\" WeekDayInMonth=\"0\" DayInMonth=\"0\" ",
            "WeeksPeriod=\"0\" DaysRepeatPeriod=\"0\">\r\n",
            "\t\t<ent:WeekDays/>\r\n",
            "\t\t<ent:Months/>\r\n",
            "\t</Schedule>\r\n",
            "</JobSchedule>"
        );
        let e = parse(Format::Edt, EDT_ERP);
        assert_eq!(
            e.weeks_period, 0,
            "root omission == 0 (NOT platform_default 1)"
        );
        let doc = formats_xml::parse(DESIGNER_ERP.as_bytes()).expect("xml");
        let (d, version) = parse_designer(&doc.root, "t").expect("designer");
        assert_eq!(
            version,
            morph1c_core::version::ERP,
            "detected source version"
        );
        assert_eq!(e, d, "edt canon == designer canon (§1.6)");
        // Byte-exact в обоих диалектах; Designer — ВЕРСИЕЙ ИСТОЧНИКА.
        assert_eq!(
            String::from_utf8(serialize_edt(&e)).unwrap(),
            EDT_ERP,
            "EDT byte-exact (self-closed childless root, weeksPeriod omitted)"
        );
        assert_eq!(
            String::from_utf8(serialize_designer(&d, version)).unwrap(),
            DESIGNER_ERP,
            "Designer byte-exact under the 2.20 source version"
        );
        // Write-side: амбьентный round-trip-таргет ERP даёт те же 2.20-байты.
        let via_ambient =
            morph1c_core::version::with_roundtrip_target(morph1c_core::version::ERP, || {
                serialize(Format::Designer, &e)
            });
        assert_eq!(String::from_utf8(via_ambient).unwrap(), DESIGNER_ERP);
    }

    /// ERP-витнесс `ПолучениеДанныхСмартвей` (выжимка): вложенный `<dailySchedules>` несёт
    /// `repeatPeriodInDay="3600"` ХВОСТОМ после трёх времён; корень — `repeatPeriodInDay`
    /// ПЕРЕД `weeksPeriod` (порядок схемы).
    #[test]
    fn erp_nested_repeat_period_roundtrips() {
        const EDT_NESTED_REPEAT: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\" ",
            "beginDate=\"0001-01-01\" endDate=\"0001-01-01\" daysRepeatPeriod=\"1\" ",
            "beginTime=\"00:00:00\" endTime=\"00:00:00\" completionTime=\"00:00:00\" ",
            "repeatPeriodInDay=\"3600\" weeksPeriod=\"1\">\r\n",
            "  <weekDays>Mon</weekDays>\r\n",
            "  <months>Jan</months>\r\n",
            "  <dailySchedules beginTime=\"00:00:00\" endTime=\"00:00:00\" ",
            "completionTime=\"00:00:00\" repeatPeriodInDay=\"3600\"/>\r\n",
            "</schedule:Schedule>\r\n"
        );
        let s = parse(Format::Edt, EDT_NESTED_REPEAT);
        assert_eq!(s.repeat_period_in_day, 3600);
        assert_eq!(s.daily_schedules.len(), 1);
        assert_eq!(s.daily_schedules[0].repeat_period_in_day, 3600);
        assert_eq!(
            s.daily_schedules[0].weeks_period, 1,
            "nested default stays 1"
        );
        assert_eq!(
            String::from_utf8(serialize_edt(&s)).unwrap(),
            EDT_NESTED_REPEAT,
            "EDT byte-exact incl. nested repeatPeriodInDay tail attr"
        );
        // Designer-проекция пере-читывается в РАВНЫЙ канон (§1.6).
        let d_bytes = serialize_designer(&s, morph1c_core::version::ERP);
        let d_doc = formats_xml::parse(&d_bytes).expect("xml");
        assert_eq!(parse_designer(&d_doc.root, "t").expect("designer").0, s);
    }

    /// §1.0: Designer-сайдкар с НЕвитнессированной версией формата → ГРОМКИЙ отказ (иначе она
    /// была бы молча перештампована таргетом).
    #[test]
    fn unwitnessed_designer_version_is_refused() {
        let bad = DESIGNER_SRC.replace("version=\"2.21\"", "version=\"2.19\"");
        let doc = formats_xml::parse(bad.as_bytes()).expect("xml");
        let err = parse_designer(&doc.root, "t").unwrap_err();
        assert!(err.contains("not a witnessed"), "{err}");
    }

    /// §1.0: незамоделированный атрибут вложенного `<dailySchedules>` → ГРОМКИЙ отказ.
    #[test]
    fn unwitnessed_nested_attribute_is_refused() {
        const BAD: &str = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<schedule:Schedule xmlns:schedule=\"http://g5.1c.ru/v8/dt/schedule\" ",
            "weeksPeriod=\"1\">\r\n",
            "  <dailySchedules beginTime=\"01:00:00\" dayInMonth=\"3\"/>\r\n",
            "</schedule:Schedule>\r\n"
        );
        let doc = formats_xml::parse(BAD.as_bytes()).expect("xml");
        let err = parse_edt(&doc.root, "t").unwrap_err();
        assert!(err.contains("only"), "got {err}");
    }
}
