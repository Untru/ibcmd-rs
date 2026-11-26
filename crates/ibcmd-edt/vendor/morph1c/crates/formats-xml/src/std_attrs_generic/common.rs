//! Диалект-агностичная машинерия standardAttributes: IR-record хелперы, вариантный
//! выбор набора, dense-leaf проекция версии и аксессоры IR-значений.

use super::*;

// ---- canonical IR helpers --------------------------------------------------

fn undefined_value() -> PropertyValue {
    PropertyValue::Value(ValueSpec {
        kind: ValueScalarKind::Undefined,
        scalar: None,
    })
}

pub(crate) fn undefined_spec() -> ValueSpec {
    ValueSpec {
        kind: ValueScalarKind::Undefined,
        scalar: None,
    }
}

/// Дефолтная (омитимая в EDT) запись variable-полей по типам слотов декларации.
/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn default_record(decl: &StdAttrsDecl) -> Vec<PropertyValue> {
    decl.var_slots
        .iter()
        .map(|k| match k {
            VarSlotKind::Loc => PropertyValue::Localized(Vec::new()),
            VarSlotKind::Str => PropertyValue::Str(String::new()),
            VarSlotKind::Bool => PropertyValue::Bool(false),
            VarSlotKind::Enum(d) => PropertyValue::Enum(Token::new(*d)),
            VarSlotKind::Value => undefined_value(),
            VarSlotKind::Cpl => PropertyValue::List(Vec::new()),
            VarSlotKind::Cp => PropertyValue::List(Vec::new()),
        })
        .collect()
}

/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn pack(records: Vec<Vec<PropertyValue>>) -> PropertyValue {
    PropertyValue::List(records.into_iter().map(PropertyValue::List).collect())
}

/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn unpack<'a>(
    decl: &StdAttrsDecl,
    value: &'a PropertyValue,
    n_attrs: usize,
) -> Result<Vec<&'a Vec<PropertyValue>>, String> {
    let n_var = decl.var_slots.len();
    let outer = match value {
        PropertyValue::List(v) => v,
        other => {
            return Err(format!(
                "{} standardAttributes IR must be List, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    };
    if outer.len() != n_attrs {
        return Err(format!(
            "{} standardAttributes: expected {n_attrs} attrs, got {}",
            decl.label,
            outer.len()
        ));
    }
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            PropertyValue::List(rec) if rec.len() == n_var => out.push(rec),
            _ => {
                return Err(format!(
                    "{} standardAttributes: each attr must be List of {n_var}",
                    decl.label
                ))
            }
        }
    }
    Ok(out)
}

pub(crate) fn name_const(attr: &AttrDecl, which: morph1c_core::spec::common::NameConstWhich) -> &str {
    attr.name_consts[which.index()]
}

/// Число записей (атрибутов) в IR-value блока (для вариадного выбора на write).
fn outer_len(decl: &StdAttrsDecl, value: &PropertyValue) -> Result<usize, String> {
    match value {
        PropertyValue::List(v) => Ok(v.len()),
        other => Err(format!(
            "{} standardAttributes IR must be List, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}

// ---- variant selection (variadic / conditional-attr / optional-block) -------

/// Выбрать набор атрибутов для варианта по НАБЛЮДАЕМОМУ счётчику (`count` = число
/// блоков EDT / `<xr:StandardAttribute>` Designer на read, или число IR-записей на write).
///
/// - Табличная часть — ВСЕГДА фиксирована ([`StdAttrsDecl::tabular_attrs`]).
/// - Корень: если вид ВАРИАДЕН ([`StdAttrsDecl::root_is_variadic`]) — среди объявленных
///   наборов выбирается тот, чья длина == `count` (условный атрибут ⇒ наборы разной длины;
///   опциональный блок ⇒ вариант длины 0). Иначе — единственный фиксированный набор.
///
/// §1.0: счётчик, не совпавший ни с одним объявленным вариантом, — типизированная ОШИБКА
/// (не догадка). Единица работы декларации гарантирует попарно-различные длины (иначе
/// декларация неоднозначна — тоже ошибка выбора).
pub(crate) fn select_attrs(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    count: usize,
) -> Result<&'static [AttrDecl], String> {
    // Блок `standardAttributes`/`StandardAttributes` ОПЦИОНАЛЕН в ОБОИХ форматах: EDT и
    // Designer опускают его целиком, когда ВСЕ стандартные атрибуты дефолтны (минимальный
    // конфиг — сверено по корпусу покрытия: 0/65 catalogs несут блок). Отсутствие ⇒ count 0
    // ⇒ пустой набор (нет записей → IR-`List` пуст → emit ничего не пишет → byte-exact; обе
    // стороны дают одинаковый пустой `List` → X-равенство). Универсально верно для всех видов
    // (std-attrs НЕ бывает обязательным), поэтому — общая ветка ДО вариадного/фикс-выбора.
    if count == 0 {
        return Ok(&[]);
    }
    match variant {
        StdAttrsVariant::Tabular => {
            let attrs = decl.tabular_attrs;
            if attrs.len() == count {
                Ok(attrs)
            } else {
                Err(format!(
                    "{} standardAttributes (tabular): expected {} attrs, found {count}",
                    decl.label,
                    attrs.len()
                ))
            }
        }
        StdAttrsVariant::Root => {
            let sets = decl.root_variant_sets();
            let mut matched: Option<&'static [AttrDecl]> = None;
            for set in sets {
                if set.len() == count {
                    if matched.is_some() {
                        return Err(format!(
                            "{} standardAttributes: ambiguous variant declaration — two \
                             root variants of length {count} (§1.0)",
                            decl.label
                        ));
                    }
                    matched = Some(set);
                }
            }
            matched.ok_or_else(|| {
                let lens: Vec<usize> = sets.iter().map(|s| s.len()).collect();
                format!(
                    "{} standardAttributes: found {count} attrs, no declared variant matches \
                     (allowed lengths {lens:?}) (§1.0)",
                    decl.label
                )
            })
        }
    }
}

/// Слот-хранилище имени атрибута ([`EdtLeaf::NameVar`]) — если объявлен, вид выбирает
/// root-вариант по ПОСЛЕДОВАТЕЛЬНОСТИ ИМЁН, а не по длине (наборы равной длины —
/// ERP-witnessed AccountingRegister).
pub(crate) fn decl_name_slot(decl: &StdAttrsDecl) -> Option<usize> {
    decl.edt_leaves.iter().find_map(|l| match l {
        EdtLeaf::NameVar(i) => Some(*i),
        _ => None,
    })
}

/// Выбрать root-набор по НАБЛЮДАЕМОЙ последовательности имён (виды с [`EdtLeaf::NameVar`]:
/// длины вариантов НЕ попарно-различны, дискриминирует ровно имя-последовательность).
/// §1.0: последовательность, не совпавшая ни с одним вариантом, — типизированная ошибка.
pub(crate) fn select_attrs_by_names(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    names: &[&str],
) -> Result<&'static [AttrDecl], String> {
    if names.is_empty() {
        return Ok(&[]);
    }
    if variant == StdAttrsVariant::Tabular {
        // Табличная часть всегда фиксирована — имя-выбор не про неё.
        return select_attrs(decl, variant, names.len());
    }
    for set in decl.root_variant_sets() {
        if set.len() == names.len() && set.iter().zip(names).all(|(a, n)| a.name == *n) {
            return Ok(set);
        }
    }
    Err(format!(
        "{} standardAttributes: observed attr names {names:?} match no declared variant (§1.0)",
        decl.label
    ))
}

/// Имена атрибутов ИЗ IR-записей (write-сторона имя-выбора): `rec[name_slot]` каждой записи.
fn record_names<'a>(
    decl: &StdAttrsDecl,
    value: &'a PropertyValue,
    name_slot: usize,
) -> Result<Vec<&'a str>, String> {
    let outer = match value {
        PropertyValue::List(v) => v,
        other => {
            return Err(format!(
                "{} standardAttributes IR must be List, got {:?}",
                decl.label,
                other.kind()
            ))
        }
    };
    outer
        .iter()
        .map(|item| match item {
            PropertyValue::List(rec) => match rec.get(name_slot) {
                Some(PropertyValue::Str(s)) => Ok(s.as_str()),
                _ => Err(format!(
                    "{} standardAttributes: record name slot {name_slot} must be Str",
                    decl.label
                )),
            },
            _ => Err(format!(
                "{} standardAttributes: each attr must be a List record",
                decl.label
            )),
        })
        .collect()
}

/// link-item листа `linkByType` std-attrs — ИМЯ-ФУНКЦИЯ атрибута: `ExtDimensionN` ⇒ N
/// (связь типа N-го субконто с типом счёта; ERP-witnessed все 9 листьев 3 регистров
/// бухгалтерии, edt+designer+cf согласны), прочие ⇒ 0 (реквизитная константа).
pub(crate) fn lbt_item(attr_name: &str) -> i64 {
    attr_name
        .strip_prefix("ExtDimension")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Листья Designer-DENSE-региона, СУЩЕСТВУЮЩИЕ в версии `version` (§1.5).
///
/// Плотный регион — это ПОЛНАЯ таблица свойств СВОЕЙ версии, а не «что задано»: платформа
/// эмитит все листья позиционно. Значит его состав — функция версии, и читать дамп 2.17
/// таблицей 2.21 нельзя (лишний лист → «expected 25 leaves, found 24»).
///
/// WITNESSED (`.fixtures/versions/probes/catalog_std_attrs/`): 2.17 → 24 листа, 2.20 → 25
/// (+`<xr:TypeReductionMode>`, since 8.3.25). Гейт снимает РОВНО его.
///
/// NB: версионные листья, что мы знаем, — name-ВЫВОДИМЫЕ константы
/// ([`DenseLeaf::VarNameConst`] / [`DenseLeaf::Text`]), т.е. IR-данных не несут. Поэтому
/// их отсутствие в старой версии — не потеря: обратная запись восстановит лист из имени
/// атрибута. Появись версионный лист с ПЕРЕМЕННЫМ слотом — он потребовал бы гейта и в IR.
pub(crate) fn dense_leaves_for(
    decl: &StdAttrsDecl,
    version: FormatVersion,
) -> Vec<&'static DenseLeafRule> {
    decl.dense_leaves
        .iter()
        .filter(|(local, _)| designer_field_available_in(local, version))
        .collect()
}

/// Выбор набора на WRITE: по числу IR-записей, а при NameVar-слоте — по слот-именам
/// записей (наборы равной длины; симметрично read-стороне).
pub(crate) fn select_attrs_for_write(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    value: &PropertyValue,
) -> Result<&'static [AttrDecl], String> {
    match decl_name_slot(decl) {
        Some(slot) => {
            let names = record_names(decl, value, slot)?;
            select_attrs_by_names(decl, variant, &names)
        }
        None => select_attrs(decl, variant, outer_len(decl, value)?),
    }
}

// ---- shared leaf helpers ----------------------------------------------------

pub(crate) fn as_value(v: &PropertyValue) -> Result<&ValueSpec, String> {
    match v {
        PropertyValue::Value(s) => Ok(s),
        other => Err(format!(
            "standardAttributes value field must be Value, got {:?}",
            other.kind()
        )),
    }
}

pub(crate) fn as_localized(v: &PropertyValue) -> Result<&Vec<(Lang, String)>, String> {
    match v {
        PropertyValue::Localized(p) => Ok(p),
        other => Err(format!(
            "standardAttributes localized field must be Localized, got {:?}",
            other.kind()
        )),
    }
}

pub(crate) fn as_str(v: &PropertyValue) -> Result<&str, String> {
    match v {
        PropertyValue::Str(s) => Ok(s),
        other => Err(format!(
            "standardAttributes str field must be Str, got {:?}",
            other.kind()
        )),
    }
}
