//! Канонический спек вида объекта `Document` (документ) — ARCHITECTURE.md §1.4/§1.6,
//! child-objects substrate. ТРЕТИЙ СТРУКТУРНЫЙ вид (после InformationRegister и Catalog):
//! переиспользует ~90% субстрата Catalog (`ir_child` base-реквизит для детей,
//! Command/FormRef/TemplateRef-паттерн, children-рекурсию, value/type/cpl/ref-list/
//! characteristics/help/producedTypes-кодеки) + Document-дельту
//! (std_attrs_document 5-атрибутный `Posted/Ref/DeletionMark/Date/Number`,
//! номерные поля `numberType/numberLength/numberAllowedLength/numberPeriodicity`,
//! проведение `posting/realTimePosting`, движения `registerRecords*`/`sequenceFilling`,
//! `numerator`, EDT-only help, TabularSection recursion).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено 11/11).
//! EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссии (сверено
//! corpus-wide — каждое omitted EDT-поле даёт РОВНО один Designer-дефолт).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас. EDT-only `help` — X-исключён (`x_ignore`), но участвует в R EDT. У Document
//! `predefined` НЕТ (документы не предопределяются).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize,
    StdAttrsDecl, VarSlotKind,
};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `numerator` — ссылка на нумератор (object-ref). Default "" (пустой во всех 11).
pub const F_NUMERATOR: FieldId = FieldId(4);
/// `numberType` — Default Number (как `codeType` у Catalog).
pub const F_NUMBER_TYPE: FieldId = FieldId(5);
/// `numberLength` — int. Default 0.
pub const F_NUMBER_LENGTH: FieldId = FieldId(6);
/// `numberAllowedLength` — Default Fixed.
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(7);
/// `numberPeriodicity` — Default Nonperiodical.
pub const F_NUMBER_PERIODICITY: FieldId = FieldId(8);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(9);
/// `autonumbering` — Default false.
pub const F_AUTONUMBERING: FieldId = FieldId(10);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (5 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(11);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(12);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(13);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(14);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(15);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(16);
/// `fullTextSearchOnInputByString` — present 11/11 у EDT даже при DontUse → required.
pub const F_FTS_ON_INPUT: FieldId = FieldId(17);
/// `choiceDataGetModeOnInputByString` — Default Directly. EDT+Designer (EDT sparse: сразу
/// после fullTextSearchOnInputByString).
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(18);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(19);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(20);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(21);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(22);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(23);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(24);
/// `posting` — Default Allow.
pub const F_POSTING: FieldId = FieldId(25);
/// `realTimePosting` — Default Allow.
pub const F_REAL_TIME_POSTING: FieldId = FieldId(26);
/// `registerRecordsDeletion` — Default AutoDeleteOnUnpost. EDT+Designer (EDT sparse: между
/// realTimePosting и registerRecords).
pub const F_REGISTER_RECORDS_DELETION: FieldId = FieldId(27);
/// `registerRecordsWritingOnPost` — Default WriteSelected. EDT+Designer (EDT sparse: между
/// realTimePosting и registerRecords).
pub const F_REGISTER_RECORDS_WRITING_ON_POST: FieldId = FieldId(28);
/// `sequenceFilling` — Default AutoFill. EDT+Designer (EDT sparse: между realTimePosting и
/// registerRecords).
pub const F_SEQUENCE_FILLING: FieldId = FieldId(29);
/// `registerRecords` — list-of-refs (item-style) регистров движений. Default [].
pub const F_REGISTER_RECORDS: FieldId = FieldId(30);
/// `postInPrivilegedMode` — Default false.
pub const F_POST_IN_PRIVILEGED_MODE: FieldId = FieldId(31);
/// `unpostInPrivilegedMode` — Default false.
pub const F_UNPOST_IN_PRIVILEGED_MODE: FieldId = FieldId(32);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(33);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(34);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(47);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(48);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(35);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(36);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(37);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(38);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(39);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(40);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(41);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(42);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(43);
/// `dataHistory` — Default DontUse. EDT+Designer (EDT sparse: `dataHistory` физически после
/// fullTextSearch, companions — в конце корня).
pub const F_DATA_HISTORY: FieldId = FieldId(44);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false. EDT+Designer (EDT — в конце корня).
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(45);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false. EDT+Designer (EDT — в конце корня).
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(46);

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
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        s(F_NUMERATOR, "numerator"),
        e(F_NUMBER_TYPE, "numberType", "Number"),
        i(F_NUMBER_LENGTH, "numberLength", 0),
        e(F_NUMBER_ALLOWED_LENGTH, "numberAllowedLength", "Fixed"),
        e(F_NUMBER_PERIODICITY, "numberPeriodicity", "Nonperiodical"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        b(F_AUTONUMBERING, "autonumbering"),
        // standardAttributes — вариативный const-блок (5 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        list(F_BASED_ON, "basedOn"),
        list(F_INPUT_BY_STRING, "inputByString"),
        // createOnInput — enum default Auto (EDT sparse опускает Auto; объект …_Авто опускает,
        // прочие несут Use/DontUse). Сверено _generated.
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer
        // DENSE эмитит всегда). Сверено _generated + корпусом (объект …_Использовать опускает Use).
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        e(F_POSTING, "posting", "Allow"),
        e(F_REAL_TIME_POSTING, "realTimePosting", "Allow"),
        e(F_REGISTER_RECORDS_DELETION, "registerRecordsDeletion", "AutoDeleteOnUnpost"),
        e(F_REGISTER_RECORDS_WRITING_ON_POST, "registerRecordsWritingOnPost", "WriteSelected"),
        e(F_SEQUENCE_FILLING, "sequenceFilling", "AutoFill"),
        list(F_REGISTER_RECORDS, "registerRecords"),
        b(F_POST_IN_PRIVILEGED_MODE, "postInPrivilegedMode"),
        b(F_UNPOST_IN_PRIVILEGED_MODE, "unpostInPrivilegedMode"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        b(F_HELP, "help").x_ignored(),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
        b(F_UPDATE_DATA_HISTORY, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing"),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено 11/11 ОБА формата:
/// Attribute < Form < TabularSection < Template < Command). ОТЛИЧАЕТСЯ от Catalog —
/// у Document `Form` идёт ПЕРЕД `TabularSection` (сверено: оба формата эмитят так).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "Document.Attribute" },
    ChildSlot { collection: "Form", child_kind: "Document.FormRef" },
    ChildSlot { collection: "TabularSection", child_kind: "Document.TabularSection" },
    ChildSlot { collection: "Template", child_kind: "Document.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "Document.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Document-семья:
// 5 variable-полей [synonym, toolTip, format, editFormat, fillValue]; `fillChecking`
// определён именем (Date=ShowError, иначе DontCheck), `fullTextSearch`=const Use.
// ============================================================================

const S_SYNONYM: usize = 0;
const S_TOOLTIP: usize = 1;
const S_FORMAT: usize = 2;
const S_EDIT_FORMAT: usize = 3;
const S_FILL_VALUE: usize = 4;
// ERP-корпус вскрыл 4 задаваемых листа Document-семьи (SSL их не варьировал — были
// константами/name-const): comment (Number ЗаявлениеОВвозеТоваровПолученное),
// fillFromFillingValue=true (Date ×16, Number ×3), fillChecking=ShowError у Number (×5 —
// name-const «только Date» ложен), fullTextSearch=DontUse (Posted/Ref/DeletionMark ×10,
// мирАктКонтроляКачества и др.). Слоты ДОБАВЛЕНЫ в конец (индексы существующих не
// двигаем); зеркалятся cf-стороной (`formats_brace::std_attrs_document` N_VAR=9) и
// decl'ом DocumentJournal (общий cf-кодек — длина записи ОБЩАЯ).
const S_COMMENT: usize = 5;
const S_FILL_FROM: usize = 6;
const S_FILL_CHECKING: usize = 7;
const S_FULL_TEXT: usize = 8;

/// 5 предопределённых атрибута КОРНЯ. `name_consts[0]` = fillChecking.
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Posted", name_consts: &["DontCheck"] },
    AttrDecl { name: "Ref", name_consts: &["DontCheck"] },
    AttrDecl { name: "DeletionMark", name_consts: &["DontCheck"] },
    AttrDecl { name: "Date", name_consts: &["ShowError"] },
    AttrDecl { name: "Number", name_consts: &["DontCheck"] },
];

/// 1 предопределённый атрибут табличной части (`LineNumber`, fillChecking DontCheck).
const TABULAR_ATTRS: &[AttrDecl] = &[AttrDecl { name: "LineNumber", name_consts: &["DontCheck"] }];

/// Variable-слоты (типы/дефолты); индексы = S_* (новые — В КОНЦЕ).
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Loc,               // format
    VarSlotKind::Loc,               // editFormat
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Str,               // comment (ERP-witnessed)
    VarSlotKind::Bool,              // fillFromFillingValue (ERP-witnessed)
    VarSlotKind::Enum("DontCheck"), // fillChecking (ERP: variable, не name-const)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (ERP: variable; EDT Use — явный)
];

/// EDT-листья блока (позиционный порядок; позиции новых — ERP-witnessed:
/// comment после synonym (Number ЗаявлениеОВвозеТоваровПолученное: name→comment→fillValue),
/// fillFromFillingValue до fillValue (Date ИнойДокументПодтвержденияНДС:
/// name→fillFromFillingValue→fillValue→fillChecking→fullTextSearch)).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptStr("comment", S_COMMENT),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::OptBool("fillFromFillingValue", S_FILL_FROM),
    EdtLeaf::ReqValue("fillValue", S_FILL_VALUE),
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::OptLoc("format", S_FORMAT),
    EdtLeaf::OptLoc("editFormat", S_EDIT_FORMAT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

/// Designer-DENSE 25 листьев (порядок фикс).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::VarBool(S_FILL_FROM)),
    ("CreateOnInput", DenseLeaf::Text("Auto")),
    ("TypeReductionMode", DenseLeaf::Text("TransformValues")),
    ("MaxValue", DenseLeaf::Nil),
    ("ToolTip", DenseLeaf::VarLoc(S_TOOLTIP)),
    ("ExtendedEdit", DenseLeaf::Text("false")),
    ("Format", DenseLeaf::VarLoc(S_FORMAT)),
    ("ChoiceForm", DenseLeaf::Empty),
    ("QuickChoice", DenseLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DenseLeaf::Text("Auto")),
    ("EditFormat", DenseLeaf::VarLoc(S_EDIT_FORMAT)),
    ("PasswordMode", DenseLeaf::Text("false")),
    ("DataHistory", DenseLeaf::Text("Use")),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(S_SYNONYM)),
    ("Comment", DenseLeaf::VarStr(S_COMMENT)),
    ("FullTextSearch", DenseLeaf::VarEnum(S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::VarValue(S_FILL_VALUE)),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация `standardAttributes` вида `Document` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "Document",
    label: "Document",
};

/// Канонический [`EntitySpec`] вида `Document` (кэш на процесс).
pub fn document() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Document",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `Document` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "DocumentObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "DocumentRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "DocumentSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "DocumentList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "DocumentManager",
        designer_order: 4,
    },
];
