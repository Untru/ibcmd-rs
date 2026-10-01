//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `Table` (Таблица), guid=e3403acd-1c95-421b-87e4-4dfa29d38b52.
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
/// `tableType`  guid=4b8645b1-82ab-4b77-bdd2-47705b455eca
pub const F_TABLE_TYPE: FieldId = FieldId(5);
/// `nameInDataSource`  guid=80d8c15a-41c3-4ce7-bfb3-4f1aede2649b
pub const F_NAME_IN_DATA_SOURCE: FieldId = FieldId(6);
/// `expressionInDataSource`  guid=e17188af-805e-4eb5-8a77-b5b296a79007
pub const F_EXPRESSION_IN_DATA_SOURCE: FieldId = FieldId(7);
/// `tableDataType`  guid=14c0a585-f30b-428b-8ed3-9c6563826997
pub const F_TABLE_DATA_TYPE: FieldId = FieldId(8);
/// `keyFields`  guid=9a526d8b-ceb4-41b6-89cd-83ae835d870b
pub const F_KEY_FIELDS: FieldId = FieldId(9);
/// `presentationField`  guid=b5937463-8629-4b26-b4ee-7914d9ac1d82
pub const F_PRESENTATION_FIELD: FieldId = FieldId(10);
/// `parentField`  guid=74ec4e99-d43b-4889-b56e-dc3397d39714
pub const F_PARENT_FIELD: FieldId = FieldId(11);
/// `unfilledParentValue`  guid=f2b51318-20cc-4aa2-8193-0aad9c6efe27
pub const F_UNFILLED_PARENT_VALUE: FieldId = FieldId(12);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(13);
/// `quickChoice`  guid=1b3ecd33-43d7-45e4-a012-776d95785465
pub const F_QUICK_CHOICE: FieldId = FieldId(14);
/// `inputByString`  guid=b28cb2f0-9b2a-4784-bd09-2cb60497027f
pub const F_INPUT_BY_STRING: FieldId = FieldId(15);
/// `createOnInput`  guid=33c74a4d-561f-4bc0-9eaa-8d21c893c0a9
pub const F_CREATE_ON_INPUT: FieldId = FieldId(16);
/// `searchStringModeOnInputByString`  guid=5bd5743d-bf9a-43bc-b7a9-c1a0b5984f78
pub const F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(17);
/// `choiceDataGetModeOnInputByString`  guid=d80df833-0fcb-49e6-91c9-dda7ea01c2a5
pub const F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING: FieldId = FieldId(18);
/// `choiceHistoryOnInput`  guid=7ba608f2-e654-42a3-8885-334fe88ca910
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(19);
/// `defaultObjectForm`  guid=b69601f3-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(20);
/// `defaultRecordForm`  guid=eb69bd85-1628-4846-8279-fade997cbcde
pub const F_DEFAULT_RECORD_FORM: FieldId = FieldId(21);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(22);
/// `defaultChoiceForm`  guid=b69601f6-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(23);
/// `objectPresentation`  guid=d79e10d1-ebb6-46c3-a053-55c3b7248b7e
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(24);
/// `extendedObjectPresentation`  guid=3613fe2b-442b-4723-a0dd-4bad030dcfa5
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(25);
/// `recordPresentation`  guid=b2f35c4e-8ffd-4e78-bb87-f7a7f14916e3
pub const F_RECORD_PRESENTATION: FieldId = FieldId(26);
/// `extendedRecordPresentation`  guid=efb47759-ee9f-4a50-b774-e4722073df6e
pub const F_EXTENDED_RECORD_PRESENTATION: FieldId = FieldId(27);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(28);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(29);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(30);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(31);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(32);
/// `readOnly`  guid=a57d5e9c-3d4d-4824-a8e0-168f2137385f
pub const F_READ_ONLY: FieldId = FieldId(33);
/// `transactionsIsolationLevel`  guid=13b847d2-79e2-43cf-aa06-0c04e47c1ea9
pub const F_TRANSACTIONS_ISOLATION_LEVEL: FieldId = FieldId(34);
/// `dataVersionField`  guid=68f69f70-8d34-4ec7-a0d2-c02daaae6e83
pub const F_DATA_VERSION_FIELD: FieldId = FieldId(35);
/// `editType`  guid=267add9b-837b-42d9-992d-374a808fd94d
pub const F_EDIT_TYPE: FieldId = FieldId(36);
/// `basedOn`  guid=a1f564fd-c8e2-4aea-9e6e-2ee5fd5c2ed4
pub const F_BASED_ON: FieldId = FieldId(37);
/// `dataLockFields`  guid=130bff15-88ce-4f99-8610-abb4a300ebb0
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(38);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(39);

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
        e(F_TABLE_TYPE, "tableType", "Table"),  // literals: {Table|Expression}
        s(F_NAME_IN_DATA_SOURCE, "nameInDataSource"),
        s(F_EXPRESSION_IN_DATA_SOURCE, "expressionInDataSource"),
        e(F_TABLE_DATA_TYPE, "tableDataType", "ObjectData"),  // literals: {ObjectData|NonobjectData}
        list(F_KEY_FIELDS, "keyFields"),  // TODO(draft): ref-list -> Field
        s(F_PRESENTATION_FIELD, "presentationField"),  // TODO(draft): ref -> Field (form-ref Str vs Ref?)
        s(F_PARENT_FIELD, "parentField"),  // TODO(draft): ref -> Field (form-ref Str vs Ref?)
        FieldSpec::required(F_UNFILLED_PARENT_VALUE, "unfilledParentValue", ValueKind::Value),  // TODO(draft): Value-кодек/дефолт
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        b(F_QUICK_CHOICE, "quickChoice"),
        list(F_INPUT_BY_STRING, "inputByString"),  // TODO(draft): ref-list -> Field
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),  // literals: {Auto|DontUse|Use}
        e(F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING, "searchStringModeOnInputByString", "Begin"),  // literals: {Begin|AnyPart}
        e(F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING, "choiceDataGetModeOnInputByString", "Directly"),  // literals: {Directly|Background}
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),  // literals: {Auto|DontUse}
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),  // TODO(draft): ref -> TableForm (form-ref Str vs Ref?)
        s(F_DEFAULT_RECORD_FORM, "defaultRecordForm"),  // TODO(draft): ref -> TableForm (form-ref Str vs Ref?)
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> TableForm (form-ref Str vs Ref?)
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),  // TODO(draft): ref -> TableForm (form-ref Str vs Ref?)
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_RECORD_PRESENTATION, "recordPresentation"),
        loc_field(F_EXTENDED_RECORD_PRESENTATION, "extendedRecordPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        b(F_READ_ONLY, "readOnly"),
        e(F_TRANSACTIONS_ISOLATION_LEVEL, "transactionsIsolationLevel", ""),  // literals: {}
        s(F_DATA_VERSION_FIELD, "dataVersionField"),  // TODO(draft): ref -> MdObject (form-ref Str vs Ref?)
        e(F_EDIT_TYPE, "editType", "InList"),  // literals: {InList|InDialog|BothWays}
        list(F_BASED_ON, "basedOn"),  // TODO(draft): ref-list -> MdObject
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),  // TODO(draft): ref-list -> Field
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Characteristic", child_kind: "Table.Characteristic" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `Table` (ЧЕРНОВИК).
pub fn table() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Table",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `Table` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "e3403acd-1c95-421b-87e4-4dfa29d38b52";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_TABLE_TYPE, "4b8645b1-82ab-4b77-bdd2-47705b455eca"),
    (F_NAME_IN_DATA_SOURCE, "80d8c15a-41c3-4ce7-bfb3-4f1aede2649b"),
    (F_EXPRESSION_IN_DATA_SOURCE, "e17188af-805e-4eb5-8a77-b5b296a79007"),
    (F_TABLE_DATA_TYPE, "14c0a585-f30b-428b-8ed3-9c6563826997"),
    (F_KEY_FIELDS, "9a526d8b-ceb4-41b6-89cd-83ae835d870b"),
    (F_PRESENTATION_FIELD, "b5937463-8629-4b26-b4ee-7914d9ac1d82"),
    (F_PARENT_FIELD, "74ec4e99-d43b-4889-b56e-dc3397d39714"),
    (F_UNFILLED_PARENT_VALUE, "f2b51318-20cc-4aa2-8193-0aad9c6efe27"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_QUICK_CHOICE, "1b3ecd33-43d7-45e4-a012-776d95785465"),
    (F_INPUT_BY_STRING, "b28cb2f0-9b2a-4784-bd09-2cb60497027f"),
    (F_CREATE_ON_INPUT, "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9"),
    (F_SEARCH_STRING_MODE_ON_INPUT_BY_STRING, "5bd5743d-bf9a-43bc-b7a9-c1a0b5984f78"),
    (F_CHOICE_DATA_GET_MODE_ON_INPUT_BY_STRING, "d80df833-0fcb-49e6-91c9-dda7ea01c2a5"),
    (F_CHOICE_HISTORY_ON_INPUT, "7ba608f2-e654-42a3-8885-334fe88ca910"),
    (F_DEFAULT_OBJECT_FORM, "b69601f3-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_RECORD_FORM, "eb69bd85-1628-4846-8279-fade997cbcde"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_DEFAULT_CHOICE_FORM, "b69601f6-4cf6-11d4-9415-008048da11f9"),
    (F_OBJECT_PRESENTATION, "d79e10d1-ebb6-46c3-a053-55c3b7248b7e"),
    (F_EXTENDED_OBJECT_PRESENTATION, "3613fe2b-442b-4723-a0dd-4bad030dcfa5"),
    (F_RECORD_PRESENTATION, "b2f35c4e-8ffd-4e78-bb87-f7a7f14916e3"),
    (F_EXTENDED_RECORD_PRESENTATION, "efb47759-ee9f-4a50-b774-e4722073df6e"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_READ_ONLY, "a57d5e9c-3d4d-4824-a8e0-168f2137385f"),
    (F_TRANSACTIONS_ISOLATION_LEVEL, "13b847d2-79e2-43cf-aa06-0c04e47c1ea9"),
    (F_DATA_VERSION_FIELD, "68f69f70-8d34-4ec7-a0d2-c02daaae6e83"),
    (F_EDIT_TYPE, "267add9b-837b-42d9-992d-374a808fd94d"),
    (F_BASED_ON, "a1f564fd-c8e2-4aea-9e6e-2ee5fd5c2ed4"),
    (F_DATA_LOCK_FIELDS, "130bff15-88ce-4f99-8610-abb4a300ebb0"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
];
