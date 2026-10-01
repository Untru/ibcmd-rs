//! Канонический спек вида объекта `SessionParameter` (параметр сеанса) —
//! ARCHITECTURE.md §1.4/§1.6.
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) — идентичность
//! объекта в [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 62 объекта edt+designer): `Synonym`, `Comment`,
//! `Type`. Платформенно-вычисляемых объектных полей у SessionParameter НЕТ (в отличие
//! от DefinedType `producedTypes`) — дескриптор несёт ровно эти три свойства плюс
//! идентичность. Порядок = канонический порядок эмиссии (Designer: Synonym, Comment,
//! Type; EDT — то же, дефолтные опущены).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа значения параметра сеанса (`ОписаниеТипов`). Required:
/// host-элемент `<type>`/`<Type>` присутствует у всех объектов (present-empty —
/// валидное значение, не дефолт-омиссия; см. дизайн §6).
pub const F_TYPE: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `SessionParameter` в каноническом порядке.
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
        // Type — обязательное поле (нет дефолта): описание типа присутствует всегда.
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
    ]
}

/// Канонический [`EntitySpec`] вида `SessionParameter` (кэш на процесс).
pub fn session_parameter() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "SessionParameter",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
