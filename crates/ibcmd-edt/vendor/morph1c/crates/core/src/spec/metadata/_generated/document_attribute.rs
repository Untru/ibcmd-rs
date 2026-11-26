//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `DocumentAttribute` (Реквизит), guid=45e46cbc-3e24-4165-8b7b-cc98a6f80211.
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
/// `extendedEdit`  guid=4de03908-56f4-4396-a61e-17253afca9ac
pub const F_EXTENDED_EDIT: FieldId = FieldId(13);
/// `minValue`  guid=c65a541f-0b91-4f33-bc88-fbaaa57f9992
pub const F_MIN_VALUE: FieldId = FieldId(14);
/// `maxValue`  guid=3eaf5a8b-06d6-47b0-ac7d-a9698247f499
pub const F_MAX_VALUE: FieldId = FieldId(15);
/// `fillChecking`  guid=2723eb98-b4c1-498a-a6f3-70444757902f
pub const F_FILL_CHECKING: FieldId = FieldId(16);
/// `choiceFoldersAndItems`  guid=c421d5c6-dcd9-4e28-9a35-0b2f4d66bef0
pub const F_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(17);
/// `choiceParameterLinks`  guid=e3da683b-c54a-457a-a243-b9b4f9bf76dd
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(18);
/// `choiceParameters`  guid=fcf503b8-1c06-454a-970c-06413e64aee5
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(19);
/// `quickChoice`  guid=6e3a1131-37a3-4da5-8895-572d9d0c9db6
pub const F_QUICK_CHOICE: FieldId = FieldId(20);
/// `createOnInput`  guid=33c74a4d-561f-4bc0-9eaa-8d21c893c0a9
pub const F_CREATE_ON_INPUT: FieldId = FieldId(21);
/// `choiceForm`  guid=6c4f7074-e7d4-48eb-b31b-132873666262
pub const F_CHOICE_FORM: FieldId = FieldId(22);
/// `linkByType`  guid=1183c14f-f814-49c6-9233-a3c26b3f64cf
pub const F_LINK_BY_TYPE: FieldId = FieldId(23);
/// `fillFromFillingValue`  guid=2c8143d5-4248-4c43-8bfb-307c0be2e415
pub const F_FILL_FROM_FILLING_VALUE: FieldId = FieldId(24);
/// `fillValue`  guid=e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208
pub const F_FILL_VALUE: FieldId = FieldId(25);
/// `indexing`  guid=a93a67a0-1955-11d6-a3c7-0050bae0a776
pub const F_INDEXING: FieldId = FieldId(26);
/// `fullTextSearch`  guid=d4232326-022b-421e-b6d3-88e418f74327
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(27);
/// `binaryDataStorageLocationUse` (since 8.3.26)  guid=43ac5d7b-7d0e-41fc-b62b-2f9e1294cce0
pub const F_BINARY_DATA_STORAGE_LOCATION_USE: FieldId = FieldId(28);
/// `binaryDataStorageLocationUseField` (since 8.3.26)  guid=5a78d87a-1b1d-4145-ad00-62891d1eadb1
pub const F_BINARY_DATA_STORAGE_LOCATION_USE_FIELD: FieldId = FieldId(29);
/// `dataHistory`  guid=9288A8ED-B259-46D0-A8E3-70D87956FF2D
pub const F_DATA_HISTORY: FieldId = FieldId(30);
/// `choiceHistoryOnInput`  guid=7ba608f2-e654-42a3-8885-334fe88ca910
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(31);

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
        b(F_EXTENDED_EDIT, "extendedEdit"),
        FieldSpec::required(F_MIN_VALUE, "minValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        FieldSpec::required(F_MAX_VALUE, "maxValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        e(F_FILL_CHECKING, "fillChecking", "DontCheck"),  // literals: {DontCheck|ShowError}
        e(F_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems", "Items"),  // literals: {Items|Folders|FoldersAndItems}
        list(F_CHOICE_PARAMETER_LINKS, "choiceParameterLinks"),  // TODO(draft): список contained ChoiceParameterLink
        list(F_CHOICE_PARAMETERS, "choiceParameters"),  // TODO(draft): список contained ChoiceParameter
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),  // literals: {Auto|Use|DontUse}
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),  // literals: {Auto|DontUse|Use}
        s(F_CHOICE_FORM, "choiceForm"),  // TODO(draft): ref -> BasicForm (form-ref Str vs Ref?)
        s(F_LINK_BY_TYPE, "linkByType"),  // TODO(draft): contained TypeLink — уточнить vk
        b(F_FILL_FROM_FILLING_VALUE, "fillFromFillingValue"),
        FieldSpec::required(F_FILL_VALUE, "fillValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        e(F_INDEXING, "indexing", "DontIndex"),  // literals: {DontIndex|Index|IndexWithAdditionalOrder}
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),  // literals: {DontUse|Use}
        e(F_BINARY_DATA_STORAGE_LOCATION_USE, "binaryDataStorageLocationUse", "DontUse")
            .gated(Since(FormatVersion::new(2, 19))),  // literals: {DontUse}
        s(F_BINARY_DATA_STORAGE_LOCATION_USE_FIELD, "binaryDataStorageLocationUseField")
            .gated(Since(FormatVersion::new(2, 19))),  // TODO(draft): ref -> MdObject (form-ref Str vs Ref?)
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),  // literals: {DontUse|Use}
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),  // literals: {Auto|DontUse}
    ]
}

/// Канонический [`EntitySpec`] вида `DocumentAttribute` (ЧЕРНОВИК).
pub fn document_attribute() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DocumentAttribute",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `DocumentAttribute` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";

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
    (F_EXTENDED_EDIT, "4de03908-56f4-4396-a61e-17253afca9ac"),
    (F_MIN_VALUE, "c65a541f-0b91-4f33-bc88-fbaaa57f9992"),
    (F_MAX_VALUE, "3eaf5a8b-06d6-47b0-ac7d-a9698247f499"),
    (F_FILL_CHECKING, "2723eb98-b4c1-498a-a6f3-70444757902f"),
    (F_CHOICE_FOLDERS_AND_ITEMS, "c421d5c6-dcd9-4e28-9a35-0b2f4d66bef0"),
    (F_CHOICE_PARAMETER_LINKS, "e3da683b-c54a-457a-a243-b9b4f9bf76dd"),
    (F_CHOICE_PARAMETERS, "fcf503b8-1c06-454a-970c-06413e64aee5"),
    (F_QUICK_CHOICE, "6e3a1131-37a3-4da5-8895-572d9d0c9db6"),
    (F_CREATE_ON_INPUT, "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9"),
    (F_CHOICE_FORM, "6c4f7074-e7d4-48eb-b31b-132873666262"),
    (F_LINK_BY_TYPE, "1183c14f-f814-49c6-9233-a3c26b3f64cf"),
    (F_FILL_FROM_FILLING_VALUE, "2c8143d5-4248-4c43-8bfb-307c0be2e415"),
    (F_FILL_VALUE, "e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208"),
    (F_INDEXING, "a93a67a0-1955-11d6-a3c7-0050bae0a776"),
    (F_FULL_TEXT_SEARCH, "d4232326-022b-421e-b6d3-88e418f74327"),
    (F_BINARY_DATA_STORAGE_LOCATION_USE, "43ac5d7b-7d0e-41fc-b62b-2f9e1294cce0"),
    (F_BINARY_DATA_STORAGE_LOCATION_USE_FIELD, "5a78d87a-1b1d-4145-ad00-62891d1eadb1"),
    (F_DATA_HISTORY, "9288A8ED-B259-46D0-A8E3-70D87956FF2D"),
    (F_CHOICE_HISTORY_ON_INPUT, "7ba608f2-e654-42a3-8885-334fe88ca910"),
];
