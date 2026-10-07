//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCalculationTypes.Attribute` (реквизит плана
//! видов расчёта) — child-objects substrate, Catalog-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Поля = IR base (incl. `fillValue`/`fillFromFillingValue`) БЕЗ `Use` (не-иерархический
//! родитель → нет папок → атрибут не несёт `Use`; сверено coverage-корпусом). См.
//! [`crate::spec::catalog_child::nonhier_attribute_fields`].

use crate::spec::catalog_child::nonhier_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `ChartOfCalculationTypes.Attribute` (кэш на процесс).
pub fn chart_of_calculation_types_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes.Attribute",
        fields: Box::leak(nonhier_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
