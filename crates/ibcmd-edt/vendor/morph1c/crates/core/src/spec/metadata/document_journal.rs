//! Канонический спек вида объекта `DocumentJournal` (журнал документов) —
//! ARCHITECTURE.md §1.4/§1.6, child-objects substrate. ВОСЬМОЙ СТРУКТУРНЫЙ вид (после
//! InformationRegister, Catalog, Document, Task, ChartOfCharacteristicTypes, ExchangePlan,
//! BusinessProcess): переиспользует ~85% субстрата Document/ExchangePlan
//! (Command/FormRef/TemplateRef-паттерн, children-рекурсию, ref-list/value/picture/help/
//! producedTypes-кодеки) + DocumentJournal-дельту:
//! * std_attrs_document_journal — 6-атрибутный набор `Type/Ref/Date/Posted/DeletionMark/
//!   Number` (ВСЕ fillChecking=DontCheck, в т.ч. Date — в отличие от Document, где
//!   Date=ShowError);
//! * `registeredDocuments` — список ссылок-документов (ref-list, item-style);
//! * `Column` — DocumentJournal-специфичная child-коллекция (графа журнала): name+uuid+
//!   synonym+comment+indexing+`references` (список ссылок на реквизиты документов).
//!
//! У DocumentJournal НЕТ Attributes/TabularSections (журнал — над-документная проекция,
//! сверено 2/2). Дети: Column < Form < Template < Command (сверено по корпусу).
//!
//! Порядок свойств = КАНОНИЧЕСКОМУ DENSE-порядку Designer `<Properties>` (сверено 2/2:
//! Synonym, Comment, DefaultForm, AuxiliaryForm, UseStandardCommands, RegisteredDocuments,
//! IncludeHelpInContents, StandardAttributes, ListPresentation, ExtendedListPresentation,
//! Explanation). EDT эмитит РАЗРЕЖЁННО; дефолты выведены из Designer-dense + EDT-омиссий.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (3 категории Selection/List/Manager, см. `produced_types`). EDT-only `help` —
//! X-исключён (`x_ignore`), но участвует в R EDT. `predefined` у журнала НЕТ.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{
    AttrDecl, ChildSlot, DenseLeaf, EdtLeaf, EntitySpec, FieldSpec, Normalize,
    StdAttrsDecl, VarSlotKind,
};

// --- FieldId'ы в порядке Designer DENSE (<Properties>) ---
/// `synonym` — локализ. Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `defaultForm` — form-ref. Default "".
pub const F_DEFAULT_FORM: FieldId = FieldId(3);
/// `auxiliaryForm` — form-ref. Default "".
pub const F_AUXILIARY_FORM: FieldId = FieldId(4);
/// `useStandardCommands` — Default false.
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(5);
/// `registeredDocuments` — список ссылок-документов (item-style). Default [].
pub const F_REGISTERED_DOCUMENTS: FieldId = FieldId(6);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(7);
/// `standardAttributes` — ВАРИАТИВНЫЙ блок (6 атрибутов). Required (всегда present).
pub const F_STANDARD_ATTRIBUTES: FieldId = FieldId(8);
/// `listPresentation` — локализ. Default [].
pub const F_LIST_PRESENTATION: FieldId = FieldId(9);
/// `extendedListPresentation` — локализ. Default [].
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(10);
/// `explanation` — локализ. Default [].
pub const F_EXPLANATION: FieldId = FieldId(11);
/// `help` — EDT-only const-блок. Default false; X-исключён.
pub const F_HELP: FieldId = FieldId(12);
/// `objectBelonging` — принадлежность объекта (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется, но
/// объявлен канонически (X by construction: оба формата дают дефолт-омиссию).
pub const F_OBJECT_BELONGING: FieldId = FieldId(13);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится корпусом SSL (всегда дефолт) → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(14);

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
fn list(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::List, PropertyValue::List(Vec::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        s(F_DEFAULT_FORM, "defaultForm"),
        s(F_AUXILIARY_FORM, "auxiliaryForm"),
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        list(F_REGISTERED_DOCUMENTS, "registeredDocuments"),
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        // standardAttributes — вариативный const-блок (6 атрибутов), всегда present.
        FieldSpec::required(F_STANDARD_ATTRIBUTES, "standardAttributes", ValueKind::List),
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
        // help — EDT-only const-блок (Designer не несёт): X-исключён, R EDT-only.
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())).x_ignored(),
        // structural-tail: objectBelonging (enum) + extendedConfigurationObject (Uuid-Str).
        // Дефолт = отсутствие в SSL-корпусе → дефолт-омиссия; проекции их НЕ размещают.
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(crate::ir::value::Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (сверено 2/2 ОБА формата:
/// Column < Form < Template < Command). У DocumentJournal НЕТ Attribute/TabularSection.
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Column", child_kind: "DocumentJournal.Column" },
    ChildSlot { collection: "Form", child_kind: "DocumentJournal.FormRef" },
    ChildSlot { collection: "Template", child_kind: "DocumentJournal.TemplateRef" },
    ChildSlot { collection: "Command", child_kind: "DocumentJournal.Command" },
];

// ============================================================================
// Декларация платформенного `standardAttributes` (data-driven; §1.6). Document-семья,
// Root-only (у журнала нет табличных частей со std-attrs). ВСЕ 6 атрибутов
// `fillChecking=DontCheck` (в т.ч. Date — в отличие от Document); `fullTextSearch`=Use.
// ============================================================================

const S_SYNONYM: usize = 0;
const S_TOOLTIP: usize = 1;
const S_FORMAT: usize = 2;
const S_EDIT_FORMAT: usize = 3;
const S_FILL_VALUE: usize = 4;
// Зеркало Document-семьи: cf-кодек ОБЩИЙ (`std_attrs_document`, N_VAR=9) — длина записи
// обязана совпадать. У DJ ERP-корпус вариативности НЕ показал (все значения дефолтны),
// слоты добавлены только ради синхронной формы записи.
const S_COMMENT: usize = 5;
const S_FILL_FROM: usize = 6;
const S_FILL_CHECKING: usize = 7;
const S_FULL_TEXT: usize = 8;

/// 6 предопределённых атрибута КОРНЯ (ВСЕ fillChecking=DontCheck).
const ROOT_ATTRS: &[AttrDecl] = &[
    AttrDecl { name: "Type", name_consts: &["DontCheck"] },
    AttrDecl { name: "Ref", name_consts: &["DontCheck"] },
    AttrDecl { name: "Date", name_consts: &["DontCheck"] },
    AttrDecl { name: "Posted", name_consts: &["DontCheck"] },
    AttrDecl { name: "DeletionMark", name_consts: &["DontCheck"] },
    AttrDecl { name: "Number", name_consts: &["DontCheck"] },
];

const VAR_SLOTS: &[VarSlotKind] = &[
    VarSlotKind::Loc,               // synonym
    VarSlotKind::Loc,               // toolTip
    VarSlotKind::Loc,               // format
    VarSlotKind::Loc,               // editFormat
    VarSlotKind::Value,             // fillValue
    VarSlotKind::Str,               // comment (зеркало Document; DJ — всегда дефолт)
    VarSlotKind::Bool,              // fillFromFillingValue
    VarSlotKind::Enum("DontCheck"), // fillChecking
    VarSlotKind::Enum("DontUse"),   // fullTextSearch
];

const EDT_LEAVES: &[EdtLeaf] = &[
    EdtLeaf::ConstText { tag: "dataHistory", text: "Use" },
    EdtLeaf::Name,
    EdtLeaf::OptLoc("synonym", S_SYNONYM),
    EdtLeaf::OptStr("comment", S_COMMENT),
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
    ("DataHistory", DenseLeaf::Text("Use")),
    ("MarkNegatives", DenseLeaf::Text("false")),
    ("MinValue", DenseLeaf::Nil),
    ("Synonym", DenseLeaf::VarLoc(S_SYNONYM)),
    ("Comment", DenseLeaf::VarStr(S_COMMENT)),
    ("FullTextSearch", DenseLeaf::VarEnum(S_FULL_TEXT)),
    ("ChoiceParameterLinks", DenseLeaf::Empty),
    ("FillValue", DenseLeaf::VarValue(S_FILL_VALUE)),
    ("Mask", DenseLeaf::Empty),
    ("ChoiceParameters", DenseLeaf::Empty),
];

/// Декларация `standardAttributes` вида `DocumentJournal` (data-driven; Root-only).
pub const STD_ATTRS: StdAttrsDecl = StdAttrsDecl {
    root_attrs: ROOT_ATTRS,
    // Фиксированный набор (не вариадный) — все атрибуты присутствуют всегда.
    root_variants: &[],
    tabular_attrs: &[],
    var_slots: VAR_SLOTS,
    edt_leaves: EDT_LEAVES,
    dense_leaves: DENSE_LEAVES,
    root_elem: "DocumentJournal",
    label: "DocumentJournal",
};

/// Канонический [`EntitySpec`] вида `DocumentJournal` (кэш на процесс).
pub fn document_journal() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DocumentJournal",
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
/// Категории producedTypes вида `DocumentJournal` в каноническом порядке IR (= EDT).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Selection",
        edt_tag: "selectionType",
        designer_category: "Selection",
        designer_type_name: "DocumentJournalSelection",
        designer_order: 0,
    },
    crate::spec::common::ProducedCategory {
        category: "List",
        edt_tag: "listType",
        designer_category: "List",
        designer_type_name: "DocumentJournalList",
        designer_order: 1,
    },
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "DocumentJournalManager",
        designer_order: 2,
    },
];
