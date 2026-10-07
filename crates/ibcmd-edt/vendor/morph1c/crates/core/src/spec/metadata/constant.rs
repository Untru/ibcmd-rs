//! Канонический спек вида объекта `Constant` (константа) — ARCHITECTURE.md §1.4/§1.6.
//! Витнес-вид для серилизованного Value-xsi-субстрата (`MinValue`/`MaxValue`).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo`
//! (Manager/ValueManager/ValueKey, три сгенерированных типа) — платформенный
//! [`crate::ir::InternalInfo`]-блок, фреймится каркасом коннектора (§3.5), не движком.
//!
//! Поля (подтверждено сканом SSL, 172 объекта edt+designer) в каноническом DENSE-
//! порядке Designer. EDT эмитит РАЗРЕЖЁННО (только не-дефолты + всегда-present host'ы
//! `type`/`minValue`/`maxValue`); Designer — DENSE (даже пустые self-closing/дефолты).
//!
//! `MinValue`/`MaxValue` — [`ValueKind::Value`] ([`crate::ir::value::ValueSpec`]):
//! nullable-скаляр. Хост ПРИСУТСТВУЕТ ВСЕГДА (это значение, не дефолт-омиссия), поэтому
//! поле `required` (без дефолта) — иначе разрежённый EDT опустил бы `<minValue
//! xsi:type="core:UndefinedValue"/>`. Кодировку (EDT `core:`/`<value>` ↔ Designer
//! `xs:`/`xsi:nil`/текст) держит общий `formats_xml::value_codec` — оба формата дают
//! РАВНЫЙ канонический [`crate::ir::value::ValueSpec`] (X by construction).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа значения константы (`ОписаниеТипов`). Required: host
/// `<type>`/`<Type>` present у всех объектов (present-empty — валидное значение).
pub const F_TYPE: FieldId = FieldId(3);
/// `useStandardCommands` — использовать стандартные команды. Default = `false`.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(4);
/// `defaultForm` — основная форма (`CommonForm.Имя` либо пусто). Default = `""`.
pub const F_DEFAULT_FORM: FieldId = FieldId(5);
/// `extendedPresentation` — расширенное представление (локализ.). Default = `[]`.
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(6);
/// `explanation` — пояснение (локализ.). Default = `[]`.
pub const F_EXPLANATION: FieldId = FieldId(7);
/// `passwordMode` — режим пароля. Default = `false`.
pub const F_PASSWORD_MODE: FieldId = FieldId(8);
/// `format` — формат (локализ. строка-параметризация). Default = `[]`.
pub const F_FORMAT: FieldId = FieldId(9);
/// `editFormat` — формат редактирования (локализ.). Default = `[]`.
pub const F_EDIT_FORMAT: FieldId = FieldId(10);
/// `toolTip` — подсказка (локализ.). Default = `[]`.
pub const F_TOOLTIP: FieldId = FieldId(11);
/// `markNegatives` — выделять отрицательные. Default = `false`.
pub const F_MARK_NEGATIVES: FieldId = FieldId(12);
/// `mask` — маска (свободный текст). Default = `""`.
pub const F_MASK: FieldId = FieldId(13);
/// `multiLine` — многострочный режим. Default = `false`.
pub const F_MULTI_LINE: FieldId = FieldId(14);
/// `extendedEdit` — расширенное редактирование. Default = `false`.
pub const F_EXTENDED_EDIT: FieldId = FieldId(15);
/// `minValue` — минимальное значение ([`ValueKind::Value`]). Required: host present
/// всегда (`core:UndefinedValue`/`xsi:nil` — это ЗНАЧЕНИЕ «не задано», не омиссия).
pub const F_MIN_VALUE: FieldId = FieldId(16);
/// `maxValue` — максимальное значение ([`ValueKind::Value`]). Required (как minValue).
pub const F_MAX_VALUE: FieldId = FieldId(17);
/// `fillChecking` — проверка заполнения. Default = `DontCheck`.
pub const F_FILL_CHECKING: FieldId = FieldId(18);
/// `choiceFoldersAndItems` — выбор групп и элементов. Default = `Items`.
pub const F_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(19);
/// `choiceParameterLinks` — связи параметров выбора (пустой блок). Default = `""`.
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(20);
/// `choiceParameters` — параметры выбора (пустой блок). Default = `""`.
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(21);
/// `quickChoice` — быстрый выбор. Default = `Auto`.
pub const F_QUICK_CHOICE: FieldId = FieldId(22);
/// `choiceForm` — форма выбора (пустая ссылка). Default = `""`.
pub const F_CHOICE_FORM: FieldId = FieldId(23);
/// `linkByType` — связь по типу (пустой блок). Default = `""`.
pub const F_LINK_BY_TYPE: FieldId = FieldId(24);
/// `choiceHistoryOnInput` — история выбора при вводе. Default = `Auto`.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(25);
/// `dataLockControlMode` — режим управления блокировкой данных. Default = `Automatic`.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(26);
/// `dataHistory` — история данных. Default = `DontUse`.
pub const F_DATA_HISTORY: FieldId = FieldId(27);
/// `updateDataHistoryImmediatelyAfterWrite` — обновлять историю сразу после записи.
/// Default = `false`.
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY: FieldId = FieldId(28);
/// `executeAfterWriteDataHistoryVersionProcessing` — выполнять обработку версии истории
/// после записи. Default = `false`.
pub const F_EXECUTE_AFTER_WRITE_DATA_HISTORY: FieldId = FieldId(29);

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}

fn empty_localized() -> PropertyValue {
    PropertyValue::Localized(Vec::new())
}

fn enum_default(token: &str) -> PropertyValue {
    PropertyValue::Enum(Token::new(token))
}

/// Локализованное поле с дефолтом `[]` + сортировкой по коду языка.
fn localized_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, empty_localized())
        .normalized(Normalize::LocalizedSortByLang)
}

/// Bool-поле с дефолтом `false`.
fn bool_false(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}

/// Str-поле с дефолтом `""` (пустые ссылки/блоки: defaultForm/mask/choice*).
fn str_empty(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, empty_str())
}

/// Enum-поле с дефолтным литералом.
fn enum_field(id: FieldId, name: &'static str, default_token: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, enum_default(default_token))
}

/// Сконструировать [`FieldSpec`] вида `Constant` в каноническом DENSE-порядке Designer.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        localized_field(F_SYNONYM, "synonym"),
        str_empty(F_COMMENT, "comment"),
        // Type — обязательное поле (host present всегда; present-empty валиден).
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        bool_false(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        str_empty(F_DEFAULT_FORM, "defaultForm"),
        localized_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        localized_field(F_EXPLANATION, "explanation"),
        bool_false(F_PASSWORD_MODE, "passwordMode"),
        localized_field(F_FORMAT, "format"),
        localized_field(F_EDIT_FORMAT, "editFormat"),
        localized_field(F_TOOLTIP, "toolTip"),
        bool_false(F_MARK_NEGATIVES, "markNegatives"),
        str_empty(F_MASK, "mask"),
        bool_false(F_MULTI_LINE, "multiLine"),
        bool_false(F_EXTENDED_EDIT, "extendedEdit"),
        // minValue/maxValue — Value-вид, host present всегда → required (без дефолта).
        FieldSpec::required(F_MIN_VALUE, "minValue", ValueKind::Value),
        FieldSpec::required(F_MAX_VALUE, "maxValue", ValueKind::Value),
        enum_field(F_FILL_CHECKING, "fillChecking", "DontCheck"),
        enum_field(F_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems", "Items"),
        // choiceParameterLinks/choiceParameters: БЫЛИ Str-плейсхолдерами (SSL не задавал) —
        // ERP несёт их на КОРНЕ Constant (witnessed ВидЦеныПлановойСтоимостиМатериаловРабот:
        // cpl «Отбор.Валюта»; cp — 6 объектов) → структурные List (кодеки ChoiceParameterLinks/
        // ChoiceParameters, как у реквизитов).
        FieldSpec::with_default(
            F_CHOICE_PARAMETER_LINKS,
            "choiceParameterLinks",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        FieldSpec::with_default(
            F_CHOICE_PARAMETERS,
            "choiceParameters",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        enum_field(F_QUICK_CHOICE, "quickChoice", "Auto"),
        str_empty(F_CHOICE_FORM, "choiceForm"),
        str_empty(F_LINK_BY_TYPE, "linkByType"),
        enum_field(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        enum_field(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        enum_field(F_DATA_HISTORY, "dataHistory", "DontUse"),
        bool_false(F_UPDATE_DATA_HISTORY_IMMEDIATELY, "updateDataHistoryImmediatelyAfterWrite"),
        bool_false(F_EXECUTE_AFTER_WRITE_DATA_HISTORY, "executeAfterWriteDataHistoryVersionProcessing"),
    ]
}

/// Канонический [`EntitySpec`] вида `Constant` (кэш на процесс).
pub fn constant() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Constant",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `Constant` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ConstantManager",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "ValueManager",
        edt_tag: "valueManagerType",
        designer_category: "ValueManager",
        designer_type_name: "ConstantValueManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "ValueKey",
        edt_tag: "valueKeyType",
        designer_category: "ValueKey",
        designer_type_name: "ConstantValueKey",
        designer_order: 2,
    },
];
