//! Канонический спек вида объекта `DocumentNumerator` (нумератор документов) —
//! ARCHITECTURE.md §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта.
//!
//! Метамодельные поля `objectBelonging` / `extendedConfigurationObject` (EDT-xcore) в
//! корпусе ERP отсутствуют (n=0) и НЕ включены: их эмиссия ломает Designer DENSE
//! byte-exact (Designer их не несёт), что совпадает с конвенцией — не-generated спеки
//! `objectBelonging` не держат. Если появится корпус с этими полями — расширить спек.
//!
//! Поля (подтверждено корпусом ERP, 5 объектов edt+designer; Designer DENSE — все поля
//! present всегда, EDT SPARSE — дефолты опущены):
//! * `synonym` — локализованный синоним;
//! * `comment` — свободный текст (default `""`);
//! * `numberType` — тип номера `{Number|String}` (default `Number`);
//! * `numberLength` — длина номера (Int; default `0`);
//! * `numberAllowedLength` — допустимая длина `{Fixed|Variable}` (default `Fixed`);
//! * `numberPeriodicity` — периодичность `{Nonperiodical|Year|Quarter|Month|Day}`
//!   (default `Nonperiodical`);
//! * `checkUnique` — контроль уникальности (bool; default `false`).
//!
//! Лист-вид: подчинённых коллекций и `standardAttributes` нет. Порядок = DENSE-порядок
//! Designer `<Properties>`.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `numberType` — тип номера `{Number|String}`. Default = `Number`.
pub const F_NUMBER_TYPE: FieldId = FieldId(3);
/// `numberLength` — длина номера (Int). Default = `0`.
pub const F_NUMBER_LENGTH: FieldId = FieldId(4);
/// `numberAllowedLength` — допустимая длина `{Fixed|Variable}`. Default = `Fixed`.
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(5);
/// `numberPeriodicity` — периодичность `{Nonperiodical|Year|Quarter|Month|Day}`.
/// Default = `Nonperiodical`.
pub const F_NUMBER_PERIODICITY: FieldId = FieldId(6);
/// `checkUnique` — контроль уникальности (bool). Default = `false`.
pub const F_CHECK_UNIQUE: FieldId = FieldId(7);

/// Сконструировать [`FieldSpec`] вида `DocumentNumerator` в каноническом порядке.
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
        FieldSpec::with_default(
            F_NUMBER_TYPE,
            "numberType",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Number")),
        ),
        FieldSpec::with_default(F_NUMBER_LENGTH, "numberLength", ValueKind::Int, PropertyValue::Int(0)),
        FieldSpec::with_default(
            F_NUMBER_ALLOWED_LENGTH,
            "numberAllowedLength",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Fixed")),
        ),
        FieldSpec::with_default(
            F_NUMBER_PERIODICITY,
            "numberPeriodicity",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Nonperiodical")),
        ),
        FieldSpec::with_default(F_CHECK_UNIQUE, "checkUnique", ValueKind::Bool, PropertyValue::Bool(false)),
    ]
}

/// Канонический [`EntitySpec`] вида `DocumentNumerator` (кэш на процесс).
pub fn document_numerator() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DocumentNumerator",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
