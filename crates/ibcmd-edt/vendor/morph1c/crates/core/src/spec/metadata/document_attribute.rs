//! Канонический спек ДОЧЕРНЕГО вида `Document.Attribute` (реквизит документа) —
//! child-objects substrate, Document-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` (27-полевой реквизит-набор:
//! synonym…dataHistory, включая fillFromFillingValue/fillValue) с ПЕРЕТИПИРОВАННЫМ
//! `choiceParameters` (Str→структурный List, как у Catalog). В отличие от
//! `Catalog.Attribute` — БЕЗ поля `use` (сверено 152/152 Document-реквизита: `<Use>`
//! НЕТ ни у edt, ни у designer; `<FillFromFillingValue>`/`<FillValue>` — есть 152/152).
//! `choiceForm`/`mask`/`linkByType` остаются Str (как у IR-base) — их несут ОБА формата
//! (ERP-корпус: choiceForm 52, mask 180, linkByType 345 непустых у Document.attributes).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS};

/// Перетипировать `choiceParameters` из Str-плейсхолдера IR-base в СТРУКТУРНЫЙ List
/// (codec ChoiceParameters). Копия `crate::spec::catalog_child::retype_choice_parameters`
/// (он приватный — см. комментарий-эталон там). ERP-корпус: Document-реквизиты несут
/// choiceParameters (4651 непустых) — Str-плейсхолдер был ложным «Designer-only».
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

/// Поля корневого реквизита документа = IR base + retype choiceParameters (List).
fn document_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v
}

/// `&'static EntitySpec` вида `Document.Attribute` (кэш на процесс).
pub fn document_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Document.Attribute",
        fields: Box::leak(document_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
