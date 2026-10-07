//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCalculationTypes.TabularSection` (табличная
//! часть плана видов расчёта) — child-objects substrate, Catalog-срез. RECURSION-УЗЕЛ:
//! несёт собственный `uuid`+producedTypes(TabularSection,TabularSectionRow)+вложенную
//! коллекцию `Attribute`. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Поля (Designer DENSE, сверено ERP-дампом 8.3.27): `synonym, comment, toolTip,
//! fillChecking, standardAttributes, lineNumberLength`. БЕЗ `use` (не-иерархический вид).
//! `standardAttributes` — TS-вариант родительского [`STD_ATTRS`] (LineNumber; ERP-witnessed
//! 10/10 TS Начисления/Удержания); дефолт `[]` — coverage-корпус блока не несёт, cf-TS
//! держит слот frame-const `{0}` и поле НЕ проецирует.
//!
//! [`STD_ATTRS`]: crate::spec::metadata::chart_of_calculation_types::STD_ATTRS

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
/// `lineNumberLength` — длина номера строки (int). Default 5. Задаваемое во ВСЕХ диалектах:
/// EDT эмитит `<lineNumberLength>` при недефолте (дефолт омитит), Designer/cf — DENSE
/// (ERP-witnessed недефолты у TS).
pub const F_LINE_NUMBER_LENGTH: FieldId = FieldId(5);
/// `standardAttributes` — TS-вариант блока (LineNumber; ERP-witnessed). Дефолт `[]`.
/// Физически МЕЖДУ fillChecking и lineNumberLength (Designer DENSE, witnessed); EDT —
/// после synonym, перед attributes (witnessed 10/10).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(6);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TOOL_TIP, "toolTip", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_FILL_CHECKING, "fillChecking", ValueKind::Enum, PropertyValue::Enum(Token::new("DontCheck"))),
        // standardAttributes: TS-вариант (LineNumber). Дефолт [] — cf-TS слот frame-const.
        FieldSpec::with_default(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List, PropertyValue::List(Vec::new())),
        FieldSpec::with_default(F_LINE_NUMBER_LENGTH, "lineNumberLength", ValueKind::Int, PropertyValue::Int(5)),
    ]
}

/// Вложенная коллекция `Attribute` (recursion).
const CHILDREN: &[ChildSlot] = &[ChildSlot {
    collection: "Attribute",
    child_kind: "ChartOfCalculationTypes.TabularSection.Attribute",
}];

/// `&'static EntitySpec` вида `ChartOfCalculationTypes.TabularSection` (кэш на процесс).
pub fn chart_of_calculation_types_tabular_section() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes.TabularSection",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории producedTypes вида `ChartOfCalculationTypes.TabularSection` (EDT-порядок).
// ============================================================================
/// Категории producedTypes вида `ChartOfCalculationTypes.TabularSection` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "TabularSection",
        edt_tag: "objectType",
        designer_category: "TabularSection",
        designer_type_name: "ChartOfCalculationTypesTabularSection",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "TabularSectionRow",
        edt_tag: "rowType",
        designer_category: "TabularSectionRow",
        designer_type_name: "ChartOfCalculationTypesTabularSectionRow",
        designer_order: 1,
    },
];
