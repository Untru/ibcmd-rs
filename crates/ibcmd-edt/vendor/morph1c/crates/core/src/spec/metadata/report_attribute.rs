//! Канонический спек ДОЧЕРНЕГО вида `Report.Attribute` (реквизит отчёта) — child-objects
//! substrate, Report-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` БЕЗ DB-полей реквизита (отчёт —
//! НЕ DB-объект): `fillFromFillingValue`, `fillValue`, `indexing`, `fullTextSearch`,
//! `dataHistory`. Также БЕЗ `use`. Зеркалит `DataProcessor.Attribute` (отчёт структурно
//! идентичен обработке по набору реквизита). В корпусе SSL отчёты реквизитов корня НЕ
//! несут (0/41), но субстрат полон для общности с DataProcessor; `choiceParameters`
//! перетипирован Str→List (ERP-корпус: 30 непустых у Report.attributes). `linkByType`/
//! `choiceForm` остаются Str — оба формата несут их (у отчётов в ERP пусто, субстрат
//! единообразен с Document/DataProcessor).

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

/// Поля корневого реквизита отчёта = base БЕЗ DB-полей (fill*/indexing/fullTextSearch/
/// dataHistory — их не несёт ни один формат для отчёта, как у обработки).
fn report_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
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

/// `&'static EntitySpec` вида `Report.Attribute` (кэш на процесс).
pub fn report_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Report.Attribute",
        fields: Box::leak(report_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
