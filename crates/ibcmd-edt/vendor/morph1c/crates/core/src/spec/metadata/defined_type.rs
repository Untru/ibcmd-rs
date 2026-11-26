//! Канонический спек вида объекта `DefinedType` (определяемый тип) — ARCHITECTURE.md
//! §1.4/§1.6.
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! Спек-свойства (подтверждено корпусом SSL, 72 объекта edt+designer): `Synonym`,
//! `Comment`, `Type` — ровно как у [`crate::spec::metadata::session_parameter`]
//! (тот же Type-несущий substrate). Порядок = канонический порядок эмиссии (Designer:
//! Synonym, Comment, Type; EDT — то же, дефолтные опущены).
//!
//! ВНЕ этого спека (намеренно): `name`/`uuid` — идентичность объекта; И платформенный
//! блок `producedTypes`/`InternalInfo>xr:GeneratedType` — это
//! [`crate::ir::InternalInfo`] на [`crate::ir::MetadataObject`]. Блок СТРУКТУРНО лежит
//! ВНЕ спек-региона (до `<name>` в EDT, до `<Properties>` в Designer), поэтому
//! фреймится КАРКАСОМ коннектора, как идентичность, а не спек-движком. Он ИДЕНТИЧЕН
//! edt↔designer (72/72 GUID совпали), значит входит в канонический IR (X by
//! construction, §1.6/§3.5) — captured-and-byte-exact, НЕ источниковая проекция и НЕ
//! `Blob` (это структурный XML, §1.0).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа значения определяемого типа (`ОписаниеТипов`). Required:
/// host-элемент `<type>`/`<Type>` присутствует у всех объектов (present-empty —
/// валидное значение, не дефолт-омиссия; дизайн §6).
pub const F_TYPE: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `DefinedType` в каноническом порядке.
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

/// Канонический [`EntitySpec`] вида `DefinedType` (кэш на процесс).
pub fn defined_type() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DefinedType",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `DefinedType` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Container",
        edt_tag: "containerType",
        designer_category: "DefinedType",
        designer_type_name: "DefinedType",
        designer_order: 0,
    },
];
