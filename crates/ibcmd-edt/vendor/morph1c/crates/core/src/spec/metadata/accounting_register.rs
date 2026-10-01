//! Канонический спек вида объекта `AccountingRegister` (регистр бухгалтерии) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate.
//! Клон структурного `accumulation_register.rs`, адаптированный под РЕГИСТР БУХГАЛТЕРИИ:
//! свойства chartOfAccounts(ref)/correspondence(bool)/periodAdjustmentLength(int) вместо
//! registerType; producedTypes 7 категорий (доп. ExtDimensions против регистра накопления).
//!
//! Вид покрыт ВСЕМИ тремя форматами (edt+designer+cf) — фикстуры
//! `coverage/{edt,designer,cf}/s2_registers/AccountingRegisters` (15 объектов).
//!
//! `standardAttributes` — ВАРИАДНЫЙ блок с наборами РАВНОЙ длины (ERP: 12/12/11; корпус
//! покрытия — 0-вариант): выбор варианта по последовательности ИМЁН, имя в IR-слоте
//! (первый NameVar-вид; см. [`STD_ATTRS`]). ВСЕ ТРИ формата проецируют поле; cf-сторона —
//! `formats_brace::std_attrs_ir::ACCOUNTING_REGISTER` (markers witnessed в erp.cf: -30
//! PeriodAdjustment, -10 Account, -9 RecordType, -5..-2, ExtDim-ПАРЫ `{K-1, guid}`;
//! linkByType — marker-ref `{-10}` с link-item = N из имени `ExtDimensionN`).
//!
//! Порядок полей = КАНОНИЧЕСКИЙ Designer-DENSE (сверено designer-фикстурами): synonym,
//! comment, useStandardCommands, includeHelpInContents, [help — EDT-only], chartOfAccounts,
//! correspondence, periodAdjustmentLength, defaultListForm, auxiliaryListForm,
//! dataLockControlMode, enableTotalsSplitting, fullTextSearch, listPresentation,
//! extendedListPresentation, explanation. EDT (sparse) опускает дефолты; порядок неизменен.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize, StdAttrsDecl,
    VarSlotKind,
};

/// `synonym` — локализ. синоним. Default []; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `includeHelpInContents` — включать справку в содержание. Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(4);
/// `help` — EDT-only const-блок (presence). Default false; X-исключён (`x_ignore`).
pub const F_HELP: FieldId = FieldId(5);
/// `chartOfAccounts` — ref → ChartOfAccounts. Default "" (пуст в корпусе покрытия; в ERP
/// задан у всех 3 регистров — cf-слот несёт object-uuid плана счетов).
pub const F_CHART_OF_ACCOUNTS: FieldId = FieldId(6);
/// `correspondence` — признак корреспонденции. Default false.
pub const F_CORRESPONDENCE: FieldId = FieldId(7);
/// `periodAdjustmentLength` — длина корректировки периода (int). Default 0 (EDT омитит;
/// Designer DENSE `<PeriodAdjustmentLength>0`). В ERP `Хозрасчетный` несёт 1 — и это
/// ДИСКРИМИНАТОР cf-раскладки (record-code 21 ⇄ 22, см. `formats_cf` коннектор вида).
pub const F_PERIOD_ADJUSTMENT_LENGTH: FieldId = FieldId(8);
/// `defaultListForm` — form-ref на СОБСТВЕННУЮ форму. Default "" (в ERP задан у
/// `Международный`/`Хозрасчетный`, пуст у `МеждународныйБезКорреспонденции`).
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(9);
/// `auxiliaryListForm` — вспомогательная форма списка (form-ref). Default "".
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(10);
/// `dataLockControlMode` — Default Automatic (EDT омитит Automatic, эмитит Managed).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(11);
/// `enableTotalsSplitting` — Default false.
pub const F_ENABLE_TOTALS_SPLITTING: FieldId = FieldId(12);
/// `fullTextSearch` — полнотекстовый поиск. Default DontUse (EDT омитит).
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(13);
/// `listPresentation` — локализ. представление списка. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(14);
/// `extendedListPresentation` — локализ. расширенное представление списка. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(15);
/// `explanation` — локализ. пояснение. Default [].
pub const F_EXPLANATION: FieldId = FieldId(16);
/// `standardAttributes` — ВАРИАДНЫЙ платформенный блок (см. [`STD_ATTRS`]): наборы
/// атрибутов РАВНОЙ длины (12/12/11/0) → выбор варианта по ПОСЛЕДОВАТЕЛЬНОСТИ ИМЁН,
/// имя атрибута живёт в IR-слоте (`EdtLeaf::NameVar`). Required (всегда present).
/// Физически: EDT МЕЖДУ defaultListForm и enableTotalsSplitting (witnessed Хозрасчетный);
/// Designer МЕЖДУ AuxiliaryListForm и DataLockControlMode (witnessed все 3) — здесь,
/// в спек-порядке, после auxiliaryListForm (обе проекции согласны).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(17);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        // help: EDT-only presence-блок; X-исключён (Designer/cf n/a).
        b(F_HELP, "help").x_ignored(),
        s(F_CHART_OF_ACCOUNTS, "chartOfAccounts"),
        b(F_CORRESPONDENCE, "correspondence"),
        FieldSpec::with_default(F_PERIOD_ADJUSTMENT_LENGTH, "periodAdjustmentLength", ValueKind::Int, PropertyValue::Int(0)),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),
        // standardAttributes: вариадный платформенный блок, IR-List. Дефолт — пустой List
        // (0-вариант = cf-слот `{0}`; corpus покрытия весь такой). Дефолт, а НЕ required:
        // 0-вариант законен во всех трёх форматах. См. [`STD_ATTRS`].
        FieldSpec::with_default(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List, PropertyValue::List(Vec::new())),
        FieldSpec::with_default(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", ValueKind::Enum, PropertyValue::Enum(Token::new("Automatic"))),
        b(F_ENABLE_TOTALS_SPLITTING, "enableTotalsSplitting"),
        FieldSpec::with_default(F_FULL_TEXT_SEARCH, "fullTextSearch", ValueKind::Enum, PropertyValue::Enum(Token::new("DontUse"))),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке = физ.порядок .mdo/.xml (сверено:
/// Dimension, Resource, Attribute; Form/Template/Command в корпусе покрытия пусты). Этот
/// порядок задаёт IR-порядок детей ВО ВСЕХ форматах (edt/designer read группируют по
/// `children()`; cf read собирает В ТОТ ЖЕ порядок) → X сравнивает дети позиционно.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Dimension", child_kind: "AccountingRegister.Dimension" },
    ChildSlot { collection: "Resource", child_kind: "AccountingRegister.Resource" },
    ChildSlot { collection: "Attribute", child_kind: "AccountingRegister.Attribute" },
    ChildSlot { collection: "Form", child_kind: "AccountingRegister.FormRef" },
    ChildSlot { collection: "Template", child_kind: "AccountingRegister.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "AccountingRegister.Command" },
];

// ============================================================================
// Декларация ВАРИАДНОГО платформенного `standardAttributes` (data-driven; §1.6).
// ERP-корпус (3 регистра бухгалтерии, edt+designer согласны) — наборы:
//   • Хозрасчетный (correspondence, periodAdjustmentLength>0) ⇒ 12:
//     PeriodAdjustment, Account, Active, LineNumber, Recorder, Period, ExtDim×3 пары;
//   • МеждународныйБезКорреспонденции (correspondence=false) ⇒ 12:
//     Account, RecordType, Active, LineNumber, Recorder, Period, ExtDim×3 пары;
//   • Международный (correspondence, PA=0) ⇒ 11: без PeriodAdjustment/RecordType;
//   • корпус покрытия (15 объектов) ⇒ 0 (блок опущен).
// Наборы РАВНОЙ длины (12/12) → длина НЕ дискриминирует: выбор варианта идёт по
// ПОСЛЕДОВАТЕЛЬНОСТИ ИМЁН, а имя атрибута хранится в IR-слоте ([`EdtLeaf::NameVar`] —
// на write имена наблюдаемы только из записей). Число ExtDimension-пар = maxExtDimensions
// плана счетов; witnessed ТОЛЬКО 3 пары — иное число даст громкую §1.0-ошибку выбора
// (расширять — новыми наборами, не догадкой).
//
// linkByType у ExtDimensionN — ССЫЛКА на Account с link-item=N (имя-функция; см.
// `formats_xml::std_attrs_generic::lbt_item`): EDT `<linkByType><linkItem>N</linkItem>
// <field>…StandardAttribute.Account</field>`, Designer `<xr:DataPath>…</xr:DataPath>
// <xr:LinkItem>N</xr:LinkItem>`; cf `{"#",9ad557b1,{3,1,{-10},N}}` (witnessed erp.cf).
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок). Witnessed-вариация: synonym у Period (все 3)
// и Recorder (Хозрасчетный), fillChecking=ShowError у Period, linkByType у ExtDimensionN,
// fullTextSearch=Use у всех.
const S_NAME: usize = 0;
const S_SYNONYM: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FILL_CHECKING: usize = 3;
const S_LINK_BY_TYPE: usize = 4;
const S_FULL_TEXT: usize = 5;

/// Базовый (correspondence, без периода корректировки) набор — 11 атрибутов
/// (witnessed Международный).
const BASE_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Account", name_consts: &[] },
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
    AttrDecl { name: "ExtDimension1", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType1", name_consts: &[] },
    AttrDecl { name: "ExtDimension2", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType2", name_consts: &[] },
    AttrDecl { name: "ExtDimension3", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType3", name_consts: &[] },
];

/// +PeriodAdjustment ПЕРВЫМ (periodAdjustmentLength>0; witnessed Хозрасчетный) — 12.
const PERIOD_ADJUSTMENT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "PeriodAdjustment", name_consts: &[] },
    AttrDecl { name: "Account", name_consts: &[] },
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
    AttrDecl { name: "ExtDimension1", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType1", name_consts: &[] },
    AttrDecl { name: "ExtDimension2", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType2", name_consts: &[] },
    AttrDecl { name: "ExtDimension3", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType3", name_consts: &[] },
];

/// +RecordType ВТОРЫМ (correspondence=false; witnessed МеждународныйБезКорреспонденции) — 12.
const RECORD_TYPE_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Account", name_consts: &[] },
    AttrDecl { name: "RecordType", name_consts: &[] },
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
    AttrDecl { name: "ExtDimension1", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType1", name_consts: &[] },
    AttrDecl { name: "ExtDimension2", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType2", name_consts: &[] },
    AttrDecl { name: "ExtDimension3", name_consts: &[] },
    AttrDecl { name: "ExtDimensionType3", name_consts: &[] },
];

/// Четыре witnessed-набора; выбор — по последовательности имён (NameVar-слот).
const ROOT_VARIANTS: &[&[AttrDecl]] = &[BASE_ATTRS, PERIOD_ADJUSTMENT_ATTRS, RECORD_TYPE_ATTRS, &[]];

/// Variable-слоты (типы/дефолты) в EDT-порядке. Слот 0 — ИМЯ атрибута (NameVar).
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Str,               // name (NameVar — дискриминатор варианта)
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Enum("DontCheck"), // fillChecking (Period несёт ShowError)
    VarSlotKind::Str,               // linkByType (путь; ExtDimensionN → Account)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch (корпус несёт Use)
];

/// EDT-листья блока (позиционный порядок, сверено корпусом: Хозрасчетный — synonym у
/// Recorder/Period, fillChecking у Period, linkByType у ExtDimensionN). Взаимный порядок
/// fillChecking/linkByType корпусом НЕ витнессится (ни один блок не несёт оба) — принят
/// реквизитный EDT-топопорядок (fillChecking < linkByType, см. dimension-order).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::NameVar(S_NAME),
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::ConstValueUndef { tag: "fillValue" },
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptLbt("linkByType", S_LINK_BY_TYPE),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

/// Designer-DENSE 25 листьев (порядок фикс — канонический регион; сверено ERP
/// designer-дампом 8.3.27, все 3 регистра бухгалтерии).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::VarLbt(S_LINK_BY_TYPE)),
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

/// Декларация ВАРИАДНОГО `standardAttributes` вида `AccountingRegister` (data-driven;
/// NameVar-выбор варианта). `root_attrs` не используется; `tabular_attrs` — `&[]`.
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: &[],
    root_variants: ROOT_VARIANTS,
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "AccountingRegister",
    label: "AccountingRegister",
};

/// Канонический [`EntitySpec`] вида `AccountingRegister` (кэш на процесс).
pub fn accounting_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccountingRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`) — §1.6/§3.5. Читается
// обобщённым кодеком `formats_xml::produced_types` через реестр (build.rs подхватывает по
// имени `PRODUCED_CATEGORIES`). Порядок массива = КАНОНИЧЕСКИЙ IR/EDT-порядок
// `<producedTypes>` (сверено .mdo): selectionType, listType, managerType, recordSetType,
// recordKeyType, recordType, extDimensionsType. Designer-порядок `<InternalInfo>` (сверено
// .xml): Record, ExtDimensions, RecordSet, RecordKey, Selection, List, Manager → `designer_order`.
// ============================================================================
/// Категории producedTypes вида `AccountingRegister` (7; доп. ExtDimensions против регистра
/// накопления) в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "AccountingRegisterSelection",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "AccountingRegisterList",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "AccountingRegisterManager",
        designer_order: 6,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "AccountingRegisterRecordSet",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordKey",
        edt_tag: "recordKeyType",
        designer_category: "RecordKey",
        designer_type_name: "AccountingRegisterRecordKey",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "AccountingRegisterRecord",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "ExtDimensions",
        edt_tag: "extDimensionsType",
        designer_category: "ExtDimensions",
        designer_type_name: "AccountingRegisterExtDimensions",
        designer_order: 1,
    },
];
