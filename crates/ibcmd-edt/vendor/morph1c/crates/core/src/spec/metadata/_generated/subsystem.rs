//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Subsystem` (Подсистема), guid=37f2fa9a-b276-11d4-9435-004095e12fc7.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::version::{FormatVersion, Since};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(5);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(6);
/// `includeInCommandInterface`  guid=5e3f7fa0-5dd7-4f18-aca7-7132bab274ed
pub const F_INCLUDE_IN_COMMAND_INTERFACE: FieldId = FieldId(7);
/// `useOneCommand` (since 8.3.21)  guid=0e30a46e-01c4-4fb7-876d-41c1ddf36b66
pub const F_USE_ONE_COMMAND: FieldId = FieldId(8);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(9);
/// `picture`  guid=58e51e5d-aee0-41ea-89c0-b0962a74d237
pub const F_PICTURE: FieldId = FieldId(10);
/// `content`  guid=c6690627-40cf-4741-8719-4e7feb832b84
pub const F_CONTENT: FieldId = FieldId(11);

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
fn list(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::List, PropertyValue::List(Vec::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        b(F_INCLUDE_IN_COMMAND_INTERFACE, "includeInCommandInterface"),
        b(F_USE_ONE_COMMAND, "useOneCommand")
            .gated(Since(FormatVersion::new(2, 14))),
        loc_field(F_EXPLANATION, "explanation"),
        s(F_PICTURE, "picture"),  // TODO(draft): PictureRef-кодек
        list(F_CONTENT, "content"),  // TODO(draft): ref-list -> MdObject
    ]
}

/// Канонический [`EntitySpec`] вида `Subsystem` (ЧЕРНОВИК).
pub fn subsystem() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Subsystem",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Subsystem` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "37f2fa9a-b276-11d4-9435-004095e12fc7";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_INCLUDE_IN_COMMAND_INTERFACE, "5e3f7fa0-5dd7-4f18-aca7-7132bab274ed"),
    (F_USE_ONE_COMMAND, "0e30a46e-01c4-4fb7-876d-41c1ddf36b66"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_PICTURE, "58e51e5d-aee0-41ea-89c0-b0962a74d237"),
    (F_CONTENT, "c6690627-40cf-4741-8719-4e7feb832b84"),
];
