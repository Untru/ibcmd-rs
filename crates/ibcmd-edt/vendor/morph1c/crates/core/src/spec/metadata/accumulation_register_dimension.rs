//! Канонический спек ДОЧЕРНЕГО вида `AccumulationRegister.Dimension` (измерение регистра
//! накопления) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = базовый набор leaf-детей [`crate::spec::ir_child::base_child_fields`] + два
//! dimension-специфичных поля: `denyIncompleteValues` (переиспользован id из ir_child) и
//! `useInTotals` (СВОЙ id — у InformationRegister.Dimension его НЕТ; регистр накопления
//! ведёт итоги по измерению). НЕТ master/mainFilter/typeReductionMode (те — только у
//! InformationRegister.Dimension). Сверено 1560 Dimension по ERP-корпусу.
//!
//! Порядок спека канонический; EDT физический порядок (сверено топосортом) задаёт
//! `field_emit_order` EDT-проекции.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::F_DENY_INCOMPLETE_VALUES;
use crate::spec::metadata::accumulation_register_resource::ar_base_child_fields;

/// `useInTotals` (учитывать в итогах) — СОБСТВЕННЫЙ id вида (нет в базовом наборе и у
/// InformationRegister.Dimension). Default false (EDT эмитит true; сверено).
pub const F_USE_IN_TOTALS: FieldId = FieldId(32);

fn bool_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}

fn build_fields() -> Vec<FieldSpec> {
    let mut v = ar_base_child_fields();
    // Два dimension-специфичных поля в КОНЕЦ канонического набора (EDT физ.порядок —
    // через field_emit_order проекции). denyIncompleteValues — id из ir_child (Default
    // false); useInTotals — свой id (Default false).
    v.push(bool_f(F_DENY_INCOMPLETE_VALUES, "denyIncompleteValues"));
    v.push(bool_f(F_USE_IN_TOTALS, "useInTotals"));
    v
}

/// `&'static EntitySpec` вида `AccumulationRegister.Dimension` (кэш на процесс).
pub fn accumulation_register_dimension() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister.Dimension",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
