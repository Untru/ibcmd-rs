//! Канонический спек типа контрола `RadioButtonField` (переключатель) — семейство
//! FormField, срез LANE-F-3. ОБЩЕЕ тело — тот же [`form_field::form_field_common`], что и у
//! прочих FormField; здесь — лишь тип-специфичный `extInfo`
//! (`form:RadioButtonsFieldExtInfo`).
//!
//! # Ключевые корпусные факты (cross-omission witness, 18 RadioButton полей SSL CommonForms)
//! * `radioButtonsType` ⟺ Designer `RadioButtonType` (РАЗНЫЕ имена: EDT plural): KEEP —
//!   EDT опускает `Auto` (11/18 absent), Designer эмитит ВСЕГДА (Auto 11 + Tumbler 6 +
//!   RadioButtons 1 = 18/18).
//! * `columnsCount` ⟺ `ColumnsCount`: СИММЕТРИЧЕН (оба опускают absent-дефолт; 11/11 absent,
//!   1×6, 2×1 — совпадают).
//! * `orientation` ⟺ `Orientation`: KEEP — EDT эмитит ВСЕГДА (Vertical 15 + HIP 3 = 18),
//!   Designer опускает `Vertical` (15/18 absent).
//! * `choiceList` (repeatable, structured ValueList) — DUAL-ENCODING: EDT повторяемый
//!   `<choiceList><presentation…/><value xsi:type="core:StringValue">…` ⟺ Designer один
//!   `<ChoiceList><xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>
//!   <xr:Value xsi:type="FormChoiceListDesTimeValue"><Presentation…/><Value xsi:type="xs:…">`.
//!   Канон — `List` из пар `[presentation:Localized, value:Value]` (X by construction).
//!
//! Канонический порядок extInfo (= EDT-порядок, topo 18/18): radioButtonsType, columnsCount,
//! choiceList, orientation.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::forms::controls::form_field::form_field_common;
use crate::spec::forms::controls::ControlSpec;

// ============================ extInfo: RadioButtonsField ============================
// Диапазон id 201+ — чтобы НЕ пересекаться с общим телом FormField (1..32), т.к.
// designer-порядок ищет поле сначала в общем теле, затем в extInfo по одному id.

/// extInfo: `radioButtonsType` (EDT) ⟺ `RadioButtonType` (Designer) — KEEP `Auto`.
pub const F_EXT_RADIO_BUTTON_TYPE: FieldId = FieldId(201);
/// extInfo: `columnsCount` ⟺ `ColumnsCount` (Int, симметрично).
pub const F_EXT_COLUMNS_COUNT: FieldId = FieldId(202);
/// extInfo: `choiceList` (repeatable ValueList) ⟺ `ChoiceList` (см. модульный док).
pub const F_EXT_CHOICE_LIST: FieldId = FieldId(203);
/// extInfo: `orientation` ⟺ `Orientation` — KEEP с ПРОТИВОПОЛОЖНЫМИ дефолтами (EDT опускает
/// `Horizontal`, Designer опускает `Vertical`; сверено корпусом покрытия).
pub const F_EXT_ORIENTATION: FieldId = FieldId(204);
/// extInfo: `itemHeight` — высота элемента (Int, симметрично; метамодель `RadioButtonsFieldExtInfo`
/// после `itemWidth`, до `itemTitleHeight`).
pub const F_EXT_ITEM_HEIGHT: FieldId = FieldId(205);
/// extInfo: `itemTitleHeight` — высота заголовка элемента (Int, симметрично; метамодель после
/// `itemHeight`, до `columnsCount`).
pub const F_EXT_ITEM_TITLE_HEIGHT: FieldId = FieldId(206);
/// extInfo: `horizontalStretch` — гориз. растяжение (Bool, симметрично; метамодель после `choiceList`).
pub const F_EXT_HORIZONTAL_STRETCH: FieldId = FieldId(207);
/// extInfo: `equalElementsWidth` ⟺ Designer `EqualColumnsWidth` (РАЗНОИМЁННЫЕ теги) — Bool,
/// Symmetric (оба эмитят `true`). Метамодель: columnsCount → **equalElementsWidth** → choiceList.
/// Witness — ПомощникСозданияОбменаДанными (EDT radioButtonsType→equalElementsWidth→choiceList
/// ⟷ Designer RadioButtonType→EqualColumnsWidth→ChoiceList).
pub const F_EXT_EQUAL_ELEMENTS_WIDTH: FieldId = FieldId(208);

/// EDT-дефолт `radioButtonsType` (омиссия `Auto`; Designer эмитит его явно).
pub const RADIO_BUTTON_TYPE_DEFAULT: &str = "Auto";
/// EDT-дефолт `orientation` (EDT ОПУСКАЕТ `Horizontal`; сверено корпусом покрытия — поле
/// `Orientation_Horizontal` несёт пустой EDT extInfo, а `Vertical` эмитится явно).
pub const ORIENTATION_EDT_DEFAULT: &str = "Horizontal";
/// Designer-дефолт `orientation` (Designer ОПУСКАЕТ `Vertical`). EDT и Designer имеют
/// ПРОТИВОПОЛОЖНЫЕ дефолты (Horizontal ⟺ Vertical) — KEEP-пара.
///
/// Designer-дамп пишет САМА ПЛАТФОРМА и опускает ровно дефолт свойства ⇒ это и есть
/// ПЛАТФОРМЕННОЕ умолчание; оно объявлено умолчанием спек-поля (`build_ext_fields`), чтобы
/// значение было разрешимо из IR (`crate::resolve`), когда в источнике свойства нет вовсе —
/// а в дампах 2.17/2.20 его нет: тег введён 2.21 (`models/version_facts.jsonl`).
pub const ORIENTATION_DESIGNER_DEFAULT: &str = "Vertical";

fn build_ext_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::required(F_EXT_RADIO_BUTTON_TYPE, "radioButtonsType", ValueKind::Enum),
        FieldSpec::required(F_EXT_ITEM_HEIGHT, "itemHeight", ValueKind::Int),
        FieldSpec::required(F_EXT_ITEM_TITLE_HEIGHT, "itemTitleHeight", ValueKind::Int),
        FieldSpec::required(F_EXT_COLUMNS_COUNT, "columnsCount", ValueKind::Int),
        FieldSpec::required(
            F_EXT_EQUAL_ELEMENTS_WIDTH,
            "equalElementsWidth",
            ValueKind::Bool,
        ),
        FieldSpec::required(F_EXT_CHOICE_LIST, "choiceList", ValueKind::List),
        FieldSpec::required(
            F_EXT_HORIZONTAL_STRETCH,
            "horizontalStretch",
            ValueKind::Bool,
        ),
        // УМОЛЧАНИЕ ПЛАТФОРМЫ — то, что опускает дамп самой платформы (Designer: `Vertical`,
        // 15/18 SSL). Оно объявлено здесь, чтобы значение свойства было РАЗРЕШИМО из IR
        // (`crate::resolve`) даже когда в мешке его нет: у дампа 2.17/2.20 тега `<Orientation>`
        // нет вовсе (свойство введено 2.21), ридер его не выдумывает, а писателю `.cf`
        // ординал нужен всегда. `ORIENTATION_EDT_DEFAULT` (`Horizontal`) — дефолт МЕТАМОДЕЛИ
        // EDT, не платформы, и каноническим умолчанием не является.
        FieldSpec::with_default(
            F_EXT_ORIENTATION,
            "orientation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new(ORIENTATION_DESIGNER_DEFAULT)),
        ),
    ]
}

fn leak_spec(entity: &'static str, fields: Vec<FieldSpec>) -> EntitySpec {
    EntitySpec {
        entity,
        fields: Box::leak(fields.into_boxed_slice()),
        children: &[],
    }
}

fn radio_ext_spec() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| leak_spec("RadioButtonsFieldExtInfo", build_ext_fields()))
}

/// Канонический [`ControlSpec`] вида `RadioButtonField` (лист; общее тело FormField +
/// extInfo переключателя).
pub fn radio_button_field() -> &'static ControlSpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<ControlSpec> = OnceLock::new();
    SPEC.get_or_init(|| ControlSpec {
        kind: "RadioButtonField",
        properties: form_field_common(),
        ext_info: radio_ext_spec(),
        container: false,
    })
}
