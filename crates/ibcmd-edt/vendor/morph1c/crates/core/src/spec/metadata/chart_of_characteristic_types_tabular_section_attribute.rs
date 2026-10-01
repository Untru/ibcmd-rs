//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCharacteristicTypes.TabularSection.Attribute`
//! (реквизит табличной части плана видов характеристик) — child-objects substrate,
//! Catalog-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля == `ChartOfCharacteristicTypes.Attribute` (тот же Catalog-base: IR base + `Use`;
//! см. [`crate::spec::catalog_child`]). Отдельный спек (а не shared kind), т.к. dotted-kind
//! различает их родитель-квалифицированно (§1.2 design).

use crate::spec::catalog_child::tabular_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `ChartOfCharacteristicTypes.TabularSection.Attribute`.
pub fn chart_of_characteristic_types_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCharacteristicTypes.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
