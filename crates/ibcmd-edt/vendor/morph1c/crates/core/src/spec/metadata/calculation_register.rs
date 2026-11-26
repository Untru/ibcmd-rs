//! Канонический спек вида объекта `CalculationRegister` (регистр расчёта) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate.
//! Семейство ERP-регистров; клон `AccumulationRegister`, адаптированный под свойства
//! регистра РАСЧЁТА: `periodicity`/`actionPeriod`/`basePeriod`/`schedule*`/
//! `chartOfCalculationTypes` (вместо `registerType`/`enableTotalsSplitting`).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для EDT
//! `.mdo` и Designer `.xml`; cf-проекция — зеркало (`formats-cf/metadata`). Каноника
//! (id/value_kind/default/порядок/нормализация) — здесь; форматы лишь проецируют (§1.6).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ Designer-DENSE (сверено designer-фикстурами
//! `coverage/designer/s2_registers/CalculationRegisters`): synonym, comment,
//! useStandardCommands, defaultListForm, auxiliaryListForm, periodicity, actionPeriod,
//! basePeriod, schedule, scheduleValue, scheduleDate, chartOfCalculationTypes,
//! includeHelpInContents, [help — EDT-only], dataLockControlMode, fullTextSearch,
//! listPresentation, extendedListPresentation, explanation. EDT (sparse) омитит дефолты;
//! его физ.порядок = подмножество этого (сверено EDT-фикстурами).
//!
//! **standardAttributes** — ВАРИАДНЫЙ блок (11 атрибутов / опущен; см. [`STD_ATTRS`],
//! ERP-witnessed Начисления/Удержания). Корпус покрытия — 0-вариант; cf-каркас пока
//! держит слот frame-const `{0}` (поле с дефолтом `[]`, не required — иначе
//! MissingRequired на cf-read). ВНЕ спека: `name`/`uuid` — идентичность;
//! `producedTypes` — платформенный [`crate::ir::InternalInfo`]-каркас (см.
//! [`PRODUCED_CATEGORIES`], 7 категорий: +RecalculationsManager). Дочерние коллекции —
//! [`ChildSlot`] (Resource/Attribute/Dimension/Form/Template/Command; Recalculation в
//! корпусе покрытия пуста).

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
/// `auxiliaryListForm` — form-ref. Default "".
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(5);
/// `periodicity` — периодичность расчёта. Default Year (EDT омитит Year, эмитит иное).
pub const F_PERIODICITY: FieldId = FieldId(6);
/// `actionPeriod` — период действия. Default false.
pub const F_ACTION_PERIOD: FieldId = FieldId(7);
/// `basePeriod` — базовый период. Default false.
pub const F_BASE_PERIOD: FieldId = FieldId(8);
/// `schedule` — ref → InformationRegister. Default "".
pub const F_SCHEDULE: FieldId = FieldId(9);
/// `scheduleValue` — ref → InformationRegisterResource. Default "".
pub const F_SCHEDULE_VALUE: FieldId = FieldId(10);
/// `scheduleDate` — ref → InformationRegisterDimension. Default "".
pub const F_SCHEDULE_DATE: FieldId = FieldId(11);
/// `chartOfCalculationTypes` — ref → ChartOfCalculationTypes (обязательный план видов
/// расчёта). Default "" (в корпусе покрытия ВСЕГДА непустой). cf — object-ref (body[22]).
pub const F_CHART_OF_CALCULATION_TYPES: FieldId = FieldId(12);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(13);
/// `help` — EDT-only const-блок (presence). Default false; X-исключён (`x_ignore`).
pub const F_HELP: FieldId = FieldId(14);
/// `dataLockControlMode` — Default Automatic (EDT омитит Automatic, эмитит иное).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(15);
/// `fullTextSearch` — полнотекстовый поиск. Default DontUse (EDT омитит).
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(16);
/// `listPresentation` — локализ. представление списка. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(17);
/// `extendedListPresentation` — локализ. расширенное представление списка. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(18);
/// `explanation` — локализ. пояснение. Default [].
pub const F_EXPLANATION: FieldId = FieldId(19);
/// `standardAttributes` — ВАРИАДНЫЙ платформенный блок (см. [`STD_ATTRS`]; 11 атрибутов
/// либо блок опущен). Дефолт `[]` (0-вариант; cf-каркас держит слот frame-const `{0}` и
/// поле НЕ проецирует — required дал бы MissingRequired на cf-read). Физически: EDT ПОСЛЕ
/// chartOfCalculationTypes (witnessed Начисления/Удержания; includeHelpInContents/help в
/// корпусе омитятся — их взаимный порядок с блоком принят по Designer); Designer МЕЖДУ
/// IncludeHelpInContents и DataLockControlMode (witnessed оба) — здесь после help.
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(20);

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
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, empty_str())
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        FieldSpec::with_default(F_USE_STANDARD_COMMANDS, "useStandardCommands", ValueKind::Bool, bool_f()),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),
        // periodicity: Default Year (EDT омитит Year). literals: {Year|Quarter|Month|Day}.
        FieldSpec::with_default(F_PERIODICITY, "periodicity", ValueKind::Enum, enum_tok("Year")),
        FieldSpec::with_default(F_ACTION_PERIOD, "actionPeriod", ValueKind::Bool, bool_f()),
        FieldSpec::with_default(F_BASE_PERIOD, "basePeriod", ValueKind::Bool, bool_f()),
        s(F_SCHEDULE, "schedule"),
        s(F_SCHEDULE_VALUE, "scheduleValue"),
        s(F_SCHEDULE_DATE, "scheduleDate"),
        s(F_CHART_OF_CALCULATION_TYPES, "chartOfCalculationTypes"),
        FieldSpec::with_default(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents", ValueKind::Bool, bool_f()),
        // help: EDT-only presence-блок. Default false; X-исключён (Designer/cf n/a).
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        // standardAttributes: вариадный платформенный блок (11/0), IR-List, дефолт [] —
        // cf-проекция вида держит слот frame-const {0} и поле не проецирует.
        FieldSpec::with_default(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List, PropertyValue::List(Vec::new())),
        // dataLockControlMode: literals {Automatic|Managed|AutomaticAndManaged}. Default Automatic.
        FieldSpec::with_default(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", ValueKind::Enum, enum_tok("Automatic")),
        // fullTextSearch: literals {DontUse|Use}. Default DontUse.
        FieldSpec::with_default(F_FULL_TEXT_SEARCH, "fullTextSearch", ValueKind::Enum, enum_tok("DontUse")),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено EDT/Designer-корпусом
/// s2+s15: R < A < D < F < T < C, как InformationRegister). Recalculation в корпусе
/// покрытия ПУСТА (cf-каркас эмитит пустую коллекцию) → не объявлена.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Resource", child_kind: "CalculationRegister.Resource" },
    ChildSlot { collection: "Attribute", child_kind: "CalculationRegister.Attribute" },
    ChildSlot { collection: "Dimension", child_kind: "CalculationRegister.Dimension" },
    ChildSlot { collection: "Form", child_kind: "CalculationRegister.FormRef" },
    ChildSlot { collection: "Template", child_kind: "CalculationRegister.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "CalculationRegister.Command" },
];

// ============================================================================
// Декларация ВАРИАДНОГО платформенного `standardAttributes` (data-driven; §1.6).
// ERP-корпус (2 регистра расчёта: Начисления, Удержания; edt+designer согласны) — ОДИН
// witnessed-набор из 11 атрибутов (у Удержания actionPeriod=false, но набор ТОТ ЖЕ):
//   RegistrationPeriod, ReversingEntry, Active, EndOfBasePeriod, BegOfBasePeriod,
//   EndOfActionPeriod, BegOfActionPeriod, ActionPeriod, CalculationType, LineNumber,
//   Recorder
// плюс 0-вариант (блок опущен — корпус покрытия). Выбор по длине (11/0 различимы).
// fillChecking несут RegistrationPeriod/EndOfActionPeriod/BegOfActionPeriod/
// CalculationType (=ShowError, оба файла); прочее — дефолт. cf-markers witnessed erp.cf:
// [-13,-11,-10,-9,-8,-7,-6,-5,-4,-3,-2] в том же порядке (маркер -12 в наборе не
// встречается — вероятно, атрибут другого варианта; не догадываемся).
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок). synonym/toolTip в корпусе пусты, но задаваемы
// (класс багов «константа вместо переменной») — объявлены слотами, как у AccumulationRegister.
const S_SYNONYM: usize = 0;
const S_TOOLTIP: usize = 1;
const S_FILL_CHECKING: usize = 2;
const S_FULL_TEXT: usize = 3;

/// Полный witnessed-набор — 11 атрибутов (Начисления/Удержания, оба формата).
const FULL_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "RegistrationPeriod", name_consts: &[] },
    AttrDecl { name: "ReversingEntry", name_consts: &[] },
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "EndOfBasePeriod", name_consts: &[] },
    AttrDecl { name: "BegOfBasePeriod", name_consts: &[] },
    AttrDecl { name: "EndOfActionPeriod", name_consts: &[] },
    AttrDecl { name: "BegOfActionPeriod", name_consts: &[] },
    AttrDecl { name: "ActionPeriod", name_consts: &[] },
    AttrDecl { name: "CalculationType", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
];

/// Два witnessed-набора (длины 11/0 попарно-различны — выбор по счётчику).
const ROOT_VARIANTS: &[&[AttrDecl]] = &[FULL_ATTRS, &[]];

/// Variable-слоты (типы/дефолты) в EDT-порядке.
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Enum("DontCheck"), // fillChecking (4 атрибута несут ShowError)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (корпус несёт Use)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом: dataHistory, name, fillValue,
/// [fillChecking], fullTextSearch, minValue, maxValue; synonym/toolTip в корпусе пусты —
/// позиции стандартные, между name и fillValue).
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

/// Designer-DENSE 25 листьев (порядок фикс — канонический регион; сверено ERP
/// designer-дампом 8.3.27: Начисления/Удержания, 11 атрибутов × 25 листьев).
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

/// Декларация ВАРИАДНОГО `standardAttributes` вида `CalculationRegister` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: &[],
    root_variants: ROOT_VARIANTS,
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "CalculationRegister",
    label: "CalculationRegister",
};

/// Канонический [`EntitySpec`] вида `CalculationRegister` (кэш на процесс).
pub fn calculation_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CalculationRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`) вида В КАНОНИЧЕСКОМ
// ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive. Читается обобщённым кодеком
// `formats_xml::produced_types`. EDT-порядок (сверено): selectionType, listType,
// managerType, recordSetType, recordKeyType, recordType, recalcsType. Отличие от
// AccumulationRegister — 7-я категория Recalcs (RecalculationsManager).
// ============================================================================
/// Категории producedTypes вида `CalculationRegister` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "CalculationRegisterSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "CalculationRegisterList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "CalculationRegisterManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "CalculationRegisterRecordSet",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordKey",
        edt_tag: "recordKeyType",
        designer_category: "RecordKey",
        designer_type_name: "CalculationRegisterRecordKey",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "CalculationRegisterRecord",
        designer_order: 0,
    },
    // Recalcs (RecalculationsManager) — доп. категория регистра расчёта (перерасчёты).
    crate::spec::common::ProducedCategory {
        category: "Recalcs",
        edt_tag: "recalcsType",
        designer_category: "Recalcs",
        designer_type_name: "RecalculationsManager",
        designer_order: 6,
    },
];
