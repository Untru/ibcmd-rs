//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Column`, guid=5aee69df-0513-4c6c-9815-103102471712.
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
/// `indexing`  guid=a93a67a0-1955-11d6-a3c7-0050bae0a776
pub const F_INDEXING: FieldId = FieldId(5);
/// `references`  guid=0f1aacc0-7817-41c0-8f6f-609b13fdb337
pub const F_REFERENCES: FieldId = FieldId(6);

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
fn list(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::List, PropertyValue::List(Vec::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        e(F_INDEXING, "indexing", "DontIndex"),  // literals: {DontIndex|Index|IndexWithAdditionalOrder}
        list(F_REFERENCES, "references"),  // TODO(draft): ref-list -> DocumentAttribute
    ]
}

/// Канонический [`EntitySpec`] вида `Column` (ЧЕРНОВИК).
pub fn column() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Column",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Column` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "5aee69df-0513-4c6c-9815-103102471712";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_INDEXING, "a93a67a0-1955-11d6-a3c7-0050bae0a776"),
    (F_REFERENCES, "0f1aacc0-7817-41c0-8f6f-609b13fdb337"),
];
