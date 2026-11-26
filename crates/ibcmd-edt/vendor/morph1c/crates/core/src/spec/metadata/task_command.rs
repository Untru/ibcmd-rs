//! Канонический спек ДОЧЕРНЕГО вида `Task.Command` (команда задачи) — child-objects
//! substrate, Task-срез. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Набор полей ИДЕНТИЧЕН `Document.Command`/`Catalog.Command` (сверено по корпусу: Name,
//! Synonym, Comment, Group, CommandParameterType, ParameterUseMode, ModifiesData,
//! Representation, ToolTip, Picture, Shortcut, OnMainServerUnavalableBehavior). EDT эмитит
//! РАЗРЕЖЁННО (физический порядок — `field_emit_order` в проекциях).

use crate::ir::value::{PropertyValue, Token, TypeSpec, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize, picture_ref_field};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `group` — группа команды (enum-литерал). Required (всегда present).
pub const F_GROUP: FieldId = FieldId(3);
/// `commandParameterType` — описание типа параметра. Default = пустой Type.
pub const F_COMMAND_PARAMETER_TYPE: FieldId = FieldId(4);
/// `parameterUseMode` — Default Single.
pub const F_PARAMETER_USE_MODE: FieldId = FieldId(5);
/// `modifiesData` — Default false.
pub const F_MODIFIES_DATA: FieldId = FieldId(6);
/// `representation` — Default `Text` (ECORE-дефолт ButtonRepresentation; ERP-witnessed
/// омиссия в EDT у ОперацииЗакрытияМесяца при designer DENSE `Text`).
pub const F_REPRESENTATION: FieldId = FieldId(7);
/// `toolTip` — локализ., Default [].
pub const F_TOOL_TIP: FieldId = FieldId(8);
/// `picture` — ссылка на стандартную картинку (picture-REF codec, data-driven). Default "".
pub const F_PICTURE: FieldId = FieldId(9);
/// `shortcut` — datatype `Shortcut` (комбинация клавиш). Default "".
pub const F_SHORTCUT: FieldId = FieldId(10);
/// `onMainServerUnavalableBehavior` — Designer-only, Default Auto.
pub const F_ON_MAIN_SERVER_UNAVAILABLE: FieldId = FieldId(11);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(12);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(13);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::required(F_GROUP, "group", ValueKind::Enum),
        FieldSpec::with_default(
            F_COMMAND_PARAMETER_TYPE,
            "commandParameterType",
            ValueKind::Type,
            PropertyValue::Type(TypeSpec { parts: Vec::new() }),
        ),
        FieldSpec::with_default(F_PARAMETER_USE_MODE, "parameterUseMode", ValueKind::Enum, PropertyValue::Enum(Token::new("Single"))),
        FieldSpec::with_default(F_MODIFIES_DATA, "modifiesData", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(
            F_REPRESENTATION,
            "representation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Text")),
        ),
        FieldSpec::with_default(F_TOOL_TIP, "toolTip", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        picture_ref_field(F_PICTURE),
        FieldSpec::with_default(F_SHORTCUT, "shortcut", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_ON_MAIN_SERVER_UNAVAILABLE, "onMainServerUnavalableBehavior", ValueKind::Enum, PropertyValue::Enum(Token::new("Auto"))),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// `&'static EntitySpec` вида `Task.Command` (кэш на процесс).
pub fn task_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task.Command",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
