//! Канонический спек ДОЧЕРНЕГО вида `AccumulationRegister.Command` (команда регистра
//! накопления) — child-objects substrate. Лист-вид. БЕЗ `HARNESS_ENTRY` (транзитивно).
//!
//! Зеркало `InformationRegister.Command` (полный симметричный sub-object). ERP-корпус
//! содержит 1 команду (`synonym/group/commandParameterType/representation` witness'ятся);
//! прочие поля объявлены канонически (Designer-dense; вид edt-only, но спек полон).
//! EDT физический порядок задаёт `field_emit_order` EDT-проекции.

use crate::ir::value::{PropertyValue, Token, TypeSpec, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize, picture_ref_field};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `group` — группа команды (enum-литерал). Required (всегда present).
pub const F_GROUP: FieldId = FieldId(3);
/// `commandParameterType` — описание типа параметра. Default = пустой Type (`parts:[]`).
pub const F_COMMAND_PARAMETER_TYPE: FieldId = FieldId(4);
/// `parameterUseMode` — Default Single (Designer-only dense).
pub const F_PARAMETER_USE_MODE: FieldId = FieldId(5);
/// `modifiesData` — Default false.
pub const F_MODIFIES_DATA: FieldId = FieldId(6);
/// `representation` — Required (всегда present, =Auto в корпусе).
pub const F_REPRESENTATION: FieldId = FieldId(7);
/// `toolTip` — локализ., Default [].
pub const F_TOOL_TIP: FieldId = FieldId(8);
/// `picture` — Designer-only, Default "".
pub const F_PICTURE: FieldId = FieldId(9);
/// `shortcut` — Designer-only, Default "".
pub const F_SHORTCUT: FieldId = FieldId(10);
/// `onMainServerUnavalableBehavior` — Designer-only, Default Auto.
pub const F_ON_MAIN_SERVER_UNAVAILABLE: FieldId = FieldId(11);

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
    ]
}

/// `&'static EntitySpec` вида `AccumulationRegister.Command` (кэш на процесс).
pub fn accumulation_register_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister.Command",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
