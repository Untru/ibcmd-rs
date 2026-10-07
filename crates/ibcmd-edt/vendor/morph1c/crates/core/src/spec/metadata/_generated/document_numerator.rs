//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `DocumentNumerator` (НумераторДокумента), guid=36a8e346-9aaa-4af9-bdbd-83be3c177977.
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
/// `numberType`  guid=60643a07-120a-4a63-9dba-67369c0f0145
pub const F_NUMBER_TYPE: FieldId = FieldId(5);
/// `numberLength`  guid=490eb6bd-24c4-4943-82aa-d0c4b666861b
pub const F_NUMBER_LENGTH: FieldId = FieldId(6);
/// `numberAllowedLength`  guid=c6c9689d-2978-42e8-863f-0280c6a85b56
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(7);
/// `numberPeriodicity`  guid=67ebd6e9-d0ad-4b29-8d2b-3bfec03ff2cb
pub const F_NUMBER_PERIODICITY: FieldId = FieldId(8);
/// `checkUnique`  guid=8336a121-59b3-427d-8e49-1c813662a550
pub const F_CHECK_UNIQUE: FieldId = FieldId(9);

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
        e(F_NUMBER_TYPE, "numberType", "Number"),  // literals: {Number|String}
        i(F_NUMBER_LENGTH, "numberLength", 0),
        e(F_NUMBER_ALLOWED_LENGTH, "numberAllowedLength", "Fixed"),  // literals: {Fixed|Variable}
        e(F_NUMBER_PERIODICITY, "numberPeriodicity", "Nonperiodical"),  // literals: {Nonperiodical|Year|Quarter|Month|Day}
        b(F_CHECK_UNIQUE, "checkUnique"),
    ]
}

/// Канонический [`EntitySpec`] вида `DocumentNumerator` (ЧЕРНОВИК).
pub fn document_numerator() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DocumentNumerator",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `DocumentNumerator` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "36a8e346-9aaa-4af9-bdbd-83be3c177977";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_NUMBER_TYPE, "60643a07-120a-4a63-9dba-67369c0f0145"),
    (F_NUMBER_LENGTH, "490eb6bd-24c4-4943-82aa-d0c4b666861b"),
    (F_NUMBER_ALLOWED_LENGTH, "c6c9689d-2978-42e8-863f-0280c6a85b56"),
    (F_NUMBER_PERIODICITY, "67ebd6e9-d0ad-4b29-8d2b-3bfec03ff2cb"),
    (F_CHECK_UNIQUE, "8336a121-59b3-427d-8e49-1c813662a550"),
];
