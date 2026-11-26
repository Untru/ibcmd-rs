//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `CommandGroup` (ГруппаКоманд), guid=1c57eabe-7349-44b3-b1de-ebfeab67b47d.
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
/// `representation`  guid=a5085b15-9218-4da8-b2e1-3c5d357b9f82
pub const F_REPRESENTATION: FieldId = FieldId(5);
/// `toolTip`  guid=4c3288f7-2704-4f31-9bf3-4b8e8e080682
pub const F_TOOL_TIP: FieldId = FieldId(6);
/// `picture`  guid=58e51e5d-aee0-41ea-89c0-b0962a74d237
pub const F_PICTURE: FieldId = FieldId(7);
/// `category`  guid=a6284a90-aa5c-4d44-80e6-b7d30b01ae78
pub const F_CATEGORY: FieldId = FieldId(8);

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
        e(F_REPRESENTATION, "representation", "Text"),  // literals: {Text|Picture|PictureAndText|Auto}
        loc_field(F_TOOL_TIP, "toolTip"),
        s(F_PICTURE, "picture"),  // TODO(draft): PictureRef-кодек
        e(F_CATEGORY, "category", "NavigationPanel"),  // literals: {NavigationPanel|ActionsPanel|FormNavigationPanel|FormCommandBar}
    ]
}

/// Канонический [`EntitySpec`] вида `CommandGroup` (ЧЕРНОВИК).
pub fn command_group() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommandGroup",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `CommandGroup` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "1c57eabe-7349-44b3-b1de-ebfeab67b47d";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_REPRESENTATION, "a5085b15-9218-4da8-b2e1-3c5d357b9f82"),
    (F_TOOL_TIP, "4c3288f7-2704-4f31-9bf3-4b8e8e080682"),
    (F_PICTURE, "58e51e5d-aee0-41ea-89c0-b0962a74d237"),
    (F_CATEGORY, "a6284a90-aa5c-4d44-80e6-b7d30b01ae78"),
];
