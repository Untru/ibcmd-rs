//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Operation` (Операция), guid=36186084-c23a-43bd-876c-a3a8ba1a9622.
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
/// `xdtoReturningValueType`  guid=4f239ce7-e38f-4ee4-95bc-8a48f81eae85
pub const F_XDTO_RETURNING_VALUE_TYPE: FieldId = FieldId(5);
/// `nillable`  guid=924dfdc9-b8da-4b1f-a631-57a8a6c05e65
pub const F_NILLABLE: FieldId = FieldId(6);
/// `transactioned`  guid=b9460636-d17c-437e-b82a-e56c91a302bc
pub const F_TRANSACTIONED: FieldId = FieldId(7);
/// `procedureName`  guid=35c0830c-c0cc-462a-ab18-9ddc77f18703
pub const F_PROCEDURE_NAME: FieldId = FieldId(8);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(9);

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
        s(F_XDTO_RETURNING_VALUE_TYPE, "xdtoReturningValueType"),  // TODO(draft): contained QName — уточнить vk
        b(F_NILLABLE, "nillable"),
        b(F_TRANSACTIONED, "transactioned"),
        s(F_PROCEDURE_NAME, "procedureName"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
    ]
}

/// Канонический [`EntitySpec`] вида `Operation` (ЧЕРНОВИК).
pub fn operation() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Operation",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Operation` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "36186084-c23a-43bd-876c-a3a8ba1a9622";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_XDTO_RETURNING_VALUE_TYPE, "4f239ce7-e38f-4ee4-95bc-8a48f81eae85"),
    (F_NILLABLE, "924dfdc9-b8da-4b1f-a631-57a8a6c05e65"),
    (F_TRANSACTIONED, "b9460636-d17c-437e-b82a-e56c91a302bc"),
    (F_PROCEDURE_NAME, "35c0830c-c0cc-462a-ab18-9ddc77f18703"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
];
