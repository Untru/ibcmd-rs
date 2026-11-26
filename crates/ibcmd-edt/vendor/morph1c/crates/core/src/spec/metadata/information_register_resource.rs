//! Канонический спек ДОЧЕРНЕГО вида `InformationRegister.Resource` (ресурс регистра) —
//! child-objects substrate, §1.1 (ребёнок — это
//! [`crate::ir::MetadataObject`] рекурсивно). Лист-вид (`children: &[]`).
//!
//! Dotted-kind (`"InformationRegister.Resource"`); stem ASCII snake_case; БЕЗ
//! `HARNESS_ENTRY` (покрыт ТРАНЗИТИВНО через родителя `InformationRegister`).
//!
//! Спек-свойства (скан SSL, 443 ресурса × edt+designer) = базовый набор leaf-детей
//! (общий с `Attribute`), порядок = DENSE-порядок Designer `<Properties>`. EDT эмитит
//! РАЗРЕЖЁННО (его физический порядок задаёт `field_emit_order` EDT-проекции). Сами
//! коды/типы/дефолты полей — в [`crate::spec::ir_child`]; `choiceParameters`
//! перетипирован Str→List (ERP-корпус: 98 непустых у IR.resources, см.
//! [`super::information_register_attribute`]).

use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::base_child_fields;
use crate::spec::metadata::information_register_attribute::retype_choice_parameters;

/// Поля ресурса = IR base + retype choiceParameters (List).
fn information_register_resource_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v
}

/// `&'static EntitySpec` вида `InformationRegister.Resource` (кэш на процесс).
pub fn information_register_resource() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "InformationRegister.Resource",
        fields: Box::leak(information_register_resource_fields().into_boxed_slice()),
        children: &[],
    })
}
