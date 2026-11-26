//! Канонический спек вида объекта `InformationRegister` (регистр сведений) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate.
//! ПЕРВЫЙ СТРУКТУРНЫЙ вид (pattern-setter для Catalog/Document/Task): полный набор
//! root-свойств + дочерние коллекции Resource/Attribute/Dimension (полные leaf-дети) +
//! Command (полный leaf-дитя) + Form/Template (ref/stub-коллекции, X по имени).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Каноника (id/value_kind/default/
//! порядок/нормализация) — здесь; формат лишь проецирует (§1.6).
//!
//! Порядок свойств = каноническому DENSE-порядку Designer `<Properties>` (сверено
//! 187/187), с EDT-only `<help>` после `includeHelpInContents` (сверено: EDT-порядок =
//! Designer-порядок + help в этой позиции, 0 нарушений). EDT эмитит РАЗРЕЖЁННО, Designer
//! DENSE. Дефолты выведены из Designer-dense-majority + EDT-омиссии (сверено).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! [`crate::ir::InternalInfo`]-блок (каркас). `standardAttributes` — ВАРИАТИВНЫЙ
//! платформенный блок (4 атрибута / опущен; data-driven [`STD_ATTRS`], codec
//! `formats_xml::std_attrs_generic`). Дочерние коллекции — [`ChildSlot`] (рекурсия движка).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize, StdAttrsDecl,
    VarSlotKind,
};

/// `synonym` — локализованный синоним. Default = []; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий. Default = "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `useStandardCommands` — Default = false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(3);
/// `editType` — режим редактирования. Default = InList (EDT омитит InList).
pub const F_EDIT_TYPE: FieldId = FieldId(4);
/// `defaultRecordForm` — form-ref. Default = "".
pub const F_DEFAULT_RECORD_FORM: FieldId = FieldId(5);
/// `defaultListForm` — form-ref. Default = "".
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(6);
/// `auxiliaryRecordForm` — form-ref. Default = "".
pub const F_AUXILIARY_RECORD_FORM: FieldId = FieldId(7);
/// `auxiliaryListForm` — form-ref. Default = "".
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(8);
/// `standardAttributes` — ВАРИАТИВНЫЙ платформенный блок (4 атрибута либо блок опущен —
/// ERP-witnessed 0-вариант, 164/1793). Required (всегда present); IR — `List` из
/// N×`List` variable-слотов (см. [`STD_ATTRS`]; codec `formats_xml::std_attrs_generic`,
/// cf — `formats_brace::std_attrs_ir`).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(9);
/// `informationRegisterPeriodicity` — Default = Nonperiodical.
pub const F_PERIODICITY: FieldId = FieldId(10);
/// `writeMode` — Default = Independent.
pub const F_WRITE_MODE: FieldId = FieldId(11);
/// `mainFilterOnPeriod` — Default = false. X-исключён (`x_ignore`): cf хранит СЫРОЙ бит в
/// body[23] (платформенный дефолт 1), а edt/Designer поверхностят ЭФФЕКТИВНОЕ значение
/// (`raw AND периодичность != Nonperiodical`) — для непериодических регистров cf-бит остаётся
/// 1, но edt/Designer показывают false. Байт-точный R требует СЫРОЙ бит (иначе теряется
/// `РегСв_ОсновнойОтборПоПериоду_Ложь`, единственный с body[23]=0), а X требует эффективное
/// значение — примирить в одном IR-поле нельзя, поэтому поле снимается из X (как EDT-only `help`).
pub const F_MAIN_FILTER_ON_PERIOD: FieldId = FieldId(12);
/// `includeHelpInContents` — Default = false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(13);
/// `help` — EDT-only const-блок (presence). Default = false; X-исключён (`x_ignore`).
pub const F_HELP: FieldId = FieldId(14);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(27);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(28);
/// `dataLockControlMode` — Default = Automatic (EDT омитит Automatic).
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(15);
/// `fullTextSearch` — Default = DontUse.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(16);
/// `enableTotalsSliceFirst` — Default = false.
pub const F_ENABLE_TOTALS_SLICE_FIRST: FieldId = FieldId(17);
/// `enableTotalsSliceLast` — Default = false.
pub const F_ENABLE_TOTALS_SLICE_LAST: FieldId = FieldId(18);
/// `recordPresentation` — локализ. Default = [].
pub const F_RECORD_PRESENTATION: FieldId = FieldId(19);
/// `extendedRecordPresentation` — локализ. Default = [].
pub const F_EXTENDED_RECORD_PRESENTATION: FieldId = FieldId(20);
/// `listPresentation` — локализ. Default = [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(21);
/// `extendedListPresentation` — локализ. Default = [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(22);
/// `explanation` — локализ. Default = [].
pub const F_EXPLANATION: FieldId = FieldId(23);
/// `dataHistory` — Default = DontUse.
pub const F_DATA_HISTORY: FieldId = FieldId(24);
/// `updateDataHistoryImmediatelyAfterWrite` — Default = false.
pub const F_UPDATE_DATA_HISTORY_IMMEDIATELY: FieldId = FieldId(25);
/// `executeAfterWriteDataHistoryVersionProcessing` — Default = false.
pub const F_EXECUTE_AFTER_WRITE_DH: FieldId = FieldId(26);

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
    vec![
        loc_field(F_SYNONYM, "synonym"),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_USE_STANDARD_COMMANDS, "useStandardCommands", ValueKind::Bool, bool_f()),
        FieldSpec::with_default(F_EDIT_TYPE, "editType", ValueKind::Enum, enum_tok("InList")),
        FieldSpec::with_default(F_DEFAULT_RECORD_FORM, "defaultRecordForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_DEFAULT_LIST_FORM, "defaultListForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_AUXILIARY_RECORD_FORM, "auxiliaryRecordForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_AUXILIARY_LIST_FORM, "auxiliaryListForm", ValueKind::Str, empty_str()),
        // standardAttributes: вариативный const-блок, IR-List, БЕЗ дефолта (всегда present).
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        FieldSpec::with_default(F_PERIODICITY, "informationRegisterPeriodicity", ValueKind::Enum, enum_tok("Nonperiodical")),
        FieldSpec::with_default(F_WRITE_MODE, "writeMode", ValueKind::Enum, enum_tok("Independent")),
        // mainFilterOnPeriod: cf raw-бит (body[23]) ≠ edt/Designer эффективное значение для
        // непериодических → X-исключён (см. док F_MAIN_FILTER_ON_PERIOD). R сохраняет сырой бит.
        FieldSpec::with_default(F_MAIN_FILTER_ON_PERIOD, "mainFilterOnPeriod", ValueKind::Bool, bool_f()).x_ignored(),
        FieldSpec::with_default(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents", ValueKind::Bool, bool_f()),
        // help: EDT-only presence-блок. Default false; X-исключён (Designer не несёт).
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        FieldSpec::with_default(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", ValueKind::Enum, enum_tok("Automatic")),
        FieldSpec::with_default(F_FULL_TEXT_SEARCH, "fullTextSearch", ValueKind::Enum, enum_tok("DontUse")),
        FieldSpec::with_default(F_ENABLE_TOTALS_SLICE_FIRST, "enableTotalsSliceFirst", ValueKind::Bool, bool_f()),
        FieldSpec::with_default(F_ENABLE_TOTALS_SLICE_LAST, "enableTotalsSliceLast", ValueKind::Bool, bool_f()),
        loc_field(F_RECORD_PRESENTATION, "recordPresentation"),
        loc_field(F_EXTENDED_RECORD_PRESENTATION, "extendedRecordPresentation"),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        FieldSpec::with_default(F_DATA_HISTORY, "dataHistory", ValueKind::Enum, enum_tok("DontUse")),
        FieldSpec::with_default(F_UPDATE_DATA_HISTORY_IMMEDIATELY, "updateDataHistoryImmediatelyAfterWrite", ValueKind::Bool, bool_f()),
        FieldSpec::with_default(F_EXECUTE_AFTER_WRITE_DH, "executeAfterWriteDataHistoryVersionProcessing", ValueKind::Bool, bool_f()),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено 187/187, обе
/// половины: R < A < D < F < T < C, коллекции сгруппированы, не чередуются).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Resource", child_kind: "InformationRegister.Resource" },
    ChildSlot { collection: "Attribute", child_kind: "InformationRegister.Attribute" },
    ChildSlot { collection: "Dimension", child_kind: "InformationRegister.Dimension" },
    ChildSlot { collection: "Form", child_kind: "InformationRegister.FormRef" },
    ChildSlot { collection: "Template", child_kind: "InformationRegister.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "InformationRegister.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Читается
// обобщённым кодеком `formats_xml::std_attrs_generic` в ОБА XML-формата; cf-сторона —
// `formats_brace::std_attrs_ir::INFORMATION_REGISTER` (слоты ОБЯЗАНЫ зеркалиться по
// числу И порядку). Вид РАНЬШЕ жил на пер-видовом кодеке `formats_xml::std_attrs_ir`
// с 4 variable-слотами ([syn,tip,fill,fts]) — ERP-корпус (1793 регистра) вскрыл ещё 5
// задаваемых листьев (класс багов «константа вместо переменной»):
//   • dataHistory — НЕ константа Use: witnessed `DontUse`-омиссия у
//     РегСв НастройкиУчетаНалогаНаПрибыльГруппОбособленныхПодразделений (все 4 атрибута;
//     designer несёт текст `DontUse`, cf — prop 9288a8ed=0);
//   • fillChecking — НЕ имя-константа (Period⇒ShowError): witnessed `DontCheck`-омиссия
//     у Period в ПроцентныеСтавкиФинансовыхИнструментов и
//     УдалитьЛицевыеСчетаСотрудниковПоЗарплатнымПроектам (cf prop 2723eb98=0);
//   • fillFromFillingValue (4: напр. СтавкиНДСНоменклатуры/Period);
//   • format (18) / editFormat (12): напр. ПлановыеАвансы/Period «ДЛФ=D» ru+en.
// ============================================================================

// Variable-слоты IR-записи (EDT-порядок).
const S_DATA_HISTORY: usize = 0;
const S_SYNONYM: usize = 1;
const S_TOOLTIP: usize = 2;
const S_FILL_FROM: usize = 3;
const S_FILL_VALUE: usize = 4;
const S_FILL_CHECKING: usize = 5;
const S_FULL_TEXT: usize = 6;
const S_FORMAT: usize = 7;
const S_EDIT_FORMAT: usize = 8;

/// 4 предопределённых атрибута КОРНЯ (сверено 187/187 SSL + 1629/1629 ERP; ERP несёт
/// либо все 4, либо блок опущен целиком — 164 объекта → 0-вариант).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Active", name_consts: &[] },
    AttrDecl { name: "LineNumber", name_consts: &[] },
    AttrDecl { name: "Recorder", name_consts: &[] },
    AttrDecl { name: "Period", name_consts: &[] },
];

/// Наборы Root-атрибутов: полный ×4 либо блок опущен (0). Выбор — по числу блоков.
const ROOT_VARIANTS: &[&[AttrDecl]] = &[ROOT_ATTRS, &[]];

/// Variable-слоты (типы/дефолты) в EDT-порядке.
const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Enum("DontUse"),   // dataHistory (ERP-witnessed variable)
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Bool,              // fillFromFillingValue (ERP-witnessed)
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Enum("DontCheck"), // fillChecking (ERP-witnessed variable)
    VarSlotKind::Enum("DontUse"),   // fullTextSearch
    VarSlotKind::Loc,               // format (ERP-witnessed)
    VarSlotKind::Loc,               // editFormat (ERP-witnessed)
];

/// EDT-листья блока (позиционный порядок; позиции новых листьев — ERP-witnessed:
/// fillFromFillingValue МЕЖДУ toolTip и fillValue (РегСв
/// ДоступностьТоваровДляВнешнихПользователей/Period), format+editFormat МЕЖДУ
/// fullTextSearch и minValue (РегСв ВременноПребывающиеПринятыеПоДолгосрочнымДоговорам,
/// ПлановыеАвансы — оба с двуязычными multi-sibling контейнерами)).
const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::OptEnum("dataHistory", S_DATA_HISTORY),
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptLoc("toolTip", S_TOOLTIP),
    EdtLeaf::OptBool("fillFromFillingValue", S_FILL_FROM),
    EdtLeaf::ReqValue("fillValue", S_FILL_VALUE),
    EdtLeaf::OptEnum("fillChecking", S_FILL_CHECKING),
    EdtLeaf::OptEnum("fullTextSearch", S_FULL_TEXT),
    EdtLeaf::OptLoc("format", S_FORMAT),
    EdtLeaf::OptLoc("editFormat", S_EDIT_FORMAT),
    EdtLeaf::ConstValueUndef { tag: "minValue" },
    EdtLeaf::ConstValueUndef { tag: "maxValue" },
];

/// Designer-DENSE 25 листьев (порядок фикс — тот же канонический регион, что у Catalog;
/// сверено ERP designer-дампом 8.3.27).
const DENSE_LEAVES: &[(&str, DenseLeaf)] = &[
    ("LinkByType", DenseLeaf::Empty),
    ("FillChecking", DenseLeaf::VarEnum(S_FILL_CHECKING)),
    ("MultiLine", DenseLeaf::Text("false")),
    ("FillFromFillingValue", DenseLeaf::VarBool(S_FILL_FROM)),
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
    ("DataHistory", DenseLeaf::VarEnum(S_DATA_HISTORY)),
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

/// Декларация `standardAttributes` вида `InformationRegister` (data-driven).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: &[],
    root_variants: ROOT_VARIANTS,
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "InformationRegister",
    label: "InformationRegister",
};

/// Канонический [`EntitySpec`] вида `InformationRegister` (кэш на процесс).
pub fn information_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "InformationRegister",
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
/// Категории producedTypes вида `InformationRegister` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "InformationRegisterSelection",
        designer_order: 2,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "InformationRegisterList",
        designer_order: 3,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "InformationRegisterManager",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordSet",
        edt_tag: "recordSetType",
        designer_category: "RecordSet",
        designer_type_name: "InformationRegisterRecordSet",
        designer_order: 4,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordKey",
        edt_tag: "recordKeyType",
        designer_category: "RecordKey",
        designer_type_name: "InformationRegisterRecordKey",
        designer_order: 5,
    },
    crate::spec::common::ProducedCategory {
        category: "Record",
        edt_tag: "recordType",
        designer_category: "Record",
        designer_type_name: "InformationRegisterRecord",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "RecordManager",
        edt_tag: "recordManagerType",
        designer_category: "RecordManager",
        designer_type_name: "InformationRegisterRecordManager",
        designer_order: 6,
    },
];
