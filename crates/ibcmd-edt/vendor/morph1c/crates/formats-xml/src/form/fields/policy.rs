//! FIELDS · read-side per-format default reconciliation (§1.6) — applies the field [`Policy`] to a read (or absent) value before pushing it into the canonical bag.

use super::*;

/// Можно ли ВОССТАНАВЛИВАТЬ отсутствующее свойство `owner`.`canonical` по конвенции диалекта.
///
/// # ЗАКОН: у отсутствия тега ДВЕ разные причины
/// * свойство ЕСТЬ в этой версии формата, но НЕ ЗАДАНО — тогда омиссия и есть ДАННЫЕ: диалект
///   ею кодирует свой дефолт, и ридер обязан его восстановить (иначе `Keep`-пары развалятся,
///   а с ними и кросс-форматное равенство);
/// * свойства В ЭТОЙ ВЕРСИИ ФОРМАТА НЕ СУЩЕСТВУЕТ — тогда омиссия не значит НИЧЕГО.
///   Восстанавливать тут нечего: любой литерал был бы выдумкой (§1.0), а положенное в мешок
///   значение потом читалось бы как «задано в источнике» и всплывало ложной потерей на
///   downgrade-гейте (witnessed: `<Orientation>` у RadioButtonField введён 2.21, но в
///   2.17-дампе ридер его достраивал — и конверсия 2.17→2.17 падала «потерей»).
///
/// Различает ВЕРСИЯ ИСТОЧНИКА: она известна ридеру Designer из корневого `version=` самого
/// файла формы, а ридеру EDT — из амбьента, который ставит whole-config-конвейер по
/// `DT-INF/PROJECT.PMF`. Версия неизвестна (`None` — самостоятельный разбор тела в тестах) ⇒
/// гейта нет, поведение прежнее.
fn fill_allowed(owner: &str, canonical: &str) -> bool {
    match morph1c_core::version::current_source_version() {
        Some(src) => morph1c_core::version::form_field_available_in(owner, canonical, src),
        None => true,
    }
}

/// Применить политику к прочитанному (или отсутствующему) значению; пуш в bag.
///
/// `owner` — вид-владелец свойства (`"RadioButtonField"`, `"Form"`, …): вместе с каноническим
/// именем поля он ключует витнессенную таблицу версий форм (`models/version_facts.jsonl`),
/// по которой решается, законна ли РЕКОНСТРУКЦИЯ отсутствующего свойства (см. [`fill_allowed`]).
pub(crate) fn apply_read_policy(
    owner: &str,
    entry: &FieldProj,
    present: Option<PropertyValue>,
    dialect: FormDialect,
    bag: &mut Vec<(FieldId, PropertyValue)>,
) -> Result<(), FormError> {
    match entry.policy_for(morph1c_core::version::current_source_version()) {
        Policy::Symmetric => {
            if let Some(v) = present {
                bag.push((entry.id, v));
            }
        }
        Policy::OppositeBool => match (dialect, present) {
            // Канон-bag несёт только true (sparse против false).
            (_, Some(PropertyValue::Bool(true))) => bag.push((entry.id, PropertyValue::Bool(true))),
            (_, Some(_)) => {}             // явный false ⇒ канон-false ⇒ drop.
            (FormDialect::Edt, None) => {} // EDT absent ⇒ false ⇒ drop.
            (FormDialect::Designer, None) => {
                bag.push((entry.id, PropertyValue::Bool(true))) // Designer absent ⇒ true.
            }
        },
        Policy::Keep(k) => {
            let v = match present {
                Some(v) => v,
                // Свойства нет в источнике: восстанавливаем ТОЛЬКО если версия источника его
                // вообще знала (иначе омиссия не несёт информации — см. `fill_allowed`).
                None if !fill_allowed(owner, entry.edt) => return Ok(()),
                None => match dialect {
                    FormDialect::Edt => parse_lit(entry.codec, k.edt_fill),
                    FormDialect::Designer => parse_lit(entry.codec, k.des_fill),
                },
            };
            bag.push((entry.id, v));
        }
        // ПЛАТФОРМЕННЫЙ ДЕФОЛТ: `des_fill` НЕ хранится в каноне (⇒ оба ридера дают ОДНО и то
        // же при незаданном свойстве), EDT-омиссия по-прежнему значит `edt_fill`.
        Policy::PlatformDefault(k) => {
            push_platform_default(owner, entry, present, dialect, k.edt_fill, k.des_fill, bag)
        }
        Policy::EditMode => push_platform_default(
            owner,
            entry,
            present,
            dialect,
            ff::EDIT_MODE_EDT_DEFAULT,
            ff::EDIT_MODE_DESIGNER_DEFAULT,
            bag,
        ),
        // ТОЧНОЕ ПРИСУТСТВИЕ, ОДИНАКОВО В ОБОИХ ДИАЛЕКТАХ: fill ЗАПРЕЩЁН (омиссии диалектов
        // НЕ дизъюнктны — ERP опускает тег в обоих, 80 252/80 252; см. `Policy::ButtonImportance`).
        Policy::ButtonImportance => {
            if let Some(v) = present {
                bag.push((entry.id, v));
            }
        }
    }
    Ok(())
}

/// Общее тело [`Policy::PlatformDefault`]/[`Policy::EditMode`] на READ-пути.
///
/// `platform` — дефолт ПЛАТФОРМЫ (литерал, который опускает Designer-дамп): равное ему
/// значение в канон НЕ кладётся, поэтому «свойство не задано» имеет ЕДИНСТВЕННОЕ каноническое
/// написание — отсутствие в bag — одинаковое у ОБОИХ ридеров (X-канон). `edt_ecore` — дефолт
/// EDT-метамодели (что означает омиссия НА EDT-стороне); он в каноне хранится ЯВНО, потому что
/// платформенным дефолтом не является — и потому же гейтится версией источника (реконструкция).
#[allow(clippy::too_many_arguments)]
fn push_platform_default(
    owner: &str,
    entry: &FieldProj,
    present: Option<PropertyValue>,
    dialect: FormDialect,
    edt_ecore: &'static str,
    platform: &'static str,
    bag: &mut Vec<(FieldId, PropertyValue)>,
) {
    match (present, dialect) {
        // Явное значение платформенного дефолта (EDT его пишет всегда) ⇒ канон-омиссия.
        (Some(v), _) if v == parse_lit(entry.codec, platform) => {}
        (Some(v), _) => bag.push((entry.id, v)),
        // EDT-омиссия означает ecore-дефолт EDT — он НЕ платформенный, хранится явно (но
        // только если версия источника это свойство знала).
        (None, FormDialect::Edt) => {
            if fill_allowed(owner, entry.edt) {
                bag.push((entry.id, parse_lit(entry.codec, edt_ecore)));
            }
        }
        // Designer-омиссия означает платформенный дефолт ⇒ канон-омиссия (та же, что у EDT
        // с явным `platform`).
        (None, FormDialect::Designer) => {}
    }
}
