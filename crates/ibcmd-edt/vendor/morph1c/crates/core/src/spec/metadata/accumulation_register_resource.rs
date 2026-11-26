//! Канонический спек ДОЧЕРНЕГО вида `AccumulationRegister.Resource` (ресурс регистра
//! накопления) — child-objects substrate. Лист-вид
//! (`children: &[]`), БЕЗ `HARNESS_ENTRY` (покрыт ТРАНЗИТИВНО через родителя).
//!
//! Поля = базовый набор leaf-детей [`crate::spec::ir_child::base_child_fields`] с ОДНОЙ
//! регистро-накопительной поправкой ([`ar_base_child_fields`]): у AccumulationRegister
//! `fillValue` в детях НЕ присутствует (0/964 ресурсов, 0/1397 реквизитов, 0/1560
//! измерений — сверено ERP-корпусом), поэтому он DEFAULTED (Undefined, омитится), а не
//! `required` как у InformationRegister (где fillValue всегда present). `minValue`/
//! `maxValue` остаются required (всегда present, Undefined). Порядок спека канонический;
//! EDT физический порядок задаёт `field_emit_order` EDT-проекции.

use crate::ir::value::{PropertyValue, ValueKind, ValueScalarKind, ValueSpec};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS, F_FILL_VALUE};

/// Базовый набор leaf-детей AccumulationRegister: как [`base_child_fields`], но с ДВУМЯ
/// регистро-накопительными поправками (сверено ERP-корпусом), т.к. `ir_child` заточён под
/// InformationRegister:
/// - `fillValue` — DEFAULTED (Undefined), т.к. дети регистра накопления его НЕ несут
///   (0/964 ресурсов / 0/1397 реквизитов / 0/1560 измерений); у IR он `required`;
/// - `choiceParameters` — `List` (choiceParameters-кодек), а НЕ Designer-only `Str`: у AR
///   он ВИТНЕССИТСЯ в EDT (3 реквизита, 15 измерений); default `[]`.
///
/// Общий для Resource/Attribute/Dimension этого вида.
pub fn ar_base_child_fields() -> Vec<FieldSpec> {
    let undefined = PropertyValue::Value(ValueSpec { kind: ValueScalarKind::Undefined, scalar: None });
    base_child_fields()
        .into_iter()
        .map(|f| {
            if f.id == F_FILL_VALUE {
                // required → defaulted (Undefined): отсутствие = дефолт, на записи опускается.
                FieldSpec::with_default(f.id, f.name, f.value_kind, undefined.clone())
            } else if f.id == F_CHOICE_PARAMETERS {
                // Str(Designer-only) → List (choiceParameters-кодек, витнессится в EDT).
                FieldSpec::with_default(f.id, f.name, ValueKind::List, PropertyValue::List(Vec::new()))
            } else {
                f
            }
        })
        .collect()
}

/// `&'static EntitySpec` вида `AccumulationRegister.Resource` (кэш на процесс).
pub fn accumulation_register_resource() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister.Resource",
        fields: Box::leak(ar_base_child_fields().into_boxed_slice()),
        children: &[],
    })
}
