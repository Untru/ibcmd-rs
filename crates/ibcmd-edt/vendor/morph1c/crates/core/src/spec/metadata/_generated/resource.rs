//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Resource`, guid=cead2290-2080-4077-a57b-73051d618683.
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
/// `type`  guid=b1053250-abe6-11d4-9434-004095e12fc7
pub const F_TYPE: FieldId = FieldId(5);
/// `passwordMode`  guid=90ae4b5d-e0fd-49ef-a008-d67c1e75038c
pub const F_PASSWORD_MODE: FieldId = FieldId(6);
/// `format`  guid=580c29e2-8af4-4258-882a-7cf8073e61c8
pub const F_FORMAT: FieldId = FieldId(7);
/// `editFormat`  guid=88149a78-9448-4767-867b-0e650d165d2e
pub const F_EDIT_FORMAT: FieldId = FieldId(8);
/// `toolTip`  guid=4690ff70-e3fa-4914-9127-6a9acc5fc949
pub const F_TOOL_TIP: FieldId = FieldId(9);
/// `markNegatives`  guid=b02800e9-a8d1-42ab-9a12-f673e92be968
pub const F_MARK_NEGATIVES: FieldId = FieldId(10);
/// `mask`  guid=f49e4ced-4033-4e6c-8755-9fbaaccd6078
pub const F_MASK: FieldId = FieldId(11);
/// `multiLine`  guid=2bbba66b-fabf-4863-8ba3-54b3c64c896e
pub const F_MULTI_LINE: FieldId = FieldId(12);
/// `choiceParameterLinks`  guid=e3da683b-c54a-457a-a243-b9b4f9bf76dd
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(13);
/// `choiceParameters`  guid=fcf503b8-1c06-454a-970c-06413e64aee5
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(14);
/// `quickChoice`  guid=6e3a1131-37a3-4da5-8895-572d9d0c9db6
pub const F_QUICK_CHOICE: FieldId = FieldId(15);
/// `choiceForm`  guid=6c4f7074-e7d4-48eb-b31b-132873666262
pub const F_CHOICE_FORM: FieldId = FieldId(16);
/// `extendedEdit`  guid=4de03908-56f4-4396-a61e-17253afca9ac
pub const F_EXTENDED_EDIT: FieldId = FieldId(17);
/// `nameInDataSource`  guid=8e4b201d-2411-4536-b40d-62d7d4028cde
pub const F_NAME_IN_DATA_SOURCE: FieldId = FieldId(18);

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
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),  // TODO(draft): Type-кодек
        b(F_PASSWORD_MODE, "passwordMode"),
        loc_field(F_FORMAT, "format"),
        loc_field(F_EDIT_FORMAT, "editFormat"),
        loc_field(F_TOOL_TIP, "toolTip"),
        b(F_MARK_NEGATIVES, "markNegatives"),
        s(F_MASK, "mask"),
        b(F_MULTI_LINE, "multiLine"),
        list(F_CHOICE_PARAMETER_LINKS, "choiceParameterLinks"),  // TODO(draft): список contained ChoiceParameterLink
        list(F_CHOICE_PARAMETERS, "choiceParameters"),  // TODO(draft): список contained ChoiceParameter
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),  // literals: {Auto|Use|DontUse}
        s(F_CHOICE_FORM, "choiceForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        b(F_EXTENDED_EDIT, "extendedEdit"),
        s(F_NAME_IN_DATA_SOURCE, "nameInDataSource"),
    ]
}

/// Канонический [`EntitySpec`] вида `Resource` (ЧЕРНОВИК).
pub fn resource() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Resource",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Resource` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "cead2290-2080-4077-a57b-73051d618683";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_TYPE, "b1053250-abe6-11d4-9434-004095e12fc7"),
    (F_PASSWORD_MODE, "90ae4b5d-e0fd-49ef-a008-d67c1e75038c"),
    (F_FORMAT, "580c29e2-8af4-4258-882a-7cf8073e61c8"),
    (F_EDIT_FORMAT, "88149a78-9448-4767-867b-0e650d165d2e"),
    (F_TOOL_TIP, "4690ff70-e3fa-4914-9127-6a9acc5fc949"),
    (F_MARK_NEGATIVES, "b02800e9-a8d1-42ab-9a12-f673e92be968"),
    (F_MASK, "f49e4ced-4033-4e6c-8755-9fbaaccd6078"),
    (F_MULTI_LINE, "2bbba66b-fabf-4863-8ba3-54b3c64c896e"),
    (F_CHOICE_PARAMETER_LINKS, "e3da683b-c54a-457a-a243-b9b4f9bf76dd"),
    (F_CHOICE_PARAMETERS, "fcf503b8-1c06-454a-970c-06413e64aee5"),
    (F_QUICK_CHOICE, "6e3a1131-37a3-4da5-8895-572d9d0c9db6"),
    (F_CHOICE_FORM, "6c4f7074-e7d4-48eb-b31b-132873666262"),
    (F_EXTENDED_EDIT, "4de03908-56f4-4396-a61e-17253afca9ac"),
    (F_NAME_IN_DATA_SOURCE, "8e4b201d-2411-4536-b40d-62d7d4028cde"),
];
