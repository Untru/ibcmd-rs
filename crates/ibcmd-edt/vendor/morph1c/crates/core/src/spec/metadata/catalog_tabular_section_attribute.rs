//! Канонический спек ДОЧЕРНЕГО вида `Catalog.TabularSection.Attribute` (реквизит
//! табличной части справочника) — child-objects substrate, Catalog-срез. Лист-вид. БЕЗ
//! `HARNESS_ENTRY` (транзитивно).
//!
//! Поля == `Catalog.Attribute` (тот же Catalog-base: IR base + `Use`; см.
//! [`crate::spec::catalog_child`]). Отдельный спек (а не shared kind), т.к. dotted-kind
//! различает их родитель-квалифицированно (§1.2 design).

use crate::spec::catalog_child::tabular_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `Catalog.TabularSection.Attribute` (кэш на процесс).
pub fn catalog_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Catalog.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
