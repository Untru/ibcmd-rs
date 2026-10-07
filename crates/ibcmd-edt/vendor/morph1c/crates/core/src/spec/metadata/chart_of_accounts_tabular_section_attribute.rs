//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.TabularSection.Attribute` (реквизит
//! табличной части плана счетов) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Поля == `Catalog.TabularSection.Attribute` (IR base БЕЗ fillValue/fillFromFillingValue/use;
//! см. [`crate::spec::catalog_child`]).

use crate::spec::catalog_child::tabular_attribute_fields;
use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `ChartOfAccounts.TabularSection.Attribute`.
pub fn chart_of_accounts_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
