//! Канонический спек ДОЧЕРНЕГО вида `ExchangePlan.Attribute` (реквизит плана обмена) —
//! child-objects substrate, ExchangePlan-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля == ЧИСТЫЙ [`crate::spec::ir_child`] `base_child_fields()` (как `Document.Attribute`:
//! 27-полевой реквизит-набор synonym…dataHistory, включая fillFromFillingValue/fillValue,
//! БЕЗ `use`). Сверено 1/1: `<FillFromFillingValue>`/`<FillValue>` присутствуют у обоих,
//! `<Use>` НЕТ. `choiceParameters`/`linkByType`/`choiceForm` — Designer-only пустые.

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::base_child_fields;

/// `&'static EntitySpec` вида `ExchangePlan.Attribute` (кэш на процесс).
pub fn exchange_plan_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExchangePlan.Attribute",
        fields: Box::leak(base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
