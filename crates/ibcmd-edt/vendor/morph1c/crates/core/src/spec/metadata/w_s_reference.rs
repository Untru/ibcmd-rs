//! Канонический спек вида объекта `WSReference` (WS-ссылка) — ARCHITECTURE.md §1.4/§1.6.
//! ОДИН [`EntitySpec`] на вид: движок выводит read/write для КАЖДОГО формата (EDT `.mdo`,
//! Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь — канонический `id`, `value_kind`,
//! `default`, нормализация и ПОРЯДОК эмиссии (§1.6). ERP-only вид (в SSL отсутствует →
//! cf-оракула нет; cf-строки инвентаря — честный `n/a`).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта; `producedTypes`
//! (единственный `Manager`-тип) — платформенный [`crate::ir::InternalInfo`]-блок,
//! фреймится каркасом коннектора через `PRODUCED_CATEGORIES` (ниже), не движком.
//!
//! Метамодельные поля `objectBelonging` / `extendedConfigurationObject` (EDT-xcore) в
//! корпусе ERP отсутствуют (n=0) и НЕ включены (та же конвенция, что у
//! `document_numerator`): их эмиссия ломает Designer DENSE byte-exact (Designer их не
//! несёт). Если появится корпус с этими полями — расширить спек.
//!
//! Поля (подтверждено корпусом ERP, 2 объекта edt+designer; Designer DENSE — все поля
//! present всегда, EDT SPARSE — дефолты опущены):
//! * `synonym` — локализованный синоним (default `[]`);
//! * `comment` — свободный текст (default `""`; Designer `<Comment/>`, EDT опускает);
//! * `locationURL` — URL/путь WSDL (свободный текст; default `""`).
//!
//! Лист-вид: подчинённых коллекций и `standardAttributes` нет. Порядок = DENSE-порядок
//! Designer `<Properties>` (Synonym, Comment, LocationURL).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `locationURL` — URL/путь WSDL (свободный текст). Default = `""`.
pub const F_LOCATION_URL: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] вида `WSReference` в каноническом DENSE-порядке Designer.
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
            F_LOCATION_URL,
            "locationURL",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `WSReference` (кэш на процесс).
pub fn w_s_reference() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WSReference",
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
//
// WSReference несёт РОВНО одну категорию — `Manager` (EDT `<managerType typeId
// valueTypeId/>`; Designer `<xr:GeneratedType name="WSReferenceManager.<Obj>"
// category="Manager">`) — сверено корпусом ERP (2/2 объекта).
// ============================================================================
/// Категории producedTypes вида `WSReference` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] =
    &[crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "WSReferenceManager",
        designer_order: 0,
    }];
