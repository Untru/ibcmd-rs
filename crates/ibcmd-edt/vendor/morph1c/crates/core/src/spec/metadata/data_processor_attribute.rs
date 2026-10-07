//! Канонический спек ДОЧЕРНЕГО вида `DataProcessor.Attribute` (реквизит обработки) —
//! child-objects substrate, DataProcessor-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` БЕЗ DB-полей реквизита, которых
//! обработка (НЕ DB-объект) не несёт НИ В ОДНОМ формате (сверено по корпусу, EDT 400 / Designer
//! 400, обе стороны 0): `fillFromFillingValue`, `fillValue`, `indexing`, `fullTextSearch`,
//! `dataHistory`. Также БЕЗ `use`. Это ОТЛИЧАЕТ корневой реквизит от Catalog/Document/
//! ExchangePlan (там эти поля dense-present) и от `DataProcessor.TabularSection.Attribute`
//! (там fillFromFillingValue/fillValue ПРИСУТСТВУЮТ); `choiceParameters` перетипирован
//! Str→List (ERP-корпус: 320 непустых у DataProcessor-реквизитов). `linkByType`/
//! `choiceForm` остаются Str — их несут ОБА формата (ERP: linkByType 17, choiceForm 15).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{
    base_child_fields, F_CHOICE_PARAMETERS, F_DATA_HISTORY, F_FILL_FROM_FILLING_VALUE,
    F_FILL_VALUE, F_FULL_TEXT_SEARCH, F_INDEXING,
};

/// Перетипировать `choiceParameters` Str→структурный List. Копия
/// `crate::spec::catalog_child::retype_choice_parameters` (он приватный).
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

/// Поля корневого реквизита обработки = base БЕЗ DB-полей (fill*/indexing/fullTextSearch/
/// dataHistory — их не несёт ни один формат для обработки).
fn data_processor_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v.retain(|f| {
        f.id != F_FILL_FROM_FILLING_VALUE
            && f.id != F_FILL_VALUE
            && f.id != F_INDEXING
            && f.id != F_FULL_TEXT_SEARCH
            && f.id != F_DATA_HISTORY
    });
    v
}

/// `&'static EntitySpec` вида `DataProcessor.Attribute` (кэш на процесс).
pub fn data_processor_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DataProcessor.Attribute",
        fields: Box::leak(data_processor_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
