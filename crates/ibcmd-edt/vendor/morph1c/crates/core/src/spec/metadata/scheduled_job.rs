//! Канонический спек вида объекта `ScheduledJob` (регламентное задание) —
//! ARCHITECTURE.md §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта; И отдельный файл
//! `Schedule.schedule` каталога объекта (EDT) — это НЕ часть дескриптора `.mdo`/`.xml`
//! (Designer вовсе не несёт `<Schedule>` в `.xml`), а внешний под-файл расписания; в R/X
//! дескриптора он не участвует.
//!
//! Поля (подтверждено корпусом SSL, 50 объектов edt+designer; Designer DENSE — все поля
//! present всегда, EDT SPARSE — дефолты опущены):
//! * `Synonym` — локализованный синоним;
//! * `Comment` — свободный текст (default `""`);
//! * `MethodName` — ссылка-обработчик `CommonModule.X.Y`. Required (50/50);
//! * `Description` — описание задания (свободная строка; default `""`);
//! * `Key` — ключ задания (свободная строка; default `""`);
//! * `Use` — bool использования (default `false`; EDT presence-bool, Designer text-bool);
//! * `Predefined` — bool предопределённости (default `false`);
//! * `RestartCountOnFailure` — число рестартов при сбое (Int; default `0`);
//! * `RestartIntervalOnFailure` — интервал рестарта при сбое, сек (Int; default `0`).
//!
//! Лист-вид: подчинённых коллекций нет. Порядок = DENSE-порядок Designer `<Properties>`.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `methodName` — ссылка-обработчик (плоская строка). Required (present у всех).
pub const F_METHOD_NAME: FieldId = FieldId(3);
/// `description` — описание (свободная строка). Default = `""`.
pub const F_DESCRIPTION: FieldId = FieldId(4);
/// `key` — ключ задания (свободная строка). Default = `""`.
pub const F_KEY: FieldId = FieldId(5);
/// `use` — использование (bool). Default = `false`.
pub const F_USE: FieldId = FieldId(6);
/// `predefined` — предопределённость (bool). Default = `false`.
pub const F_PREDEFINED: FieldId = FieldId(7);
/// `restartCountOnFailure` — число рестартов при сбое (Int). Default = `0`.
pub const F_RESTART_COUNT: FieldId = FieldId(8);
/// `restartIntervalOnFailure` — интервал рестарта при сбое, сек (Int). Default = `0`.
pub const F_RESTART_INTERVAL: FieldId = FieldId(9);

/// Сконструировать [`FieldSpec`] вида `ScheduledJob` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // MethodName — плоская строка-ссылка (required).
        FieldSpec::required(F_METHOD_NAME, "methodName", ValueKind::Str),
        FieldSpec::with_default(F_DESCRIPTION, "description", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_KEY, "key", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_USE, "use", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(F_PREDEFINED, "predefined", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(F_RESTART_COUNT, "restartCountOnFailure", ValueKind::Int, PropertyValue::Int(0)),
        FieldSpec::with_default(
            F_RESTART_INTERVAL,
            "restartIntervalOnFailure",
            ValueKind::Int,
            PropertyValue::Int(0),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `ScheduledJob` (кэш на процесс).
pub fn scheduled_job() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ScheduledJob",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
