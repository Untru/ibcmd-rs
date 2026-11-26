//! Канонический спек ДОЧЕРНЕГО вида `Task.TabularSection` (табличная часть задачи) —
//! child-objects substrate, Task-срез. RECURSION-УЗЕЛ: несёт собственный
//! `uuid`+producedTypes(objectType,rowType)+stdAttrs(LineNumber)+вложенную коллекцию
//! `Attribute`. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Свойства (Designer DENSE-порядок, как `Document.TabularSection`): `synonym, comment,
//! toolTip, fillChecking, standardAttributes, lineNumberLength`. БЕЗ поля `use`.
//! standardAttributes — ВАРИАТИВНЫЙ блок (1 атрибут LineNumber; codec `std_attrs_task`
//! variant Tabular). producedTypes/InternalInfo — платформенный каркас (рекурсия в
//! `children.rs`).

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
/// `lineNumberLength` — длина номера строки (int). Default 5. Задаваемое во ВСЕХ диалектах:
/// EDT эмитит `<lineNumberLength>` при недефолте (дефолт омитит), Designer/cf — DENSE
/// (ERP-witnessed недефолты у TS).
pub const F_LINE_NUMBER_LENGTH: FieldId = FieldId(6);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TOOL_TIP, "toolTip", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_FILL_CHECKING, "fillChecking", ValueKind::Enum, PropertyValue::Enum(Token::new("DontCheck"))),
        // standardAttributes — required const-ish блок (всегда present; codec Task/Tabular).
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        FieldSpec::with_default(F_LINE_NUMBER_LENGTH, "lineNumberLength", ValueKind::Int, PropertyValue::Int(5)),
    ]
}

/// Вложенная коллекция `Attribute` (recursion).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Attribute", child_kind: "Task.TabularSection.Attribute" }];

/// `&'static EntitySpec` вида `Task.TabularSection` (кэш на процесс).
pub fn task_tabular_section() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task.TabularSection",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `Task.TabularSection` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "TabularSection",
        edt_tag: "objectType",
        designer_category: "TabularSection",
        designer_type_name: "TaskTabularSection",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "TabularSectionRow",
        edt_tag: "rowType",
        designer_category: "TabularSectionRow",
        designer_type_name: "TaskTabularSectionRow",
        designer_order: 1,
    },
];
