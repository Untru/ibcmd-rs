//! Канонический спек вида объекта `ChartOfCharacteristicTypes` (план видов характеристик)
//! — ARCHITECTURE.md §1.4/§1.6, child-objects substrate. ПЯТЫЙ СТРУКТУРНЫЙ вид (после
//! InformationRegister, Catalog, Document, Task): переиспользует ~90% субстрата Catalog
//! (`ir_child` base-реквизит для детей, Command/FormRef/TemplateRef-паттерн, children-
//! рекурсию, value/type/cpl/ref-list/characteristics/help/producedTypes-кодеки,
//! TabularSection recursion, EDT-only predefined) + ChartOfCharacteristicTypes-дельту:
//! * КОРНЕВОЕ поле `type` — TypeSet значений характеристик (root Type, codec `Type`);
//! * `characteristicExtValues` — ссылка на каталог доп. значений (Str-ref);
//! * std_attrs_chart_of_characteristic_types — 9-атрибутный набор
//!   `PredefinedDataName/ValueType/Description/Code/IsFolder/Parent/Predefined/
//!   DeletionMark/Ref` (variable-поля 1:1 как у Catalog: synonym/comment/toolTip/
//!   fillFromFillingValue/fillValue/choiceParameterLinks/fillChecking/fullTextSearch);
//! * БЕЗ Catalog-специфичных `owners`/`subordinationUse`/`hierarchyType`/`limitLevelCount`/
//!   `levelCount` (план видов характеристик их не несёт).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено 4/4).
//! EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссий (сверено
//! corpus-wide).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (6 категорий incl. Characteristic, см. `produced_types::categories_for`). EDT-only
//! `help`+`predefined` — X-исключены (`x_ignore`), но участвуют в R EDT.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, NameConstWhich, Normalize,
    StdAttrsDecl, VarSlotKind,
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
/// `characteristicExtValues` — ссылка на каталог доп. значений (Str-ref). Default "".
pub const F_CHARACTERISTIC_EXT_VALUES: FieldId = FieldId(5);
/// `type` — TypeSet значений характеристик (root Type). Required (всегда present).
pub const F_TYPE: FieldId = FieldId(6);
/// `hierarchical` — Default false.
pub const F_HIERARCHICAL: FieldId = FieldId(7);
/// `foldersOnTop` — Default false.
pub const F_FOLDERS_ON_TOP: FieldId = FieldId(8);
/// `codeLength` — int. Default 0.
pub const F_CODE_LENGTH: FieldId = FieldId(9);
/// `codeAllowedLength` — Default Fixed (EDT омитит при Fixed; Variable эмитится).
pub const F_CODE_ALLOWED_LENGTH: FieldId = FieldId(10);
/// `descriptionLength` — int. Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(11);
/// `codeSeries` — Designer-only DENSE. Default WholeCharacteristicKind.
pub const F_CODE_SERIES: FieldId = FieldId(12);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(13);
/// `autonumbering` — Default false.
pub const F_AUTONUMBERING: FieldId = FieldId(14);
/// `defaultPresentation` — present 4/4 у EDT → required.
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(15);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (9 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(16);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(17);
/// `predefinedDataUpdate` — Default Auto.
pub const F_PREDEFINED_DATA_UPDATE: FieldId = FieldId(18);
/// `editType` — present 4/4 у EDT → required.
pub const F_EDIT_TYPE: FieldId = FieldId(19);
/// `quickChoice` — Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(20);
/// `choiceMode` — Default FromForm.
pub const F_CHOICE_MODE: FieldId = FieldId(21);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(22);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(23);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(24);
/// `choiceDataGetModeOnInputByString` — Designer-only DENSE. Default Directly.
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(25);
/// `fullTextSearchOnInputByString` — present у EDT даже при DontUse → required.
pub const F_FTS_ON_INPUT: FieldId = FieldId(26);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(27);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(28);
/// `defaultFolderForm` — form-ref. Default "".
pub const F_DEFAULT_FOLDER_FORM: FieldId = FieldId(29);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(30);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(31);
/// `defaultFolderChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_FOLDER_CHOICE_FORM: FieldId = FieldId(32);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(33);
/// `auxiliaryFolderForm` — Designer-only. Default "".
pub const F_AUX_FOLDER_FORM: FieldId = FieldId(34);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(35);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(36);
/// `auxiliaryFolderChoiceForm` — Designer-only. Default "".
pub const F_AUX_FOLDER_CHOICE_FORM: FieldId = FieldId(37);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(38);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(39);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(40);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(41);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(42);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(43);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(44);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(45);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(46);
/// `dataHistory` — Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(47);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(48);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(49);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(50);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(52);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(53);
/// `predefined` — EDT-only данные (List); отсутствие = дефолт-омиссия; X-исключён.
pub const F_PREDEFINED: FieldId = FieldId(51);

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
        s(F_CHARACTERISTIC_EXT_VALUES, "characteristicExtValues"),
        // type — корневой TypeSet значений характеристик; present 4/4 у обоих (план без
        // типа не бывает) → required (EDT всегда несёт, без дефолт-омиссии).
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        b(F_HIERARCHICAL, "hierarchical"),
        b(F_FOLDERS_ON_TOP, "foldersOnTop"),
        i(F_CODE_LENGTH, "codeLength", 0),
        e(F_CODE_ALLOWED_LENGTH, "codeAllowedLength", "Fixed"),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        e(F_CODE_SERIES, "codeSeries", "WholeCharacteristicKind"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        b(F_AUTONUMBERING, "autonumbering"),
        // defaultPresentation — present 4/4 у EDT → required.
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),
        // standardAttributes — вариативный const-блок (9 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        e(F_PREDEFINED_DATA_UPDATE, "predefinedDataUpdate", "Auto"),
        // editType — present 4/4 у EDT → required.
        e(F_EDIT_TYPE, "editType", "InList"),
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),
        list(F_INPUT_BY_STRING, "inputByString"),
        // createOnInput — present 4/4 у обоих → required (EDT всегда несёт).
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer DENSE эмитит всегда). Сверено _generated + корпусом.
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_FOLDER_FORM, "defaultFolderForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_DEFAULT_FOLDER_CHOICE_FORM, "defaultFolderChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_FOLDER_FORM, "auxiliaryFolderForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        s(F_AUX_FOLDER_CHOICE_FORM, "auxiliaryFolderChoiceForm"),
        list(F_BASED_ON, "basedOn"),
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
        b(F_HELP, "help").x_ignored(),
        // predefined — EDT-only; List; отсутствие = дефолт-омиссия; X-исключён.
        list(F_PREDEFINED, "predefined").x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (как Catalog, сверено 4/4:
/// Attribute < TabularSection < Form < Template < Command).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "ChartOfCharacteristicTypes.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "ChartOfCharacteristicTypes.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "ChartOfCharacteristicTypes.FormRef" },
    ChildSlot { collection: "Template", child_kind: "ChartOfCharacteristicTypes.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "ChartOfCharacteristicTypes.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Читается
// обобщённым кодеком `formats_xml::std_attrs_generic` в ОБА XML-формата. Машинерия
// 1:1 как у Catalog (тот же 25-листовой Designer-регион + EDT-разрежённость),
// различие — только набор атрибутов и обёртка `<ChartOfCharacteristicTypes>`.
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок, 1:1 как Catalog).
const S_SYNONYM: usize = 0;
const S_COMMENT: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FILL_FROM: usize = 3;
const S_FILL_VALUE: usize = 4;
const S_CPL: usize = 5;
const S_FILL_CHECKING: usize = 6;
const S_FULL_TEXT: usize = 7;
// Слоты 8..10 зеркалят Catalog (общий cf-кодек `std_attrs_catalog`, RootCct-вариант, N_VAR
// общий): у CCT в ERP эти листья всегда дефолтны — байты не меняются, слоты нужны для
// единой IR-арности.
const S_CHOICE_PARAMS: usize = 8;
const S_MASK: usize = 9;
const S_CHOICE_FORM: usize = 10;
// dataHistory std-attr: 4-й случай «const-modeled settable» — ERP несёт DontUse у
// Description (КлассификаторДОПОГЭПД, мирСкважины; witnessed cf prop 9288a8ed…: Use=1,
// DontUse=0). EDT: лист ОПУЩЕН ⟺ DontUse (дефолт слота), явный `Use` — прежние байты.
const S_DATA_HISTORY: usize = 11;

/// 9 предопределённых атрибута КОРНЯ (все `TypeReductionMode=TransformValues` — нет
/// Owner-атрибута). `name_consts[0]` = TypeReductionMode.
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "PredefinedDataName", name_consts: &["TransformValues"] },
    AttrDecl { name: "ValueType", name_consts: &["TransformValues"] },
    AttrDecl { name: "Description", name_consts: &["TransformValues"] },
    AttrDecl { name: "Code", name_consts: &["TransformValues"] },
    AttrDecl { name: "IsFolder", name_consts: &["TransformValues"] },
    AttrDecl { name: "Parent", name_consts: &["TransformValues"] },
    AttrDecl { name: "Predefined", name_consts: &["TransformValues"] },
    AttrDecl { name: "DeletionMark", name_consts: &["TransformValues"] },
    AttrDecl { name: "Ref", name_consts: &["TransformValues"] },
];

/// 1 предопределённый атрибут табличной части (`LineNumber`, TRM=TransformValues).
const TABULAR_ATTRS: &[AttrDecl] =
    &[AttrDecl { name: "LineNumber", name_consts: &["TransformValues"] }];

/// Variable-слоты (типы/дефолты) в EDT-порядке.
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Str,               // comment
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Bool,              // fillFromFillingValue
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Cpl,               // choiceParameterLinks
    VarSlotKind::Enum("DontCheck"), // fillChecking (variable)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (variable)
    VarSlotKind::Cp,                // choiceParameters (слот-зеркало Catalog; CCT — дефолт)
    VarSlotKind::Str,               // mask (слот-зеркало Catalog; CCT — дефолт)
    VarSlotKind::Str,               // choiceForm (слот-зеркало Catalog; CCT — дефолт)
    VarSlotKind::Enum("DontUse"), // dataHistory (ERP-witnessed переменная)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом; позиции новых листьев — как у
/// Catalog, ERP-witnessed там же).
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

/// Designer-DENSE 25 листьев (порядок фикс; ИДЕНТИЧЕН Catalog).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
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

/// Декларация `standardAttributes` вида `ChartOfCharacteristicTypes` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "ChartOfCharacteristicTypes",
    label: "ChartOfCharacteristicTypes",
};

/// Канонический [`EntitySpec`] вида `ChartOfCharacteristicTypes` (кэш на процесс).
pub fn chart_of_characteristic_types() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfCharacteristicTypes",
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
/// Категории producedTypes вида `ChartOfCharacteristicTypes` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "ChartOfCharacteristicTypesObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "ChartOfCharacteristicTypesRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "ChartOfCharacteristicTypesSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "ChartOfCharacteristicTypesList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "ChartOfCharacteristicTypesManager",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Characteristic",
        edt_tag: "containerType",
        designer_category: "Characteristic",
        designer_type_name: "Characteristic",
        designer_order: 4,
    },
];
