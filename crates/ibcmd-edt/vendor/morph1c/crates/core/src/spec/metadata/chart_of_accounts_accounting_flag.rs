//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.AccountingFlag` (Признак учёта плана
//! счетов) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! ERP-witnessed (Международный ×4, Хозрасчетный ×5; edt+designer+cf): attribute-подобный
//! ребёнок с БАЗОВЫМ реквизит-набором, но БЕЗ `indexing` И БЕЗ `fullTextSearch` — designer
//! DENSE их НЕ эмитит (26 листьев вместо 28), edt их не несёт, cf-обёртка `{6, INNER27,
//! dataHistory}` не имеет их слотов (у Attribute они в INNER10). `choiceParameters`
//! перетипирован в структурный List (codec ChoiceParameters) — как у Attribute.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS, F_FULL_TEXT_SEARCH, F_INDEXING};

/// Поля признака учёта = IR base БЕЗ `indexing`/`fullTextSearch` + retype choiceParameters.
/// Порядок = designer DENSE (ERP-witnessed 26 листьев: … ChoiceHistoryOnInput → DataHistory).
pub fn chart_of_accounts_accounting_flag_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    v.retain(|f| f.id != F_INDEXING && f.id != F_FULL_TEXT_SEARCH);
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

/// `&'static EntitySpec` вида `ChartOfAccounts.AccountingFlag` (кэш на процесс).
pub fn chart_of_accounts_accounting_flag() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.AccountingFlag",
        fields: Box::leak(chart_of_accounts_accounting_flag_fields().into_boxed_slice()),
        children: &[],
    })
}
