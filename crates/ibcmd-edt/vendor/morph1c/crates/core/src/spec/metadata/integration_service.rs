//! Канонический спек вида объекта `IntegrationService` (сервис интеграции) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate.
//! ERP-only вид (в SSL отсутствует → cf-оракула нет, cf-строки инвентаря `n/a`).
//!
//! СТРУКТУРНЫЙ вид: корневые свойства + дочерняя коллекция `Channel`
//! (`IntegrationService.Channel` — recursion-узел с собственным
//! `producedTypes`/`InternalInfo`(Manager)). Родитель сам несёт `producedTypes`
//! (одна категория `Manager`) — R2 data-drive через per-kind [`PRODUCED_CATEGORIES`].
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта;
//! `producedTypes`/`InternalInfo` — платформенный каркас (рекурсия в `children.rs`).
//!
//! Метамодельные поля `objectBelonging`/`extendedConfigurationObject` (EDT-xcore) в
//! корпусе ERP отсутствуют (n=0) и НЕ включены (конвенция не-generated спеков, как у
//! `document_numerator`): их эмиссия сломала бы Designer DENSE byte-exact (Designer их не
//! несёт). Появится корпус с ними — расширить спек.
//!
//! Поля (Designer DENSE-порядок `<Properties>`, сверено корпусом ERP, 1 объект
//! edt+designer; Designer DENSE — все поля present всегда, EDT SPARSE — дефолты опущены):
//! * `synonym` — локализованный синоним (default []);
//! * `comment` — свободный текст (default `""`);
//! * `externalIntegrationServiceAddress` — внешний адрес сервиса (default `""`).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `externalIntegrationServiceAddress` — внешний адрес сервиса. Default = `""`.
pub const F_EXTERNAL_INTEGRATION_SERVICE_ADDRESS: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `IntegrationService` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_EXTERNAL_INTEGRATION_SERVICE_ADDRESS,
            "externalIntegrationServiceAddress",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
    ]
}

/// Дочерняя коллекция `Channel` (recursion-узел, собственный producedTypes).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Channel", child_kind: "IntegrationService.Channel" }];

/// Канонический [`EntitySpec`] вида `IntegrationService` (кэш на процесс).
pub fn integration_service() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "IntegrationService",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `IntegrationService` в каноническом порядке IR (= EDT).
/// Одна категория `Manager` (сверено корпусом: EDT `<managerType>`, Designer
/// `IntegrationServiceManager.<Obj>` category `Manager`).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[crate::spec::common::ProducedCategory {
    category: "Manager",
    edt_tag: "managerType",
    designer_category: "Manager",
    designer_type_name: "IntegrationServiceManager",
    designer_order: 0,
}];
