//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `SettingsStorage` (ХранилищеНастроек), guid=46b4cd97-fd13-4eaa-aba2-3bddd7699218.
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
/// `defaultSaveForm`  guid=f5094eee-42ee-4e29-8536-a6f06f20b66a
pub const F_DEFAULT_SAVE_FORM: FieldId = FieldId(5);
/// `defaultLoadForm`  guid=48fe5600-73bd-42d1-9c4f-de6647049f4a
pub const F_DEFAULT_LOAD_FORM: FieldId = FieldId(6);
/// `auxiliarySaveForm`  guid=7cb99c54-28fe-422e-9c07-1262b43e3bf4
pub const F_AUXILIARY_SAVE_FORM: FieldId = FieldId(7);
/// `auxiliaryLoadForm`  guid=8993266c-719b-4ad0-ae5f-b86bd62a09c6
pub const F_AUXILIARY_LOAD_FORM: FieldId = FieldId(8);

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

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        s(F_DEFAULT_SAVE_FORM, "defaultSaveForm"),  // TODO(draft): ref -> SettingsStorageForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LOAD_FORM, "defaultLoadForm"),  // TODO(draft): ref -> SettingsStorageForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_SAVE_FORM, "auxiliarySaveForm"),  // TODO(draft): ref -> SettingsStorageForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LOAD_FORM, "auxiliaryLoadForm"),  // TODO(draft): ref -> SettingsStorageForm (form-ref Str vs Ref?)
    ]
}

/// Канонический [`EntitySpec`] вида `SettingsStorage` (ЧЕРНОВИК).
pub fn settings_storage() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "SettingsStorage",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `SettingsStorage` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "46b4cd97-fd13-4eaa-aba2-3bddd7699218";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_DEFAULT_SAVE_FORM, "f5094eee-42ee-4e29-8536-a6f06f20b66a"),
    (F_DEFAULT_LOAD_FORM, "48fe5600-73bd-42d1-9c4f-de6647049f4a"),
    (F_AUXILIARY_SAVE_FORM, "7cb99c54-28fe-422e-9c07-1262b43e3bf4"),
    (F_AUXILIARY_LOAD_FORM, "8993266c-719b-4ad0-ae5f-b86bd62a09c6"),
];
