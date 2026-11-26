//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `ScheduledJob` (РегламентноеЗадание), guid=11bdaf85-d5ad-4d91-bb24-aa0eee139052.
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
/// `methodName`  guid=84fe0d1a-994d-45ba-9285-cf45eb673132
pub const F_METHOD_NAME: FieldId = FieldId(5);
/// `description`  guid=e37fc39f-6a18-4f1e-a3cc-8fb26fb80bd0
pub const F_DESCRIPTION: FieldId = FieldId(6);
/// `key`  guid=5397f83c-1502-4b85-8737-0c4824fb8884
pub const F_KEY: FieldId = FieldId(7);
/// `use`  guid=84add6a6-0384-4af6-8094-f5ec355d5cee
pub const F_USE: FieldId = FieldId(8);
/// `predefined`  guid=07ea7a90-663a-48cc-a132-b2cd62dec212
pub const F_PREDEFINED: FieldId = FieldId(9);
/// `restartCountOnFailure`  guid=b33f19f8-5fe5-4559-89c6-31a873a3fd38
pub const F_RESTART_COUNT_ON_FAILURE: FieldId = FieldId(10);
/// `restartIntervalOnFailure`  guid=dba7697a-4f98-4bf6-a8d6-c985a9004004
pub const F_RESTART_INTERVAL_ON_FAILURE: FieldId = FieldId(11);

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
        s(F_METHOD_NAME, "methodName"),
        s(F_DESCRIPTION, "description"),
        s(F_KEY, "key"),
        b(F_USE, "use"),
        b(F_PREDEFINED, "predefined"),
        i(F_RESTART_COUNT_ON_FAILURE, "restartCountOnFailure", 0),
        i(F_RESTART_INTERVAL_ON_FAILURE, "restartIntervalOnFailure", 0),
    ]
}

/// Канонический [`EntitySpec`] вида `ScheduledJob` (ЧЕРНОВИК).
pub fn scheduled_job() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ScheduledJob",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `ScheduledJob` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "11bdaf85-d5ad-4d91-bb24-aa0eee139052";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_METHOD_NAME, "84fe0d1a-994d-45ba-9285-cf45eb673132"),
    (F_DESCRIPTION, "e37fc39f-6a18-4f1e-a3cc-8fb26fb80bd0"),
    (F_KEY, "5397f83c-1502-4b85-8737-0c4824fb8884"),
    (F_USE, "84add6a6-0384-4af6-8094-f5ec355d5cee"),
    (F_PREDEFINED, "07ea7a90-663a-48cc-a132-b2cd62dec212"),
    (F_RESTART_COUNT_ON_FAILURE, "b33f19f8-5fe5-4559-89c6-31a873a3fd38"),
    (F_RESTART_INTERVAL_ON_FAILURE, "dba7697a-4f98-4bf6-a8d6-c985a9004004"),
];
