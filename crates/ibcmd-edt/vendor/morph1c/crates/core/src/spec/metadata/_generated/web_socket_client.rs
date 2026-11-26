//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `WebSocketClient` (WebSocketКлиент), guid=053855df-dd0e-418f-b5b5-51a22636e930.
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
/// `predefined`  guid=936dde56-19be-4920-b47c-365b19e54500
pub const F_PREDEFINED: FieldId = FieldId(5);
/// `autoConnect`  guid=88f198f5-2bbe-4383-a95b-77e0ea2c1920
pub const F_AUTO_CONNECT: FieldId = FieldId(6);
/// `serverURL`  guid=4cdd958b-bdd6-4499-a349-674a5a514c5c
pub const F_SERVER_U_R_L: FieldId = FieldId(7);
/// `user`  guid=2fa51bc5-fbd1-4294-baf0-7f5c3ba7ce2f
pub const F_USER: FieldId = FieldId(8);
/// `password`  guid=a55d20a7-521a-4809-9aaa-e6af353e746e
pub const F_PASSWORD: FieldId = FieldId(9);
/// `useOSProxy`  guid=6b25825a-792a-4378-bbdd-02db80db9bbf
pub const F_USE_O_S_PROXY: FieldId = FieldId(10);
/// `useOSAuthentication`  guid=fddabaf4-ac47-4492-be60-0410b7580b3b
pub const F_USE_O_S_AUTHENTICATION: FieldId = FieldId(11);
/// `timeout`  guid=854859fa-91f9-4b12-805b-6caaf7111a93
pub const F_TIMEOUT: FieldId = FieldId(12);

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
fn i(id: FieldId, name: &'static str, def: i64) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Int, PropertyValue::Int(def))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        b(F_PREDEFINED, "predefined"),
        b(F_AUTO_CONNECT, "autoConnect"),
        s(F_SERVER_U_R_L, "serverURL"),
        s(F_USER, "user"),
        s(F_PASSWORD, "password"),
        b(F_USE_O_S_PROXY, "useOSProxy"),
        b(F_USE_O_S_AUTHENTICATION, "useOSAuthentication"),
        i(F_TIMEOUT, "timeout", 0),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Header", child_kind: "WebSocketClient.Header" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `WebSocketClient` (ЧЕРНОВИК).
pub fn web_socket_client() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebSocketClient",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `WebSocketClient` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "053855df-dd0e-418f-b5b5-51a22636e930";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_PREDEFINED, "936dde56-19be-4920-b47c-365b19e54500"),
    (F_AUTO_CONNECT, "88f198f5-2bbe-4383-a95b-77e0ea2c1920"),
    (F_SERVER_U_R_L, "4cdd958b-bdd6-4499-a349-674a5a514c5c"),
    (F_USER, "2fa51bc5-fbd1-4294-baf0-7f5c3ba7ce2f"),
    (F_PASSWORD, "a55d20a7-521a-4809-9aaa-e6af353e746e"),
    (F_USE_O_S_PROXY, "6b25825a-792a-4378-bbdd-02db80db9bbf"),
    (F_USE_O_S_AUTHENTICATION, "fddabaf4-ac47-4492-be60-0410b7580b3b"),
    (F_TIMEOUT, "854859fa-91f9-4b12-805b-6caaf7111a93"),
];
