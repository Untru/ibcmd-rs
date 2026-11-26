//! Канонический спек вида объекта `ChartOfAccounts` (План счетов) — ARCHITECTURE.md §1.4/§1.6,
//! child-objects substrate. Ссылочно-порождающий вид (как `ChartOfCharacteristicTypes`), НО с
//! бухгалтерской спецификой: `basedOn`/`extDimensionTypes`(ref)/`maxExtDimensionCount`/`codeMask`/
//! `autoOrderByCode`/`orderLength`/`codeSeries`(WholeChartOfAccounts). 7 producedTypes
//! (Object/Ref/Selection/List/Manager + ExtDimensionTypes/ExtDimensionTypesRow). БЕЗ иерархии
//! (нет hierarchical/foldersOnTop/autonumbering/codeAllowedLength/defaultFolderForm).
//!
//! `standardAttributes` — СВОЙ 10-атрибутный блок (декларация [`STD_ATTRS`] ниже); coverage
//! его не кастомизирует (блок опущен, count=0), ERP несёт полностью (оба объекта
//! Хозрасчетный/Международный ×10 блоков, witnessed edt+designer+cf).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено фикстурой
//! coverage/designer/s2_registers/ChartsOfAccounts + ERP designer_8.3.27). EDT эмитит
//! РАЗРЕЖЁННО; дефолты выведены corpus-wide (49 объектов × edt+designer + cf-RE эталона
//! s2_registers.cf; ERP-корпус добавил witnessed-позиции extDimensionTypes-блока).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (7 категорий, см. `PRODUCED_CATEGORIES`). EDT-only `help`+`predefined` — X-исключены.
//! Дочерние коллекции AccountingFlag / ExtDimensionAccountingFlag — АКТИВНЫЕ child-слоты
//! (ERP-witnessed: Международный 4+2, Хозрасчетный 5+3); ExtDimensionType (вложенный вид
//! субконто) остаётся схемной заготовкой.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, DenseLeafRule, EdtLeaf, EntitySpec, FieldSpec,
    NameConstWhich, Normalize, StdAttrsDecl, StdTsDecl, StdTsSection, VarSlotKind,
};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(4);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(5);
/// `extDimensionTypes` — ссылка на План видов характеристик (Str-ref). Default "".
pub const F_EXT_DIMENSION_TYPES: FieldId = FieldId(6);
/// `maxExtDimensionCount` — int. Default 0.
pub const F_MAX_EXT_DIMENSION_COUNT: FieldId = FieldId(7);
/// `codeMask` — маска кода (Str). Default "".
pub const F_CODE_MASK: FieldId = FieldId(8);
/// `codeLength` — int. Default 0.
pub const F_CODE_LENGTH: FieldId = FieldId(9);
/// `descriptionLength` — int. Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(10);
/// `codeSeries` — Default WholeChartOfAccounts.
pub const F_CODE_SERIES: FieldId = FieldId(11);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(12);
/// `defaultPresentation` — Default AsCode.
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(13);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(14);
/// `predefinedDataUpdate` — Default Auto.
pub const F_PREDEFINED_DATA_UPDATE: FieldId = FieldId(15);
/// `editType` — Default InList.
pub const F_EDIT_TYPE: FieldId = FieldId(16);
/// `quickChoice` — Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(17);
/// `choiceMode` — Default FromForm.
pub const F_CHOICE_MODE: FieldId = FieldId(18);
/// `inputByString` — list-of-refs (field-style). Default [] (coverage несёт [Description,Code]).
pub const F_INPUT_BY_STRING: FieldId = FieldId(19);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(20);
/// `fullTextSearchOnInputByString` — Default Use.
pub const F_FTS_ON_INPUT: FieldId = FieldId(21);
/// `choiceDataGetModeOnInputByString` — Default Directly.
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(22);
/// `createOnInput` — Default Auto.
pub const F_CREATE_ON_INPUT: FieldId = FieldId(23);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(24);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(25);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(26);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(27);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(28);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(29);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(30);
/// `autoOrderByCode` — Default false.
pub const F_AUTO_ORDER_BY_CODE: FieldId = FieldId(31);
/// `orderLength` — int. Default 0.
pub const F_ORDER_LENGTH: FieldId = FieldId(32);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(33);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(34);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(35);
/// `dataHistory` — Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(36);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(37);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(38);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(39);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(40);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(41);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(42);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(43);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(44);
/// `predefined` — EDT-only данные (List); отсутствие = дефолт-омиссия; X-исключён.
pub const F_PREDEFINED: FieldId = FieldId(45);
/// `standardAttributes` — вариативный платформенный блок (10 атрибутов, [`STD_ATTRS`]).
/// Default [] (отсутствие блока = count 0 = пустой List; в ОТЛИЧИЕ от Catalog поле НЕ
/// required — у cf-проекции CoA пока нет кодека слота b[38], required уронил бы cf-read
/// MissingRequired'ом на каждом объекте). Позиция DENSE: между `defaultPresentation` и
/// `characteristics` (ERP-witnessed designer).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(46);
/// `standardTabularSections` — стандартная табличная часть `ExtDimensionTypes` (Виды
/// субконто) со СВОИМ вложенным std-attrs-блоком ([`STS_STD_ATTRS`]). Default [] (SSL/
/// coverage не несут). Позиция DENSE: между `characteristics` и `predefinedDataUpdate`
/// (ERP-witnessed designer). Кодеки: XML — `Codec::StdTabularSections` c декларацией
/// [`STD_TABULAR_SECTIONS`]; cf — `BraceCodec::StdTabularSections` (b[39], ТЧ-маркер
/// −12, вложенные атрибут-маркеры [`STS_ATTR_CF_MARKERS`]). IR-схема записи-ТЧ —
/// см. `spec::common::StdTsDecl`.
pub const F_STANDARD_TABULAR_SECTIONS: FieldId = FieldId(47);

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
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        list(F_BASED_ON, "basedOn"),
        s(F_EXT_DIMENSION_TYPES, "extDimensionTypes"),
        i(F_MAX_EXT_DIMENSION_COUNT, "maxExtDimensionCount", 0),
        s(F_CODE_MASK, "codeMask"),
        i(F_CODE_LENGTH, "codeLength", 0),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        e(F_CODE_SERIES, "codeSeries", "WholeChartOfAccounts"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),
        // standardAttributes — вариативный блок (10 атрибутов); Default [] (см. док у
        // F_STANDARD_ATTRIBUTES). Позиция ERP-witnessed (designer DENSE).
        list(F_STANDARD_ATTRIBUTES, "standardAttributes"),
        list(F_CHARACTERISTICS, "characteristics"),
        // standardTabularSections — стандартная ТЧ ExtDimensionTypes (декларация
        // STD_TABULAR_SECTIONS; вложенный std-attrs-блок — STS_STD_ATTRS).
        list(F_STANDARD_TABULAR_SECTIONS, "standardTabularSections"),
        e(F_PREDEFINED_DATA_UPDATE, "predefinedDataUpdate", "Auto"),
        e(F_EDIT_TYPE, "editType", "InList"),
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        // fullTextSearchOnInputByString — default Use (EDT sparse опускает Use; coverage несёт DontUse).
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        b(F_AUTO_ORDER_BY_CODE, "autoOrderByCode"),
        i(F_ORDER_LENGTH, "orderLength", 0),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
        b(F_UPDATE_DATA_HISTORY, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing"),
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        list(F_PREDEFINED, "predefined").x_ignored(),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (Attribute < TabularSection <
/// AccountingFlag < ExtDimensionAccountingFlag < Form < Template < Command).
///
/// Порядок = EDT xcore-метамодели (`inventory/metamodel.jsonl`: attributes, tabularSections,
/// accountingFlags, extDimensionAccountingFlags, forms, templates, commands) и подтверждён
/// ERP-witnessed обоими XML-диалектами (edt Международный: attributes → accountingFlags →
/// extDimensionAccountingFlags → forms; designer ChildObjects — тот же порядок).
///
/// `AccountingFlag`/`ExtDimensionAccountingFlag` — ПРЯМЫЕ дети корня (ERP-witnessed:
/// Международный 4+2, Хозрасчетный 5+3); их спеки — `chart_of_accounts_accounting_flag`/
/// `chart_of_accounts_ext_dimension_accounting_flag` (attribute-подобные, БЕЗ
/// indexing/fullTextSearch). Вложенный `ExtDimensionType` (вид субконто) остаётся
/// схемной заготовкой (не подключён; cf-коллекция `4c7fec95-…` empty-only).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "ChartOfAccounts.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "ChartOfAccounts.TabularSection" },
    ChildSlot { collection: "AccountingFlag", child_kind: "ChartOfAccounts.AccountingFlag" },
    ChildSlot {
        collection: "ExtDimensionAccountingFlag",
        child_kind: "ChartOfAccounts.ExtDimensionAccountingFlag",
    },
    ChildSlot { collection: "Form", child_kind: "ChartOfAccounts.FormRef" },
    ChildSlot { collection: "Template", child_kind: "ChartOfAccounts.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "ChartOfAccounts.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Читается
// обобщённым кодеком `formats_xml::std_attrs_generic` в ОБА XML-формата.
//
// ERP-witnessed (Хозрасчетный + Международный, edt+designer+cf b[38]):
// 10 атрибутов в каноническом порядке (= EDT-порядок блоков = designer-порядок
// <xr:StandardAttribute> = cf-порядок маркеров). Все typeReductionMode =
// TransformValues (cf trm=0). Задаваемые листья, witnessed по ERP CoA: toolTip
// (OffBalance ×2 объекта), synonym (Description/Code Хозрасчетный), fillChecking
// (Type/Description/Code = ShowError), fillFromFillingValue (Type/Parent),
// fillValue (Parent = ReferenceValue EmptyRef; Type Хозрасчетный = AccountType
// ActivePassive — тип пока НЕ поддержан value-кодеками, см. отчёт), mask (Code
// Хозрасчетный "@@@.@@.@"), choiceParameters (Parent Хозрасчетный ×1),
// fullTextSearch (= Use во ВСЕХ 20 witnessed блоках; слот variable, дефолт DontUse).
//
// cf-сторона: та же catalog-family машинерия (`formats_brace::std_attrs_catalog`,
// блок {1,{1,10,(marker,510405d3-…,BAG25)×10}} в body[38]) — нужен НОВЫЙ
// Variant::RootChartOfAccounts с маркерами из [`ROOT_ATTR_CF_MARKERS`] (файл
// std_attrs_catalog.rs правит владелец машинерии, см. отчёт).
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок; зеркало Catalog).
const S_SYNONYM: usize = 0;
const S_COMMENT: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FILL_FROM: usize = 3;
const S_FILL_VALUE: usize = 4;
const S_CPL: usize = 5;
const S_FILL_CHECKING: usize = 6;
const S_FULL_TEXT: usize = 7;
const S_CHOICE_PARAMS: usize = 8;
const S_MASK: usize = 9;
const S_CHOICE_FORM: usize = 10;
// dataHistory std-attr: 4-й случай «const-modeled settable» — ERP несёт DontUse у
// Description (КлассификаторДОПОГЭПД, мирСкважины; witnessed cf prop 9288a8ed…: Use=1,
// DontUse=0). EDT: лист ОПУЩЕН ⟺ DontUse (дефолт слота), явный `Use` — прежние байты.
const S_DATA_HISTORY: usize = 11;

/// 10 предопределённых атрибутов КОРНЯ ChartOfAccounts в каноническом порядке
/// (ERP-witnessed edt-блоками обоих объектов; порядок cf-маркеров тот же).
/// `name_consts[0]` = TypeReductionMode (у CoA ВСЕГДА TransformValues; сверено
/// designer 10/10 ×2 объекта + cf trm=0 10/10 ×2).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "PredefinedDataName", name_consts: &["TransformValues"] },
    AttrDecl { name: "Order", name_consts: &["TransformValues"] },
    AttrDecl { name: "OffBalance", name_consts: &["TransformValues"] },
    AttrDecl { name: "Type", name_consts: &["TransformValues"] },
    AttrDecl { name: "Description", name_consts: &["TransformValues"] },
    AttrDecl { name: "Code", name_consts: &["TransformValues"] },
    AttrDecl { name: "Parent", name_consts: &["TransformValues"] },
    AttrDecl { name: "Predefined", name_consts: &["TransformValues"] },
    AttrDecl { name: "DeletionMark", name_consts: &["TransformValues"] },
    AttrDecl { name: "Ref", name_consts: &["TransformValues"] },
];

/// cf-маркеры атрибутов корня (erp.cf b[38], оба объекта; порядок == [`ROOT_ATTRS`]).
/// ДАННЫЕ ДЛЯ `formats_brace::std_attrs_catalog::Variant::RootChartOfAccounts` (файл —
/// чужая зона; таблица здесь как witnessed-источник, см. отчёт). trm везде 0.
pub const ROOT_ATTR_CF_MARKERS: &[(&str, i64)] = &[
    ("PredefinedDataName", -28),
    ("Order", -17),
    ("OffBalance", -11),
    ("Type", -10),
    ("Description", -8),
    ("Code", -7),
    ("Parent", -6),
    ("Predefined", -5),
    ("DeletionMark", -4),
    ("Ref", -2),
];

/// 1 предопределённый атрибут табличной части (`LineNumber`) — как у Catalog/CCT
/// (ТЧ планов счетов в корпусе body-free; путь не витнессится, набор платформенный).
const TABULAR_ATTRS: &[AttrDecl] =
    &[AttrDecl { name: "LineNumber", name_consts: &["TransformValues"] }];

/// Variable-слоты (типы/дефолты) в EDT-порядке — зеркало Catalog (та же 25-проп
/// bag-структура в cf; ERP-witnessed).
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Str,               // comment
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Bool,              // fillFromFillingValue
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Cpl,               // choiceParameterLinks
    VarSlotKind::Enum("DontCheck"), // fillChecking (variable; ShowError witnessed)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (variable; Use witnessed 20/20)
    VarSlotKind::Cp,                // choiceParameters (ERP-witnessed, Parent)
    VarSlotKind::Str,               // mask (ERP-witnessed, Code "@@@.@@.@")
    VarSlotKind::Str,               // choiceForm (не витнессирован на CoA; слот единообразен)
    VarSlotKind::Enum("DontUse"), // dataHistory (ERP-witnessed переменная)
];

/// EDT-листья блока (позиционный порядок == Catalog; ERP-witnessed на CoA:
/// synonym/toolTip/fillFromFillingValue/fillValue/choiceParameters/fillChecking/
/// fullTextSearch/mask — в тех же позициях, что у Catalog).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::OptEnum("dataHistory", S_DATA_HISTORY),
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptStr("comment", S_COMMENT),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::OptBool("fillFromFillingValue", S_FILL_FROM),
    EdtLeaf::ReqValue("fillValue", S_FILL_VALUE),
    // fillChecking ДО choiceParameterLinks — ERP-witness Catalog.УпаковкиЕдиницыИзмерения
    // (Parent: fillValue→fillChecking→CPL×2→CP×2→fullTextSearch); корпус-переписью 13/13
    // co-occurrence подтверждён, обратного порядка нет.
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptCpl("choiceParameterLinks", S_CPL),
    EdtLeaf::OptCp("choiceParameters", S_CHOICE_PARAMS),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::OptStr("mask", S_MASK),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
    EdtLeaf::OptStr("choiceForm", S_CHOICE_FORM),
];

/// Designer-DENSE 25 листьев (порядок фикс; == Catalog; ERP-witnessed на CoA 10/10
/// атрибутов ×2 объекта byte-порядком).
const DENSE_LEAVES: &[DenseLeafRule] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::VarBool(S_FILL_FROM)),
    ("CreateOnInput", DenseLeaf::Text("Auto")),
    ("TypeReductionMode", DenseLeaf::VarNameConst(NameConstWhich::C0)),
    ("MaxValue", DenseLeaf::Nil),
    ("ToolTip", DenseLeaf::VarLoc(S_TOOLTIP)),
    ("ExtendedEdit", DenseLeaf::Text("false")),
    ("Format", DenseLeaf::Empty),
    ("ChoiceForm", DenseLeaf::VarStr(S_CHOICE_FORM)),
    ("QuickChoice", DenseLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DenseLeaf::Text("Auto")),
    ("EditFormat", DenseLeaf::Empty),
    ("PasswordMode", DenseLeaf::Text("false")),
    ("DataHistory", DenseLeaf::VarEnum(S_DATA_HISTORY)),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(S_SYNONYM)),
    ("Comment", DenseLeaf::VarStr(S_COMMENT)),
    ("FullTextSearch", DenseLeaf::VarEnum(S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::VarCpl(S_CPL)),
    ("FillValue", DenseLeaf::VarValue(S_FILL_VALUE)),
    ("Mask", DenseLeaf::VarStr(S_MASK)),
    ("ChoiceParameters", DenseLeaf::VarCp(S_CHOICE_PARAMS)),
];

/// Декларация `standardAttributes` вида `ChartOfAccounts` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — при наличии блока все 10 атрибутов присутствуют
    // (ERP-witnessed 10/10 ×2; отсутствие блока целиком = count 0 — общая ветка кодека).
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "ChartOfAccounts",
    label: "ChartOfAccounts",
};

// ============================================================================
// `standardTabularSections` — субстрат стандартных ТЧ ([`STD_TABULAR_SECTIONS`]);
// формы всех трёх диалектов ERP-witnessed, оба объекта (Хозрасчетный/Международный):
//
// EDT (.mdo, inline после checkUnique/characteristics, перед predefined):
//   <standardTabularSections>
//     <name>ExtDimensionTypes</name>
//     <synonym><key></key><value>Виды субконто</value></synonym>   ← ПУСТОЙ lang!
//     <standardAttributes>…</standardAttributes> ×4                ← вложенный блок,
//   </standardTabularSections>                                       грамматика EDT_LEAVES
//
// Designer (<Properties>, между Characteristics и PredefinedDataUpdate):
//   <StandardTabularSections>
//     <xr:StandardTabularSection name="ExtDimensionTypes">
//       <xr:Synonym><v8:item><v8:lang/><v8:content>Виды субконто</...></xr:Synonym>
//       <xr:Comment/> <xr:ToolTip/> <xr:FillChecking>DontCheck</xr:FillChecking>
//       <xr:StandardAttributes>…</xr:StandardAttributes>            ← 4 атрибута,
//     </xr:StandardTabularSection>                                    DENSE_LEAVES ×25
//   </StandardTabularSections>
//
// cf (body[39]; coverage = {0} — блок опущен):
//   {1, {0, 1, -12, STS_BODY}}                       ← count 1, маркер ТЧ = -12 (АТОМ)
//   STS_BODY = {3, <synonym loc {1,"","Виды субконто"}>, <comment "">, 0, 0,
//               {1, 4, ({marker}, 510405d3-…, BAG25)×4},  ← вложенный std-attrs INNER
//               {0}}                                        (BAG25 == шаблон b[38])
//   слоты [3]/[4] (toolTip/fillChecking) witnessed только 0 (пусто/DontCheck).
// ============================================================================

/// 4 предопределённых атрибута стандартной ТЧ `ExtDimensionTypes` в каноническом
/// порядке (ERP-witnessed edt+designer+cf, оба объекта). `ExtDimensionType` несёт
/// fillChecking=ShowError КАК ДАННЫЕ (variable-слот), не name-константу.
const STS_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "TurnoversOnly", name_consts: &["TransformValues"] },
    AttrDecl { name: "Predefined", name_consts: &["TransformValues"] },
    AttrDecl { name: "ExtDimensionType", name_consts: &["TransformValues"] },
    AttrDecl { name: "LineNumber", name_consts: &["TransformValues"] },
];

/// cf-маркеры атрибутов вложенного std-attrs-блока STS (erp.cf b[39], оба объекта).
pub const STS_ATTR_CF_MARKERS: &[(&str, i64)] = &[
    ("TurnoversOnly", -15),
    ("Predefined", -14),
    ("ExtDimensionType", -13),
    ("LineNumber", -12),
];

/// Декларация ВЛОЖЕННОГО std-attrs-блока стандартной ТЧ `ExtDimensionTypes`
/// (грамматика листьев идентична корневой; `root_elem` для вложенного блока не
/// используется — обёртку даёт сам STS-кодек `formats_xml::std_tabular_sections`).
pub const STS_STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: STS_ATTRS,
    root_variants: &[],
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "ChartOfAccounts",
    label: "ChartOfAccounts.StandardTabularSection",
};

/// Декларация `standardTabularSections` вида `ChartOfAccounts`: единственная
/// стандартная ТЧ `ExtDimensionTypes` («Виды субконто»; ERP-witnessed оба объекта,
/// все три диалекта — раскладки выше). cf-маркер ТЧ −12 — у cf-кодека
/// (`formats_brace::std_tabular_sections`).
pub const STD_TABULAR_SECTIONS: StdTsDecl = StdTsDecl {
    sections: &[StdTsSection { name: "ExtDimensionTypes", attrs: &STS_STD_ATTRS }],
    root_elem: "ChartOfAccounts",
    label: "ChartOfAccounts",
};

/// Канонический [`EntitySpec`] вида `ChartOfAccounts` (кэш на процесс).
pub fn chart_of_accounts() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccounts",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок, = cf-DISK-порядок; сверено RE 49/49).
// ============================================================================
/// Категории producedTypes вида `ChartOfAccounts` в каноническом порядке IR (= EDT = cf-disk).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ChartOfAccountsObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "ChartOfAccountsRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "ChartOfAccountsSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "ChartOfAccountsList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ChartOfAccountsManager",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "ExtDimensionTypes",
        edt_tag: "extDimensionTypes",
        designer_category: "ExtDimensionTypes",
        designer_type_name: "ChartOfAccountsExtDimensionTypes",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "ExtDimensionTypesRow",
        edt_tag: "extDimensionTypesRow",
        designer_category: "ExtDimensionTypesRow",
        designer_type_name: "ChartOfAccountsExtDimensionTypesRow",
        designer_order: 6,
    },
];
