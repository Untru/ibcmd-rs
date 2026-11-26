//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCharacteristicTypes.Attribute` (реквизит
//! плана видов характеристик) — child-objects substrate, Catalog-срез. Лист-вид. БЕЗ
//! `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = Catalog-base (IR base + `Use`; см. [`crate::spec::catalog_child`]) — тот же
//! набор, что у `Catalog.Attribute`. Отдельный спек (а не shared kind), т.к. dotted-kind
//! различает их родитель-квалифицированно (§1.2 design); реестр запрещает дубль кода вида.

use crate::spec::catalog_child::catalog_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `ChartOfCharacteristicTypes.Attribute` (кэш на процесс).
pub fn chart_of_characteristic_types_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCharacteristicTypes.Attribute",
        fields: Box::leak(catalog_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
