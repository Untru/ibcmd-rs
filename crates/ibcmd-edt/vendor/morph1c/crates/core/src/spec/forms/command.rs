//! Канонический спек ПОЛЬЗОВАТЕЛЬСКОЙ КОМАНДЫ формы (`<formCommands>` EDT /
//! `<Commands><Command>` Designer) — ARCHITECTURE.md §1.4/§1.6, под-IR L1f.
//!
//! Команда формы — элемент форм-уровневой коллекции команд: имя+id (идентичность, в
//! каркасе коннектора) + property-bag по каноническому [`FieldId`] ЭТОГО спека. Поля,
//! порядок и дефолты сверены по КОРПУСУ (512 команд в 93 из 104 SSL CommonForms;
//! слайс LANE-F-1):
//!
//! | канон. поле | EDT | Designer | эмиссия |
//! |---|---|---|---|
//! | `title` | `<title>` (key/value) | `<Title>` (`v8:item`) | оба всегда (512/512) |
//! | `toolTip` | `<toolTip>` | `<ToolTip>` | оба одинаково (392) |
//! | `shortcut` | `<shortcut>` | `<Shortcut>` | оба одинаково (19) |
//! | `picture` | `<picture xsi:type="core:PictureRef"><picture>Ref` | `<Picture><xr:Ref>Ref + <xr:LoadTransparent>` | оба одинаково (260) |
//! | `action` | `<action xsi:type="form:FormCommandHandlerContainer"><handler><name>` | `<Action>текст` | оба одинаково (510; 2 команды БЕЗ action) |
//! | `actionPurpose` | `<actionPurpose>` | `<ActionPurpose>` | оба одинаково (102) |
//! | `representation` | `<representation>` | `<Representation>` | оба одинаково (144; общий дефолт `Auto` опущен) |
//! | `modifiesStoredData` | `<modifiesStoredData>` | `<ModifiesSavedData>` | оба одинаково (21; РАЗНЫЕ теги) |
//! | `currentRowUse` | `<currentRowUse>` | `<CurrentRowUse>` | ПЕР-ФОРМАТНЫЕ дефолты (см. ниже) |
//! | `associatedTableElementId` | `<… xsi:type="core:StringValue"><value>` | `<… xsi:type="xs:string">текст` | оба одинаково (88) |
//! | `selectedRowsUse` | `<selectedRowsUse>` | `<SelectedRowsUse>` | ПЕР-ФОРМАТНЫЕ дефолты |
//!
//! # Пер-форматные дефолты `currentRowUse`/`selectedRowsUse` (cross-сверка 512 команд)
//! EDT опускает `Use` (свой дефолт), Designer опускает `Auto` (свой дефолт) — счёт сходится
//! ТОЧНО: `currentRowUse` EDT эмитит Auto=199/DontUse=246 (опущено 67), Designer эмитит
//! DontUse=246/Use=67 (опущено 199). Поле НЕЛЬЗЯ дропать из канон-bag (ни один writer не
//! восстановил бы значение) ⇒ канонический bag несёт его ВСЕГДА (required); ридер каждого
//! формата заполняет СВОЙ дефолт при отсутствии, writer опускает == своему дефолту.
//!
//! # Формат-константы (в IR НЕ хранятся, каркас коннектора)
//! * EDT `<use><common>true</common></use>` — эмитится ВСЕГДА (512/512), Designer-аналога
//!   в SSL нет (реконструируется);
//! * Designer `<xr:LoadTransparent>` картинки — ДЕНОРМАЛИЗАЦИЯ вида ссылки: `StdPicture.*`
//!   → `true` (160/160), `CommonPicture.*` → `false` (100/100); реконструируется из ссылки,
//!   несоответствие — типизированная ошибка (§1.0).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `title`/`Title` — локализованный заголовок команды (оба формата эмитят всегда).
pub const F_TITLE: FieldId = FieldId(1);
/// `toolTip`/`ToolTip` — локализованная подсказка (оба эмитят одинаково; дефолт пусто).
pub const F_TOOL_TIP: FieldId = FieldId(2);
/// `shortcut`/`Shortcut` — сочетание клавиш (`Ctrl+S`, `F5`; текст; дефолт пусто).
pub const F_SHORTCUT: FieldId = FieldId(3);
/// `picture`/`Picture` — ссылка на картинку (`StdPicture.X`/`CommonPicture.X`; дефолт пусто).
pub const F_PICTURE: FieldId = FieldId(4);
/// `action`/`Action` — имя процедуры-обработчика в модуле формы (дефолт пусто = нет action).
pub const F_ACTION: FieldId = FieldId(5);
/// `actionPurpose`/`ActionPurpose` — назначение действия (`Create`/`Finish`; общий дефолт).
pub const F_ACTION_PURPOSE: FieldId = FieldId(6);
/// `representation`/`Representation` — представление (`Text`/`Picture`/`TextPicture`).
pub const F_REPRESENTATION: FieldId = FieldId(7);
/// `modifiesStoredData` (EDT) / `ModifiesSavedData` (Designer) — изменяет сохранённые
/// данные (Bool; дефолт false). ЕДИНСТВЕННОЕ поле команды с РАЗНОИМЁННЫМИ тегами.
pub const F_MODIFIES_STORED_DATA: FieldId = FieldId(8);
/// `currentRowUse`/`CurrentRowUse` — использование текущей строки. ПЕР-ФОРМАТНЫЕ дефолты
/// (EDT `Use` / Designer `Auto`) ⇒ канон-bag несёт всегда (required).
pub const F_CURRENT_ROW_USE: FieldId = FieldId(9);
/// `associatedTableElementId`/`AssociatedTableElementId` — имя связанной таблицы формы
/// (строка; EDT кодирует `core:StringValue`-обёрткой, Designer — `xs:string`-текстом).
pub const F_ASSOCIATED_TABLE_ELEMENT_ID: FieldId = FieldId(10);
/// `selectedRowsUse`/`SelectedRowsUse` — использование выделенных строк. ПЕР-ФОРМАТНЫЕ
/// дефолты (EDT `Use` / Designer `Auto`) ⇒ канон-bag несёт всегда (required).
pub const F_SELECTED_ROWS_USE: FieldId = FieldId(11);
/// `functionalOptions`/`FunctionalOptions` — функциональные опции команды (список
/// `FunctionalOption.<Имя>`-ссылок; SSL 39⟷39, все по одной). DUAL-ENCODING: EDT
/// ПОВТОРЯЕМЫЙ `<functionalOptions>Ref</functionalOptions>` ⟺ Designer контейнер
/// `<FunctionalOptions><Item>Ref</Item>…</FunctionalOptions>`. Канон — `List` из `Ref`.
/// Метамодель `FormCommand`: `actionPurpose` (8) → **functionalOptions (9)** →
/// `representation` (10) — обе стороны эмитят между Action/ActionPurpose и
/// Representation/CurrentRowUse (сверено 39/39).
pub const F_FUNCTIONAL_OPTIONS: FieldId = FieldId(12);

/// EDT-дефолт `currentRowUse`/`selectedRowsUse` (EDT ОПУСКАЕТ `Use`; cross-сверка 67/47).
pub const ROW_USE_EDT_DEFAULT: &str = "Use";
/// Designer-дефолт `currentRowUse`/`selectedRowsUse` (Designer ОПУСКАЕТ `Auto`; 199/438).
pub const ROW_USE_DESIGNER_DEFAULT: &str = "Auto";
/// Общий (обоим форматам) дефолт `representation` — опускается ОБОИМИ одинаково.
pub const REPRESENTATION_DEFAULT: &str = "Auto";
/// Общий дефолт `actionPurpose` (платформенный `Use`) — опускается ОБОИМИ одинаково;
/// в корпусе witnessed только не-дефолтные `Create`/`Finish`.
pub const ACTION_PURPOSE_DEFAULT: &str = "Use";
/// Префикс ссылок стандартных картинок: `StdPicture.*` ⇒ Designer `LoadTransparent=true`;
/// иначе (`CommonPicture.*`) ⇒ `false` (сверено 260/260 картинок корпуса).
pub const PICTURE_STD_PREFIX: &str = "StdPicture.";

/// Поля команды в КАНОНИЧЕСКОМ порядке эмиссии (= порядок EDT-тела `<formCommands>`,
/// сверен по 512 командам; Designer переставляет к своему порядку в проекции).
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_TITLE,
            "title",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(
            F_TOOL_TIP,
            "toolTip",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(
            F_SHORTCUT,
            "shortcut",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        FieldSpec::with_default(
            F_PICTURE,
            "picture",
            ValueKind::Ref,
            PropertyValue::Ref(String::new()),
        ),
        FieldSpec::with_default(
            F_ACTION,
            "action",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        FieldSpec::with_default(
            F_ACTION_PURPOSE,
            "actionPurpose",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(ACTION_PURPOSE_DEFAULT)),
        ),
        FieldSpec::with_default(
            F_FUNCTIONAL_OPTIONS,
            "functionalOptions",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        FieldSpec::with_default(
            F_REPRESENTATION,
            "representation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(REPRESENTATION_DEFAULT)),
        ),
        FieldSpec::with_default(
            F_MODIFIES_STORED_DATA,
            "modifiesStoredData",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // currentRowUse/selectedRowsUse: required — канон-bag несёт значение ВСЕГДА
        // (пер-форматные дефолты противоположны, дроп невосстановим; см. модуль-док).
        FieldSpec::required(F_CURRENT_ROW_USE, "currentRowUse", ValueKind::Enum),
        FieldSpec::with_default(
            F_ASSOCIATED_TABLE_ELEMENT_ID,
            "associatedTableElementId",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        FieldSpec::required(F_SELECTED_ROWS_USE, "selectedRowsUse", ValueKind::Enum),
    ]
}

/// Канонический [`EntitySpec`] команды формы (кэш на процесс).
pub fn form_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "FormCommand",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
