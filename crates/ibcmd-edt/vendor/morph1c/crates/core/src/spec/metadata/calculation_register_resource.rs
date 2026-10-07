//! Канонический спек ДОЧЕРНЕГО вида `CalculationRegister.Resource` (ресурс регистра
//! расчёта) — child-objects substrate. Лист-вид
//! (`children: &[]`), БЕЗ `HARNESS_ENTRY` (покрыт ТРАНЗИТИВНО через родителя).
//!
//! Поля = базовый набор leaf-детей [`crate::spec::ir_child::base_child_fields`] с той же
//! регистро-семейной поправкой, что у AccumulationRegister ([`cr_base_child_fields`]):
//! `fillValue` DEFAULTED (Undefined — дети регистра расчёта его НЕ несут), `choiceParameters`
//! — `List` (choiceParameters-кодек). `minValue`/`maxValue` остаются required (Undefined).
//! Порядок спека канонический; EDT физический порядок задаёт `field_emit_order` проекции.

use crate::ir::value::{PropertyValue, ValueKind, ValueScalarKind, ValueSpec};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS, F_FILL_VALUE};

/// Базовый набор leaf-детей CalculationRegister: как [`base_child_fields`], но с двумя
/// регистро-семейными поправками (сверено coverage-корпусом), т.к. `ir_child` заточён под
/// InformationRegister:
/// - `fillValue` — DEFAULTED (Undefined), т.к. дети регистра расчёта его НЕ несут;
/// - `choiceParameters` — `List` (choiceParameters-кодек), а НЕ Designer-only `Str`.
///
/// Общий для Resource/Attribute/Dimension этого вида.
pub fn cr_base_child_fields() -> Vec<FieldSpec> {
    let undefined = PropertyValue::Value(ValueSpec { kind: ValueScalarKind::Undefined, scalar: None });
    base_child_fields()
        .into_iter()
        .map(|f| {
            if f.id == F_FILL_VALUE {
                FieldSpec::with_default(f.id, f.name, f.value_kind, undefined.clone())
            } else if f.id == F_CHOICE_PARAMETERS {
                FieldSpec::with_default(f.id, f.name, ValueKind::List, PropertyValue::List(Vec::new()))
            } else {
                f
            }
        })
        .collect()
}

/// `&'static EntitySpec` вида `CalculationRegister.Resource` (кэш на процесс).
pub fn calculation_register_resource() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CalculationRegister.Resource",
        fields: Box::leak(cr_base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
