//! Канонический спек ДОЧЕРНЕГО вида `Report.TabularSection.Attribute` (реквизит табличной
//! части отчёта) — child-objects substrate, Report-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`
//! (транзитивно). Зеркалит `DataProcessor.TabularSection.Attribute`.
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` с `fillFromFillingValue`/
//! `fillValue` (ПРИСУТСТВУЮТ у табличного реквизита), но БЕЗ `indexing`/`fullTextSearch`/
//! `dataHistory` (отчёт — не DB-объект) и БЕЗ `use`; `choiceParameters` перетипирован
//! Str→List (см. [`super::report_attribute`]). `linkByType`/`choiceForm` остаются Str.
//! В корпусе SSL не встречается (0/41), субстрат полон.

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::{
    base_child_fields, F_DATA_HISTORY, F_FILL_VALUE, F_FULL_TEXT_SEARCH, F_INDEXING,
};
use crate::spec::metadata::report_attribute::retype_choice_parameters;

/// Поля реквизита табличной части = base БЕЗ indexing/fullTextSearch/dataHistory
/// (fillFromFillingValue/fillValue остаются — они present у табличного реквизита),
/// choiceParameters — структурный List (retype).
fn tabular_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v.retain(|f| f.id != F_INDEXING && f.id != F_FULL_TEXT_SEARCH && f.id != F_DATA_HISTORY);
    // fillValue — X-исключён (см. `data_processor_tabular_section_attribute`): cf несёт
    // тип-независимую const `{"S",""}`, EDT/Designer — тип-зависимое пустое значение. cf-IR
    // не может воспроизвести XML-тип → X исключает поле; R byte-exact на каждой стороне.
    for f in v.iter_mut() {
        if f.id == F_FILL_VALUE {
            f.x_ignore = true;
        }
    }
    v
}

/// `&'static EntitySpec` вида `Report.TabularSection.Attribute` (кэш на процесс).
pub fn report_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Report.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
