//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `IntegrationServiceChannel` (КаналСервисаИнтеграции), guid=acb7e81f-0637-4ebd-88ff-954ba075ae51.
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
/// `externalIntegrationServiceChannelName`  guid=41046bf7-ae10-490c-9f4a-5c1ffe33f40a
pub const F_EXTERNAL_INTEGRATION_SERVICE_CHANNEL_NAME: FieldId = FieldId(5);
/// `messageDirection`  guid=9356cb0f-d2e1-4a45-b438-37c76de00e23
pub const F_MESSAGE_DIRECTION: FieldId = FieldId(6);
/// `receiveMessageProcessing`  guid=8a054f40-1922-4331-9c14-8dfc07f2e860
pub const F_RECEIVE_MESSAGE_PROCESSING: FieldId = FieldId(7);
/// `transactioned`  guid=b9460636-d17c-437e-b82a-e56c91a302bc
pub const F_TRANSACTIONED: FieldId = FieldId(8);

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
        s(F_EXTERNAL_INTEGRATION_SERVICE_CHANNEL_NAME, "externalIntegrationServiceChannelName"),
        e(F_MESSAGE_DIRECTION, "messageDirection", "Receive"),  // literals: {Receive}
        s(F_RECEIVE_MESSAGE_PROCESSING, "receiveMessageProcessing"),
        b(F_TRANSACTIONED, "transactioned"),
    ]
}

/// Канонический [`EntitySpec`] вида `IntegrationServiceChannel` (ЧЕРНОВИК).
pub fn integration_service_channel() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "IntegrationServiceChannel",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `IntegrationServiceChannel` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "acb7e81f-0637-4ebd-88ff-954ba075ae51";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_EXTERNAL_INTEGRATION_SERVICE_CHANNEL_NAME, "41046bf7-ae10-490c-9f4a-5c1ffe33f40a"),
    (F_MESSAGE_DIRECTION, "9356cb0f-d2e1-4a45-b438-37c76de00e23"),
    (F_RECEIVE_MESSAGE_PROCESSING, "8a054f40-1922-4331-9c14-8dfc07f2e860"),
    (F_TRANSACTIONED, "b9460636-d17c-437e-b82a-e56c91a302bc"),
];
