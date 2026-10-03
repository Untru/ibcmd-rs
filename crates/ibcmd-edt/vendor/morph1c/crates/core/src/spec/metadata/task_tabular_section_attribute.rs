//! Канонический спек ДОЧЕРНЕГО вида `Task.TabularSection.Attribute` (реквизит табличной
//! части задачи) — child-objects substrate, Task-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`
//! (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` БЕЗ `fillFromFillingValue`/
//! `fillValue` (как `Document.TabularSection.Attribute`: их НЕ несут оба формата) и БЕЗ
//! `use`. `choiceParameters`/`linkByType`/`choiceForm` — Designer-only пустые.

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::{base_child_fields, F_FILL_FROM_FILLING_VALUE, F_FILL_VALUE};

/// Поля реквизита табличной части = base БЕЗ fillFromFillingValue/fillValue.
fn tabular_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
    let mut v = base_child_fields();
    v.retain(|f| f.id != F_FILL_FROM_FILLING_VALUE && f.id != F_FILL_VALUE);
    v
}

/// `&'static EntitySpec` вида `Task.TabularSection.Attribute` (кэш на процесс).
pub fn task_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
