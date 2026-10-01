//! Канонический спек вида объекта `Task` (бизнес-задача) — ARCHITECTURE.md §1.4/§1.6,
//! child-objects substrate. ЧЕТВЁРТЫЙ СТРУКТУРНЫЙ вид (после InformationRegister, Catalog,
//! Document): переиспользует ~90% субстрата Catalog/Document (`ir_child` base-реквизит для
//! детей, Command/FormRef/TemplateRef-паттерн, children-рекурсию, value/type/cpl/ref-list/
//! characteristics/help/producedTypes-кодеки) + Task-дельту:
//! * std_attrs_task — 8-атрибутный `Executed/Description/RoutePoint/BusinessProcess/Ref/
//!   DeletionMark/Date/Number` (Task-специфичный набор стандартных атрибутов);
//! * номерные поля `numberType/numberLength/numberAllowedLength` + Task-специфичный
//!   `taskNumberAutoPrefix` (вместо `numberAutoPrefix`/`numberPeriodicity`);
//! * описание `descriptionLength`;
//! * адресация `addressing`(ref)/`mainAddressingAttribute`(ref)/`currentPerformer`(ref);
//! * НОВАЯ дочерняя коллекция `AddressingAttribute` (реквизит адресации);
//! * EDT-only help, TabularSection recursion.
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено по
//! корпусу). EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссий.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас. EDT-only `help` — X-исключён (`x_ignore`), но участвует в R EDT. У Task
//! `predefined`/`numerator` НЕТ (задачи не предопределяются и без нумератора).

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
/// `numberType` — Default String (у Task единственное значение в корпусе — String).
pub const F_NUMBER_TYPE: FieldId = FieldId(4);
/// `numberLength` — int. Default 0.
pub const F_NUMBER_LENGTH: FieldId = FieldId(5);
/// `numberAllowedLength` — Default Fixed (EDT омитит при Fixed; Designer dense эмитит).
pub const F_NUMBER_ALLOWED_LENGTH: FieldId = FieldId(6);
/// `checkUnique` — Default false.
pub const F_CHECK_UNIQUE: FieldId = FieldId(7);
/// `autonumbering` — Default false.
pub const F_AUTONUMBERING: FieldId = FieldId(8);
/// `taskNumberAutoPrefix` — Task-специфичный автопрефикс номера. Default DontUse.
pub const F_TASK_NUMBER_AUTO_PREFIX: FieldId = FieldId(9);
/// `descriptionLength` — длина наименования (int). Default 0.
pub const F_DESCRIPTION_LENGTH: FieldId = FieldId(10);
/// `addressing` — ссылка на регистр адресации (object-ref). Default "".
pub const F_ADDRESSING: FieldId = FieldId(11);
/// `mainAddressingAttribute` — ссылка на основной реквизит адресации. Default "".
pub const F_MAIN_ADDRESSING_ATTRIBUTE: FieldId = FieldId(12);
/// `currentPerformer` — ссылка на параметр сеанса текущего исполнителя. Default "".
pub const F_CURRENT_PERFORMER: FieldId = FieldId(13);
/// `basedOn` — list-of-refs (item-style). Default [].
pub const F_BASED_ON: FieldId = FieldId(14);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (8 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(15);
/// `characteristics` — список связок. Default [].
pub const F_CHARACTERISTICS: FieldId = FieldId(16);
/// `defaultPresentation` — Default AsDescription.
pub const F_DEFAULT_PRESENTATION: FieldId = FieldId(17);
/// `editType` — Default InDialog.
pub const F_EDIT_TYPE: FieldId = FieldId(18);
/// `inputByString` — list-of-refs (field-style). Default [].
pub const F_INPUT_BY_STRING: FieldId = FieldId(19);
/// `searchStringModeOnInputByString` — Default Begin.
pub const F_SEARCH_STRING_MODE: FieldId = FieldId(20);
/// `fullTextSearchOnInputByString` — present у EDT даже при DontUse → required.
pub const F_FTS_ON_INPUT: FieldId = FieldId(21);
/// `choiceDataGetModeOnInputByString` — Designer-only DENSE. Default Directly.
pub const F_CHOICE_DATA_GET_MODE: FieldId = FieldId(22);
/// `createOnInput` — Required (всегда present у обоих).
pub const F_CREATE_ON_INPUT: FieldId = FieldId(23);
/// `defaultObjectForm` — form-ref. Default "".
pub const F_DEFAULT_OBJECT_FORM: FieldId = FieldId(24);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(25);
/// `defaultChoiceForm` — form-ref. Default "".
pub const F_DEFAULT_CHOICE_FORM: FieldId = FieldId(26);
/// `auxiliaryObjectForm` — Designer-only. Default "".
pub const F_AUX_OBJECT_FORM: FieldId = FieldId(27);
/// `auxiliaryListForm` — Designer-only. Default "".
pub const F_AUX_LIST_FORM: FieldId = FieldId(28);
/// `auxiliaryChoiceForm` — Designer-only. Default "".
pub const F_AUX_CHOICE_FORM: FieldId = FieldId(29);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(30);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(31);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(32);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(44);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(45);
/// `dataLockFields` — list-of-refs (field-style). Default [].
pub const F_DATA_LOCK_FIELDS: FieldId = FieldId(33);
/// `dataLockControlMode` — Default Automatic.
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(34);
/// `fullTextSearch` — Default DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(35);
/// `objectPresentation` — локализ. Default [].
pub const F_OBJECT_PRESENTATION: FieldId = FieldId(36);
/// `extendedObjectPresentation` — локализ. Default [].
pub const F_EXTENDED_OBJECT_PRESENTATION: FieldId = FieldId(37);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(38);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(39);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(40);
/// `dataHistory` — Designer-only DENSE. Default DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(41);
/// `updateDataHistoryImmediatelyAfterWrite` — Designer-only DENSE. Default false.
pub const F_UPDATE_DATA_HISTORY: FieldId = FieldId(42);
/// `executeAfterWriteDataHistoryVersionProcessing` — Designer-only DENSE. Default false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(43);

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
        // numberType — present у EDT даже при String → required (EDT всегда несёт).
        e(F_NUMBER_TYPE, "numberType", "Number"),
        i(F_NUMBER_LENGTH, "numberLength", 0),
        e(F_NUMBER_ALLOWED_LENGTH, "numberAllowedLength", "Fixed"),
        b(F_CHECK_UNIQUE, "checkUnique"),
        b(F_AUTONUMBERING, "autonumbering"),
        // taskNumberAutoPrefix — платформенный EDT (sparse) ОПУСКАЕТ при дефолте `DontUse`.
        FieldSpec::with_default(
            F_TASK_NUMBER_AUTO_PREFIX,
            "taskNumberAutoPrefix",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("BusinessProcessNumber")),
        ),
        i(F_DESCRIPTION_LENGTH, "descriptionLength", 0),
        s(F_ADDRESSING, "addressing"),
        s(F_MAIN_ADDRESSING_ATTRIBUTE, "mainAddressingAttribute"),
        s(F_CURRENT_PERFORMER, "currentPerformer"),
        list(F_BASED_ON, "basedOn"),
        // standardAttributes — вариативный const-блок (8 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        list(F_CHARACTERISTICS, "characteristics"),
        // defaultPresentation/editType — present у EDT (всегда несёт) → required.
        e(F_DEFAULT_PRESENTATION, "defaultPresentation", "AsNumber"),
        e(F_EDIT_TYPE, "editType", "InList"),
        list(F_INPUT_BY_STRING, "inputByString"),
        e(F_SEARCH_STRING_MODE, "searchStringModeOnInputByString", "Begin"),
        // fullTextSearchOnInputByString — enum default Use (EDT sparse опускает Use; Designer DENSE эмитит всегда). Сверено _generated + корпусом.
        e(F_FTS_ON_INPUT, "fullTextSearchOnInputByString", "Use"),
        e(F_CHOICE_DATA_GET_MODE, "choiceDataGetModeOnInputByString", "Directly"),
        // createOnInput — present у обоих → required (EDT всегда несёт).
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        s(F_DEFAULT_OBJECT_FORM, "defaultObjectForm"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_DEFAULT_CHOICE_FORM, "defaultChoiceForm"),
        s(F_AUX_OBJECT_FORM, "auxiliaryObjectForm"),
        s(F_AUX_LIST_FORM, "auxiliaryListForm"),
        s(F_AUX_CHOICE_FORM, "auxiliaryChoiceForm"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
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
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено по корпусу ОБА
/// формата: Attribute < Form < AddressingAttribute < TabularSection < Template <
/// Command). `AddressingAttribute` — Task-специфичная НОВАЯ коллекция (реквизит адресации).
// Физический порядок дочерних коллекций (edt .mdo + designer ChildObjects, обе форматные
// фикстуры Задача_ПокрытиеСоставных): attributes → tabularSections → addressingAttributes.
// Forms/Templates/Commands в корпусе не витнессированы — позиция принята по конвенции Catalog.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Attribute", child_kind: "Task.Attribute" },
    ChildSlot { collection: "TabularSection", child_kind: "Task.TabularSection" },
    ChildSlot { collection: "AddressingAttribute", child_kind: "Task.AddressingAttribute" },
    ChildSlot { collection: "Form", child_kind: "Task.FormRef" },
    ChildSlot { collection: "Template", child_kind: "Task.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "Task.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Task-семья:
// 6 variable-полей [synonym, comment, toolTip, format, editFormat, fillValue] (на ОДНО
// больше Document — пер-атрибутный `comment`); `fillChecking` определён именем
// (Description=ShowError, иначе DontCheck); `fullTextSearch`=const Use.
// ============================================================================

const S_SYNONYM: usize = 0;
const S_COMMENT: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FORMAT: usize = 3;
const S_EDIT_FORMAT: usize = 4;
const S_FILL_VALUE: usize = 5;

/// 8 предопределённых атрибута КОРНЯ. `name_consts[0]` = fillChecking.
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Executed", name_consts: &["DontCheck"] },
    AttrDecl { name: "Description", name_consts: &["ShowError"] },
    AttrDecl { name: "RoutePoint", name_consts: &["DontCheck"] },
    AttrDecl { name: "BusinessProcess", name_consts: &["DontCheck"] },
    AttrDecl { name: "Ref", name_consts: &["DontCheck"] },
    AttrDecl { name: "DeletionMark", name_consts: &["DontCheck"] },
    AttrDecl { name: "Date", name_consts: &["DontCheck"] },
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

/// Декларация `standardAttributes` вида `Task` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: TABULAR_ATTRS,
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "Task",
    label: "Task",
};

/// Канонический [`EntitySpec`] вида `Task` (кэш на процесс).
pub fn task() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Task",
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
/// Категории producedTypes вида `Task` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Object",
        edt_tag: "objectType",
        designer_category: "Object",
        designer_type_name: "TaskObject",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "Ref",
        edt_tag: "refType",
        designer_category: "Ref",
        designer_type_name: "TaskRef",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "TaskSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "TaskList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "TaskManager",
        designer_order: 4,
    },
];
