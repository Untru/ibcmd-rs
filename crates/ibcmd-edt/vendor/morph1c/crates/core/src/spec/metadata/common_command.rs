//! Канонический спек вида объекта `CommonCommand` (общая команда) — ARCHITECTURE.md
//! §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок выводит read/write для КАЖДОГО формата
//! (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь — канонический `id`,
//! `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта.
//!
//! Поля (подтверждено корпусом SSL, 63 объекта edt+designer; Designer DENSE — все поля
//! present всегда, EDT SPARSE — дефолты опущены):
//! * `Synonym` — локализованный синоним;
//! * `Comment` — свободный текст (default `""`);
//! * `Group` — группа команды: enum-литерал (`NavigationPanelOrdinary`/…) ИЛИ ссылка
//!   `CommandGroup.Имя`. Плоская строка, present у всех 63 → required;
//! * `Representation` — литерал представления (`Auto`/`Picture`/…). Required;
//! * `ToolTip` — локализованная подсказка (default пустой список);
//! * `Picture` — ссылка на картинку (`PictureRef`; default `""`);
//! * `Shortcut` — горячая клавиша (свободная строка; default `""`; в корпусе пусто);
//! * `IncludeHelpInContents` — bool включения справки (default `false`);
//! * `CommandParameterType` — тип параметра команды (`ОписаниеТипов`/`TypeSet`). Default =
//!   ПУСТОЕ описание (`parts:[]`): EDT опускает, Designer DENSE `<CommandParameterType/>`;
//! * `ParameterUseMode` — литерал режима параметра (`Single`/`Multiple`; default `Single`);
//! * `ModifiesData` — bool изменения данных (default `false`);
//! * `OnMainServerUnavalableBehavior` — литерал поведения при недоступности (default `Auto`).
//!
//! Лист-вид: подчинённых коллекций нет. Порядок = DENSE-порядок Designer `<Properties>`.

use crate::ir::value::{PropertyValue, Token, TypeSpec, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize, picture_ref_field};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `group` — группа команды (enum-литерал `NavigationPanelOrdinary`/… ИЛИ ссылка
/// `CommandGroup.Имя`). Перечислимый литерал (как `Catalog.Command.group`): дотированная
/// ссылка — лишь текст токена, round-trip'ится byte-exact. Required.
pub const F_GROUP: FieldId = FieldId(3);
/// `representation` — литерал представления (перечислимый). Required.
pub const F_REPRESENTATION: FieldId = FieldId(4);
/// `toolTip` — локализованная подсказка. Default = пустой список; сорт по коду языка.
pub const F_TOOLTIP: FieldId = FieldId(5);
/// `picture` — ссылка на картинку (`PictureRef`). Default = `""`.
pub const F_PICTURE: FieldId = FieldId(6);
/// `shortcut` — горячая клавиша (свободная строка). Default = `""`.
pub const F_SHORTCUT: FieldId = FieldId(7);
/// `includeHelpInContents` — включить справку в содержание (bool). Default = `false`.
pub const F_INCLUDE_HELP: FieldId = FieldId(8);
/// `commandParameterType` — тип параметра команды (`ОписаниеТипов`). Default = пустое
/// описание (`parts:[]`): EDT опускает, Designer DENSE `<CommandParameterType/>`.
pub const F_COMMAND_PARAMETER_TYPE: FieldId = FieldId(9);
/// `parameterUseMode` — режим использования параметра (перечислимый). Default = `Single`.
pub const F_PARAMETER_USE_MODE: FieldId = FieldId(10);
/// `modifiesData` — изменяет данные (bool). Default = `false`.
pub const F_MODIFIES_DATA: FieldId = FieldId(11);
/// `onMainServerUnavalableBehavior` — поведение при недоступности (перечислимый). Default = `Auto`.
pub const F_ON_MAIN_SERVER_UNAVALABLE: FieldId = FieldId(12);

/// Сконструировать [`FieldSpec`] вида `CommonCommand` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // Group — перечислимый литерал (enum-токен ИЛИ дотированная ссылка как текст), required.
        FieldSpec::required(F_GROUP, "group", ValueKind::Enum),
        FieldSpec::with_default(
            F_REPRESENTATION,
            "representation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Text")),
        ),
        FieldSpec::with_default(
            F_TOOLTIP,
            "toolTip",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        picture_ref_field(F_PICTURE),
        FieldSpec::with_default(F_SHORTCUT, "shortcut", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_INCLUDE_HELP,
            "includeHelpInContents",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // CommandParameterType — описание типов; default = ПУСТОЕ (`parts:[]`).
        FieldSpec::with_default(
            F_COMMAND_PARAMETER_TYPE,
            "commandParameterType",
            ValueKind::Type,
            PropertyValue::Type(TypeSpec { parts: Vec::new() }),
        ),
        FieldSpec::with_default(
            F_PARAMETER_USE_MODE,
            "parameterUseMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Single")),
        ),
        FieldSpec::with_default(F_MODIFIES_DATA, "modifiesData", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_ON_MAIN_SERVER_UNAVALABLE,
            "onMainServerUnavalableBehavior",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Auto")),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonCommand` (кэш на процесс).
pub fn common_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonCommand",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
