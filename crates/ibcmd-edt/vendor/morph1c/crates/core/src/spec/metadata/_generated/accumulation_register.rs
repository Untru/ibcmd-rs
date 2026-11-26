//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `AccumulationRegister` (РегистрНакопления), guid=b64d9a40-1642-11d6-a3c7-0050bae0a776.
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
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(5);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(6);
/// `auxiliaryListForm`  guid=367b60f5-2ad5-4399-a6b0-3dd8e4ac3650
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(7);
/// `registerType`  guid=bd533460-4001-11d6-a3c7-0050bae0a776
pub const F_REGISTER_TYPE: FieldId = FieldId(8);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(9);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(10);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(11);
/// `fullTextSearch`  guid=d4232326-022b-421e-b6d3-88e418f74327
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(12);
/// `enableTotalsSplitting`  guid=9c703426-16bd-4e3f-8dd5-0591a614e6de
pub const F_ENABLE_TOTALS_SPLITTING: FieldId = FieldId(13);
/// `aggregates`  guid=04e7c33a-87b9-429a-a8d7-49d2e7d7c298
pub const F_AGGREGATES: FieldId = FieldId(14);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(15);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(16);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(17);

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
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> AccumulationRegisterForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),  // TODO(draft): ref -> AccumulationRegisterForm (form-ref Str vs Ref?)
        e(F_REGISTER_TYPE, "registerType", "Balance"),  // literals: {Balance|Turnovers}
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),  // literals: {DontUse|Use}
        b(F_ENABLE_TOTALS_SPLITTING, "enableTotalsSplitting"),
        s(F_AGGREGATES, "aggregates"),  // TODO(draft): contained AbstractAggregates — уточнить vk
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "AccumulationRegister.StandardAttribute" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `AccumulationRegister` (ЧЕРНОВИК).
pub fn accumulation_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `AccumulationRegister` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "b64d9a40-1642-11d6-a3c7-0050bae0a776";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_AUXILIARY_LIST_FORM, "367b60f5-2ad5-4399-a6b0-3dd8e4ac3650"),
    (F_REGISTER_TYPE, "bd533460-4001-11d6-a3c7-0050bae0a776"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
    (F_FULL_TEXT_SEARCH, "d4232326-022b-421e-b6d3-88e418f74327"),
    (F_ENABLE_TOTALS_SPLITTING, "9c703426-16bd-4e3f-8dd5-0591a614e6de"),
    (F_AGGREGATES, "04e7c33a-87b9-429a-a8d7-49d2e7d7c298"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
];
