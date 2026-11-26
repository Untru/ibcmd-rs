//! Канонический спек ДОЧЕРНЕГО вида `ChartOfCalculationTypes.Command` (команда плана видов
//! расчёта) — child-objects substrate, Catalog-срез. Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! ПОЛНЫЙ симметричный sub-object (как `Catalog.Command`). Coverage-корпус команд не несёт,
//! но коллекция объявлена для схемной полноты Catalog-субстрата. EDT эмитит РАЗРЕЖЁННО.

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
/// `picture` — Designer-only, Default "".
pub const F_PICTURE: FieldId = FieldId(9);
/// `shortcut` — datatype `Shortcut`. Default "".
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

/// `&'static EntitySpec` вида `ChartOfCalculationTypes.Command` (кэш на процесс).
pub fn chart_of_calculation_types_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes.Command",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
