//! Канонический спек ДОЧЕРНЕГО вида `AccumulationRegister.Attribute` (реквизит регистра
//! накопления) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Набор полей == `Resource` (сверено 1397 Attribute по ERP-корпусу: набор ⊆ базового).
//! Отдельный спек (не shared kind), т.к. dotted-kind различает их родитель-квалифицированно
//! (§1.2); поля — общие из [`crate::spec::ir_child`].

use crate::spec::common::EntitySpec;
use crate::spec::metadata::accumulation_register_resource::ar_base_child_fields;

/// `&'static EntitySpec` вида `AccumulationRegister.Attribute` (кэш на процесс).
pub fn accumulation_register_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister.Attribute",
        fields: Box::leak(ar_base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
