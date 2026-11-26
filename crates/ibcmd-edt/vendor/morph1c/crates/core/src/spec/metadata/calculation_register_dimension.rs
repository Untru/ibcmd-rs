//! Канонический спек ДОЧЕРНЕГО вида `CalculationRegister.Dimension` (измерение регистра
//! расчёта) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = базовый набор [`cr_base_child_fields`] + три расчёт-специфичных поля:
//! `denyIncompleteValues` (id из ir_child), `baseDimension` (СВОЙ id) и `scheduleLink`
//! (СВОЙ id, общий с Attribute). НЕТ `useInTotals` (то — только у AccumulationRegister.
//! Dimension). Базовый набор несёт `indexing`/`fullTextSearch`. Designer физ.порядок
//! (сверено фикстурой): ..., choiceHistoryOnInput, denyIncompleteValues, baseDimension,
//! scheduleLink, indexing, fullTextSearch — задаёт `field_emit_order` проекции.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::F_DENY_INCOMPLETE_VALUES;
use crate::spec::metadata::calculation_register_attribute::F_SCHEDULE_LINK;
use crate::spec::metadata::calculation_register_resource::cr_base_child_fields;

/// `baseDimension` (базовое измерение) — СОБСТВЕННЫЙ id вида (нет в базовом наборе).
/// Default false (в корпусе покрытия ВСЕГДА false → омитится).
pub const F_BASE_DIMENSION: FieldId = FieldId(41);

fn bool_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}

fn build_fields() -> Vec<FieldSpec> {
    let mut v = cr_base_child_fields();
    // Три dimension-специфичных поля в КОНЕЦ канонического набора (Designer физ.порядок —
    // через field_emit_order проекции). denyIncompleteValues — id из ir_child; baseDimension
    // — свой id; scheduleLink — общий с Attribute id.
    v.push(bool_f(F_DENY_INCOMPLETE_VALUES, "denyIncompleteValues"));
    v.push(bool_f(F_BASE_DIMENSION, "baseDimension"));
    v.push(FieldSpec::with_default(F_SCHEDULE_LINK, "scheduleLink", ValueKind::Str, PropertyValue::Str(String::new())));
    v
}

/// `&'static EntitySpec` вида `CalculationRegister.Dimension` (кэш на процесс).
pub fn calculation_register_dimension() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CalculationRegister.Dimension",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
