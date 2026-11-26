//! Канонический спек ДОЧЕРНЕГО вида `DataProcessor.TabularSection.Attribute` (реквизит
//! табличной части обработки) — child-objects substrate, DataProcessor-срез. Лист-вид.
//! БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` с `fillFromFillingValue`/`fillValue`
//! (ПРИСУТСТВУЮТ: Designer DENSE 145/145 несёт оба; EDT — fillValue 145/145, fillFromFillingValue
//! опускает дефолтом), но БЕЗ `indexing`/`fullTextSearch`/`dataHistory` (обработка — не DB-объект;
//! 0/145 в обоих форматах) и БЕЗ `use`. Сверено по корпусу; `choiceParameters` перетипирован
//! Str→List (см. [`super::data_processor_attribute`]; ERP: ТЧ-реквизиты обработок несут его).
//! `linkByType`/`choiceForm` остаются Str.

use crate::spec::common::EntitySpec;
use crate::spec::ir_child::{
    base_child_fields, F_DATA_HISTORY, F_FILL_VALUE, F_FULL_TEXT_SEARCH, F_INDEXING,
};
use crate::spec::metadata::data_processor_attribute::retype_choice_parameters;

/// Поля реквизита табличной части = base БЕЗ indexing/fullTextSearch/dataHistory
/// (fillFromFillingValue/fillValue остаются — они present у табличного реквизита),
/// choiceParameters — структурный List (retype).
fn tabular_attribute_fields() -> Vec<crate::spec::common::FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v.retain(|f| f.id != F_INDEXING && f.id != F_FULL_TEXT_SEARCH && f.id != F_DATA_HISTORY);
    // fillValue — X-исключён: cf несёт ТИП-НЕЗАВИСИМУЮ const `{"S",""}` (пустая строка) для
    // ВСЕХ типов реквизита, тогда как EDT/Designer сериализуют ПУСТОЕ fillValue тип-зависимо
    // (String→StringValue(""), иначе→UndefinedValue — witnessed coverage-корпусом). cf физически
    // не несёт type-info в этой ячейке → cf-IR не может воспроизвести XML-тип. R byte-exact на
    // каждой стороне (cf пишет свою const, XML — своё тип-значение); X исключает поле (как help).
    for f in v.iter_mut() {
        if f.id == F_FILL_VALUE {
            f.x_ignore = true;
        }
    }
    v
}

/// `&'static EntitySpec` вида `DataProcessor.TabularSection.Attribute` (кэш на процесс).
pub fn data_processor_tabular_section_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DataProcessor.TabularSection.Attribute",
        fields: Box::leak(tabular_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
