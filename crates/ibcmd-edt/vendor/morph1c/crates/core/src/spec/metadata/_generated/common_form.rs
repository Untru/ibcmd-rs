//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `CommonForm` (ОбщаяФорма), guid=07ee8426-87f1-11d5-b99c-0050bae0a95d.
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
/// `formType`  guid=e3331ed0-3854-478d-b6b5-4f14acdd6edb
pub const F_FORM_TYPE: FieldId = FieldId(5);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(6);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(7);
/// `usePurposes`  guid=da648ef9-2f12-418f-8e2f-8956bc10a66f
pub const F_USE_PURPOSES: FieldId = FieldId(8);
/// `useInInterfaceCompatibilityMode` (since 8.5.1)  guid=c137bf33-c377-41a9-a131-f0c6ef763891
pub const F_USE_IN_INTERFACE_COMPATIBILITY_MODE: FieldId = FieldId(9);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(10);
/// `extendedPresentation`  guid=c9ba86bf-a1e7-440f-9450-adb61f886f88
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(11);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(12);

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
        e(F_FORM_TYPE, "formType", "Managed"),  // literals: {Ordinary|Managed}
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        e(F_USE_PURPOSES, "usePurposes", "PersonalComputer"),  // literals: {PersonalComputer|MobileDevice}
        e(F_USE_IN_INTERFACE_COMPATIBILITY_MODE, "useInInterfaceCompatibilityMode", "Any")
            .gated(Since(FormatVersion::new(2, 21))),  // literals: {Any|Taxi|Version85}
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        loc_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonForm` (ЧЕРНОВИК).
pub fn common_form() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonForm",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `CommonForm` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "07ee8426-87f1-11d5-b99c-0050bae0a95d";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_FORM_TYPE, "e3331ed0-3854-478d-b6b5-4f14acdd6edb"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_USE_PURPOSES, "da648ef9-2f12-418f-8e2f-8956bc10a66f"),
    (F_USE_IN_INTERFACE_COMPATIBILITY_MODE, "c137bf33-c377-41a9-a131-f0c6ef763891"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_EXTENDED_PRESENTATION, "c9ba86bf-a1e7-440f-9450-adb61f886f88"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
];
