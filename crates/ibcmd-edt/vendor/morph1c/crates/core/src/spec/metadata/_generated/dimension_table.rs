//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `DimensionTable` (ТаблицаИзмерения), guid=631411f3-8c65-4066-a78f-57a72e015c1c.
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
/// `nameInDataSource`  guid=8d13725e-bf6f-4cee-9add-cc60ed338079
pub const F_NAME_IN_DATA_SOURCE: FieldId = FieldId(5);
/// `presentationField`  guid=489424a7-3c35-4603-b7ab-db2f0d94b108
pub const F_PRESENTATION_FIELD: FieldId = FieldId(6);
/// `hierarchyNameInDataSource`  guid=d3a4b6ee-f394-4fbd-87d9-2b7635d83edf
pub const F_HIERARCHY_NAME_IN_DATA_SOURCE: FieldId = FieldId(7);
/// `levelNumber`  guid=147b7839-a578-4721-8679-9e3a66518461
pub const F_LEVEL_NUMBER: FieldId = FieldId(8);
/// `hierarchical`  guid=c5e68bcb-2df3-45fd-8b55-65dbe3f6b054
pub const F_HIERARCHICAL: FieldId = FieldId(9);
/// `unfilledParentValue`  guid=f2b51318-20cc-4aa2-8193-0aad9c6efe27
pub const F_UNFILLED_PARENT_VALUE: FieldId = FieldId(10);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(11);
/// `quickChoice`  guid=1b3ecd33-43d7-45e4-a012-776d95785465
pub const F_QUICK_CHOICE: FieldId = FieldId(12);
/// `defaultObjectForm`  guid=b69601f3-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(13);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(14);
/// `defaultChoiceForm`  guid=b69601f6-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(15);
/// `objectPresentation`  guid=d79e10d1-ebb6-46c3-a053-55c3b7248b7e
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(16);
/// `extendedObjectPresentation`  guid=3613fe2b-442b-4723-a0dd-4bad030dcfa5
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(17);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(18);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(19);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(20);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(21);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(22);

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
        s(F_NAME_IN_DATA_SOURCE, "nameInDataSource"),
        s(F_PRESENTATION_FIELD, "presentationField"),  // TODO(draft): ref -> Field (form-ref Str vs Ref?)
        s(F_HIERARCHY_NAME_IN_DATA_SOURCE, "hierarchyNameInDataSource"),
        s(F_LEVEL_NUMBER, "levelNumber"),  // TODO(draft): тип BigDecimal — уточнить кодировку
        b(F_HIERARCHICAL, "hierarchical"),
        FieldSpec::required(F_UNFILLED_PARENT_VALUE, "unfilledParentValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        b(F_QUICK_CHOICE, "quickChoice"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
    ]
}

/// Канонический [`EntitySpec`] вида `DimensionTable` (ЧЕРНОВИК).
pub fn dimension_table() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DimensionTable",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `DimensionTable` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "631411f3-8c65-4066-a78f-57a72e015c1c";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_NAME_IN_DATA_SOURCE, "8d13725e-bf6f-4cee-9add-cc60ed338079"),
    (F_PRESENTATION_FIELD, "489424a7-3c35-4603-b7ab-db2f0d94b108"),
    (F_HIERARCHY_NAME_IN_DATA_SOURCE, "d3a4b6ee-f394-4fbd-87d9-2b7635d83edf"),
    (F_LEVEL_NUMBER, "147b7839-a578-4721-8679-9e3a66518461"),
    (F_HIERARCHICAL, "c5e68bcb-2df3-45fd-8b55-65dbe3f6b054"),
    (F_UNFILLED_PARENT_VALUE, "f2b51318-20cc-4aa2-8193-0aad9c6efe27"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_QUICK_CHOICE, "1b3ecd33-43d7-45e4-a012-776d95785465"),
    (F_DEFAULT_OBJECT_FORM, "b69601f3-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_CHOICE_FORM, "b69601f6-4cf6-11d4-9415-008048da11f9"),
    (F_OBJECT_PRESENTATION, "d79e10d1-ebb6-46c3-a053-55c3b7248b7e"),
    (F_EXTENDED_OBJECT_PRESENTATION, "3613fe2b-442b-4723-a0dd-4bad030dcfa5"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
];
