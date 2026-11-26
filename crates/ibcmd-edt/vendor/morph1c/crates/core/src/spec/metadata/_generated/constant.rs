//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Constant` (Константа), guid=0195e80c-b157-11d4-9435-004095e12fc7.
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
/// `dataHistory`  guid=9288A8ED-B259-46D0-A8E3-70D87956FF2D
pub const F_DATA_HISTORY: FieldId = FieldId(5);
/// `type`  guid=b1053250-abe6-11d4-9434-004095e12fc7
pub const F_TYPE: FieldId = FieldId(6);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(7);
/// `defaultForm`  guid=3e8c558e-64b4-405d-8ad3-7faf63b866ba
pub const F_DEFAULT_FORM: FieldId = FieldId(8);
/// `extendedPresentation`  guid=c9ba86bf-a1e7-440f-9450-adb61f886f88
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(9);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(10);
/// `passwordMode`  guid=90ae4b5d-e0fd-49ef-a008-d67c1e75038c
pub const F_PASSWORD_MODE: FieldId = FieldId(11);
/// `format`  guid=580c29e2-8af4-4258-882a-7cf8073e61c8
pub const F_FORMAT: FieldId = FieldId(12);
/// `editFormat`  guid=88149a78-9448-4767-867b-0e650d165d2e
pub const F_EDIT_FORMAT: FieldId = FieldId(13);
/// `toolTip`  guid=4690ff70-e3fa-4914-9127-6a9acc5fc949
pub const F_TOOL_TIP: FieldId = FieldId(14);
/// `markNegatives`  guid=b02800e9-a8d1-42ab-9a12-f673e92be968
pub const F_MARK_NEGATIVES: FieldId = FieldId(15);
/// `mask`  guid=f49e4ced-4033-4e6c-8755-9fbaaccd6078
pub const F_MASK: FieldId = FieldId(16);
/// `multiLine`  guid=2bbba66b-fabf-4863-8ba3-54b3c64c896e
pub const F_MULTI_LINE: FieldId = FieldId(17);
/// `extendedEdit`  guid=4de03908-56f4-4396-a61e-17253afca9ac
pub const F_EXTENDED_EDIT: FieldId = FieldId(18);
/// `minValue`  guid=c65a541f-0b91-4f33-bc88-fbaaa57f9992
pub const F_MIN_VALUE: FieldId = FieldId(19);
/// `maxValue`  guid=3eaf5a8b-06d6-47b0-ac7d-a9698247f499
pub const F_MAX_VALUE: FieldId = FieldId(20);
/// `fillChecking`  guid=2723eb98-b4c1-498a-a6f3-70444757902f
pub const F_FILL_CHECKING: FieldId = FieldId(21);
/// `choiceFoldersAndItems`  guid=c421d5c6-dcd9-4e28-9a35-0b2f4d66bef0
pub const F_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(22);
/// `choiceParameterLinks`  guid=e3da683b-c54a-457a-a243-b9b4f9bf76dd
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(23);
/// `choiceParameters`  guid=fcf503b8-1c06-454a-970c-06413e64aee5
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(24);
/// `quickChoice`  guid=6e3a1131-37a3-4da5-8895-572d9d0c9db6
pub const F_QUICK_CHOICE: FieldId = FieldId(25);
/// `choiceForm`  guid=6c4f7074-e7d4-48eb-b31b-132873666262
pub const F_CHOICE_FORM: FieldId = FieldId(26);
/// `linkByType`  guid=1183c14f-f814-49c6-9233-a3c26b3f64cf
pub const F_LINK_BY_TYPE: FieldId = FieldId(27);
/// `choiceHistoryOnInput`  guid=7ba608f2-e654-42a3-8885-334fe88ca910
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(28);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(29);
/// `updateDataHistoryImmediatelyAfterWrite` (since 8.3.15)  guid=0912c390-3c7e-4de9-abd0-f9c9cb19f0e6
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE: FieldId = FieldId(30);
/// `executeAfterWriteDataHistoryVersionProcessing` (since 8.3.15)  guid=c4986a1c-1315-4f86-9587-f1d116329b58
pub const F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING: FieldId = FieldId(31);

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
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),  // literals: {DontUse|Use}
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),  // TODO(draft): Type-кодек
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        s(F_DEFAULT_FORM, "defaultForm"),  // TODO(draft): ref -> CommonForm (form-ref Str vs Ref?)
        loc_field(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        b(F_PASSWORD_MODE, "passwordMode"),
        loc_field(F_FORMAT, "format"),
        loc_field(F_EDIT_FORMAT, "editFormat"),
        loc_field(F_TOOL_TIP, "toolTip"),
        b(F_MARK_NEGATIVES, "markNegatives"),
        s(F_MASK, "mask"),
        b(F_MULTI_LINE, "multiLine"),
        b(F_EXTENDED_EDIT, "extendedEdit"),
        FieldSpec::required(F_MIN_VALUE, "minValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        FieldSpec::required(F_MAX_VALUE, "maxValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        e(F_FILL_CHECKING, "fillChecking", "DontCheck"),  // literals: {DontCheck|ShowError}
        e(F_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems", "Items"),  // literals: {Items|Folders|FoldersAndItems}
        list(F_CHOICE_PARAMETER_LINKS, "choiceParameterLinks"),  // TODO(draft): список contained ChoiceParameterLink
        list(F_CHOICE_PARAMETERS, "choiceParameters"),  // TODO(draft): список contained ChoiceParameter
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),  // literals: {Auto|Use|DontUse}
        s(F_CHOICE_FORM, "choiceForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_LINK_BY_TYPE, "linkByType"),  // TODO(draft): contained TypeLink — уточнить vk
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),  // literals: {Auto|DontUse}
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        b(F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "executeAfterWriteDataHistoryVersionProcessing"),
    ]
}

/// Канонический [`EntitySpec`] вида `Constant` (ЧЕРНОВИК).
pub fn constant() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Constant",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `Constant` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "0195e80c-b157-11d4-9435-004095e12fc7";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_DATA_HISTORY, "9288A8ED-B259-46D0-A8E3-70D87956FF2D"),
    (F_TYPE, "b1053250-abe6-11d4-9434-004095e12fc7"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_DEFAULT_FORM, "3e8c558e-64b4-405d-8ad3-7faf63b866ba"),
    (F_EXTENDED_PRESENTATION, "c9ba86bf-a1e7-440f-9450-adb61f886f88"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_PASSWORD_MODE, "90ae4b5d-e0fd-49ef-a008-d67c1e75038c"),
    (F_FORMAT, "580c29e2-8af4-4258-882a-7cf8073e61c8"),
    (F_EDIT_FORMAT, "88149a78-9448-4767-867b-0e650d165d2e"),
    (F_TOOL_TIP, "4690ff70-e3fa-4914-9127-6a9acc5fc949"),
    (F_MARK_NEGATIVES, "b02800e9-a8d1-42ab-9a12-f673e92be968"),
    (F_MASK, "f49e4ced-4033-4e6c-8755-9fbaaccd6078"),
    (F_MULTI_LINE, "2bbba66b-fabf-4863-8ba3-54b3c64c896e"),
    (F_EXTENDED_EDIT, "4de03908-56f4-4396-a61e-17253afca9ac"),
    (F_MIN_VALUE, "c65a541f-0b91-4f33-bc88-fbaaa57f9992"),
    (F_MAX_VALUE, "3eaf5a8b-06d6-47b0-ac7d-a9698247f499"),
    (F_FILL_CHECKING, "2723eb98-b4c1-498a-a6f3-70444757902f"),
    (F_CHOICE_FOLDERS_AND_ITEMS, "c421d5c6-dcd9-4e28-9a35-0b2f4d66bef0"),
    (F_CHOICE_PARAMETER_LINKS, "e3da683b-c54a-457a-a243-b9b4f9bf76dd"),
    (F_CHOICE_PARAMETERS, "fcf503b8-1c06-454a-970c-06413e64aee5"),
    (F_QUICK_CHOICE, "6e3a1131-37a3-4da5-8895-572d9d0c9db6"),
    (F_CHOICE_FORM, "6c4f7074-e7d4-48eb-b31b-132873666262"),
    (F_LINK_BY_TYPE, "1183c14f-f814-49c6-9233-a3c26b3f64cf"),
    (F_CHOICE_HISTORY_ON_INPUT, "7ba608f2-e654-42a3-8885-334fe88ca910"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
    (F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "0912c390-3c7e-4de9-abd0-f9c9cb19f0e6"),
    (F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "c4986a1c-1315-4f86-9587-f1d116329b58"),
];
