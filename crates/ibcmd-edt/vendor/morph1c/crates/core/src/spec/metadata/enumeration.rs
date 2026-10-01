//! Канонический спек вида объекта `Enum` (перечисление) — ARCHITECTURE.md §1.4/§1.6,
//! child-objects substrate. ПЕРВЫЙ child-несущий вид:
//! доказывает рекурсивный субстрат подчинённых коллекций на минимальном пилоте
//! (`EnumValues` — чистая одноуровневая leaf-коллекция).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация, ПОРЯДОК эмиссии и СЛОТ
//! дочерней коллекции `enumValues → Enum.EnumValue` (§1.6).
//!
//! Спек-свойства (подтверждено сканом SSL, 104 объекта edt+designer). Порядок =
//! каноническому DENSE-порядку Designer: `Synonym, Comment, UseStandardCommands,
//! StandardAttributes, Characteristics, QuickChoice, ChoiceMode, DefaultListForm,
//! DefaultChoiceForm, AuxiliaryListForm, AuxiliaryChoiceForm, ListPresentation,
//! ExtendedListPresentation, Explanation, ChoiceHistoryOnInput`. EDT эмитит РАЗРЕЖЁННО
//! (только не-дефолты); Designer — DENSE (даже пустые self-closing).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo`
//! — платформенный [`crate::ir::InternalInfo`]-блок (каркас коннектора). Дочерние
//! `enumValues` — НЕ свойства, а [`ChildSlot`] (рекурсия движка).
//!
//! `StandardAttributes` — платформенный КОНСТАНТНЫЙ блок (Order+Ref), для Enum
//! побайтово-неизменный во всём корпусе (§3.3 design). Моделируется presence-маркером
//! `Bool(true)` (всегда present, без дефолта → не опускается); byte-exact-блок
//! воспроизводит кодек проекции. X by construction (оба формата → `Bool(true)`).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — использовать стандартные команды. Default = `false`.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `standardAttributes` — платформенный блок Order+Ref. ВАРИАТИВЕН (не константа):
/// `fullTextSearch` атрибута задаётся пользователем. IR — `List` из 2×`List([synonym,
/// toolTip, fillValue, fullTextSearch])`; пустой `List` = блок опущен целиком.
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(4);
/// `characteristics` — Designer-only пустой self-closing блок. Default = `""`.
pub const F_CHARACTERISTICS: FieldId = FieldId(5);
/// `quickChoice` — быстрый выбор. Default = `false` (в SSL всегда `true` → эмитится).
pub const F_QUICK_CHOICE: FieldId = FieldId(6);
/// `choiceMode` — режим выбора. Default = `FromForm` (EDT омитит FromForm; сверено корпусом).
pub const F_CHOICE_MODE: FieldId = FieldId(7);
/// `defaultListForm` — Designer-only пустая ссылка. Default = `""`.
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(8);
/// `defaultChoiceForm` — Designer-only пустая ссылка. Default = `""`.
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(9);
/// `auxiliaryListForm` — Designer-only пустая ссылка. Default = `""`.
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(10);
/// `auxiliaryChoiceForm` — Designer-only пустая ссылка. Default = `""`.
pub const F_AUXILIARY_CHOICE_FORM: FieldId = FieldId(11);
/// `listPresentation` — локализованное представление списка. Default = пустой список.
pub const F_LIST_PRESENTATION: FieldId = FieldId(12);
/// `extendedListPresentation` — расширенное локализ. представление. Default = пусто.
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(13);
/// `explanation` — локализованное пояснение. Default = пустой список.
pub const F_EXPLANATION: FieldId = FieldId(14);
/// `choiceHistoryOnInput` — история выбора при вводе. Default = `Auto`.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(15);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(16);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(17);

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}

/// Сконструировать [`FieldSpec`] вида `Enum` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, empty_str()),
        FieldSpec::with_default(
            F_USE_STANDARD_COMMANDS,
            "useStandardCommands",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // standardAttributes: платформенный блок Order+Ref. НЕ константа: `fullTextSearch`
        // каждого атрибута ЗАДАЁТСЯ (witnessed `DontUse` в integration_subsystem) → IR несёт
        // `List` из 2×`List([syn,tip,fill,fts])` (тот же канон, что у InformationRegister —
        // одна декларация-данные, см. `formats_xml::std_attrs_ir`). Default = пустой `List`
        // (блок опущен целиком: все атрибуты дефолтны; cf кодирует ту же ситуацию слотом `{0}`).
        FieldSpec::with_default(
            F_STANDARD_ATTRIBUTES,
            "standardAttributes",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        FieldSpec::with_default(F_CHARACTERISTICS, "characteristics", ValueKind::Str, empty_str()),
        // quickChoice/choiceMode: разрежённый EDT ОПУСКАЕТ их при дефолте (минимальный
        // корпус покрытия витнессит `Пер_БыстрыйВыбор_Ложь` без <quickChoice> и
        // `Пер_РежимВыбора_ИзФормы` без <choiceMode>) → with_default, НЕ required. Designer
        // DENSE несёт их всегда (read-сторона сжимает дефолт), cf DENSE-позиционен
        // (emit_defaults). Дефолты сверены по корпусу: quickChoice=false, choiceMode=FromForm.
        FieldSpec::with_default(F_QUICK_CHOICE, "quickChoice", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_CHOICE_MODE,
            "choiceMode",
            ValueKind::Enum,
            PropertyValue::Enum(crate::ir::value::Token::new("FromForm")),
        ),
        FieldSpec::with_default(F_DEFAULT_LIST_FORM, "defaultListForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_AUXILIARY_LIST_FORM, "auxiliaryListForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(
            F_AUXILIARY_CHOICE_FORM,
            "auxiliaryChoiceForm",
            ValueKind::Str,
            empty_str(),
        ),
        // Презентации/пояснение — ЛОКАЛИЗОВАННЫЕ строки (default []; в SSL почти все
        // пустые → Designer self-closing `<Tag/>`, EDT опускает; 1 explanation непуст).
        FieldSpec::with_default(
            F_LIST_PRESENTATION,
            "listPresentation",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(
            F_EXTENDED_LIST_PRESENTATION,
            "extendedListPresentation",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(
            F_EXPLANATION,
            "explanation",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(
            F_CHOICE_HISTORY_ON_INPUT,
            "choiceHistoryOnInput",
            ValueKind::Enum,
            PropertyValue::Enum(crate::ir::value::Token::new("Auto")),
        ),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций вида `Enum` в КАНОНИЧЕСКОМ порядке эмиссии (сверено
/// s15_subordinate обоими форматами): EnumValue < Form < Template < Command. Forms/
/// Templates — REF/STUB-коллекции (EDT inline-стаб, Designer bare-ссылка), Command —
/// полный sub-object.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "EnumValue", child_kind: "Enum.EnumValue" },
    ChildSlot { collection: "Form", child_kind: "Enum.FormRef" },
    ChildSlot { collection: "Template", child_kind: "Enum.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "Enum.Command" },
];

/// Канонический [`EntitySpec`] вида `Enum` (кэш на процесс).
///
/// NB: имя функции = stem файла (`enumeration`, т.к. `enum` — ключевое слово Rust и
/// невалидно как имя модуля/функции; build.rs генерирует
/// `crate::spec::metadata::enumeration::enumeration`). Канонический КОД вида
/// (`"Enum"`) задаёт `EntitySpec.entity`, а не имя файла/функции (§1.6).
pub fn enumeration() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Enum",
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
/// Категории producedTypes вида `Enum` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "EnumRef",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "EnumList",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "EnumManager",
        designer_order: 1,
    },
];
