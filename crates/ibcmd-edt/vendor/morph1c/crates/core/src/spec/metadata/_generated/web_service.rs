//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `WebService` (WebСервис), guid=8657032e-7740-4e1d-a3ba-5dd6e8afb78f.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `namespace`  guid=c8a6c337-57d5-479f-84c5-76c73e6761ee
pub const F_NAMESPACE: FieldId = FieldId(5);
/// `descriptorFileName`  guid=2d15f3d3-9c28-4a47-8d76-2ca859b04f54
pub const F_DESCRIPTOR_FILE_NAME: FieldId = FieldId(6);
/// `reuseSessions`  guid=e1d1cc9f-5e20-4065-9205-c502efb5f8ec
pub const F_REUSE_SESSIONS: FieldId = FieldId(7);
/// `sessionMaxAge`  guid=d58dbde9-0ae8-406f-a288-13c7a5670dd6
pub const F_SESSION_MAX_AGE: FieldId = FieldId(8);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(def)))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn i(id: FieldId, name: &'static str, def: i64) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Int, PropertyValue::Int(def))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        s(F_NAMESPACE, "namespace"),
        s(F_DESCRIPTOR_FILE_NAME, "descriptorFileName"),
        e(F_REUSE_SESSIONS, "reuseSessions", "DontUse"),  // literals: {DontUse|Use|AutoUse}
        i(F_SESSION_MAX_AGE, "sessionMaxAge", 0),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "XdtoPackage", child_kind: "WebService.XdtoPackage" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `WebService` (ЧЕРНОВИК).
pub fn web_service() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebService",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `WebService` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "8657032e-7740-4e1d-a3ba-5dd6e8afb78f";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_NAMESPACE, "c8a6c337-57d5-479f-84c5-76c73e6761ee"),
    (F_DESCRIPTOR_FILE_NAME, "2d15f3d3-9c28-4a47-8d76-2ca859b04f54"),
    (F_REUSE_SESSIONS, "e1d1cc9f-5e20-4065-9205-c502efb5f8ec"),
    (F_SESSION_MAX_AGE, "d58dbde9-0ae8-406f-a288-13c7a5670dd6"),
];
