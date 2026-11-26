//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.Attribute` (реквизит плана счетов) —
//! child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля = IR base-реквизит ([`crate::spec::ir_child`] `base_child_fields`) БЕЗ Catalog-`use`
//! (План счетов НЕ иерархичен — у реквизита нет `Use ForItem/ForFolderAndItem`; сверено
//! фикстурой). `choiceParameters` перетипирован в структурный List (codec ChoiceParameters).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS};

/// Поля реквизита `ChartOfAccounts.Attribute` = IR base (без `use`) + retype choiceParameters.
pub fn chart_of_accounts_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    if let Some(f) = v.iter_mut().find(|f| f.id == F_CHOICE_PARAMETERS) {
        *f = FieldSpec::with_default(
            F_CHOICE_PARAMETERS,
            "choiceParameters",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        );
    }
    v
}

/// `&'static EntitySpec` вида `ChartOfAccounts.Attribute` (кэш на процесс).
pub fn chart_of_accounts_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.Attribute",
        fields: Box::leak(chart_of_accounts_attribute_fields().into_boxed_slice()),
        children: &[],
    })
}
