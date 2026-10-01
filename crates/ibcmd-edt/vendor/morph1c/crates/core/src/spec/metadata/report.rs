//! Канонический спек вида объекта `Report` (Отчет) — ARCHITECTURE.md §1.0/§1.6,
//! child-objects substrate. СТРУКТУРНО-ПОДОБНЫЙ вид (как `DataProcessor`), но БЕЗ
//! DB-persistence (нет нумерации/кода/иерархии/проводок/standardAttributes у корня):
//! переиспользует ~85% субстрата DataProcessor/Catalog (`ir_child` base-реквизит для детей,
//! Command/FormRef/TemplateRef-паттерн, children-рекурсию, value/type/cpl/picture/help/
//! producedTypes-кодеки, TabularSection-рекурсию) + Report-дельту:
//! * у корня НЕТ `standardAttributes` (сверено: DB-объектом не является, 0/41);
//! * producedTypes корня — ДВЕ категории Object/Manager (как DataProcessor; БЕЗ Ref/
//!   Selection/List — отчёт не имеет ссылочного типа; сверено 41/41);
//! * корень несёт 7 Report-СПЕЦИФИЧНЫХ form/storage-ref полей сверх DataProcessor:
//!   `mainDataCompositionSchema` (ref на Template-схему КД), `defaultSettingsForm`,
//!   `auxiliarySettingsForm`, `defaultVariantForm`, `auxiliaryVariantForm`,
//!   `variantsStorage`, `settingsStorage` — ВСЕ скаляр-строки (plain-text ref; новый
//!   кодек не требуется). TabularSection — Object/Row (как у структурных видов).
//!
//! Поля (Designer DENSE `<Properties>`, сверено 41/41): `synonym, comment,
//! useStandardCommands, defaultForm, auxiliaryForm, mainDataCompositionSchema,
//! defaultSettingsForm, auxiliarySettingsForm, defaultVariantForm, auxiliaryVariantForm,
//! variantsStorage, settingsStorage, includeHelpInContents, extendedPresentation,
//! explanation`. EDT эмитит РАЗРЕЖЁННО + EDT-only `help` (X-исключён); физический порядок
//! EDT — `field_emit_order` в проекции (… includeHelpInContents, help, extendedPresentation,
//! explanation).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (2 категории Object/Manager, см. `produced_types`). EDT-only `help` — X-исключён
//! (`x_ignore`), но участвует в R EDT. `predefined` у отчёта в корпусе НЕТ.

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
/// `mainDataCompositionSchema` — ref на Template-схему КД. Default "".
pub const F_MAIN_DATA_COMPOSITION_SCHEMA: FieldId = FieldId(6);
/// `defaultSettingsForm` — form-ref. Default "".
pub const F_DEFAULT_SETTINGS_FORM: FieldId = FieldId(7);
/// `auxiliarySettingsForm` — form-ref. Default "".
pub const F_AUXILIARY_SETTINGS_FORM: FieldId = FieldId(8);
/// `defaultVariantForm` — form-ref. Default "".
pub const F_DEFAULT_VARIANT_FORM: FieldId = FieldId(9);
/// `auxiliaryVariantForm` — form-ref. Default "".
pub const F_AUXILIARY_VARIANT_FORM: FieldId = FieldId(10);
/// `variantsStorage` — storage-ref. Default "".
pub const F_VARIANTS_STORAGE: FieldId = FieldId(11);
/// `settingsStorage` — storage-ref. Default "".
pub const F_SETTINGS_STORAGE: FieldId = FieldId(12);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(13);
/// `extendedPresentation` — локализ. Default [].
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(14);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(15);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(16);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(17);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(18);

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
        s(F_MAIN_DATA_COMPOSITION_SCHEMA, "mainDataCompositionSchema"),
        s(F_DEFAULT_SETTINGS_FORM, "defaultSettingsForm"),
        s(F_AUXILIARY_SETTINGS_FORM, "auxiliarySettingsForm"),
        s(F_DEFAULT_VARIANT_FORM, "defaultVariantForm"),
        // auxiliaryVariantForm — Since(2.21): Designer-DENSE 2.20 его НЕ несёт
        // (ERP-witness 0/1273; SSL 2.21 — 41/41). Гейт срабатывает только на
        // designer-write в 2.20-таргет (EDT-фасад пишет SSL-версией).
        s(F_AUXILIARY_VARIANT_FORM, "auxiliaryVariantForm")
            .gated(crate::version::Since(crate::version::SSL)),
        s(F_VARIANTS_STORAGE, "variantsStorage"),
        s(F_SETTINGS_STORAGE, "settingsStorage"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        loc_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        // help — EDT-only const-блок; отсутствие = дефолт-омиссия; X-исключён.
        b(F_HELP, "help").x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (как DataProcessor/Catalog,
/// сверено: Attribute < TabularSection < Form < Template < Command). В корпусе SSL отчёты
/// несут лишь Form/Template, но субстрат полон (как DataProcessor).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "Report.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "Report.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "Report.FormRef" },
    ChildSlot { collection: "Template", child_kind: "Report.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "Report.Command" },
];

/// Канонический [`EntitySpec`] вида `Report` (кэш на процесс).
pub fn report() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Report",
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
/// Категории producedTypes вида `Report` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ReportObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ReportManager",
        designer_order: 1,
    },
];
