//! Канонический спек ДОЧЕРНЕГО вида `AccountingRegister.Dimension` (измерение регистра
//! бухгалтерии) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = базовый набор ([`ac_base_child_fields`]) + три dimension-специфичных: `balance`
//! + `accountingFlag` (общие с Resource) и `denyIncompleteValues` (id из `ir_child`). НЕТ
//!   `extDimensionAccountingFlag` (это Resource-only) и `useInTotals` (регистр накопления).
//!   Сверено designer/edt/cf-фикстурами (6 измерений на объект).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::F_DENY_INCOMPLETE_VALUES;
use crate::spec::metadata::accounting_register_resource::{
    ac_base_child_fields, F_ACCOUNTING_FLAG, F_BALANCE,
};

fn bool_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn str_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    let mut v = ac_base_child_fields();
    // balance/accountingFlag/denyIncompleteValues В КОНЕЦ (физ.порядок — field_emit_order).
    v.push(bool_f(F_BALANCE, "balance"));
    v.push(str_f(F_ACCOUNTING_FLAG, "accountingFlag"));
    v.push(bool_f(F_DENY_INCOMPLETE_VALUES, "denyIncompleteValues"));
    v
}

/// `&'static EntitySpec` вида `AccountingRegister.Dimension` (кэш на процесс).
pub fn accounting_register_dimension() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccountingRegister.Dimension",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
