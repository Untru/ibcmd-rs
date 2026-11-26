//! Канонический спек ДОЧЕРНЕГО вида `AccountingRegister.Attribute` (реквизит регистра
//! бухгалтерии) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Набор полей == базовый ([`ac_base_child_fields`]) — БЕЗ balance/accountingFlag (те у
//! Resource/Dimension). Структурно идентичен `AccumulationRegister.Attribute`; отдельный
//! спек (dotted-kind различает родитель-квалифицированно, §1.2). Сверено 11 реквизитов
//! фикстуры `РегБух_ПокрытиеСоставных`.

use crate::spec::common::EntitySpec;
use crate::spec::metadata::accounting_register_resource::ac_base_child_fields;

/// `&'static EntitySpec` вида `AccountingRegister.Attribute` (кэш на процесс).
pub fn accounting_register_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccountingRegister.Attribute",
        fields: Box::leak(ac_base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
