//! Канонический спек вида объекта `ExchangePlan` (план обмена) — ARCHITECTURE.md
//! §1.4/§1.6, child-objects substrate. ШЕСТОЙ СТРУКТУРНЫЙ вид (после InformationRegister,
//! Catalog, Document, Task, ChartOfCharacteristicTypes): переиспользует ~90% субстрата
//! Catalog (`ir_child` base-реквизит для детей, Command/FormRef/TemplateRef-паттерн,
//! children-рекурсию, value/type/cpl/ref-list/value/help/producedTypes-кодеки,
//! TabularSection recursion) + ExchangePlan-дельту:
//! * std_attrs_exchange_plan — 8-атрибутный набор `ExchangeDate/ThisNode/ReceivedNo/
//!   SentNo/Ref/DeletionMark/Description/Code` (ВАРИАТИВНЫЕ fillChecking И
//!   fullTextSearch — ERP-witness, Catalog-паттерн; см. декларацию ниже);
//! * `content` — EDT-only состав плана обмена (codec `ExchangePlanContent`, X-исключён);
//! * `distributedInfoBase`/`includeConfigurationExtensions` — bool-поля плана обмена;
//! * code-нумерация Catalog-стиля (`codeLength`/`codeAllowedLength`/`descriptionLength`),
//!   БЕЗ numberType/checkUnique/autonumbering/codeType/codeSeries (сверено: их НЕТ).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено 1/1).
//! EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссий.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (5 категорий Object/Ref/Selection/List/Manager, см. `produced_types`). EDT-only
//! `content` — X-исключён (`x_ignore`), но участвует в R EDT. `help` — EDT-only const-блок
//! (в SSL-плане отсутствует; витнессится ERP-корпусом 7/16 — structural-tail T1).
//! `predefined` у плана обмена в корпусе НЕТ.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize, StdAttrsDecl,
    VarSlotKind,
};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `codeLength` — int. Default 0.
pub const F_CODE_LENGTH: FieldId = FieldId(4);
/// `codeAllowedLength` — Default Fixed (как Catalog/Document; Variable эмитится).
pub const F_CODE_ALLOWED_LENGTH: FieldId = FieldId(5);
/// `descriptionLength` — int. Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(6);
/// `defaultPresentation` — present 1/1 у EDT → required.
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(7);
/// `editType` — present 1/1 у EDT → required.
pub const F_EDIT_TYPE: FieldId = FieldId(8);
/// `quickChoice` — Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(9);
/// `choiceMode` — Default FromForm.
pub const F_CHOICE_MODE: FieldId = FieldId(10);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(11);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(12);
/// `fullTextSearchOnInputByString` — present 1/1 у EDT даже при DontUse → required.
pub const F_FTS_ON_INPUT: FieldId = FieldId(13);
/// `choiceDataGetModeOnInputByString` — Designer-only DENSE. Default Directly.
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(14);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(15);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(16);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(17);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(18);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(19);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(20);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (8 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(21);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(22);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(23);
/// `distributedInfoBase` — РИБ-признак (bool). Default false.
pub const F_DISTRIBUTED_INFO_BASE: FieldId = FieldId(24);
/// `includeConfigurationExtensions` — bool. Default false.
pub const F_INCLUDE_CONFIGURATION_EXTENSIONS: FieldId = FieldId(25);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(26);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(27);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(28);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(29);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(30);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(31);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(32);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(33);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(34);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(35);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(36);
/// `dataHistory` — Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(37);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(38);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(39);
/// `content` — EDT-only состав плана обмена (List). Default []; X-исключён.
pub const F_CONTENT: FieldId = FieldId(40);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(41);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(42);
/// `help` — EDT-only const-блок (`<help><pages><lang>ru</lang></pages></help>`). Default
/// false; X-исключён (как у Catalog/Document/…). Витнессится ERP-корпусом (7/16 планов);
/// SSL-план его не несёт (дефолт-омиссия).
pub const F_HELP: FieldId = FieldId(43);

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
        i(F_CODE_LENGTH, "codeLength", 0),
        e(F_CODE_ALLOWED_LENGTH, "codeAllowedLength", "Fixed"),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        // defaultPresentation — present 1/1 у EDT → required.
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),
        // editType — present 1/1 у EDT → required.
        e(F_EDIT_TYPE, "editType", "InList"),
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer DENSE эмитит всегда). Сверено _generated + корпусом.
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        // standardAttributes — вариативный const-блок (8 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        list(F_BASED_ON, "basedOn"),
        b(F_DISTRIBUTED_INFO_BASE, "distributedInfoBase"),
        b(F_INCLUDE_CONFIGURATION_EXTENSIONS, "includeConfigurationExtensions"),
        // createOnInput — present 1/1 у обоих → required (EDT всегда несёт).
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        // help — EDT-only const-блок; отсутствие = дефолт-омиссия; X-исключён (как Catalog).
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
        b(F_UPDATE_DATA_HISTORY, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing"),
        // content — EDT-only состав; List; отсутствие = дефолт-омиссия; X-исключён.
        list(F_CONTENT, "content").x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (как Catalog, сверено 1/1:
/// Attribute < TabularSection < Form < Template < Command).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "ExchangePlan.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "ExchangePlan.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "ExchangePlan.FormRef" },
    ChildSlot { collection: "Template", child_kind: "ExchangePlan.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "ExchangePlan.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Document-семья
// с ВАРИАТИВНЫМИ `fillChecking` (слот-дефолт DontCheck) И `fullTextSearch` (слот-дефолт
// DontUse) — полный Catalog-паттерн. Прежние name-const гипотезы ОПРОВЕРГНУТЫ
// ERP-корпусом (structural-tail T1): `МиграцияПриложений.Code` несёт
// fillChecking=DontCheck (не ShowError), `ИнтеграцияС1С….ExchangeDate` несёт
// fullTextSearch=Use (не DontUse), `МобильноеПриложениеЗаказыКлиентов.Code` —
// fullTextSearch=DontUse (не Use). Root-only (табличная часть std-attrs не проецирует).
// ============================================================================

const S_SYNONYM: usize = 0;
const S_TOOLTIP: usize = 1;
const S_FORMAT: usize = 2;
const S_EDIT_FORMAT: usize = 3;
const S_FILL_VALUE: usize = 4;
const S_FILL_CHECKING: usize = 5;
const S_FULL_TEXT: usize = 6;

/// 8 предопределённых атрибута КОРНЯ (name-констант больше нет — оба enum-листа
/// вариативны, ERP-witness).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "ExchangeDate", name_consts: &[] },
    AttrDecl { name: "ThisNode", name_consts: &[] },
    AttrDecl { name: "ReceivedNo", name_consts: &[] },
    AttrDecl { name: "SentNo", name_consts: &[] },
    AttrDecl { name: "Ref", name_consts: &[] },
    AttrDecl { name: "DeletionMark", name_consts: &[] },
    AttrDecl { name: "Description", name_consts: &[] },
    AttrDecl { name: "Code", name_consts: &[] },
];

const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Loc,               // format
    VarSlotKind::Loc,               // editFormat
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Enum("DontCheck"), // fillChecking (variable — ERP-witness)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (variable — ERP-witness)
];

const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::ReqValue("fillValue", S_FILL_VALUE),
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::OptLoc("format", S_FORMAT),
    EdtLeaf::OptLoc("editFormat", S_EDIT_FORMAT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::Text("false")),
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
    ("Comment", DenseLeaf::Empty),
    ("FullTextSearch", DenseLeaf::VarEnum(S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::VarValue(S_FILL_VALUE)),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация `standardAttributes` вида `ExchangePlan` (data-driven; Root-only).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "ExchangePlan",
    label: "ExchangePlan",
};

/// Канонический [`EntitySpec`] вида `ExchangePlan` (кэш на процесс).
pub fn exchange_plan() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExchangePlan",
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
/// Категории producedTypes вида `ExchangePlan` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ExchangePlanObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "ExchangePlanRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "ExchangePlanSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "ExchangePlanList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ExchangePlanManager",
        designer_order: 4,
    },
];
