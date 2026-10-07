//! Канонический спек ДОЧЕРНЕГО вида `Catalog.Attribute` (реквизит справочника) —
//! child-objects substrate, Catalog-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = Catalog-base (IR base + `Use`; см. [`crate::spec::catalog_child`]). Сверено
//! 799 атрибутов × edt+designer.

use crate::spec::catalog_child::catalog_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `Catalog.Attribute` (кэш на процесс).
pub fn catalog_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Catalog.Attribute",
        fields: Box::leak(catalog_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
