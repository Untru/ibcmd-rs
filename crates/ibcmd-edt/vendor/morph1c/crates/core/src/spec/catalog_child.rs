//! Базовый набор полей leaf-детей `Catalog` (`Attribute`, `TabularSection.Attribute`) —
//! child-objects substrate, Catalog-срез. ПЕРЕИСПОЛЬЗУЕТ [`crate::spec::ir_child`]
//! `base_child_fields()` (тот же 27-полевой реквизит-набор) + ОДНО Catalog-специфичное
//! поле `Use` (`use`/`Use`, default `ForItem`) — сверено 799 атрибутов × edt+designer.
//!
//! НЕ вид: модуль-помощник под `spec/` (вне `metadata/`, чтобы build.rs не принял его за
//! вид). Здесь ЛИШЬ канонические коды/типы/дефолты, РАЗДЕЛЯЕМЫЕ Catalog-child-спеками;
//! имена тегов — в проекциях форматов (§1.6).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::FieldSpec;
use crate::spec::ir_child::{
    base_child_fields, F_CHOICE_PARAMETERS, F_FILL_FROM_FILLING_VALUE, F_FILL_VALUE, F_INDEXING,
    F_LINK_BY_TYPE,
};

/// `use` — назначение реквизита (`ForItem`/`ForFolderAndItem`). Default `ForItem`
/// (EDT омитит; Designer DENSE). Catalog-специфичное поле (нет у IR-реквизита).
/// Id вне диапазона базового набора (1..=27) — конфликта нет.
pub const F_USE: FieldId = FieldId(40);

/// Перетипировать `choiceParameters` в СТРУКТУРНЫЙ List (codec ChoiceParameters), а не
/// пустой Str как у IR-base. `linkByType` остаётся Str (codec LinkByType различает форматы).
fn retype_choice_parameters(v: &mut [FieldSpec]) {
    if let Some(f) = v.iter_mut().find(|f| f.id == F_CHOICE_PARAMETERS) {
        *f = FieldSpec::with_default(
            F_CHOICE_PARAMETERS,
            "choiceParameters",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        );
    }
    let _ = F_LINK_BY_TYPE; // структурный, но IR-нагрузка Str (как base).
}

/// Поля КОРНЕВОГО реквизита `Catalog.Attribute` = IR base + `Use` (после
/// choiceHistoryOnInput, перед indexing — Designer DENSE). `fillValue` ВСЕГДА present
/// (799/799 → required, остаётся из base). Сверено edt+designer.
pub fn catalog_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    let idx = v
        .iter()
        .position(|f| f.id == F_INDEXING)
        .expect("indexing present in base");
    v.insert(
        idx,
        FieldSpec::with_default(
            F_USE,
            "use",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("ForItem")),
        ),
    );
    v
}

/// Поля КОРНЕВОГО реквизита НЕ-иерархического ссылочного вида (напр.
/// `ChartOfCalculationTypes.Attribute`) = IR base (incl. `fillValue`/`fillFromFillingValue`)
/// БЕЗ `Use` (у не-иерархического вида нет папок → атрибут не несёт `Use`; сверено корпусом
/// CCalcT). Прочее — как base (`retype_choice_parameters`).
pub fn nonhier_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v
}

/// Поля реквизита ТАБЛИЧНОЙ ЧАСТИ `Catalog.TabularSection.Attribute` = IR base БЕЗ
/// `fillFromFillingValue`/`fillValue` и БЕЗ `Use` (сверено: 200/200 их не несут у обоих
/// форматов). Прочее — как base.
pub fn tabular_attribute_fields() -> Vec<FieldSpec> {
    let mut v = base_child_fields();
    retype_choice_parameters(&mut v);
    v.retain(|f| f.id != F_FILL_FROM_FILLING_VALUE && f.id != F_FILL_VALUE);
    v
}
