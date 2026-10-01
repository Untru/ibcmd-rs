//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.ExtDimensionAccountingFlag`
//! (Признак учёта субконто) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! ERP-witnessed (Международный ×2, Хозрасчетный ×3; edt+designer+cf): это ПРЯМОЙ ребёнок
//! корня плана счетов (edt `<extDimensionAccountingFlags uuid>`, designer
//! `ChildObjects/<ExtDimensionAccountingFlag>`, cf-коллекция `c70ca527-…`), а НЕ вложение
//! под ExtDimensionType, как предполагал черновик. Раскладка полей ИДЕНТИЧНА
//! `ChartOfAccounts.AccountingFlag` (без indexing/fullTextSearch; та же cf-обёртка
//! `{6, INNER27, dataHistory}`; witnessed непустой fillValue Boolean=true у
//! Количественный/Международный).

use crate::spec::common::EntitySpec;
use crate::spec::metadata::chart_of_accounts_accounting_flag::chart_of_accounts_accounting_flag_fields;

/// `&'static EntitySpec` вида `ChartOfAccounts.ExtDimensionAccountingFlag` (кэш на процесс).
pub fn chart_of_accounts_ext_dimension_accounting_flag() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.ExtDimensionAccountingFlag",
        fields: Box::leak(chart_of_accounts_accounting_flag_fields().into_boxed_slice()),
        children: &[],
    })
}
