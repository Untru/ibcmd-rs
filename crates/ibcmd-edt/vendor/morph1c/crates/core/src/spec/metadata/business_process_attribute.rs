//! Канонический спек ДОЧЕРНЕГО вида `BusinessProcess.Attribute` (реквизит бизнес-процесса)
//! — child-objects substrate, BusinessProcess-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`
//! (транзитивно).
//!
//! Поля == ЧИСТЫЙ [`crate::spec::ir_child`] `base_child_fields()` (как `Document.Attribute`/
//! `Task.Attribute` — БЕЗ поля `use`; `<FillFromFillingValue>`/`<FillValue>` присутствуют).
//! `choiceParameters`/`linkByType`/`choiceForm` — Designer-only пустые (как у IR-base).

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::base_child_fields;

/// `&'static EntitySpec` вида `BusinessProcess.Attribute` (кэш на процесс).
pub fn business_process_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "BusinessProcess.Attribute",
        fields: Box::leak(base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
