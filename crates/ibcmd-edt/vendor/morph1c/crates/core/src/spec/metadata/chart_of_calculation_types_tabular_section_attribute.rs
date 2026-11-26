//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCalculationTypes.TabularSection.Attribute`
//! (реквизит табличной части плана видов расчёта) — child-objects substrate, Catalog-срез.
//! Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля == `Catalog.TabularSection.Attribute` (IR base БЕЗ `fillValue`/`fillFromFillingValue`
//! и БЕЗ `Use`; см. [`crate::spec::catalog_child::tabular_attribute_fields`]).

use crate::spec::catalog_child::tabular_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `ChartOfCalculationTypes.TabularSection.Attribute`.
pub fn chart_of_calculation_types_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
