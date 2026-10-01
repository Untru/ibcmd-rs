//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `CommonModule` (ОбщийМодуль), guid=0fe48980-252d-11d6-a3c7-0050bae0a776.
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
/// `global`  guid=7dbb2bc7-6bae-4b81-91cb-681317272a0b
pub const F_GLOBAL: FieldId = FieldId(5);
/// `clientManagedApplication`  guid=74ce8a02-abd2-46a6-8544-8cfbb4e8c6e0
pub const F_CLIENT_MANAGED_APPLICATION: FieldId = FieldId(6);
/// `server`  guid=6275a02e-96f0-4347-975a-2d661e6a0675
pub const F_SERVER: FieldId = FieldId(7);
/// `externalConnection`  guid=d12660a6-7298-4ae2-a332-b95a6459a280
pub const F_EXTERNAL_CONNECTION: FieldId = FieldId(8);
/// `clientOrdinaryApplication`  guid=436af77a-e846-4084-818b-740a3378518e
pub const F_CLIENT_ORDINARY_APPLICATION: FieldId = FieldId(9);
/// `serverCall`  guid=c474bab9-d13a-4fbd-bfb0-9214d6dc2fde
pub const F_SERVER_CALL: FieldId = FieldId(10);
/// `privileged`  guid=334033a1-6bda-4dba-bc6d-0095d1b66f0b
pub const F_PRIVILEGED: FieldId = FieldId(11);
/// `returnValuesReuse`  guid=07ddee68-6fc0-4b88-9616-7792446d12b8
pub const F_RETURN_VALUES_REUSE: FieldId = FieldId(12);

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
        b(F_GLOBAL, "global"),
        b(F_CLIENT_MANAGED_APPLICATION, "clientManagedApplication"),
        b(F_SERVER, "server"),
        b(F_EXTERNAL_CONNECTION, "externalConnection"),
        b(F_CLIENT_ORDINARY_APPLICATION, "clientOrdinaryApplication"),
        b(F_SERVER_CALL, "serverCall"),
        b(F_PRIVILEGED, "privileged"),
        e(F_RETURN_VALUES_REUSE, "returnValuesReuse", "DontUse"),  // literals: {DontUse|DuringRequest|DuringSession}
    ]
}

/// Канонический [`EntitySpec`] вида `CommonModule` (ЧЕРНОВИК).
pub fn common_module() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonModule",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `CommonModule` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "0fe48980-252d-11d6-a3c7-0050bae0a776";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_GLOBAL, "7dbb2bc7-6bae-4b81-91cb-681317272a0b"),
    (F_CLIENT_MANAGED_APPLICATION, "74ce8a02-abd2-46a6-8544-8cfbb4e8c6e0"),
    (F_SERVER, "6275a02e-96f0-4347-975a-2d661e6a0675"),
    (F_EXTERNAL_CONNECTION, "d12660a6-7298-4ae2-a332-b95a6459a280"),
    (F_CLIENT_ORDINARY_APPLICATION, "436af77a-e846-4084-818b-740a3378518e"),
    (F_SERVER_CALL, "c474bab9-d13a-4fbd-bfb0-9214d6dc2fde"),
    (F_PRIVILEGED, "334033a1-6bda-4dba-bc6d-0095d1b66f0b"),
    (F_RETURN_VALUES_REUSE, "07ddee68-6fc0-4b88-9616-7792446d12b8"),
];
