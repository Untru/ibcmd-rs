//! Канонический спек вида объекта `ChartOfCalculationTypes` (план видов расчёта) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate. НЕ-иерархический ссылочный вид,
//! структурно близкий `ChartOfCharacteristicTypes` (CCT) МИНУС иерархия/характеристики,
//! ПЛЮС calc-специфика:
//! * БЕЗ `type`/`characteristicExtValues`/`hierarchical`/`foldersOnTop`/`codeSeries`/
//!   `checkUnique`/`autonumbering`/folder-форм (не-иерархический);
//! * `codeType` (Number|String) — как Catalog;
//! * calc-специфика `dependenceOnCalculationTypes`(enum)/`baseCalculationTypes`(ref-list)/
//!   `actionPeriodUse`(bool);
//! * 11 producedTypes: Object/Ref/Selection/List/Manager + DisplacingCalculationTypes(+Row)/
//!   BaseCalculationTypes(+Row)/LeadingCalculationTypes(+Row).
//!
//! Порядок полей `fields` = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено
//! корпусом coverage). EDT эмитит РАЗРЕЖЁННО. Дети Attribute/TabularSection/Command — как CCT.
//! standardAttributes — ВАРИАДНЫЙ блок корня (7/0; ERP-witnessed) + TS-вариант LineNumber
//! (см. [`STD_ATTRS`]); cf-слоты std-attrs заморожены как envelope-const (поле с дефолтом
//! `[]`). standardTabularSections — ТРИ стандартные ТЧ Leading/Displacing/
//! BaseCalculationTypes (см. [`STD_TABULAR_SECTIONS`]; ERP-witnessed Начисления/Удержания,
//! все три диалекта).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (см. `PRODUCED_CATEGORIES`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, NameConstWhich, Normalize,
    StdAttrsDecl, StdTsDecl, StdTsSection, VarSlotKind,
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
/// `descriptionLength` — int. Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(5);
/// `codeType` — Default Number (Number|String).
pub const F_CODE_TYPE: FieldId = FieldId(6);
/// `codeAllowedLength` — Default Fixed (Fixed|Variable).
pub const F_CODE_ALLOWED_LENGTH: FieldId = FieldId(7);
/// `defaultPresentation` — Default AsCode (AsCode|AsDescription).
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(8);
/// `editType` — Default InList (InList|InDialog|BothWays).
pub const F_EDIT_TYPE: FieldId = FieldId(9);
/// `quickChoice` — Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(10);
/// `choiceMode` — Default FromForm (FromForm|QuickChoice|BothWays).
pub const F_CHOICE_MODE: FieldId = FieldId(11);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(12);
/// `searchStringModeOnInputByString` — Default Begin (Begin|AnyPart).
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(13);
/// `fullTextSearchOnInputByString` — Default Use (Use|DontUse).
pub const F_FTS_ON_INPUT: FieldId = FieldId(14);
/// `choiceDataGetModeOnInputByString` — Default Directly (Directly|Background).
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(15);
/// `createOnInput` — Default Auto (Auto|DontUse|Use).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(16);
/// `choiceHistoryOnInput` — Default Auto (Auto|DontUse).
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(17);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(18);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(19);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(20);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(21);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(22);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(23);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(24);
/// `dependenceOnCalculationTypes` — Default DontUse (DontUse|OnActionPeriod|OnRegistrationPeriod).
pub const F_DEPENDENCE_ON_CALC_TYPES: FieldId = FieldId(25);
/// `baseCalculationTypes` — list-of-refs. Default [].
pub const F_BASE_CALCULATION_TYPES: FieldId = FieldId(26);
/// `actionPeriodUse` — Default false.
pub const F_ACTION_PERIOD_USE: FieldId = FieldId(27);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(28);
/// `predefinedDataUpdate` — Default Auto (Auto|AutoUpdate|DontAutoUpdate).
pub const F_PREDEFINED_DATA_UPDATE: FieldId = FieldId(29);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(30);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(31);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(32);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(33);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(34);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(35);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(36);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(37);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(38);
/// `dataHistory` — Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(39);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(40);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(41);
/// `help` — EDT-only const-блок (presence; Designer держит справку в `Ext/Help.xml`).
/// Default false; X-исключён (`x_ignore`). Витнессится s15_subordinate.
pub const F_HELP: FieldId = FieldId(42);
/// `standardAttributes` — ВАРИАДНЫЙ платформенный блок КОРНЯ (см. [`STD_ATTRS`]; 7
/// атрибутов либо опущен). Дефолт `[]` (cf-каркас держит слот frame-const и поле НЕ
/// проецирует). Физически: EDT МЕЖДУ fullTextSearchOnInputByString и characteristics
/// (witnessed Начисления/Удержания); Designer МЕЖДУ ActionPeriodUse и Characteristics —
/// здесь, в спек-порядке (=Designer DENSE), после actionPeriodUse.
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(43);
/// `standardTabularSections` — ТРИ стандартные ТЧ Leading/Displacing/BaseCalculationTypes,
/// каждая со СВОИМ вложенным std-attrs-блоком (см. [`STD_TABULAR_SECTIONS`]). Default []
/// (coverage/SSL не несут; блок опционален целиком). Позиция DENSE: между
/// `characteristics` и `predefinedDataUpdate` (ERP-witnessed designer, оба объекта);
/// EDT физически — ПОСЛЕ actionPeriodUse, последним свойством перед детьми (witnessed).
/// Кодеки: XML — `Codec::StdTabularSections`; cf — `BraceCodec::StdTabularSections`
/// (b[44], ТЧ-маркеры −30/−20/−10, вложенные атрибут-маркеры [`STS_ATTR_CF_MARKERS`]).
pub const F_STANDARD_TABULAR_SECTIONS: FieldId = FieldId(44);

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
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        e(F_CODE_TYPE, "codeType", "Number"),
        e(F_CODE_ALLOWED_LENGTH, "codeAllowedLength", "Fixed"),
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),
        e(F_EDIT_TYPE, "editType", "InList"),
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
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
        list(F_BASED_ON, "basedOn"),
        e(F_DEPENDENCE_ON_CALC_TYPES, "dependenceOnCalculationTypes", "DontUse"),
        list(F_BASE_CALCULATION_TYPES, "baseCalculationTypes"),
        b(F_ACTION_PERIOD_USE, "actionPeriodUse"),
        // standardAttributes: вариадный платформенный блок корня (7/0), IR-List, дефолт []
        // — cf-проекция вида держит std-attrs-слоты frame-const и поле не проецирует.
        FieldSpec::with_default(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List, PropertyValue::List(Vec::new())),
        list(F_CHARACTERISTICS, "characteristics"),
        // standardTabularSections — 3 стандартные ТЧ (декларация STD_TABULAR_SECTIONS;
        // вложенный std-attrs-блок каждой — STS_STD_ATTRS). Позиция ERP-witnessed.
        list(F_STANDARD_TABULAR_SECTIONS, "standardTabularSections"),
        e(F_PREDEFINED_DATA_UPDATE, "predefinedDataUpdate", "Auto"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        // help: EDT-only presence-блок (физически МЕЖДУ includeHelpInContents и
        // dataLockControlMode, сверено s15). Default false; X-исключён (Designer n/a).
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
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии IR (как CCT/Catalog:
/// Attribute < TabularSection < Form < Template < Command).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "ChartOfCalculationTypes.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "ChartOfCalculationTypes.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "ChartOfCalculationTypes.FormRef" },
    ChildSlot { collection: "Template", child_kind: "ChartOfCalculationTypes.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "ChartOfCalculationTypes.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6): КОРЕНЬ — 7
// атрибутов ref-вида + calc-специфичный ActionPeriodIsBasic (ERP-witnessed
// Начисления/Удержания, edt+designer согласны; fillChecking=ShowError у Description) —
// либо 0-вариант (корпус покрытия); ТАБЛИЧНАЯ часть — LineNumber (по одному блоку на
// каждую из 10 ERP-TS; всё дефолтно, кроме самого присутствия блока).
//
// ВНЕ декларации: `standardTabularSections` (Leading/Displacing/BaseCalculationTypes —
// имя+synonym+3 своих std-attrs, Predefined/CalculationType/LineNumber с
// fillChecking=ShowError у CalculationType) — ОТДЕЛЬНЫЙ субстрат, см.
// [`STD_TABULAR_SECTIONS`] ниже.
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок). synonym/toolTip в корпусе пусты, но задаваемы
// (класс багов «константа вместо переменной») — объявлены слотами.
const S_SA_SYNONYM: usize = 0;
const S_SA_TOOLTIP: usize = 1;
const S_SA_FILL_CHECKING: usize = 2;
const S_SA_FULL_TEXT: usize = 3;

/// 7 предопределённых атрибутов КОРНЯ (ERP-witnessed порядок).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "PredefinedDataName", name_consts: &[] },
    AttrDecl { name: "Predefined", name_consts: &[] },
    AttrDecl { name: "Ref", name_consts: &[] },
    AttrDecl { name: "DeletionMark", name_consts: &[] },
    AttrDecl { name: "ActionPeriodIsBasic", name_consts: &[] },
    AttrDecl { name: "Description", name_consts: &[] },
    AttrDecl { name: "Code", name_consts: &[] },
];

/// Витнессенные наборы корня (длины 7/0 попарно-различны — выбор по счётчику).
const ROOT_VARIANTS: &[&[AttrDecl]] = &[ROOT_ATTRS, &[]];

/// 1 атрибут табличной части (`LineNumber`; witnessed 10/10 ERP-TS).
const TABULAR_ATTRS: &[AttrDecl] = &[AttrDecl { name: "LineNumber", name_consts: &[] }];

/// Variable-слоты (типы/дефолты) в EDT-порядке.
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Enum("DontCheck"), // fillChecking (Description несёт ShowError)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (корпус несёт Use)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом: dataHistory, name, fillValue,
/// [fillChecking], fullTextSearch, minValue, maxValue; synonym/toolTip в корпусе пусты —
/// позиции стандартные, между name и fillValue).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SA_SYNONYM),
    EdtLeaf::OptLoc("toolTip", S_SA_TOOLTIP),
    EdtLeaf::ConstValueUndef { tag: "fillValue" },
    EdtLeaf::OptEnum("fillChecking", S_SA_FILL_CHECKING),
    EdtLeaf::OptEnum("fullTextSearch", S_SA_FULL_TEXT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

/// Designer-DENSE 25 листьев (порядок фикс — канонический регион; сверено ERP
/// designer-дампом 8.3.27: корень 7 × 25 и TS-обёртки 1 × 25).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_SA_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::Text("false")),
    ("CreateOnInput", DenseLeaf::Text("Auto")),
    ("TypeReductionMode", DenseLeaf::Text("TransformValues")),
    ("MaxValue", DenseLeaf::Nil),
    ("ToolTip", DenseLeaf::VarLoc(S_SA_TOOLTIP)),
    ("ExtendedEdit", DenseLeaf::Text("false")),
    ("Format", DenseLeaf::Empty),
    ("ChoiceForm", DenseLeaf::Empty),
    ("QuickChoice", DenseLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DenseLeaf::Text("Auto")),
    ("EditFormat", DenseLeaf::Empty),
    ("PasswordMode", DenseLeaf::Text("false")),
    ("DataHistory", DenseLeaf::Text("Use")),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(S_SA_SYNONYM)),
    ("Comment", DenseLeaf::Empty),
    ("FullTextSearch", DenseLeaf::VarEnum(S_SA_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::Nil),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация `standardAttributes` вида `ChartOfCalculationTypes` (data-driven;
/// корень 7/0 + табличная часть LineNumber).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: &[],
    root_variants: ROOT_VARIANTS,
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "ChartOfCalculationTypes",
    label: "ChartOfCalculationTypes",
};

// ============================================================================
// `standardTabularSections` — ТРИ стандартные ТЧ Leading/Displacing/
// BaseCalculationTypes ([`STD_TABULAR_SECTIONS`]); формы всех трёх диалектов
// ERP-witnessed (Начисления/Удержания, по 3 ТЧ в каждом объекте):
//
// EDT (.mdo, после actionPeriodUse, последним свойством перед <attributes>):
//   <standardTabularSections>
//     <name>LeadingCalculationTypes</name>
//     <synonym><key></key><value>Ведущие виды расчета</value></synonym> ← ПУСТОЙ lang!
//     <standardAttributes>…</standardAttributes> ×3      ← Predefined/CalculationType/
//   </standardTabularSections>                             LineNumber, STS_EDT_LEAVES
//
// Designer (<Properties>, между Characteristics и PredefinedDataUpdate):
//   <StandardTabularSections>
//     <xr:StandardTabularSection name="LeadingCalculationTypes">
//       <xr:Synonym><v8:item><v8:lang/><v8:content>Ведущие виды расчета</…></xr:Synonym>
//       <xr:Comment/> <xr:ToolTip/> <xr:FillChecking>DontCheck</xr:FillChecking>
//       <xr:StandardAttributes>…</xr:StandardAttributes>  ← 3 атрибута ×25 DENSE-листьев
//     </xr:StandardTabularSection>                          (CalculationType несёт
//     … ×3                                                   FillChecking=ShowError)
//   </StandardTabularSections>
//
// cf (body[44]; coverage = {0} — блок опущен):
//   {1, {0, 3, -30, TS_BODY, -20, TS_BODY, -10, TS_BODY}}   ← ТЧ-маркеры-АТОМЫ:
//   TS_BODY = {3, <synonym loc {1,"","…"}>, <comment "">, 0, 0,   Leading=-30,
//              {1, 3, ({marker}, 510405d3-…, BAG25)×3},           Displacing=-20,
//              {0}}                                               Base=-10
//   BAG25 == общий catalog-family шаблон (`ir_std_attrs_bag.brace`); witnessed
//   переменные: fillChecking=1 (ShowError) у CalculationType, fullTextSearch=1 (Use)
//   у всех, trm=0 у всех; прочие 25-проп значения == шаблону (сверено 6 ТЧ × 3 attr).
//
// Грамматика ВЛОЖЕННОГО std-attrs-блока ТЧ — платформенная catalog-family запись
// (11 variable-слотов) — ИДЕНТИЧНА CoA-STS (kind-агностична; сверено designer-дампом
// 8.3.27 обоих видов лист-в-лист). Таблицы ниже — зеркало
// `chart_of_accounts::{VAR_SLOTS,EDT_LEAVES,DENSE_LEAVES}` (данные вида — в его
// спек-файле, parallel-safe fan-out).
// ============================================================================

// Variable-слоты записи ВЛОЖЕННОГО std-attrs-блока ТЧ (EDT-порядок; catalog-family).
const STS_S_SYNONYM: usize = 0;
const STS_S_COMMENT: usize = 1;
const STS_S_TOOLTIP: usize = 2;
const STS_S_FILL_FROM: usize = 3;
const STS_S_FILL_VALUE: usize = 4;
const STS_S_CPL: usize = 5;
const STS_S_FILL_CHECKING: usize = 6;
const STS_S_FULL_TEXT: usize = 7;
const STS_S_CHOICE_PARAMS: usize = 8;
const STS_S_MASK: usize = 9;
const STS_S_CHOICE_FORM: usize = 10;

/// 3 предопределённых атрибута КАЖДОЙ стандартной ТЧ в каноническом порядке
/// (ERP-witnessed edt+designer+cf, оба объекта × 3 ТЧ). `CalculationType` несёт
/// fillChecking=ShowError КАК ДАННЫЕ (variable-слот), не name-константу.
const STS_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Predefined", name_consts: &["TransformValues"] },
    AttrDecl { name: "CalculationType", name_consts: &["TransformValues"] },
    AttrDecl { name: "LineNumber", name_consts: &["TransformValues"] },
];

/// cf-маркеры атрибутов вложенного std-attrs-блока ТЧ (erp.cf b[44], оба объекта,
/// все 3 ТЧ — набор ТЧ-инвариантен).
pub const STS_ATTR_CF_MARKERS: &[(&str, i64)] = &[
    ("Predefined", -102),
    ("CalculationType", -101),
    ("LineNumber", -100),
];

/// Variable-слоты вложенного блока (типы/дефолты) в EDT-порядке — catalog-family
/// 11-слотовая запись (зеркало CoA-STS; fullTextSearch witnessed Use 18/18).
const STS_VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Str,               // comment
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Bool,              // fillFromFillingValue
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Cpl,               // choiceParameterLinks
    VarSlotKind::Enum("DontCheck"), // fillChecking (CalculationType несёт ShowError)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (Use witnessed 18/18)
    VarSlotKind::Cp,                // choiceParameters
    VarSlotKind::Str,               // mask
    VarSlotKind::Str,               // choiceForm
];

/// EDT-листья вложенного блока (позиционный порядок == CoA-STS; ERP-witnessed:
/// dataHistory/name/fillValue/[fillChecking]/fullTextSearch/minValue/maxValue).
const STS_EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", STS_S_SYNONYM),
    EdtLeaf::OptStr("comment", STS_S_COMMENT),
    EdtLeaf::OptLoc("toolTip", STS_S_TOOLTIP),
    EdtLeaf::OptBool("fillFromFillingValue", STS_S_FILL_FROM),
    EdtLeaf::ReqValue("fillValue", STS_S_FILL_VALUE),
    // fillChecking ДО choiceParameterLinks (корпус-witnessed, см. catalog.rs).
    EdtLeaf::OptEnum("fillChecking", STS_S_FILL_CHECKING),
    EdtLeaf::OptCpl("choiceParameterLinks", STS_S_CPL),
    EdtLeaf::OptCp("choiceParameters", STS_S_CHOICE_PARAMS),
    EdtLeaf::OptEnum("fullTextSearch", STS_S_FULL_TEXT),
    EdtLeaf::OptStr("mask", STS_S_MASK),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
    EdtLeaf::OptStr("choiceForm", STS_S_CHOICE_FORM),
];

/// Designer-DENSE 25 листьев вложенного блока (порядок фикс == CoA-STS; ERP-witnessed
/// designer 8.3.27: 3 атрибута × 3 ТЧ × 2 объекта byte-порядком).
const STS_DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(STS_S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::VarBool(STS_S_FILL_FROM)),
    ("CreateOnInput", DenseLeaf::Text("Auto")),
    ("TypeReductionMode", DenseLeaf::VarNameConst(NameConstWhich::C0)),
    ("MaxValue", DenseLeaf::Nil),
    ("ToolTip", DenseLeaf::VarLoc(STS_S_TOOLTIP)),
    ("ExtendedEdit", DenseLeaf::Text("false")),
    ("Format", DenseLeaf::Empty),
    ("ChoiceForm", DenseLeaf::VarStr(STS_S_CHOICE_FORM)),
    ("QuickChoice", DenseLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DenseLeaf::Text("Auto")),
    ("EditFormat", DenseLeaf::Empty),
    ("PasswordMode", DenseLeaf::Text("false")),
    ("DataHistory", DenseLeaf::Text("Use")),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(STS_S_SYNONYM)),
    ("Comment", DenseLeaf::VarStr(STS_S_COMMENT)),
    ("FullTextSearch", DenseLeaf::VarEnum(STS_S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::VarCpl(STS_S_CPL)),
    ("FillValue", DenseLeaf::VarValue(STS_S_FILL_VALUE)),
    ("Mask", DenseLeaf::VarStr(STS_S_MASK)),
    ("ChoiceParameters", DenseLeaf::VarCp(STS_S_CHOICE_PARAMS)),
];

/// Декларация ВЛОЖЕННОГО std-attrs-блока стандартной ТЧ (ОДНА на все 3 ТЧ — набор
/// атрибутов ТЧ-инвариантен, witnessed 6/6; `root_elem` вложенным блоком не
/// используется — обёртку даёт STS-кодек `formats_xml::std_tabular_sections`).
pub const STS_STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: STS_ATTRS,
    root_variants: &[],
    tabular_attrs: &[],
    var_slots: STS_VAR_SLOTS,
    edt_leaves: STS_EDT_LEAVES,
    dense_leaves: STS_DENSE_LEAVES,
    root_elem: "ChartOfCalculationTypes",
    label: "ChartOfCalculationTypes.StandardTabularSection",
};

/// Декларация `standardTabularSections` вида `ChartOfCalculationTypes`: три
/// стандартные ТЧ в каноническом порядке (= EDT = Designer = cf-порядок маркеров
/// −30/−20/−10; ERP-witnessed оба объекта). cf-маркеры ТЧ — у cf-кодека
/// (`formats_brace::std_tabular_sections`).
pub const STD_TABULAR_SECTIONS: StdTsDecl = StdTsDecl {
    sections: &[
        StdTsSection { name: "LeadingCalculationTypes", attrs: &STS_STD_ATTRS },
        StdTsSection { name: "DisplacingCalculationTypes", attrs: &STS_STD_ATTRS },
        StdTsSection { name: "BaseCalculationTypes", attrs: &STS_STD_ATTRS },
    ],
    root_elem: "ChartOfCalculationTypes",
    label: "ChartOfCalculationTypes",
};

/// Канонический [`EntitySpec`] вида `ChartOfCalculationTypes` (кэш на процесс).
pub fn chart_of_calculation_types() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCalculationTypes",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок = cf-DISK-порядок). §1.6/§3.5.
// 11 категорий (5 стандартных + 6 calc-специфичных, в EDT-порядке .mdo).
// ============================================================================
/// Категории producedTypes вида `ChartOfCalculationTypes` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ChartOfCalculationTypesObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "ChartOfCalculationTypesRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "ChartOfCalculationTypesSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "ChartOfCalculationTypesList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ChartOfCalculationTypesManager",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "DisplacingCalculationTypes",
        edt_tag: "displacingCalculationTypesType",
        designer_category: "DisplacingCalculationTypes",
        designer_type_name: "DisplacingCalculationTypes",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "DisplacingCalculationTypesRow",
        edt_tag: "displacingCalculationTypesRowType",
        designer_category: "DisplacingCalculationTypesRow",
        designer_type_name: "DisplacingCalculationTypesRow",
        designer_order: 6,
    },
    crate::spec::common::ProducedCategory {
        category: "BaseCalculationTypes",
        edt_tag: "baseCalculationTypesType",
        designer_category: "BaseCalculationTypes",
        designer_type_name: "BaseCalculationTypes",
        designer_order: 7,
    },
    crate::spec::common::ProducedCategory {
        category: "BaseCalculationTypesRow",
        edt_tag: "baseCalculationTypesRowType",
        designer_category: "BaseCalculationTypesRow",
        designer_type_name: "BaseCalculationTypesRow",
        designer_order: 8,
    },
    crate::spec::common::ProducedCategory {
        category: "LeadingCalculationTypes",
        edt_tag: "leadingCalculationTypesType",
        designer_category: "LeadingCalculationTypes",
        designer_type_name: "LeadingCalculationTypes",
        designer_order: 9,
    },
    crate::spec::common::ProducedCategory {
        category: "LeadingCalculationTypesRow",
        edt_tag: "leadingCalculationTypesRowType",
        designer_category: "LeadingCalculationTypesRow",
        designer_type_name: "LeadingCalculationTypesRow",
        designer_order: 10,
    },
];
