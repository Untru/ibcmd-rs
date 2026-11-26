//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `InformationRegister` (РегистрСведений), guid=13134201-f60b-11d5-a3c7-0050bae0a776.
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
/// `dataHistory`  guid=9288A8ED-B259-46D0-A8E3-70D87956FF2D
pub const F_DATA_HISTORY: FieldId = FieldId(5);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(6);
/// `editType`  guid=267add9b-837b-42d9-992d-374a808fd94d
pub const F_EDIT_TYPE: FieldId = FieldId(7);
/// `defaultRecordForm`  guid=353c5ce0-0b3c-11d6-a3c7-0050bae0a776
pub const F_DEFAULT_RECORD_FORM: FieldId = FieldId(8);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(9);
/// `auxiliaryRecordForm`  guid=5b0405ec-b187-4828-a005-f15921422b2b
pub const F_AUXILIARY_RECORD_FORM: FieldId = FieldId(10);
/// `auxiliaryListForm`  guid=367b60f5-2ad5-4399-a6b0-3dd8e4ac3650
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(11);
/// `informationRegisterPeriodicity`  guid=13134205-f60b-11d5-a3c7-0050bae0a776
pub const F_INFORMATION_REGISTER_PERIODICITY: FieldId = FieldId(12);
/// `writeMode`  guid=09c412e0-0f30-11d6-a3c7-0050bae0a776
pub const F_WRITE_MODE: FieldId = FieldId(13);
/// `mainFilterOnPeriod`  guid=6e4cd931-ddd4-4f35-86d7-88c59f3e26e7
pub const F_MAIN_FILTER_ON_PERIOD: FieldId = FieldId(14);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(15);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(16);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(17);
/// `fullTextSearch`  guid=d4232326-022b-421e-b6d3-88e418f74327
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(18);
/// `enableTotalsSliceFirst`  guid=d200da23-b24d-4d2d-8601-d5fdfa61e245
pub const F_ENABLE_TOTALS_SLICE_FIRST: FieldId = FieldId(19);
/// `enableTotalsSliceLast`  guid=382758de-f473-4894-b7a6-1a9418d8646e
pub const F_ENABLE_TOTALS_SLICE_LAST: FieldId = FieldId(20);
/// `recordPresentation`  guid=b2f35c4e-8ffd-4e78-bb87-f7a7f14916e3
pub const F_RECORD_PRESENTATION: FieldId = FieldId(21);
/// `extendedRecordPresentation`  guid=efb47759-ee9f-4a50-b774-e4722073df6e
pub const F_EXTENDED_RECORD_PRESENTATION: FieldId = FieldId(22);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(23);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(24);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(25);
/// `updateDataHistoryImmediatelyAfterWrite` (since 8.3.15)  guid=0912c390-3c7e-4de9-abd0-f9c9cb19f0e6
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE: FieldId = FieldId(26);
/// `executeAfterWriteDataHistoryVersionProcessing` (since 8.3.15)  guid=c4986a1c-1315-4f86-9587-f1d116329b58
pub const F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING: FieldId = FieldId(27);

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
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),  // literals: {DontUse|Use}
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        e(F_EDIT_TYPE, "editType", "InList"),  // literals: {InList|InDialog|BothWays}
        s(F_DEFAULT_RECORD_FORM, "defaultRecordForm"),  // TODO(draft): ref -> InformationRegisterForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> InformationRegisterForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_RECORD_FORM, "auxiliaryRecordForm"),  // TODO(draft): ref -> InformationRegisterForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),  // TODO(draft): ref -> InformationRegisterForm (form-ref Str vs Ref?)
        e(F_INFORMATION_REGISTER_PERIODICITY, "informationRegisterPeriodicity", "Nonperiodical"),  // literals: {Nonperiodical|RecorderPosition|Second|Day|Month|Quarter|Year}
        e(F_WRITE_MODE, "writeMode", "Independent"),  // literals: {Independent|RecorderSubordinate}
        b(F_MAIN_FILTER_ON_PERIOD, "mainFilterOnPeriod"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),  // literals: {DontUse|Use}
        b(F_ENABLE_TOTALS_SLICE_FIRST, "enableTotalsSliceFirst"),
        b(F_ENABLE_TOTALS_SLICE_LAST, "enableTotalsSliceLast"),
        loc_field(F_RECORD_PRESENTATION, "recordPresentation"),
        loc_field(F_EXTENDED_RECORD_PRESENTATION, "extendedRecordPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        b(F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "executeAfterWriteDataHistoryVersionProcessing"),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "InformationRegister.StandardAttribute" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `InformationRegister` (ЧЕРНОВИК).
pub fn information_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "InformationRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `InformationRegister` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "13134201-f60b-11d5-a3c7-0050bae0a776";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_DATA_HISTORY, "9288A8ED-B259-46D0-A8E3-70D87956FF2D"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_EDIT_TYPE, "267add9b-837b-42d9-992d-374a808fd94d"),
    (F_DEFAULT_RECORD_FORM, "353c5ce0-0b3c-11d6-a3c7-0050bae0a776"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_AUXILIARY_RECORD_FORM, "5b0405ec-b187-4828-a005-f15921422b2b"),
    (F_AUXILIARY_LIST_FORM, "367b60f5-2ad5-4399-a6b0-3dd8e4ac3650"),
    (F_INFORMATION_REGISTER_PERIODICITY, "13134205-f60b-11d5-a3c7-0050bae0a776"),
    (F_WRITE_MODE, "09c412e0-0f30-11d6-a3c7-0050bae0a776"),
    (F_MAIN_FILTER_ON_PERIOD, "6e4cd931-ddd4-4f35-86d7-88c59f3e26e7"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
    (F_FULL_TEXT_SEARCH, "d4232326-022b-421e-b6d3-88e418f74327"),
    (F_ENABLE_TOTALS_SLICE_FIRST, "d200da23-b24d-4d2d-8601-d5fdfa61e245"),
    (F_ENABLE_TOTALS_SLICE_LAST, "382758de-f473-4894-b7a6-1a9418d8646e"),
    (F_RECORD_PRESENTATION, "b2f35c4e-8ffd-4e78-bb87-f7a7f14916e3"),
    (F_EXTENDED_RECORD_PRESENTATION, "efb47759-ee9f-4a50-b774-e4722073df6e"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "0912c390-3c7e-4de9-abd0-f9c9cb19f0e6"),
    (F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "c4986a1c-1315-4f86-9587-f1d116329b58"),
];
