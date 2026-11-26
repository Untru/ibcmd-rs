//! Канонический спек ДОЧЕРНЕГО вида `ChartOfAccounts.TabularSection` (табличная часть плана
//! счетов) — child-objects substrate. RECURSION-УЗЕЛ (несёт `uuid`+producedTypes+stdAttrs
//! (LineNumber)+вложенную коллекцию `Attribute`). БЕЗ `HARNESS_ENTRY`. Поля == как у
//! `Catalog.TabularSection`. Coverage не несёт ТЧ у планов счетов — вид объявлен для схемной
//! полноты; std-attrs codec переиспользует CCT-декларацию (tabular-часть кинд-агностична).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `toolTip` — локализ., Default [].
pub const F_TOOL_TIP: FieldId = FieldId(3);
/// `fillChecking` — Default DontCheck.
pub const F_FILL_CHECKING: FieldId = FieldId(4);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (LineNumber). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(5);
/// `use` — назначение. Default ForItem.
pub const F_USE: FieldId = FieldId(6);
/// `lineNumberLength` — длина номера строки (int). Default 5. Задаваемое во ВСЕХ диалектах:
/// EDT эмитит `<lineNumberLength>` при недефолте (дефолт омитит), Designer — DENSE; в cf
/// TS-коллекция плана счетов empty-only (§1.0). ERP-witnessed недефолты у TS других видов.
pub const F_LINE_NUMBER_LENGTH: FieldId = FieldId(7);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TOOL_TIP, "toolTip", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_FILL_CHECKING, "fillChecking", ValueKind::Enum, PropertyValue::Enum(Token::new("DontCheck"))),
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        FieldSpec::with_default(F_USE, "use", ValueKind::Enum, PropertyValue::Enum(Token::new("ForItem"))),
        FieldSpec::with_default(F_LINE_NUMBER_LENGTH, "lineNumberLength", ValueKind::Int, PropertyValue::Int(5)),
    ]
}

/// Вложенная коллекция `Attribute` (recursion).
const CHILDREN: &[ChildSlot] = &[ChildSlot {
    collection: "Attribute",
    child_kind: "ChartOfAccounts.TabularSection.Attribute",
}];

/// `&'static EntitySpec` вида `ChartOfAccounts.TabularSection` (кэш на процесс).
pub fn chart_of_accounts_tabular_section() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts.TabularSection",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// Категории producedTypes вида `ChartOfAccounts.TabularSection` в каноническом порядке IR.
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "TabularSection",
        edt_tag: "objectType",
        designer_category: "TabularSection",
        designer_type_name: "ChartOfAccountsTabularSection",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "TabularSectionRow",
        edt_tag: "rowType",
        designer_category: "TabularSectionRow",
        designer_type_name: "ChartOfAccountsTabularSectionRow",
        designer_order: 1,
    },
];
