//! Канонический спек ДОЧЕРНЕГО вида `Task.AddressingAttribute` (реквизит адресации задачи)
//! — child-objects substrate, Task-специфичная НОВАЯ коллекция. Лист-вид. БЕЗ
//! `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = [`crate::spec::ir_child`] `base_child_fields()` (как `Task.Attribute`) + ОДНО
//! Task-специфичное поле `addressingDimension` (ссылка на измерение регистра адресации,
//! object-ref). Порядок Designer DENSE (сверено по корпусу): `addressingDimension` — МЕЖДУ
//! `indexing` и `fullTextSearch` (та же позиция в EDT). `choiceParameters`/`linkByType`/
//! `choiceForm` — Designer-only пустые (как у IR-base).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_FULL_TEXT_SEARCH};

/// `addressingDimension` — ссылка на измерение регистра адресации. Default "".
/// FieldId не пересекается с base (base использует 1..=27 для Attribute-полей).
pub const F_ADDRESSING_DIMENSION: FieldId = FieldId(28);

/// Поля реквизита адресации = base + addressingDimension (перед fullTextSearch).
fn addressing_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    let idx_fts = v
        .iter()
        .position(|f| f.id == F_FULL_TEXT_SEARCH)
        .expect("fullTextSearch present in base_child_fields");
    v.insert(
        idx_fts,
        FieldSpec::with_default(
            F_ADDRESSING_DIMENSION,
            "addressingDimension",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
    );
    v
}

/// `&'static EntitySpec` вида `Task.AddressingAttribute` (кэш на процесс).
pub fn task_addressing_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task.AddressingAttribute",
        fields: Box::leak(addressing_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
