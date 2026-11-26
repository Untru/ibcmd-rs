//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Report` (Отчет), guid=631b75a0-29e2-11d6-a3c7-0050bae0a776.
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
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(5);
/// `defaultForm`  guid=3e8c558e-64b4-405d-8ad3-7faf63b866ba
pub const F_DEFAULT_FORM: FieldId = FieldId(6);
/// `auxiliaryForm`  guid=2b017861-cb7f-449d-812a-3bd6a81b0177
pub const F_AUXILIARY_FORM: FieldId = FieldId(7);
/// `mainDataCompositionSchema`  guid=43678597-7217-4072-81e0-568f44ff6f3d
pub const F_MAIN_DATA_COMPOSITION_SCHEMA: FieldId = FieldId(8);
/// `defaultSettingsForm`  guid=60b51f1c-0ba8-4004-bfe3-430bd20b68a8
pub const F_DEFAULT_SETTINGS_FORM: FieldId = FieldId(9);
/// `auxiliarySettingsForm`  guid=4f4cf10a-99fa-4139-9a31-731e409d3a9e
pub const F_AUXILIARY_SETTINGS_FORM: FieldId = FieldId(10);
/// `defaultVariantForm`  guid=1021f579-255c-47c6-a880-01bfd163449e
pub const F_DEFAULT_VARIANT_FORM: FieldId = FieldId(11);
/// `auxiliaryVariantForm` (since 8.5.1)  guid=1831ac47-f9ec-4720-919a-480f5a5c874d
pub const F_AUXILIARY_VARIANT_FORM: FieldId = FieldId(12);
/// `variantsStorage`  guid=70b23efb-6bc8-4d80-b36f-466417e36014
pub const F_VARIANTS_STORAGE: FieldId = FieldId(13);
/// `settingsStorage`  guid=df9755ae-d8fb-48b6-80fa-ec97cb875921
pub const F_SETTINGS_STORAGE: FieldId = FieldId(14);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(15);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(16);
/// `extendedPresentation`  guid=c9ba86bf-a1e7-440f-9450-adb61f886f88
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(17);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(18);

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
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        s(F_DEFAULT_FORM, "defaultForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_FORM, "auxiliaryForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_MAIN_DATA_COMPOSITION_SCHEMA, "mainDataCompositionSchema"),  // TODO(draft): ref -> BasicTemplate (form-ref Str vs Ref?)
        s(F_DEFAULT_SETTINGS_FORM, "defaultSettingsForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_SETTINGS_FORM, "auxiliarySettingsForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_DEFAULT_VARIANT_FORM, "defaultVariantForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_VARIANT_FORM, "auxiliaryVariantForm")
            .gated(Since(FormatVersion::new(2, 21))),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_VARIANTS_STORAGE, "variantsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        s(F_SETTINGS_STORAGE, "settingsStorage"),  // TODO(draft): ref -> SettingsStorage (form-ref Str vs Ref?)
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        loc_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

/// Канонический [`EntitySpec`] вида `Report` (ЧЕРНОВИК).
pub fn report() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Report",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Report` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "631b75a0-29e2-11d6-a3c7-0050bae0a776";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_DEFAULT_FORM, "3e8c558e-64b4-405d-8ad3-7faf63b866ba"),
    (F_AUXILIARY_FORM, "2b017861-cb7f-449d-812a-3bd6a81b0177"),
    (F_MAIN_DATA_COMPOSITION_SCHEMA, "43678597-7217-4072-81e0-568f44ff6f3d"),
    (F_DEFAULT_SETTINGS_FORM, "60b51f1c-0ba8-4004-bfe3-430bd20b68a8"),
    (F_AUXILIARY_SETTINGS_FORM, "4f4cf10a-99fa-4139-9a31-731e409d3a9e"),
    (F_DEFAULT_VARIANT_FORM, "1021f579-255c-47c6-a880-01bfd163449e"),
    (F_AUXILIARY_VARIANT_FORM, "1831ac47-f9ec-4720-919a-480f5a5c874d"),
    (F_VARIANTS_STORAGE, "70b23efb-6bc8-4d80-b36f-466417e36014"),
    (F_SETTINGS_STORAGE, "df9755ae-d8fb-48b6-80fa-ec97cb875921"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_EXTENDED_PRESENTATION, "c9ba86bf-a1e7-440f-9450-adb61f886f88"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
];
