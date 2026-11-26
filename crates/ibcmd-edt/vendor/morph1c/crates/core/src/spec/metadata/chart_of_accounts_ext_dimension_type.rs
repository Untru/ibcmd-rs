//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.ExtDimensionType` (Вид субконто плана
//! счетов) — child-objects substrate. RECURSION-УЗЕЛ (несёт вложенную коллекцию
//! `AccountingFlag` = ExtDimensionAccountingFlag). БЕЗ `HARNESS_ENTRY`.
//!
//! ⚠️ Coverage s2_registers НЕ несёт видов субконто → объявлен для схемной полноты, НЕ
//! подключён в активные child-bindings (cf эмитит его коллекцию пустой). Минимальный набор
//! полей (synonym/comment/toolTip); точная раскладка — при первом witnessed-объекте.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `toolTip` — локализ., Default [].
pub const F_TOOL_TIP: FieldId = FieldId(3);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TOOL_TIP, "toolTip", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
    ]
}

/// Вложенная коллекция `AccountingFlag`. ⚠️ ERP-witnessed реальность: признаки учёта
/// субконто — ПРЯМЫЕ дети корня плана счетов (`ChartOfAccounts.ExtDimensionAccountingFlag`,
/// см. одноимённый спек), а не вложение сюда; слот-черновик указывает на тот же вид.
const CHILDREN: &[ChildSlot] = &[ChildSlot {
    collection: "AccountingFlag",
    child_kind: "ChartOfAccounts.ExtDimensionAccountingFlag",
}];

/// `&'static EntitySpec` вида `ChartOfAccounts.ExtDimensionType` (кэш на процесс).
pub fn chart_of_accounts_ext_dimension_type() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.ExtDimensionType",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
