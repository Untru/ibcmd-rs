//! Канонический спек ДОЧЕРНЕГО вида `Task.Attribute` (реквизит задачи) — child-objects
//! substrate, Task-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля == ЧИСТЫЙ [`crate::spec::ir_child`] `base_child_fields()` (27-полевой реквизит-
//! набор: synonym…dataHistory, включая fillFromFillingValue/fillValue). Как
//! `Document.Attribute` — БЕЗ поля `use` (сверено по корпусу: `<Use>` НЕТ; а
//! `<FillFromFillingValue>`/`<FillValue>` — есть). `choiceParameters`/`linkByType`/
//! `choiceForm` — Designer-only пустые (как у IR-base).

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::base_child_fields;

/// `&'static EntitySpec` вида `Task.Attribute` (кэш на процесс).
pub fn task_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task.Attribute",
        fields: Box::leak(base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
