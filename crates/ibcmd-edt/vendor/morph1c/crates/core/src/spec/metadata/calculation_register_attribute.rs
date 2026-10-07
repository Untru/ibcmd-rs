//! Канонический спек ДОЧЕРНЕГО вида `CalculationRegister.Attribute` (реквизит регистра
//! расчёта) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = базовый набор [`cr_base_child_fields`] + одно расчёт-специфичное поле:
//! `scheduleLink` (ссылка на измерение графика — reference-путь, СВОЙ id). Базовый набор
//! уже несёт `indexing`/`fullTextSearch`. Designer физ.порядок (сверено фикстурой):
//! ..., choiceHistoryOnInput, scheduleLink, indexing, fullTextSearch — задаёт
//! `field_emit_order` проекции.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::metadata::calculation_register_resource::cr_base_child_fields;

/// `scheduleLink` (связь с измерением графика) — СОБСТВЕННЫЙ id вида (нет в базовом наборе).
/// Reference-путь. Default "" (в корпусе покрытия ВСЕГДА пусто → омитится).
pub const F_SCHEDULE_LINK: FieldId = FieldId(40);

fn build_fields() -> Vec<FieldSpec> {
    let mut v = cr_base_child_fields();
    v.push(FieldSpec::with_default(F_SCHEDULE_LINK, "scheduleLink", ValueKind::Str, PropertyValue::Str(String::new())));
    v
}

/// `&'static EntitySpec` вида `CalculationRegister.Attribute` (кэш на процесс).
pub fn calculation_register_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CalculationRegister.Attribute",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
