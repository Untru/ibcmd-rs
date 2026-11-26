//! Канонический спек ДОЧЕРНЕГО вида `Document.TabularSection.Attribute` (реквизит
//! табличной части документа) — child-objects substrate, Document-срез. Лист-вид. БЕЗ
//! `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` БЕЗ `fillFromFillingValue`/
//! `fillValue` (сверено 110/110: их НЕ несут оба формата) и БЕЗ `use`; `choiceParameters`
//! перетипирован Str→List (см. [`super::document_attribute`]). `linkByType`/`choiceForm`
//! остаются Str — их несут ОБА формата (ERP-корпус: непустые у ТЧ-реквизитов документов).

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::{base_child_fields, F_FILL_FROM_FILLING_VALUE, F_FILL_VALUE};
use crate::spec::metadata::document_attribute::retype_choice_parameters;

/// Поля реквизита табличной части = base БЕЗ fillFromFillingValue/fillValue,
/// choiceParameters — структурный List (retype, как у корневого реквизита).
fn tabular_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v.retain(|f| f.id != F_FILL_FROM_FILLING_VALUE && f.id != F_FILL_VALUE);
    v
}

/// `&'static EntitySpec` вида `Document.TabularSection.Attribute` (кэш на процесс).
pub fn document_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Document.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
