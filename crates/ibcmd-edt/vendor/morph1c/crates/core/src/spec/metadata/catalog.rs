//! Канонический спек вида объекта `Catalog` (справочник) — ARCHITECTURE.md §1.4/§1.6,
//! child-objects substrate. ВТОРОЙ СТРУКТУРНЫЙ вид (после InformationRegister):
//! переиспользует ~90% субстрата (ir_child base+Use, Command/FormRef/TemplateRef-паттерн,
//! children-рекурсию, value/type/cpl/help/producedTypes-кодеки) + Catalog-дельту
//! (std_attrs_catalog 9-атрибутный, characteristics, ref-list basedOn/owners/
//! dataLockFields/inputByString, EDT-only predefined-блок, TabularSection recursion).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено 74/74).
//! EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссии (сверено
//! corpus-wide — каждое omitted EDT-поле даёт РОВНО один Designer-дефолт).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас. EDT-only `help`+`predefined` — X-исключены (`x_ignore`), но участвуют в R EDT.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, NameConstWhich, Normalize,
    StdAttrsDecl, VarSlotKind,
};

// --- FieldId'ы в порядке Designer DENSE ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `hierarchical` — Default false.
pub const F_HIERARCHICAL: FieldId = FieldId(3);
/// `hierarchyType` — Default HierarchyFoldersAndItems.
pub const F_HIERARCHY_TYPE: FieldId = FieldId(4);
/// `limitLevelCount` — Default false.
pub const F_LIMIT_LEVEL_COUNT: FieldId = FieldId(5);
/// `levelCount` — int. Required (всегда present у обоих; EDT всегда несёт).
pub const F_LEVEL_COUNT: FieldId = FieldId(6);
/// `foldersOnTop` — Default false.
pub const F_FOLDERS_ON_TOP: FieldId = FieldId(7);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(8);
/// `owners` — list-of-refs (item-style). Default [].
pub const F_OWNERS: FieldId = FieldId(9);
/// `subordinationUse` — Designer-only DENSE. Default ToItems (EDT нет поля → дефолт-эмиссия Designer).
pub const F_SUBORDINATION_USE: FieldId = FieldId(10);
/// `codeLength` — int. Default 0.
pub const F_CODE_LENGTH: FieldId = FieldId(11);
/// `descriptionLength` — int. Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(12);
/// `codeType` — Default Number.
pub const F_CODE_TYPE: FieldId = FieldId(13);
/// `codeAllowedLength` — Default Fixed.
pub const F_CODE_ALLOWED_LENGTH: FieldId = FieldId(14);
/// `codeSeries` — Default WholeCatalog.
pub const F_CODE_SERIES: FieldId = FieldId(15);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(16);
/// `autonumbering` — Default false.
pub const F_AUTONUMBERING: FieldId = FieldId(17);
/// `defaultPresentation` — Default AsCode.
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(18);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (9 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(19);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(20);
/// `predefinedDataUpdate` — Default Auto.
pub const F_PREDEFINED_DATA_UPDATE: FieldId = FieldId(21);
/// `editType` — Default InList.
pub const F_EDIT_TYPE: FieldId = FieldId(22);
/// `quickChoice` — Default false.
pub const F_QUICK_CHOICE: FieldId = FieldId(23);
/// `choiceMode` — Default FromForm.
pub const F_CHOICE_MODE: FieldId = FieldId(24);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(25);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(26);
/// `fullTextSearchOnInputByString` — Default DontUse (но present 74/74 у обоих).
pub const F_FTS_ON_INPUT: FieldId = FieldId(27);
/// `choiceDataGetModeOnInputByString` — Default Directly (EDT не несёт).
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(28);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(29);
/// `defaultFolderForm` — form-ref. Default "".
pub const F_DEFAULT_FOLDER_FORM: FieldId = FieldId(30);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(31);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(32);
/// `defaultFolderChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_FOLDER_CHOICE_FORM: FieldId = FieldId(33);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(34);
/// `auxiliaryFolderForm` — Designer-only. Default "".
pub const F_AUX_FOLDER_FORM: FieldId = FieldId(35);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(36);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(37);
/// `auxiliaryFolderChoiceForm` — Designer-only. Default "".
pub const F_AUX_FOLDER_CHOICE_FORM: FieldId = FieldId(38);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(39);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(40);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(41);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(42);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(43);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(44);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(45);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(46);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(47);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(48);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(49);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(50);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(51);
/// `dataHistory` — Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(52);
/// `updateDataHistoryImmediatelyAfterWrite` — Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(53);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(54);
/// `predefined` — EDT-only данные. Default — отсутствие (List, x_ignore). См.
/// [`crate::spec::common`]: храним как `List`; отсутствие = дефолт-омиссия.
pub const F_PREDEFINED: FieldId = FieldId(55);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(56);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(57);

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
        b(F_HIERARCHICAL, "hierarchical"),
        e(F_HIERARCHY_TYPE, "hierarchyType", "HierarchyFoldersAndItems"),
        b(F_LIMIT_LEVEL_COUNT, "limitLevelCount"),
        // levelCount — int, present 74/74 у обоих → required (EDT всегда несёт).
        FieldSpec::required(F_LEVEL_COUNT, "levelCount", ValueKind::Int),
        b(F_FOLDERS_ON_TOP, "foldersOnTop"),
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        list(F_OWNERS, "owners"),
        e(F_SUBORDINATION_USE, "subordinationUse", "ToItems"),
        i(F_CODE_LENGTH, "codeLength", 0),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        e(F_CODE_TYPE, "codeType", "Number"),
        e(F_CODE_ALLOWED_LENGTH, "codeAllowedLength", "Fixed"),
        e(F_CODE_SERIES, "codeSeries", "WholeCatalog"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        b(F_AUTONUMBERING, "autonumbering"),
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsCode"),
        // standardAttributes — вариативный const-блок (9 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        e(F_PREDEFINED_DATA_UPDATE, "predefinedDataUpdate", "Auto"),
        e(F_EDIT_TYPE, "editType", "InList"),
        b(F_QUICK_CHOICE, "quickChoice"),
        e(F_CHOICE_MODE, "choiceMode", "FromForm"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer
        // DENSE эмитит всегда). Сверено _generated + корпусом (объект …_Использовать опускает Use).
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
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
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        b(F_HELP, "help").x_ignored(),
        list(F_BASED_ON, "basedOn"),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        // createOnInput — enum default Auto (EDT sparse опускает Auto; корпус 64/65 несут
        // Use/DontUse, объект …_Авто опускает). Сверено _generated.
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
        b(F_UPDATE_DATA_HISTORY, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing"),
        // predefined — EDT-only; List; отсутствие = дефолт-омиссия; X-исключён.
        list(F_PREDEFINED, "predefined").x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено 74/74:
/// Attribute < TabularSection < Form < Template < Command).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "Catalog.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "Catalog.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "Catalog.FormRef" },
    ChildSlot { collection: "Template", child_kind: "Catalog.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "Catalog.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Читается
// обобщённым кодеком `formats_xml::std_attrs_generic` в ОБА XML-формата.
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок).
const S_SYNONYM: usize = 0;
const S_COMMENT: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FILL_FROM: usize = 3;
const S_FILL_VALUE: usize = 4;
const S_CPL: usize = 5;
const S_FILL_CHECKING: usize = 6;
const S_FULL_TEXT: usize = 7;
// ERP-корпус вскрыл ещё три задаваемых листа std-attrs (SSL их не нёс — были константами):
// choiceParameters (19 связок), mask (5, напр. Code «XXXX XXXX XXXX XXXXXXX» у
// БанковскиеКартыКонтрагентов), choiceForm (1). Слоты зеркалятся cf-стороной
// (`formats_brace::std_attrs_catalog`: I_CHOICE_PARAMS/I_MASK/I_CHOICE_FORM).
const S_CHOICE_PARAMS: usize = 8;
const S_MASK: usize = 9;
const S_CHOICE_FORM: usize = 10;
// dataHistory std-attr: 4-й случай «const-modeled settable» — ERP несёт DontUse у
// Description (КлассификаторДОПОГЭПД, мирСкважины; witnessed cf prop 9288a8ed…: Use=1,
// DontUse=0). EDT: лист ОПУЩЕН ⟺ DontUse (дефолт слота), явный `Use` — прежние байты.
const S_DATA_HISTORY: usize = 11;
// Genuine UH ПредметыКомментирования/Description: mutable multiLine=true.
const S_MULTI_LINE: usize = 12;

/// 9 предопределённых атрибута КОРНЯ. `name_consts[0]` = TypeReductionMode
/// (Owner=Deny, иначе TransformValues — сверено 74/74).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "PredefinedDataName", name_consts: &["TransformValues"] },
    AttrDecl { name: "Predefined", name_consts: &["TransformValues"] },
    AttrDecl { name: "Ref", name_consts: &["TransformValues"] },
    AttrDecl { name: "DeletionMark", name_consts: &["TransformValues"] },
    AttrDecl { name: "IsFolder", name_consts: &["TransformValues"] },
    AttrDecl { name: "Owner", name_consts: &["Deny"] },
    AttrDecl { name: "Parent", name_consts: &["TransformValues"] },
    AttrDecl { name: "Description", name_consts: &["TransformValues"] },
    AttrDecl { name: "Code", name_consts: &["TransformValues"] },
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
    VarSlotKind::Cp,                // choiceParameters (ERP-witnessed)
    VarSlotKind::Str,               // mask (ERP-witnessed)
    VarSlotKind::Str,               // choiceForm (ERP-witnessed; путь Kind.Name.Form.F)
    VarSlotKind::Enum("DontUse"), // dataHistory (ERP-witnessed переменная)
    VarSlotKind::Bool, // multiLine (UH witnessed Description=true)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом; позиции новых листьев —
/// ERP-witnessed: choiceParameters МЕЖДУ fillChecking и fullTextSearch, mask МЕЖДУ
/// fullTextSearch и minValue (Спр БанковскиеКартыКонтрагентов/Code), choiceForm — В КОНЦЕ
/// после maxValue).
const EDT_LEAVES: &[EdtLeaf] = &[
    // ИЗВЕСТНОЕ НАРУШЕНИЕ (const-modeled settable, 4-й случай класса): ERP-перепись
    // 1151 каталогов ×9 атрибутов — у ДВУХ объектов (КлассификаторДОПОГЭПД, мирСкважины)
    // Description несёт dataHistory=DontUse (EDT ОПУСКАЕТ лист; Designer пишет DontUse;
    // cf: prop 9288a8ed-b259-46d0-a8e3-70d87956ff2d = {"#",d46ea122-…,{d46ea122-…,0}},
    // Use=1). Фикс = новый var-слот S_DATA_HISTORY (OptEnum деф. DontUse + VarEnum) —
    // требует СИНХРОННОЙ правки cf-стороны (`formats_brace::std_attrs_catalog`: N_VAR
    // 11→12, маппинг guid'а 9288a8ed вместо константы шаблона) — файл другой работы;
    // до неё блок этих двух объектов честно уходит в leftover (typed, не молча).
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
    EdtLeaf::OptBool("multiLine", S_MULTI_LINE),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
    EdtLeaf::OptStr("choiceForm", S_CHOICE_FORM),
];

/// Designer-DENSE 25 листьев (порядок фикс).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::VarBool(S_MULTI_LINE)),
    ("FillFromFillingValue", DenseLeaf::VarBool(S_FILL_FROM)),
    ("CreateOnInput", DenseLeaf::Text("Auto")),
    ("TypeReductionMode", DenseLeaf::VarNameConst(NameConstWhich::C0)),
    ("MaxValue", DenseLeaf::Nil),
    ("ToolTip", DenseLeaf::VarLoc(S_TOOLTIP)),
    ("ExtendedEdit", DenseLeaf::Text("false")),
    ("Format", DenseLeaf::Empty),
    // ChoiceForm БЫЛ Empty-константой — ERP-witnessed путь формы (текст-лист).
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
    // Mask/ChoiceParameters БЫЛИ Empty-константами (SSL не нёс) — ERP-корпус вскрыл
    // задаваемость (Mask 5, ChoiceParameters 15 непустых на designer-стороне).
    ("Mask", DenseLeaf::VarStr(S_MASK)),
    ("ChoiceParameters", DenseLeaf::VarCp(S_CHOICE_PARAMS)),
];

/// Декларация `standardAttributes` вида `Catalog` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    // ERP-перепись 2026-07-15 (1151 каталогов оба диалекта, вкл. не-иерархические и
    // безвладельческие — КлассификаторДОПОГЭПД и пр.): РОВНО ОДИН набор из 9 имён в одном
    // порядке; гипотеза «у не-иерархических другой набор» ОПРОВЕРГНУТА. (57 каталогов
    // вообще без блока — в ОБОИХ диалектах, напр. Диадок_Файлы: ни <standardAttributes>
    // в .mdo, ни <StandardAttributes> в Designer-XML — блок как ЦЕЛОЕ опционален.)
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "Catalog",
    label: "Catalog",
};

/// Канонический [`EntitySpec`] вида `Catalog` (кэш на процесс).
pub fn catalog() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Catalog",
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
/// Категории producedTypes вида `Catalog` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "CatalogObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "CatalogRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "CatalogSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "CatalogList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "CatalogManager",
        designer_order: 4,
    },
];
