//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `ChartOfCalculationTypes` (ПланВидовРасчета), guid=30b100d6-b29f-47ac-aec7-cb8ca8a54767.
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
/// `inputByString`  guid=b28cb2f0-9b2a-4784-bd09-2cb60497027f
pub const F_INPUT_BY_STRING: FieldId = FieldId(6);
/// `searchStringModeOnInputByString`  guid=5bd5743d-bf9a-43bc-b7a9-c1a0b5984f78
pub const F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(7);
/// `fullTextSearchOnInputByString`  guid=9221cdfb-8935-49e7-9b1a-0b24d205297c
pub const F_FULL_TEXT_SEARCH_ON_INPUT_BY_STRING: FieldId = FieldId(8);
/// `choiceDataGetModeOnInputByString`  guid=d80df833-0fcb-49e6-91c9-dda7ea01c2a5
pub const F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(9);
/// `basedOn`  guid=a1f564fd-c8e2-4aea-9e6e-2ee5fd5c2ed4
pub const F_BASED_ON: FieldId = FieldId(10);
/// `createOnInput`  guid=33c74a4d-561f-4bc0-9eaa-8d21c893c0a9
pub const F_CREATE_ON_INPUT: FieldId = FieldId(11);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(12);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(13);
/// `dataLockFields`  guid=130bff15-88ce-4f99-8610-abb4a300ebb0
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(14);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(15);
/// `fullTextSearch`  guid=d4232326-022b-421e-b6d3-88e418f74327
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(16);
/// `objectPresentation`  guid=d79e10d1-ebb6-46c3-a053-55c3b7248b7e
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(17);
/// `extendedObjectPresentation`  guid=3613fe2b-442b-4723-a0dd-4bad030dcfa5
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(18);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(19);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(20);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(21);
/// `dataHistory`  guid=9288A8ED-B259-46D0-A8E3-70D87956FF2D
pub const F_DATA_HISTORY: FieldId = FieldId(22);
/// `codeLength`  guid=37f2fa9d-b276-11d4-9435-004095e12fc7
pub const F_CODE_LENGTH: FieldId = FieldId(23);
/// `descriptionLength`  guid=37f2fa9f-b276-11d4-9435-004095e12fc7
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(24);
/// `codeType`  guid=0040f024-6db7-460b-baba-bf38860f8a6b
pub const F_CODE_TYPE: FieldId = FieldId(25);
/// `codeAllowedLength`  guid=bf9cc511-eb2a-48b6-a666-7afb78b83f36
pub const F_CODE_ALLOWED_LENGTH: FieldId = FieldId(26);
/// `defaultPresentation`  guid=231ac367-0abd-4874-a1c2-b96b314eca21
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(27);
/// `editType`  guid=267add9b-837b-42d9-992d-374a808fd94d
pub const F_EDIT_TYPE: FieldId = FieldId(28);
/// `quickChoice`  guid=1b3ecd33-43d7-45e4-a012-776d95785465
pub const F_QUICK_CHOICE: FieldId = FieldId(29);
/// `choiceMode`  guid=cab319ad-716c-4d9e-9240-639aab7e9a0c
pub const F_CHOICE_MODE: FieldId = FieldId(30);
/// `choiceHistoryOnInput`  guid=7ba608f2-e654-42a3-8885-334fe88ca910
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(31);
/// `defaultObjectForm`  guid=b69601f3-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(32);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(33);
/// `defaultChoiceForm`  guid=b69601f6-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(34);
/// `auxiliaryObjectForm`  guid=98388ad1-7b6f-4049-bfdb-02b3e3789cdc
pub const F_AUXILIARY_OBJECT_FORM: FieldId = FieldId(35);
/// `auxiliaryListForm`  guid=367b60f5-2ad5-4399-a6b0-3dd8e4ac3650
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(36);
/// `auxiliaryChoiceForm`  guid=4f0c02fe-c9da-4f51-ae93-e69119d4eb2c
pub const F_AUXILIARY_CHOICE_FORM: FieldId = FieldId(37);
/// `dependenceOnCalculationTypes`  guid=62e9b78b-2fa1-4984-bab6-1a90a69d2dee
pub const F_DEPENDENCE_ON_CALCULATION_TYPES: FieldId = FieldId(38);
/// `baseCalculationTypes`  guid=6de17c14-70b9-4da1-966c-d0c61f5b81a5
pub const F_BASE_CALCULATION_TYPES: FieldId = FieldId(39);
/// `actionPeriodUse`  guid=d029903f-b2ae-4120-ae74-097811c24fc6
pub const F_ACTION_PERIOD_USE: FieldId = FieldId(40);
/// `predefined`  guid=f440939a-f130-413b-9c9a-2b18c4af69c6
pub const F_PREDEFINED: FieldId = FieldId(41);
/// `predefinedDataUpdate`  guid=e0f7a40c-d85a-468b-a598-d816bef1bb6c
pub const F_PREDEFINED_DATA_UPDATE: FieldId = FieldId(42);
/// `updateDataHistoryImmediatelyAfterWrite` (since 8.3.15)  guid=0912c390-3c7e-4de9-abd0-f9c9cb19f0e6
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE: FieldId = FieldId(43);
/// `executeAfterWriteDataHistoryVersionProcessing` (since 8.3.15)  guid=c4986a1c-1315-4f86-9587-f1d116329b58
pub const F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING: FieldId = FieldId(44);

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
fn i(id: FieldId, name: &'static str, def: i64) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Int, PropertyValue::Int(def))
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
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        list(F_INPUT_BY_STRING, "inputByString"),  // TODO(draft): ref-list -> Field
        e(F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING, "searchStringModeOnInputByString", "Begin"),  // literals: {Begin|AnyPart}
        e(F_FULL_TEXT_SEARCH_ON_INPUT_BY_STRING, "fullTextSearchOnInputByString", "Use"),  // literals: {Use|DontUse}
        e(F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING, "choiceDataGetModeOnInputByString", "Directly"),  // literals: {Directly|Background}
        list(F_BASED_ON, "basedOn"),  // TODO(draft): ref-list -> MdObject
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),  // literals: {Auto|DontUse|Use}
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),  // TODO(draft): ref-list -> Field
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),  // literals: {DontUse|Use}
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),  // literals: {DontUse|Use}
        i(F_CODE_LENGTH, "codeLength", 0),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        e(F_CODE_TYPE, "codeType", "Number"),  // literals: {Number|String}
        e(F_CODE_ALLOWED_LENGTH, "codeAllowedLength", "Fixed"),  // literals: {Fixed|Variable}
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),  // literals: {AsCode|AsDescription}
        e(F_EDIT_TYPE, "editType", "InList"),  // literals: {InList|InDialog|BothWays}
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),  // literals: {FromForm|QuickChoice|BothWays}
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),  // literals: {Auto|DontUse}
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_OBJECT_FORM, "auxiliaryObjectForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_CHOICE_FORM, "auxiliaryChoiceForm"),  // TODO(draft): ref -> ChartOfCalculationTypesForm (form-ref Str vs Ref?)
        e(F_DEPENDENCE_ON_CALCULATION_TYPES, "dependenceOnCalculationTypes", "DontUse"),  // literals: {DontUse|OnActionPeriod|OnRegistrationPeriod}
        list(F_BASE_CALCULATION_TYPES, "baseCalculationTypes"),  // TODO(draft): ref-list -> ChartOfCalculationTypes
        b(F_ACTION_PERIOD_USE, "actionPeriodUse"),
        s(F_PREDEFINED, "predefined").x_ignored(),  // TODO(draft): contained ChartOfCalculationTypesPredefined — уточнить vk  EDT-only?
        e(F_PREDEFINED_DATA_UPDATE, "predefinedDataUpdate", "Auto"),  // literals: {Auto|AutoUpdate|DontAutoUpdate}
        b(F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "executeAfterWriteDataHistoryVersionProcessing"),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "ChartOfCalculationTypes.StandardAttribute" },  // TODO(draft): Ref/stub?
    ChildSlot { collection: "Characteristic", child_kind: "ChartOfCalculationTypes.Characteristic" },  // TODO(draft): Ref/stub?
    ChildSlot { collection: "StandardTabularSection", child_kind: "ChartOfCalculationTypes.StandardTabularSection" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `ChartOfCalculationTypes` (ЧЕРНОВИК).
pub fn chart_of_calculation_types() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `ChartOfCalculationTypes` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "30b100d6-b29f-47ac-aec7-cb8ca8a54767";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_INPUT_BY_STRING, "b28cb2f0-9b2a-4784-bd09-2cb60497027f"),
    (F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING, "5bd5743d-bf9a-43bc-b7a9-c1a0b5984f78"),
    (F_FULL_TEXT_SEARCH_ON_INPUT_BY_STRING, "9221cdfb-8935-49e7-9b1a-0b24d205297c"),
    (F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING, "d80df833-0fcb-49e6-91c9-dda7ea01c2a5"),
    (F_BASED_ON, "a1f564fd-c8e2-4aea-9e6e-2ee5fd5c2ed4"),
    (F_CREATE_ON_INPUT, "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_DATA_LOCK_FIELDS, "130bff15-88ce-4f99-8610-abb4a300ebb0"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
    (F_FULL_TEXT_SEARCH, "d4232326-022b-421e-b6d3-88e418f74327"),
    (F_OBJECT_PRESENTATION, "d79e10d1-ebb6-46c3-a053-55c3b7248b7e"),
    (F_EXTENDED_OBJECT_PRESENTATION, "3613fe2b-442b-4723-a0dd-4bad030dcfa5"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_DATA_HISTORY, "9288A8ED-B259-46D0-A8E3-70D87956FF2D"),
    (F_CODE_LENGTH, "37f2fa9d-b276-11d4-9435-004095e12fc7"),
    (F_DESCRIPTION_LENGTH, "37f2fa9f-b276-11d4-9435-004095e12fc7"),
    (F_CODE_TYPE, "0040f024-6db7-460b-baba-bf38860f8a6b"),
    (F_CODE_ALLOWED_LENGTH, "bf9cc511-eb2a-48b6-a666-7afb78b83f36"),
    (F_DEFAULT_PRESENTATION, "231ac367-0abd-4874-a1c2-b96b314eca21"),
    (F_EDIT_TYPE, "267add9b-837b-42d9-992d-374a808fd94d"),
    (F_QUICK_CHOICE, "1b3ecd33-43d7-45e4-a012-776d95785465"),
    (F_CHOICE_MODE, "cab319ad-716c-4d9e-9240-639aab7e9a0c"),
    (F_CHOICE_HISTORY_ON_INPUT, "7ba608f2-e654-42a3-8885-334fe88ca910"),
    (F_DEFAULT_OBJECT_FORM, "b69601f3-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_CHOICE_FORM, "b69601f6-4cf6-11d4-9415-008048da11f9"),
    (F_AUXILIARY_OBJECT_FORM, "98388ad1-7b6f-4049-bfdb-02b3e3789cdc"),
    (F_AUXILIARY_LIST_FORM, "367b60f5-2ad5-4399-a6b0-3dd8e4ac3650"),
    (F_AUXILIARY_CHOICE_FORM, "4f0c02fe-c9da-4f51-ae93-e69119d4eb2c"),
    (F_DEPENDENCE_ON_CALCULATION_TYPES, "62e9b78b-2fa1-4984-bab6-1a90a69d2dee"),
    (F_BASE_CALCULATION_TYPES, "6de17c14-70b9-4da1-966c-d0c61f5b81a5"),
    (F_ACTION_PERIOD_USE, "d029903f-b2ae-4120-ae74-097811c24fc6"),
    (F_PREDEFINED, "f440939a-f130-413b-9c9a-2b18c4af69c6"),
    (F_PREDEFINED_DATA_UPDATE, "e0f7a40c-d85a-468b-a598-d816bef1bb6c"),
    (F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "0912c390-3c7e-4de9-abd0-f9c9cb19f0e6"),
    (F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "c4986a1c-1315-4f86-9587-f1d116329b58"),
];
