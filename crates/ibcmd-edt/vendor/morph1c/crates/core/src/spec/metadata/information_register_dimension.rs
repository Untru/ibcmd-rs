//! Канонический спек ДОЧЕРНЕГО вида `InformationRegister.Dimension` (измерение регистра)
//! — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = базовый набор leaf-детей + 4 dimension-only (`master`/`mainFilter`/
//! `denyIncompleteValues`/`typeReductionMode`); сверено 395 Dimension × edt+designer.
//! Порядок Designer DENSE: master/mainFilter/denyIncompleteValues после
//! choiceHistoryOnInput (перед indexing), typeReductionMode — в конце. Поля — из
//! [`crate::spec::ir_child`]; `choiceParameters` перетипирован Str→List (ERP-корпус:
//! 98 непустых у IR.dimensions, см. [`super::information_register_attribute`]).

use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::dimension_fields;
use crate::spec::metadata::information_register_attribute::retype_choice_parameters;

/// Поля измерения = dimension base + retype choiceParameters (List).
fn information_register_dimension_fields() -> Vec<FieldSpec> {
    let mut v = dimension_fields();
    retype_choice_parameters(&mut v);
    v
}

/// `&'static EntitySpec` вида `InformationRegister.Dimension` (кэш на процесс).
pub fn information_register_dimension() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "InformationRegister.Dimension",
        fields: Box::leak(information_register_dimension_fields().into_boxed_slice()),
        children: &[],
    })
}
