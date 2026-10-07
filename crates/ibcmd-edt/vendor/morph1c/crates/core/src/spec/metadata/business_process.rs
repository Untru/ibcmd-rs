//! Канонический спек вида объекта `BusinessProcess` (бизнес-процесс) — ARCHITECTURE.md
//! §1.4/§1.6, child-objects substrate. СТРУКТУРНЫЙ вид (после InformationRegister, Catalog,
//! Document, Task, ExchangePlan): переиспользует ~90% субстрата Document/Task (`ir_child`
//! base-реквизит для детей, Command/FormRef/TemplateRef-паттерн, children-рекурсию, value/
//! type/cpl/ref-list/characteristics/help/producedTypes-кодеки) + BusinessProcess-дельту:
//! * std_attrs_business_process — 7-атрибутный `Started/HeadTask/Completed/Ref/DeletionMark/
//!   Date/Number` (BP-специфичный набор стандартных атрибутов; Date=ShowError, иначе
//!   DontCheck — сверено по корпусу);
//! * номерные поля `numberType/numberLength/numberAllowedLength/checkUnique/autonumbering`
//!   + `numberPeriodicity` (Designer-only DENSE, default Nonperiodical);
//! * `task` — ссылка на вид `Task`, тип задачи, ведущий процесс (object-ref);
//! * `createTaskInPrivilegedMode`(bool) — создавать задачи в привилегир. режиме;
//! * basedOn/characteristics (reuse), EDT-only help (X-исключён), TabularSection recursion.
//!
//! Графическая карта маршрута (Flowchart) НЕ в дескрипторе — отдельный файл (EDT
//! `Flowchart.scheme`, Designer `Ext/Flowchart.xml`), как тела форм. В спеке его НЕТ.
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено по
//! корпусу). EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссий.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (6 категорий, включая BP-специфичный `RoutePointRef`). У BusinessProcess
//! `defaultPresentation`/`descriptionLength`/`numerator`/`predefined` НЕТ (сверено).

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
/// `editType` — present у EDT (всегда несёт) → required.
pub const F_EDIT_TYPE: FieldId = FieldId(4);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(5);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(6);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(7);
/// `choiceDataGetModeOnInputByString` — Designer-only DENSE. Default Directly.
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(8);
/// `fullTextSearchOnInputByString` — present у EDT даже при DontUse → required.
pub const F_FTS_ON_INPUT: FieldId = FieldId(9);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(10);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(11);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(12);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(13);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(14);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(15);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(16);
/// `numberType` — present у EDT даже при String → required (EDT всегда несёт).
pub const F_NUMBER_TYPE: FieldId = FieldId(17);
/// `numberLength` — int. Default 0.
pub const F_NUMBER_LENGTH: FieldId = FieldId(18);
/// `numberAllowedLength` — Default Fixed (EDT омитит при Fixed; Designer dense эмитит).
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(19);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(20);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (7 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(21);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(22);
/// `autonumbering` — Default false.
pub const F_AUTONUMBERING: FieldId = FieldId(23);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(24);
/// `numberPeriodicity` — Designer-only DENSE. Default Nonperiodical.
pub const F_NUMBER_PERIODICITY: FieldId = FieldId(25);
/// `task` — ссылка на вид Task (тип задачи процесса; object-ref). Default "".
pub const F_TASK: FieldId = FieldId(26);
/// `createTaskInPrivilegedMode` — bool. Default false.
pub const F_CREATE_TASK_IN_PRIVILEGED_MODE: FieldId = FieldId(27);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(28);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(29);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(30);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(31);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(41);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(42);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(32);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(33);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(34);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(35);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(36);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(37);
/// `dataHistory` — Designer-only DENSE. Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(38);
/// `updateDataHistoryImmediatelyAfterWrite` — Designer-only DENSE. Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(39);
/// `executeAfterWriteDataHistoryVersionProcessing` — Designer-only DENSE. Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(40);

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
        // editType — present у EDT (всегда несёт) → required.
        e(F_EDIT_TYPE, "editType", "InList"),
        list(F_INPUT_BY_STRING, "inputByString"),
        // createOnInput — present у обоих → required.
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer DENSE эмитит всегда). Сверено _generated + корпусом.
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        // numberType — present у EDT даже при String → required.
        e(F_NUMBER_TYPE, "numberType", "Number"),
        i(F_NUMBER_LENGTH, "numberLength", 0),
        e(F_NUMBER_ALLOWED_LENGTH, "numberAllowedLength", "Fixed"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        // standardAttributes — вариативный const-блок (7 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        b(F_AUTONUMBERING, "autonumbering"),
        list(F_BASED_ON, "basedOn"),
        e(F_NUMBER_PERIODICITY, "numberPeriodicity", "Nonperiodical"),
        s(F_TASK, "task"),
        b(F_CREATE_TASK_IN_PRIVILEGED_MODE, "createTaskInPrivilegedMode"),
        list(F_DATA_LOCK_FIELDS, "dataLockFields"),
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        loc_field(F_OBJECT_PRESENTATION, "objectPresentation"),
        loc_field(F_EXTENDED_OBJECT_PRESENTATION, "extendedObjectPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
        b(F_UPDATE_DATA_HISTORY, "updateDataHistoryImmediatelyAfterWrite"),
        b(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing"),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии: `Attribute < TabularSection <
/// Form < Template < Command`. Сверено по ERP-корпусу (ЗаявкаСотрудникаОтпуск.mdo:
/// `attributes` < `tabularSections` < `forms`; Designer `<ChildObjects>` — тот же порядок).
/// BusinessProcess НЕ несёт AddressingAttribute, в отличие от Task. (Прежний порядок
/// `Form < TabularSection` был невитнессирован: единственный SSL-БП «Задание» табличных
/// частей не имеет — ошибка вскрылась на ERP-БП с непустыми `tabularSections`.)
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "BusinessProcess.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "BusinessProcess.TabularSection" },
    ChildSlot { collection: "Form", child_kind: "BusinessProcess.FormRef" },
    ChildSlot { collection: "Template", child_kind: "BusinessProcess.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "BusinessProcess.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Task-семья:
// 6 variable-полей [synonym, comment, toolTip, format, editFormat, fillValue];
// `fillChecking` определён именем (Date=ShowError, иначе DontCheck); `fullTextSearch`=Use.
// ============================================================================

const S_SYNONYM: usize = 0;
const S_COMMENT: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FORMAT: usize = 3;
const S_EDIT_FORMAT: usize = 4;
const S_FILL_VALUE: usize = 5;

/// 7 предопределённых атрибута КОРНЯ. `name_consts[0]` = fillChecking.
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Started", name_consts: &["DontCheck"] },
    AttrDecl { name: "HeadTask", name_consts: &["DontCheck"] },
    AttrDecl { name: "Completed", name_consts: &["DontCheck"] },
    AttrDecl { name: "Ref", name_consts: &["DontCheck"] },
    AttrDecl { name: "DeletionMark", name_consts: &["DontCheck"] },
    AttrDecl { name: "Date", name_consts: &["ShowError"] },
    AttrDecl { name: "Number", name_consts: &["DontCheck"] },
];

/// 1 предопределённый атрибут табличной части (`LineNumber`, fillChecking DontCheck).
const TABULAR_ATTRS: &[AttrDecl] = &[AttrDecl { name: "LineNumber", name_consts: &["DontCheck"] }];

const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,   // synonym
    VarSlotKind::Str,   // comment
    VarSlotKind::Loc,   // toolTip
    VarSlotKind::Loc,   // format
    VarSlotKind::Loc,   // editFormat
    VarSlotKind::Value, // fillValue
];

const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptStr("comment", S_COMMENT),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::ReqValue("fillValue", S_FILL_VALUE),
    EdtLeaf::NameConst { tag: "fillChecking", which: NameConstWhich::C0, omit: "DontCheck" },
    EdtLeaf::ConstText { tag: "fullTextSearch", text: "Use" },
    EdtLeaf::OptLoc("format", S_FORMAT),
    EdtLeaf::OptLoc("editFormat", S_EDIT_FORMAT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarNameConst(NameConstWhich::C0)),
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
    ("Comment", DenseLeaf::VarStr(S_COMMENT)),
    ("FullTextSearch", DenseLeaf::Text("Use")),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::VarValue(S_FILL_VALUE)),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация `standardAttributes` вида `BusinessProcess` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "BusinessProcess",
    label: "BusinessProcess",
};

/// Канонический [`EntitySpec`] вида `BusinessProcess` (кэш на процесс).
pub fn business_process() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "BusinessProcess",
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
/// Категории producedTypes вида `BusinessProcess` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "BusinessProcessObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "BusinessProcessRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "BusinessProcessSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "BusinessProcessList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "BusinessProcessManager",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "RoutePointRef",
        edt_tag: "routePointRef",
        designer_category: "RoutePointRef",
        designer_type_name: "BusinessProcessRoutePointRef",
        designer_order: 5,
    },
];
