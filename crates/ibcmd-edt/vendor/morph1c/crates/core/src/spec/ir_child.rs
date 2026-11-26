//! Общие FieldId + базовый набор полей для leaf-детей `InformationRegister`
//! (`Resource`/`Attribute`/`Dimension`) — child-objects substrate
//!. НЕ вид: модуль-помощник под `spec/` (вне `metadata/`,
//! чтобы build.rs не принял его за вид). Здесь ЛИШЬ канонические коды/типы/дефолты,
//! РАЗДЕЛЯЕМЫЕ тремя дочерними спеками; имена тегов — в проекциях форматов (§1.6).
//!
//! Набор полей и дефолты сверены по SSL (443 Resource + 289 Attribute + 395 Dimension,
//! edt+designer). Resource == Attribute по набору; Dimension = базовый + 4 поля
//! (master/mainFilter/denyIncompleteValues/typeReductionMode), заводимые его спеком.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{FieldSpec, Normalize};

// --- Базовые поля (общие Resource/Attribute/Dimension), порядок = Designer DENSE ---
/// Metadata families whose reference child roster is covered by the canonical
/// standard-field registry and CURRENT declared fields/tabular sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceChildRosterFamily {
    Catalog,
    Document,
    Enum,
}
impl ReferenceChildRosterFamily {
    pub fn for_kind(kind: &str) -> Option<Self> {
        match kind {
            "Catalog" => Some(Self::Catalog),
            "Document" => Some(Self::Document),
            "Enum" => Some(Self::Enum),
            _ => None,
        }
    }
}

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `type` — описание типа (`Type`-codec). Required (всегда present).
pub const F_TYPE: FieldId = FieldId(3);
/// `passwordMode` — Default false.
pub const F_PASSWORD_MODE: FieldId = FieldId(4);
/// `format` — локализ., Default [].
pub const F_FORMAT: FieldId = FieldId(5);
/// `editFormat` — локализ., Default [].
pub const F_EDIT_FORMAT: FieldId = FieldId(6);
/// `toolTip` — локализ., Default [].
pub const F_TOOL_TIP: FieldId = FieldId(7);
/// `markNegatives` — Default false.
pub const F_MARK_NEGATIVES: FieldId = FieldId(8);
/// `mask` — Default "".
pub const F_MASK: FieldId = FieldId(9);
/// `multiLine` — Default false.
pub const F_MULTI_LINE: FieldId = FieldId(10);
/// `extendedEdit` — Default false.
pub const F_EXTENDED_EDIT: FieldId = FieldId(11);
/// `minValue` — Value-xsi. Required (всегда present, всегда Undefined в корпусе).
pub const F_MIN_VALUE: FieldId = FieldId(12);
/// `maxValue` — Value-xsi. Required (всегда present, всегда Undefined).
pub const F_MAX_VALUE: FieldId = FieldId(13);
/// `fillFromFillingValue` — Default false.
pub const F_FILL_FROM_FILLING_VALUE: FieldId = FieldId(14);
/// `fillValue` — Value-xsi. Required (всегда present).
pub const F_FILL_VALUE: FieldId = FieldId(15);
/// `fillChecking` — Default DontCheck.
pub const F_FILL_CHECKING: FieldId = FieldId(16);
/// `choiceFoldersAndItems` — Default Items.
pub const F_CHOICE_FOLDERS_AND_ITEMS: FieldId = FieldId(17);
/// `choiceParameterLinks` — список связей; Default [].
pub const F_CHOICE_PARAMETER_LINKS: FieldId = FieldId(18);
/// `choiceParameters` — Designer-only пустой; Default "".
pub const F_CHOICE_PARAMETERS: FieldId = FieldId(19);
/// `quickChoice` — Default Auto.
pub const F_QUICK_CHOICE: FieldId = FieldId(20);
/// `createOnInput` — Default Auto.
pub const F_CREATE_ON_INPUT: FieldId = FieldId(21);
/// `choiceForm` — form-ref; Default "".
pub const F_CHOICE_FORM: FieldId = FieldId(22);
/// `linkByType` — Designer-only пустой; Default "".
pub const F_LINK_BY_TYPE: FieldId = FieldId(23);
/// `choiceHistoryOnInput` — Default Auto.
pub const F_CHOICE_HISTORY_ON_INPUT: FieldId = FieldId(24);
/// `indexing` — Default DontIndex.
pub const F_INDEXING: FieldId = FieldId(25);
/// `fullTextSearch` — Default Use.
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(26);
/// `dataHistory` — Default Use.
pub const F_DATA_HISTORY: FieldId = FieldId(27);

// --- Dimension-only поля (после choiceHistoryOnInput / dataHistory у Designer) ---
/// `master` (Dimension) — Default false.
pub const F_MASTER: FieldId = FieldId(28);
/// `mainFilter` (Dimension) — Default TRUE (majority; сверено).
pub const F_MAIN_FILTER: FieldId = FieldId(29);
/// `denyIncompleteValues` (Dimension) — Default false.
pub const F_DENY_INCOMPLETE_VALUES: FieldId = FieldId(30);
/// `typeReductionMode` (Dimension) — Default TransformValues.
pub const F_TYPE_REDUCTION_MODE: FieldId = FieldId(31);

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}
fn empty_loc() -> PropertyValue {
    PropertyValue::Localized(Vec::new())
}
fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, empty_loc())
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(
        id,
        name,
        ValueKind::Enum,
        PropertyValue::Enum(Token::new(def)),
    )
}
fn s_empty(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, empty_str())
}
/// minValue/maxValue/fillValue — required Value (всегда present). minValue/maxValue —
/// всегда Undefined; fillValue варьируется.
fn val_required(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::required(id, name, ValueKind::Value)
}

/// Базовый набор полей (Resource/Attribute), в каноническом DENSE-порядке Designer.
pub fn base_child_fields() -> Vec<FieldSpec> {
    vec![
        loc(F_SYNONYM, "synonym"),
        s_empty(F_COMMENT, "comment"),
        FieldSpec::required(F_TYPE, "type", ValueKind::Type),
        b(F_PASSWORD_MODE, "passwordMode"),
        loc(F_FORMAT, "format"),
        loc(F_EDIT_FORMAT, "editFormat"),
        loc(F_TOOL_TIP, "toolTip"),
        b(F_MARK_NEGATIVES, "markNegatives"),
        s_empty(F_MASK, "mask"),
        b(F_MULTI_LINE, "multiLine"),
        b(F_EXTENDED_EDIT, "extendedEdit"),
        val_required(F_MIN_VALUE, "minValue"),
        val_required(F_MAX_VALUE, "maxValue"),
        b(F_FILL_FROM_FILLING_VALUE, "fillFromFillingValue"),
        val_required(F_FILL_VALUE, "fillValue"),
        e(F_FILL_CHECKING, "fillChecking", "DontCheck"),
        e(F_CHOICE_FOLDERS_AND_ITEMS, "choiceFoldersAndItems", "Items"),
        // choiceParameterLinks: список (List); Default [].
        FieldSpec::with_default(
            F_CHOICE_PARAMETER_LINKS,
            "choiceParameterLinks",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        s_empty(F_CHOICE_PARAMETERS, "choiceParameters"),
        e(F_QUICK_CHOICE, "quickChoice", "Auto"),
        e(F_CREATE_ON_INPUT, "createOnInput", "Auto"),
        s_empty(F_CHOICE_FORM, "choiceForm"),
        s_empty(F_LINK_BY_TYPE, "linkByType"),
        e(F_CHOICE_HISTORY_ON_INPUT, "choiceHistoryOnInput", "Auto"),
        e(F_INDEXING, "indexing", "DontIndex"),
        // fullTextSearch/dataHistory: Default DontUse (EDT омитит DontUse, эмитит Use).
        // Сверено: Resource/Dimension всегда Use(present); Attribute 11 опускают (DontUse).
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),
        e(F_DATA_HISTORY, "dataHistory", "DontUse"),
    ]
}

/// Поля вида `Dimension` = базовый набор + 4 dimension-only поля. Порядок Designer DENSE:
/// master/mainFilter/denyIncompleteValues — ПОСЛЕ choiceHistoryOnInput (перед indexing);
/// typeReductionMode — ПОСЛЕ dataHistory (в конце). Реализуем порядком вставки.
pub fn dimension_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    // Найти позицию indexing — вставить master/mainFilter/denyIncompleteValues перед ним.
    let idx_indexing = v
        .iter()
        .position(|f| f.id == F_INDEXING)
        .expect("indexing present");
    let extras = vec![
        b(F_MASTER, "master"),
        // mainFilter: Default FALSE (EDT эмитит true(369), омитит false(26) — сверено).
        b(F_MAIN_FILTER, "mainFilter"),
        b(F_DENY_INCOMPLETE_VALUES, "denyIncompleteValues"),
    ];
    for (k, f) in extras.into_iter().enumerate() {
        v.insert(idx_indexing + k, f);
    }
    // typeReductionMode — в КОНЕЦ (после dataHistory).
    v.push(e(
        F_TYPE_REDUCTION_MODE,
        "typeReductionMode",
        "TransformValues",
    ));
    v
}
