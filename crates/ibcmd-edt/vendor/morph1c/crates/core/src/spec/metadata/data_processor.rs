//! Канонический спек вида объекта `DataProcessor` (Обработка) — ARCHITECTURE.md §1.0/§1.6,
//! child-objects substrate. СТРУКТУРНО-ПОДОБНЫЙ вид (как ExchangePlan/Catalog/Document), но
//! БЕЗ DB-persistence (нет нумерации/кода/иерархии/проводок/standardAttributes у корня):
//! переиспользует ~85% субстрата Catalog/Document (`ir_child` base-реквизит для детей,
//! Command/FormRef/TemplateRef-паттерн, children-рекурсию, value/type/cpl/picture/help/
//! producedTypes-кодеки, TabularSection-рекурсию) + DataProcessor-дельту:
//! * у корня НЕТ `standardAttributes` (сверено: DB-объектом не является);
//! * у корня НЕТ кода/нумерации/представлений/dataLock/fullTextSearch — лишь презентационные
//!   поля + form-refs;
//! * producedTypes корня — ДВЕ категории Object/Manager (без Ref/Selection/List — обработка
//!   не имеет ссылочного типа); TabularSection — Object/Row (как у структурных видов).
//!
//! Поля (Designer DENSE `<Properties>`, сверено 78/78): `synonym, comment, useStandardCommands,
//! defaultForm, auxiliaryForm, includeHelpInContents, extendedPresentation, explanation`.
//! EDT эмитит РАЗРЕЖЁННО + EDT-only `help` (X-исключён); физический порядок EDT —
//! `field_emit_order` в проекции (synonym, comment, useStandardCommands, defaultForm,
//! auxiliaryForm, includeHelpInContents, help, extendedPresentation, explanation).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (2 категории Object/Manager, см. `produced_types`). EDT-only `help` — X-исключён
//! (`x_ignore`), но участвует в R EDT. `predefined` у обработки в корпусе НЕТ.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `defaultForm` — form-ref. Default "".
pub const F_DEFAULT_FORM: FieldId = FieldId(4);
/// `auxiliaryForm` — form-ref. Default "".
pub const F_AUXILIARY_FORM: FieldId = FieldId(5);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(6);
/// `extendedPresentation` — локализ. Default [].
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(7);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(8);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(9);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(10);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(11);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        s(F_DEFAULT_FORM, "defaultForm"),
        s(F_AUXILIARY_FORM, "auxiliaryForm"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        loc_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        // help — EDT-only const-блок; отсутствие = дефолт-омиссия; X-исключён.
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (как Catalog/ExchangePlan,
/// сверено: Attribute < TabularSection < Form < Template < Command).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "DataProcessor.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "DataProcessor.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "DataProcessor.FormRef" },
    ChildSlot { collection: "Template", child_kind: "DataProcessor.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "DataProcessor.Command" },
];

/// Канонический [`EntitySpec`] вида `DataProcessor` (кэш на процесс).
pub fn data_processor() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DataProcessor",
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
/// Категории producedTypes вида `DataProcessor` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "DataProcessorObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "DataProcessorManager",
        designer_order: 1,
    },
];
