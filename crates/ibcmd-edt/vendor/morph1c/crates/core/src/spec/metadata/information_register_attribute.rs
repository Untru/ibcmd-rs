//! Канонический спек ДОЧЕРНЕГО вида `InformationRegister.Attribute` (реквизит регистра)
//! — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Набор полей == `Resource` (сверено: 289 Attribute × edt+designer, тот же базовый
//! набор и порядок); `choiceParameters` перетипирован Str→структурный List (ERP-корпус:
//! 12 непустых у IR.attributes — Str-плейсхолдер был ложным «Designer-only»). Отдельный
//! спек (а не shared kind), т.к. dotted-kind различает их родитель-квалифицированно
//! (§1.2 design); поля — общие из [`crate::spec::ir_child`].

use crate::ir::value::{PropertyValue, ValueKind};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS};

/// Перетипировать `choiceParameters` Str→структурный List. Копия
/// `crate::spec::catalog_child::retype_choice_parameters` (он приватный). Общая для
/// Attribute/Dimension/Resource (IR-срез).
pub(crate) fn retype_choice_parameters(v: &mut [FieldSpec]) {
    if let Some(f) = v.iter_mut().find(|f| f.id == F_CHOICE_PARAMETERS) {
        *f = FieldSpec::with_default(
            F_CHOICE_PARAMETERS,
            "choiceParameters",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        );
    }
}

/// Поля реквизита регистра = IR base + retype choiceParameters (List).
fn information_register_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v
}

/// `&'static EntitySpec` вида `InformationRegister.Attribute` (кэш на процесс).
pub fn information_register_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "InformationRegister.Attribute",
        fields: Box::leak(information_register_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
