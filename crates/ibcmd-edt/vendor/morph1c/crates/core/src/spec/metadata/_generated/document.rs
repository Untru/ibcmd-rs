//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Document` (Документ), guid=061d872a-5787-460e-95ac-ed74ea3a3e84.
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
/// `numerator`  guid=216e0434-4ec5-4d1b-8e60-643d982d5fd9
pub const F_NUMERATOR: FieldId = FieldId(23);
/// `numberType`  guid=60643a07-120a-4a63-9dba-67369c0f0145
pub const F_NUMBER_TYPE: FieldId = FieldId(24);
/// `numberLength`  guid=490eb6bd-24c4-4943-82aa-d0c4b666861b
pub const F_NUMBER_LENGTH: FieldId = FieldId(25);
/// `numberAllowedLength`  guid=c6c9689d-2978-42e8-863f-0280c6a85b56
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(26);
/// `numberPeriodicity`  guid=67ebd6e9-d0ad-4b29-8d2b-3bfec03ff2cb
pub const F_NUMBER_PERIODICITY: FieldId = FieldId(27);
/// `checkUnique`  guid=8336a121-59b3-427d-8e49-1c813662a550
pub const F_CHECK_UNIQUE: FieldId = FieldId(28);
/// `autonumbering`  guid=05720713-cc30-4c4e-a417-87b1676d8d85
pub const F_AUTONUMBERING: FieldId = FieldId(29);
/// `defaultObjectForm`  guid=b69601f3-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(30);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(31);
/// `defaultChoiceForm`  guid=b69601f6-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(32);
/// `auxiliaryObjectForm`  guid=98388ad1-7b6f-4049-bfdb-02b3e3789cdc
pub const F_AUXILIARY_OBJECT_FORM: FieldId = FieldId(33);
/// `auxiliaryListForm`  guid=367b60f5-2ad5-4399-a6b0-3dd8e4ac3650
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(34);
/// `auxiliaryChoiceForm`  guid=4f0c02fe-c9da-4f51-ae93-e69119d4eb2c
pub const F_AUXILIARY_CHOICE_FORM: FieldId = FieldId(35);
/// `posting`  guid=68548ca0-1f29-11d6-a3c7-0050bae0a776
pub const F_POSTING: FieldId = FieldId(36);
/// `realTimePosting`  guid=6c363173-16b4-4fe9-93d9-c2c4193e4065
pub const F_REAL_TIME_POSTING: FieldId = FieldId(37);
/// `registerRecordsDeletion`  guid=cb89c6e0-1f2a-11d6-a3c7-0050bae0a776
pub const F_REGISTER_RECORDS_DELETION: FieldId = FieldId(38);
/// `registerRecordsWritingOnPost`  guid=ffb10f15-34c5-4e17-846c-aa3415eb0f96
pub const F_REGISTER_RECORDS_WRITING_ON_POST: FieldId = FieldId(39);
/// `sequenceFilling`  guid=78019747-bba9-4901-9e02-3646ae7a0424
pub const F_SEQUENCE_FILLING: FieldId = FieldId(40);
/// `registerRecords`  guid=aa7de5c0-0f31-11d6-a3c7-0050bae0a776
pub const F_REGISTER_RECORDS: FieldId = FieldId(41);
/// `postInPrivilegedMode`  guid=99d272f6-0aeb-4743-9295-3aa0b2c27356
pub const F_POST_IN_PRIVILEGED_MODE: FieldId = FieldId(42);
/// `unpostInPrivilegedMode`  guid=14076db5-1fdc-4ba4-b9f1-9714c8363c89
pub const F_UNPOST_IN_PRIVILEGED_MODE: FieldId = FieldId(43);
/// `choiceHistoryOnInput`  guid=7ba608f2-e654-42a3-8885-334fe88ca910
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(44);
/// `updateDataHistoryImmediatelyAfterWrite` (since 8.3.15)  guid=0912c390-3c7e-4de9-abd0-f9c9cb19f0e6
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE: FieldId = FieldId(45);
/// `executeAfterWriteDataHistoryVersionProcessing` (since 8.3.15)  guid=c4986a1c-1315-4f86-9587-f1d116329b58
pub const F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING: FieldId = FieldId(46);

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
        s(F_NUMERATOR, "numerator"),  // TODO(draft): ref -> DocumentNumerator (form-ref Str vs Ref?)
        e(F_NUMBER_TYPE, "numberType", "Number"),  // literals: {Number|String}
        i(F_NUMBER_LENGTH, "numberLength", 0),
        e(F_NUMBER_ALLOWED_LENGTH, "numberAllowedLength", "Fixed"),  // literals: {Fixed|Variable}
        e(F_NUMBER_PERIODICITY, "numberPeriodicity", "Nonperiodical"),  // literals: {Nonperiodical|Year|Quarter|Month|Day}
        b(F_CHECK_UNIQUE, "checkUnique"),
        b(F_AUTONUMBERING, "autonumbering"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_OBJECT_FORM, "auxiliaryObjectForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_CHOICE_FORM, "auxiliaryChoiceForm"),  // TODO(draft): ref -> DocumentForm (form-ref Str vs Ref?)
        e(F_POSTING, "posting", "Allow"),  // literals: {Allow|Deny}
        e(F_REAL_TIME_POSTING, "realTimePosting", "Allow"),  // literals: {Allow|Deny}
        e(F_REGISTER_RECORDS_DELETION, "registerRecordsDeletion", "AutoDeleteOnUnpost"),  // literals: {AutoDeleteOnUnpost|AutoDelete|AutoDeleteOff}
        e(F_REGISTER_RECORDS_WRITING_ON_POST, "registerRecordsWritingOnPost", "WriteSelected"),  // literals: {WriteSelected|WriteModified}
        e(F_SEQUENCE_FILLING, "sequenceFilling", "AutoFill"),  // literals: {AutoFill|AutoFillOff}
        list(F_REGISTER_RECORDS, "registerRecords"),  // TODO(draft): ref-list -> BasicRegister
        b(F_POST_IN_PRIVILEGED_MODE, "postInPrivilegedMode"),
        b(F_UNPOST_IN_PRIVILEGED_MODE, "unpostInPrivilegedMode"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),  // literals: {Auto|DontUse}
        b(F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "executeAfterWriteDataHistoryVersionProcessing"),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "Document.StandardAttribute" },  // TODO(draft): Ref/stub?
    ChildSlot { collection: "Characteristic", child_kind: "Document.Characteristic" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `Document` (ЧЕРНОВИК).
pub fn document() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Document",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `Document` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "061d872a-5787-460e-95ac-ed74ea3a3e84";

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
    (F_NUMERATOR, "216e0434-4ec5-4d1b-8e60-643d982d5fd9"),
    (F_NUMBER_TYPE, "60643a07-120a-4a63-9dba-67369c0f0145"),
    (F_NUMBER_LENGTH, "490eb6bd-24c4-4943-82aa-d0c4b666861b"),
    (F_NUMBER_ALLOWED_LENGTH, "c6c9689d-2978-42e8-863f-0280c6a85b56"),
    (F_NUMBER_PERIODICITY, "67ebd6e9-d0ad-4b29-8d2b-3bfec03ff2cb"),
    (F_CHECK_UNIQUE, "8336a121-59b3-427d-8e49-1c813662a550"),
    (F_AUTONUMBERING, "05720713-cc30-4c4e-a417-87b1676d8d85"),
    (F_DEFAULT_OBJECT_FORM, "b69601f3-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_CHOICE_FORM, "b69601f6-4cf6-11d4-9415-008048da11f9"),
    (F_AUXILIARY_OBJECT_FORM, "98388ad1-7b6f-4049-bfdb-02b3e3789cdc"),
    (F_AUXILIARY_LIST_FORM, "367b60f5-2ad5-4399-a6b0-3dd8e4ac3650"),
    (F_AUXILIARY_CHOICE_FORM, "4f0c02fe-c9da-4f51-ae93-e69119d4eb2c"),
    (F_POSTING, "68548ca0-1f29-11d6-a3c7-0050bae0a776"),
    (F_REAL_TIME_POSTING, "6c363173-16b4-4fe9-93d9-c2c4193e4065"),
    (F_REGISTER_RECORDS_DELETION, "cb89c6e0-1f2a-11d6-a3c7-0050bae0a776"),
    (F_REGISTER_RECORDS_WRITING_ON_POST, "ffb10f15-34c5-4e17-846c-aa3415eb0f96"),
    (F_SEQUENCE_FILLING, "78019747-bba9-4901-9e02-3646ae7a0424"),
    (F_REGISTER_RECORDS, "aa7de5c0-0f31-11d6-a3c7-0050bae0a776"),
    (F_POST_IN_PRIVILEGED_MODE, "99d272f6-0aeb-4743-9295-3aa0b2c27356"),
    (F_UNPOST_IN_PRIVILEGED_MODE, "14076db5-1fdc-4ba4-b9f1-9714c8363c89"),
    (F_CHOICE_HISTORY_ON_INPUT, "7ba608f2-e654-42a3-8885-334fe88ca910"),
    (F_UPDATE_DATA_HISTORY_IMMEDIATELY_AFTER_WRITE, "0912c390-3c7e-4de9-abd0-f9c9cb19f0e6"),
    (F_EXECUTE_AFTER_WRITE_DATA_HISTORY_VERSION_PROCESSING, "c4986a1c-1315-4f86-9587-f1d116329b58"),
];
