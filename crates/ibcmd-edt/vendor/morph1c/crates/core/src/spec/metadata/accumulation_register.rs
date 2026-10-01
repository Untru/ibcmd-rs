//! Канонический спек вида объекта `AccumulationRegister` (регистр накопления) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate.
//! Первый вид семейства ERP-регистров, доказывающий ВАРИАДНЫЙ/УСЛОВНЫЙ std-attrs
//! субстрат (см. [`STD_ATTRS`]). Мирроринг `InformationRegister` (structural).
//! ERP-дамп Designer 8.3.27 НЕСЁТ AccumulationRegisters (223 объекта) → std-attrs
//! проецируются в ОБА XML-формата; cf-сторона —
//! `formats_brace::std_attrs_ir::ACCUMULATION_REGISTER`.
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата. Каноника (id/value_kind/default/порядок/нормализация) — здесь;
//! формат лишь проецирует (§1.6).
//!
//! Порядок свойств = каноническому EDT-порядку `.mdo` (сверено топосортом по всему
//! ERP-корпусу, 233 объекта, 0 конфликтов): synonym, comment, useStandardCommands,
//! defaultListForm, registerType, help, standardAttributes, dataLockControlMode,
//! enableTotalsSplitting, explanation. Дефолты выведены из EDT-омиссии (сверено).
//!
//! **standardAttributes — ВАРИАДНЫ + register-type-conditional** (корпус-факт):
//! `registerType==Balance` (дефолт, тег омитится) ⇒ 5 атрибутов
//! (RecordType/Active/LineNumber/Recorder/Period); `registerType==Turnovers` ⇒ 4 (БЕЗ
//! RecordType); блок может ОТСУТСТВОВАТЬ целиком ⇒ 0. Обобщённый кодек выбирает вариант
//! по числу блоков (см. [`STD_ATTRS`] + `formats_xml::std_attrs_generic`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes` — платформенный
//! [`crate::ir::InternalInfo`]-каркас (см. [`PRODUCED_CATEGORIES`]). Дочерние коллекции —
//! [`ChildSlot`] (рекурсия движка).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize, StdAttrsDecl,
    VarSlotKind,
};

/// `synonym` — локализованный синоним. Default []; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `defaultListForm` — form-ref. Default "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(4);
/// `registerType` — KEYING-поле std-attrs (Balance⇒5 attrs / Turnovers⇒4). Default
/// Balance (EDT омитит Balance, эмитит Turnovers). Читается ДО блока std-attrs (порядок).
pub const F_REGISTER_TYPE: FieldId = FieldId(5);
/// `help` — EDT-only const-блок (presence). Default false; X-исключён (`x_ignore`).
pub const F_HELP: FieldId = FieldId(6);
/// `standardAttributes` — ВАРИАДНЫЙ платформенный блок (5/4/0 атрибутов). Required
/// (всегда present, даже пустой `List([])` при отсутствии блока); IR — `List` записей.
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(7);
/// `dataLockControlMode` — Default Automatic (EDT омитит Automatic, эмитит Managed).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(8);
/// `enableTotalsSplitting` — Default false (EDT эмитит true).
pub const F_ENABLE_TOTALS_SPLITTING: FieldId = FieldId(9);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(10);
/// `includeHelpInContents` — включать справку в содержание. Default false. EDT размещает
/// физически ПЕРЕД `help` (сверено фикстурой РегНак_ВключатьВСодержаниеСправки_Истина).
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(11);
/// `fullTextSearch` — полнотекстовый поиск. Default DontUse (EDT омитит). Физически ПОСЛЕ
/// dataLockControlMode, перед enableTotalsSplitting (зеркало InformationRegister; сверено
/// фикстурой РегНак_ПолнотекстовыйПоиск_Использовать).
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(12);
/// `auxiliaryListForm` — вспомогательная форма списка (form-ref). Default "". Designer DENSE
/// эмитит `<AuxiliaryListForm/>` (сверено), EDT/cf опускают при пустоте (cf → body[22]
/// zeroGuid). Физически ПОСЛЕ defaultListForm (Designer-порядок).
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(13);
/// `listPresentation` — локализ. представление списка. Default []. Designer DENSE эмитит
/// `<ListPresentation/>`; cf несёт слот body[23]. Между enableTotalsSplitting и explanation.
pub const F_LIST_PRESENTATION: FieldId = FieldId(14);
/// `extendedListPresentation` — локализ. расширенное представление списка. Default [].
/// Designer `<ExtendedListPresentation/>`; cf body[24]. После listPresentation.
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(15);

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}
fn empty_loc() -> PropertyValue {
    PropertyValue::Localized(Vec::new())
}
fn enum_tok(s: &str) -> PropertyValue {
    PropertyValue::Enum(Token::new(s))
}
fn bool_f() -> PropertyValue {
    PropertyValue::Bool(false)
}
fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, empty_loc())
        .normalized(Normalize::LocalizedSortByLang)
}

fn build_fields() -> Vec<FieldSpec> {
    // Порядок = КАНОНИЧЕСКИЙ Designer-DENSE (сверено designer-фикстурами): synonym, comment,
    // useStandardCommands, defaultListForm, auxiliaryListForm, registerType,
    // includeHelpInContents, [help — EDT-only], [standardAttributes — EDT/cf], dataLockControlMode,
    // fullTextSearch, enableTotalsSplitting, listPresentation, extendedListPresentation, explanation.
    // EDT (sparse) опускает дефолты (auxiliaryListForm/listPresentation/… пусты в корпусе) —
    // его физ.порядок неизменен; Designer (dense) эмитит всё, кроме help (edt-only) и пустого
    // standardAttributes (0-вариант → 0 узлов).
    vec![
        loc_field(F_SYNONYM, "synonym"),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_USE_STANDARD_COMMANDS, "useStandardCommands", ValueKind::Bool, bool_f()),
        FieldSpec::with_default(F_DEFAULT_LIST_FORM, "defaultListForm", ValueKind::Str, empty_str()),
        // auxiliaryListForm: Designer DENSE `<AuxiliaryListForm/>`; EDT/cf пусты (default).
        FieldSpec::with_default(F_AUXILIARY_LIST_FORM, "auxiliaryListForm", ValueKind::Str, empty_str()),
        // registerType: KEYING-поле std-attrs. Default Balance (EDT омитит).
        FieldSpec::with_default(F_REGISTER_TYPE, "registerType", ValueKind::Enum, enum_tok("Balance")),
        // includeHelpInContents: EDT-bool, Default false (омитится). Физически ПЕРЕД help.
        FieldSpec::with_default(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents", ValueKind::Bool, bool_f()),
        // help: EDT-only presence-блок. Default false; X-исключён (Designer/cf n/a).
        FieldSpec::with_default(F_HELP, "help", ValueKind::Bool, bool_f()).x_ignored(),
        // standardAttributes: вариадный const-блок, IR-List, БЕЗ дефолта (всегда present).
        // В корпусе покрытия — 0-вариант (пустой List): Designer/EDT опускают, cf → body[21]={0}.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        FieldSpec::with_default(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", ValueKind::Enum, enum_tok("Automatic")),
        // fullTextSearch: Default DontUse (EDT омитит). Между dataLockControlMode и
        // enableTotalsSplitting (зеркало InformationRegister-порядка).
        FieldSpec::with_default(F_FULL_TEXT_SEARCH, "fullTextSearch", ValueKind::Enum, enum_tok("DontUse")),
        FieldSpec::with_default(F_ENABLE_TOTALS_SPLITTING, "enableTotalsSplitting", ValueKind::Bool, bool_f()),
        // listPresentation/extendedListPresentation: Designer DENSE; cf body[23]/[24]. Пусты в корпусе.
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено топосортом по
/// ERP-корпусу: R < A < D < F < T < C, как InformationRegister).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Resource", child_kind: "AccumulationRegister.Resource" },
    ChildSlot { collection: "Attribute", child_kind: "AccumulationRegister.Attribute" },
    ChildSlot { collection: "Dimension", child_kind: "AccumulationRegister.Dimension" },
    ChildSlot { collection: "Form", child_kind: "AccumulationRegister.FormRef" },
    ChildSlot { collection: "Template", child_kind: "AccumulationRegister.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "AccumulationRegister.Command" },
];

/// Канонический [`EntitySpec`] вида `AccumulationRegister` (кэш на процесс).
pub fn accumulation_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Декларация ВАРИАДНОГО платформенного `standardAttributes` (data-driven; §1.6).
// Читается обобщённым кодеком `formats_xml::std_attrs_generic` в ОБА XML-формата
// (ERP-дамп Designer 8.3.27 НЕСЁТ AccumulationRegisters — 223 объекта с блоком);
// cf-сторона — `formats_brace::std_attrs_ir::ACCUMULATION_REGISTER` (слоты зеркалятся).
// Три альтернативных набора корневых атрибутов, различимых по ДЛИНЕ:
//   • Balance  (registerType омитится) ⇒ 5: RecordType, Active, LineNumber, Recorder, Period
//   • Turnovers (registerType=Turnovers) ⇒ 4: Active, LineNumber, Recorder, Period (без RecordType)
//   • блок отсутствует ⇒ 0 (пустой набор)
// Кодек выбирает вариант по числу блоков (read) / числу IR-записей (write) — БЕЗ
// повторного чтения registerType (длины 5/4/0 попарно-различны; ERP designer: 128/95/10).
// Условный атрибут (RecordType) = разница между 5- и 4-набором; опциональный блок = 0-набор.
//
// Форма листьев ОДНА для всех атрибутов: dataHistory=Use, name, [synonym?, toolTip?
// — variable, witnessed только у Period], fillValue=Undefined(const), [fillChecking —
// variable, дефолт DontCheck; корпус несёт ShowError у Period], fullTextSearch —
// variable (корпус несёт Use у всех), minValue/maxValue=Undefined. fillChecking/
// fullTextSearch — ЗАДАВАЕМЫЕ поля (класс багов «константа вместо переменной»:
// witnessed-вариативность на том же плотном регионе InformationRegister/Catalog).
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок). Witnessed-вариация: synonym/toolTip у Period
// (designer ПланыПроизводства/РезервыПредстоящихРасходов), fillChecking=ShowError у
// Period, fullTextSearch=Use у всех.
const S_SYNONYM: usize = 0;
const S_TOOLTIP: usize = 1;
const S_FILL_CHECKING: usize = 2;
const S_FULL_TEXT: usize = 3;

/// Полный (Balance) набор — 5 атрибутов. RecordType присутствует ТОЛЬКО здесь
/// (условный атрибут: registerType==Balance).
const BALANCE_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "RecordType", name_consts: &[] },
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
];

/// Turnovers набор — 4 атрибута (БЕЗ RecordType). Прочие идентичны Balance.
const TURNOVERS_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
];

/// Три альтернативных набора корневых атрибутов (попарно-различные длины 5/4/0).
/// Порядок в массиве не важен — выбор по длине.
const ROOT_VARIANTS: &[&[AttrDecl]] = &[BALANCE_ATTRS, TURNOVERS_ATTRS, &[]];

/// Variable-слоты (типы/дефолты) в EDT-порядке.
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Enum("DontCheck"), // fillChecking (Period несёт ShowError)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (корпус несёт Use)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом). synonym/toolTip —
/// опциональны; fillChecking омитится при DontCheck (корпус: присутствует только у
/// Period=ShowError); fullTextSearch омитится при DontUse (корпус: всегда Use).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::ConstValueUndef { tag: "fillValue" },
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

/// Designer-DENSE 25 листьев (порядок фикс — канонический регион, тот же, что у
/// Catalog/InformationRegister; сверено ERP designer-дампом 8.3.27, первый объект —
/// АвансовыеПлатежиИностранцевПоНДФЛ, 5 атрибутов × 25 листьев).
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
    ("Format", DenseLeaf::Empty),
    ("ChoiceForm", DenseLeaf::Empty),
    ("QuickChoice", DenseLeaf::Text("Auto")),
    ("ChoiceHistoryOnInput", DenseLeaf::Text("Auto")),
    ("EditFormat", DenseLeaf::Empty),
    ("PasswordMode", DenseLeaf::Text("false")),
    ("DataHistory", DenseLeaf::Text("Use")),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(S_SYNONYM)),
    ("Comment", DenseLeaf::Empty),
    ("FullTextSearch", DenseLeaf::VarEnum(S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::Nil),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация ВАРИАДНОГО `standardAttributes` вида `AccumulationRegister` (data-driven).
/// `root_attrs` не используется (вариадный путь `root_variants`); `tabular_attrs` — `&[]`.
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    // Вариадный (не фиксированный) набор: выбор по числу блоков/записей. root_attrs
    // остаётся как fallback единичного варианта — здесь пуст (не используется).
    root_attrs: &[],
    root_variants: ROOT_VARIANTS,
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "AccumulationRegister",
    label: "AccumulationRegister",
};

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`) вида В КАНОНИЧЕСКОМ
// ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive. Читается обобщённым кодеком
// `formats_xml::produced_types` через реестр; build.rs подхватывает `pub const` по имени.
// EDT-порядок (сверено): selectionType, listType, managerType, recordSetType,
// recordKeyType, recordType.
// ============================================================================
/// Категории producedTypes вида `AccumulationRegister` в каноническом порядке IR (= EDT).
/// Designer-порядок/type-name'ы объявлены для полноты, НО Designer-проекции у регистра
/// нет (edt-only) — они не используются (нет designer-харнесса).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "AccumulationRegisterSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "AccumulationRegisterList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "AccumulationRegisterManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "AccumulationRegisterRecordSet",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordKey",
        edt_tag: "recordKeyType",
        designer_category: "RecordKey",
        designer_type_name: "AccumulationRegisterRecordKey",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "AccumulationRegisterRecord",
        designer_order: 0,
    },
];
