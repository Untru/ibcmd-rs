//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `DimensionTableCommand`, guid=8352a2a3-943d-43a2-95d9-1c592b1d337d.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `group`  guid=482411f7-457f-4889-a7c9-9adbfb1c7bd4
pub const F_GROUP: FieldId = FieldId(5);
/// `commandParameterType`  guid=7d14f63a-87e8-4188-a28b-02da93f6bcbd
pub const F_COMMAND_PARAMETER_TYPE: FieldId = FieldId(6);
/// `parameterUseMode`  guid=5aa63d14-f91f-4329-9cc2-03d9dfd84146
pub const F_PARAMETER_USE_MODE: FieldId = FieldId(7);
/// `modifiesData`  guid=39588d76-1919-4761-a171-771b67b44ee8
pub const F_MODIFIES_DATA: FieldId = FieldId(8);
/// `representation`  guid=a5085b15-9218-4da8-b2e1-3c5d357b9f82
pub const F_REPRESENTATION: FieldId = FieldId(9);
/// `toolTip`  guid=4c3288f7-2704-4f31-9bf3-4b8e8e080682
pub const F_TOOL_TIP: FieldId = FieldId(10);
/// `picture`  guid=58e51e5d-aee0-41ea-89c0-b0962a74d237
pub const F_PICTURE: FieldId = FieldId(11);
/// `shortcut`  guid=7f647b00-8821-4452-b29c-0e953d757fde
pub const F_SHORTCUT: FieldId = FieldId(12);
/// `onMainServerUnavalableBehavior` (since 8.3.16)  guid=4994a976-b6d8-4efa-9484-2891e0d31f80
pub const F_ON_MAIN_SERVER_UNAVALABLE_BEHAVIOR: FieldId = FieldId(13);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(def)))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        s(F_GROUP, "group"),  // TODO(draft): ref -> CommandGroup (form-ref Str vs Ref?)
        FieldSpec::required(F_COMMAND_PARAMETER_TYPE, "commandParameterType", ValueKind::Type),  // TODO(draft): Type-кодек
        e(F_PARAMETER_USE_MODE, "parameterUseMode", "Single"),  // literals: {Single|Multiple}
        b(F_MODIFIES_DATA, "modifiesData"),
        e(F_REPRESENTATION, "representation", "Text"),  // literals: {Text|Picture|PictureAndText|Auto}
        loc_field(F_TOOL_TIP, "toolTip"),
        s(F_PICTURE, "picture"),  // TODO(draft): PictureRef-кодек
        s(F_SHORTCUT, "shortcut"),  // TODO(draft): contained Shortcut — уточнить vk
        e(F_ON_MAIN_SERVER_UNAVALABLE_BEHAVIOR, "onMainServerUnavalableBehavior", "Auto"),  // literals: {Auto|MakeDisable|DontChangeBehavior}
    ]
}

/// Канонический [`EntitySpec`] вида `DimensionTableCommand` (ЧЕРНОВИК).
pub fn dimension_table_command() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DimensionTableCommand",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `DimensionTableCommand` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "8352a2a3-943d-43a2-95d9-1c592b1d337d";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_GROUP, "482411f7-457f-4889-a7c9-9adbfb1c7bd4"),
    (F_COMMAND_PARAMETER_TYPE, "7d14f63a-87e8-4188-a28b-02da93f6bcbd"),
    (F_PARAMETER_USE_MODE, "5aa63d14-f91f-4329-9cc2-03d9dfd84146"),
    (F_MODIFIES_DATA, "39588d76-1919-4761-a171-771b67b44ee8"),
    (F_REPRESENTATION, "a5085b15-9218-4da8-b2e1-3c5d357b9f82"),
    (F_TOOL_TIP, "4c3288f7-2704-4f31-9bf3-4b8e8e080682"),
    (F_PICTURE, "58e51e5d-aee0-41ea-89c0-b0962a74d237"),
    (F_SHORTCUT, "7f647b00-8821-4452-b29c-0e953d757fde"),
    (F_ON_MAIN_SERVER_UNAVALABLE_BEHAVIOR, "4994a976-b6d8-4efa-9484-2891e0d31f80"),
];
