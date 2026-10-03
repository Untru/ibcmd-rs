//! ПРАВИЛО 2 (апгрейд): цель НОВЕЕ источника — дописать то, что цель ставит САМА.
//!
//! # Зачем проход
//! Мешок IR несёт ТОЛЬКО то, что было в источнике. Когда цель новее, этого мало: у новой
//! версии есть свойства, которые она проставляет КАЖДОМУ узлу, поднимая старый дамп, — и
//! конфигурация, записанная без них, отличается от той, что получила бы сама платформа.
//! Отсутствие в мешке при этом не «умолчание, которое писатель допишет»: у писателя `.cf`
//! ординал отсутствия — СОБСТВЕННЫЙ (витнесс: `UsualGroup.group` отсутствует ⇒ ординал
//! `Auto`, тогда как платформа, поднимая 2.17, кладёт `HorizontalIfPossible`).
//!
//! Симметрия с [`super::guard`] полная: тот отказывает, когда цель СТАРЕЕ и данные пропали
//! бы; этот дописывает, когда цель НОВЕЕ и данных не хватает. Оба судят по ВЕРСИИ ИСТОЧНИКА
//! из IR ([`Configuration::source_version`]) и оба собирают ПОЛНЫЙ список, а не первый случай.
//!
//! # Три класса переходов, и все три — ДАННЫЕ
//! 1. **Материализация** ([`super::form_materialized_on_upgrade`], факт `materialized`
//!    `models/version_facts.jsonl`): у КАЖДОГО узла, где тега не было, новая версия его
//!    поставила, и значение у всех одно. Факт действует на ТОЧНО ТОМ переходе, который
//!    наблюдался: монотонности тут нет (8.5.1 материализует `group`, поднимая дамп 2.17, и
//!    НЕ материализует, поднимая 2.20). Значение берётся швом
//!    [`crate::resolve::field_upgrade_value`] для ЦЕЛЕВОЙ версии; если его не знает никто —
//!    [`UpgradeError`], а не догадка (§1.0). «Тега не было» в мешке выглядит ДВОЯКО: поля нет
//!    вовсе — либо в нём стоит ЛИТЕРАЛ МОЛЧАНИЯ
//!    ([`crate::spec::forms::controls::SilentLiteral`]), которым ридер восстановил омиссию;
//!    оба состояния значат «источник промолчал», и оба заменяются.
//! 2. **Переименование** ([`super::form_renamed_on_upgrade`], факт `renamed`): значение
//!    целиком переехало из старого тега в новый. Значение КОПИРУЕТСЯ под новое имя, старое
//!    остаётся: у части полей писатель `.cf` читает ОБА (витнесс `Table`: HEAD-ячейка
//!    дериватна от плоского `useAlternationRowColor`, а TAIL-ячейка — от `…BWA`-твина;
//!    выбросив плоское имя, мы погасили бы HEAD-ячейку).
//! 3. **Разворачивание свёрнутой пары** ([`crate::spec::forms::controls::FOLDED_LITERALS`]):
//!    канон свернул два тега дампа в один литерал, и в версии, где КОМПАНЬОНА ещё нет,
//!    оба литерала пишутся одинаково. Поднимая такой источник, платформа разрешает их в
//!    «с компаньоном». Версия введения компаньона — из фактов, сама свёртка — из спека.
//!
//! # Чего проход НЕ делает
//! Не трогает ПЛОТНУЮ область дампа (дескрипторы метаданных): там платформа эмитит каждое
//! свойство каждого объекта, поэтому «версия проставила сама» уже воспроизводится писателем
//! из умолчания цели, и добавление копии в мешок ничего не меняет. Факты `materialized` для
//! метаданных в данных ЕСТЬ (`area: "metadata"`), но их владелец — XML-обёртка `Properties`/
//! `StandardAttribute`, а не элемент реестра, поэтому ключ пришлось бы додумывать; до
//! появления этого перевода правило к ним не применяется (и в них нет нужды: гейт матрицы
//! показывает расхождение апгрейда ЦЕЛИКОМ в телах форм).

use crate::ir::value::PropertyValue;
use crate::ir::{Configuration, FieldId, FormItem, MetadataObject};
use crate::resolve::field_upgrade_value;
use crate::spec::common::EntitySpec;
use crate::spec::forms::controls::{control_spec_for, silent_literal, FOLDED_LITERALS};
use crate::version::availability::PropertyScope;
use crate::version::FormatVersion;

/// Свойство, которое ЦЕЛЬ обязана нести, а значения для него не знает никто.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingUpgradeValue {
    /// Вид контрола-владельца (`"UsualGroup"`).
    pub owner: String,
    /// Где именно (`"<объект>.<форма>.<элемент>"`).
    pub at: String,
    /// Каноническое имя свойства.
    pub field: &'static str,
    /// Целевая версия, которая свойство требует.
    pub target: FormatVersion,
}

impl std::fmt::Display for MissingUpgradeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{} (обязательно в формате {}, умолчания нет ни в реестре, ни в спеке)",
            self.owner, self.at, self.field, self.target
        )
    }
}

/// Ошибка апгрейда: цель требует свойств, значения которых не засвидетельствованы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeError {
    /// Запрошенная целевая версия формата.
    pub target: FormatVersion,
    /// ВСЕ свойства без значения (не первое — весь список за один заход).
    pub missing: Vec<MissingUpgradeValue>,
}

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "отказ писать формат {}: версия проставляет {} свойств{}, значения которых \
             НЕ ЗАСВИДЕТЕЛЬСТВОВАНЫ — подставить их значило бы выдумать данные (§1.0)",
            self.target,
            self.missing.len(),
            if self.missing.len() == 1 { "о" } else { "" },
        )?;
        for m in &self.missing {
            writeln!(f, "  {m}")?;
        }
        write!(
            f,
            "почините модель: добавьте умолчание в models/version_registry.jsonl \
             (прогон scripts/version_surface_probe.sh) либо в спек поля"
        )
    }
}

impl std::error::Error for UpgradeError {}

/// Свернуть накопленный список в результат: пусто → `Ok(())`, иначе — [`UpgradeError`].
pub fn upgrade_result(
    target: FormatVersion,
    missing: Vec<MissingUpgradeValue>,
) -> Result<(), Box<UpgradeError>> {
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Box::new(UpgradeError { target, missing }))
    }
}

/// Применить правило 2 ко ВСЕЙ конфигурации (версия источника — из самого IR).
pub fn apply_upgrade(
    cfg: &mut Configuration,
    target: FormatVersion,
) -> Result<(), Box<UpgradeError>> {
    let Some(source) = cfg.source_version else {
        // Версия входа неизвестна — апгрейд не определён: неизвестно, ОТКУДА поднимаем.
        // Это не тихий пропуск, а отсутствие вопроса (§1.0): фактов «из V в T» без V нет.
        return Ok(());
    };
    let mut missing = Vec::new();
    for obj in &mut cfg.objects {
        upgrade_object(obj, source, target, &mut missing);
    }
    upgrade_result(target, missing)
}

/// Правило 2 для ОДНОГО объекта (и его child-объектов) — для потокового лейна, который
/// держит объекты по одному ([`crate::version::guard::collect_downgrade_violations`]-симметрия).
pub fn upgrade_object(
    obj: &mut MetadataObject,
    source: FormatVersion,
    target: FormatVersion,
    out: &mut Vec<MissingUpgradeValue>,
) {
    if target <= source {
        return;
    }
    for form in &mut obj.form_bodies {
        let where_ = format!("{}.{}", obj.name, form.name);
        for item in &mut form.body.items {
            upgrade_form_item(&where_, item, source, target, out);
        }
    }
    for child in &mut obj.children {
        upgrade_object(child, source, target, out);
    }
}

fn upgrade_form_item(
    where_: &str,
    item: &mut FormItem,
    source: FormatVersion,
    target: FormatVersion,
    out: &mut Vec<MissingUpgradeValue>,
) {
    let kind = item.kind.as_str().to_string();
    if let Some(spec) = control_spec_for(&kind) {
        let at = format!("{where_}.{}", item.name);
        let scope = PropertyScope::Form { owner: &kind };
        // Один и тот же факт адресует поле, которое у контрола может лежать и в общем
        // регионе, и в extInfo, — поэтому обрабатываются ОБА, а найдено оно ровно в том,
        // где его объявил спек.
        upgrade_region(
            &kind,
            &at,
            scope,
            spec.properties,
            &mut item.properties,
            source,
            target,
            out,
        );
        upgrade_region(
            &kind,
            &at,
            scope,
            spec.ext_info,
            &mut item.ext_info,
            source,
            target,
            out,
        );
    }
    for child in &mut item.children {
        upgrade_form_item(where_, child, source, target, out);
    }
    for add in &mut item.additions {
        upgrade_form_item(where_, add, source, target, out);
    }
}

/// Правило 2 для ОДНОГО региона свойств контрола (общего либо extInfo).
#[allow(clippy::too_many_arguments)]
fn upgrade_region(
    owner: &str,
    at: &str,
    scope: PropertyScope<'_>,
    spec: &EntitySpec,
    bag: &mut Vec<(FieldId, PropertyValue)>,
    source: FormatVersion,
    target: FormatVersion,
    out: &mut Vec<MissingUpgradeValue>,
) {
    // (2) ПЕРЕИМЕНОВАНИЕ — до материализации: после копии новое имя в мешке ЕСТЬ, и
    //     материализовать его уже не надо (значение источника сильнее умолчания версии).
    for (from_name, to_name) in crate::version::form_renamed_on_upgrade(owner, source, target) {
        let (Some(from), Some(to)) = (field_by_name(spec, from_name), field_by_name(spec, to_name))
        else {
            continue;
        };
        if bag.iter().any(|(k, _)| *k == to) {
            continue;
        }
        if let Some((_, v)) = bag.iter().find(|(k, _)| *k == from) {
            let value = v.clone();
            bag.push((to, value));
        }
    }

    // (1) МАТЕРИАЛИЗАЦИЯ.
    for (name, _witnessed) in crate::version::form_materialized_on_upgrade(owner, source, target) {
        let Some(fs) = spec.fields().iter().find(|f| f.name == name) else {
            continue;
        };
        // «Источник промолчал» — это либо пустое место в мешке, либо ЛИТЕРАЛ МОЛЧАНИЯ, которым
        // ридер восстановил омиссию тега (см. `SilentLiteral`): Designer этот литерал ОПУСКАЕТ,
        // поэтому явно задать его в дампе нельзя, и подмена данных исключена.
        let silent = silent_literal(owner, fs.name);
        let existing = bag.iter().position(|(k, _)| *k == fs.id);
        if let Some(i) = existing {
            let is_silence = silent.is_some_and(|lit| {
                matches!(&bag[i].1, PropertyValue::Enum(t) if t.as_str() == lit)
            });
            if !is_silence {
                continue;
            }
        }
        match field_upgrade_value(scope, fs, target) {
            Some(v) => match existing {
                Some(i) => bag[i].1 = v.clone(),
                None => bag.push((fs.id, v.clone())),
            },
            None => out.push(MissingUpgradeValue {
                owner: owner.to_string(),
                at: at.to_string(),
                field: fs.name,
                target,
            }),
        }
    }

    // (3) СВЁРНУТАЯ ПАРА: литерал, неотличимый в источнике, разрешается в «с компаньоном».
    for fold in FOLDED_LITERALS {
        // Компаньон существует в цели и НЕ существовал в источнике — иначе источник сам
        // различал оба литерала, и трогать его значение было бы порчей данных.
        if crate::version::form_field_available_in(owner, fold.companion, source)
            || !crate::version::form_field_available_in(owner, fold.companion, target)
        {
            continue;
        }
        let Some(fs) = spec.fields().iter().find(|f| f.name == fold.field) else {
            continue;
        };
        for (id, value) in bag.iter_mut() {
            if *id != fs.id {
                continue;
            }
            if matches!(value, PropertyValue::Enum(t) if t.as_str() == fold.without_companion) {
                *value = PropertyValue::Enum(crate::ir::value::Token::new(fold.with_companion));
            }
        }
    }
}

/// `FieldId` поля спека по каноническому имени.
fn field_by_name(spec: &EntitySpec, name: &str) -> Option<FieldId> {
    spec.fields().iter().find(|f| f.name == name).map(|f| f.id)
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::ir::value::Token;
    use crate::ir::{FormControlKind, NamedFormBody, ObjectKind, Uuid};

    const V217: FormatVersion = FormatVersion::new(2, 17);
    const V220: FormatVersion = FormatVersion::new(2, 20);
    const V221: FormatVersion = FormatVersion::new(2, 21);

    fn object_with(items: Vec<FormItem>) -> MetadataObject {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Тест", Uuid([0u8; 16]));
        let mut body = crate::ir::FormBody::default();
        body.items = items;
        obj.form_bodies.push(NamedFormBody {
            name: "ФормаЭлемента".into(),
            body,
            module: None,
            help: Vec::new(),
            help_resources: Vec::new(),
        });
        obj
    }

    fn ext_of<'a>(obj: &'a MetadataObject, name: &str) -> &'a [(FieldId, PropertyValue)] {
        &obj.form_bodies[0]
            .body
            .items
            .iter()
            .find(|i| i.name == name)
            .expect("контрол на месте")
            .ext_info
    }

    /// Материализация: у `UsualGroup` в мешке нет `group`, цель 2.21 ставит его САМА.
    /// Значение — витнесс реестра для 2.21 (`HorizontalIfPossible`), а не «умолчание».
    #[test]
    fn materializes_group_on_upgrade() {
        let mut obj = object_with(vec![FormItem::new(
            FormControlKind::new("UsualGroup"),
            "Группа",
            1,
        )]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V217, V221, &mut missing);
        assert!(missing.is_empty(), "{missing:?}");
        let fid = crate::spec::forms::controls::form_group::F_EXT_GROUP;
        assert_eq!(
            ext_of(&obj, "Группа")
                .iter()
                .find(|(k, _)| *k == fid)
                .map(|(_, v)| v),
            Some(&PropertyValue::Enum(Token::new("HorizontalIfPossible")))
        );
    }

    /// НЕ-МОНОТОННОСТЬ материализации, витнессированная платформой: тот же тег, та же цель,
    /// но источник 2.20 — и платформа НЕ ставит ничего (в отличие от источника 2.17).
    /// Дописать здесь значение = сделать конфигурацию ОТЛИЧНОЙ от платформенной, что и
    /// показывал `/CompareCfg` на ячейке 2.20→2.21.
    #[test]
    fn materialization_is_not_monotonic_in_the_source_version() {
        let mut obj = object_with(vec![FormItem::new(
            FormControlKind::new("UsualGroup"),
            "Группа",
            1,
        )]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V220, V221, &mut missing);
        assert!(missing.is_empty());
        assert!(
            ext_of(&obj, "Группа").is_empty(),
            "2.20 → 2.21: платформа `group` не материализует"
        );
    }

    /// Байт-НЕЙТРАЛЬНОСТЬ: цель == источник ⇒ правило 2 не дописывает НИЧЕГО.
    #[test]
    fn same_version_changes_nothing() {
        for v in [V217, V220, V221] {
            let mut obj = object_with(vec![FormItem::new(
                FormControlKind::new("UsualGroup"),
                "Группа",
                1,
            )]);
            let mut missing = Vec::new();
            upgrade_object(&mut obj, v, v, &mut missing);
            assert!(missing.is_empty());
            assert!(
                ext_of(&obj, "Группа").is_empty(),
                "версия {v}: мешок обязан остаться пустым"
            );
        }
    }

    /// ЛИТЕРАЛ МОЛЧАНИЯ в мешке — то же «источник промолчал», что и пустое место:
    /// ридер восстановил им омиссию тега, и материализация обязана его ЗАМЕНИТЬ. Без этого
    /// 37 групп витнесса уезжали в цель с ординалом `Auto` вместо платформенного.
    #[test]
    fn silence_literal_is_replaced_like_an_absent_field() {
        let fid = crate::spec::forms::controls::form_group::F_EXT_GROUP;
        let mut item = FormItem::new(FormControlKind::new("Page"), "Страница", 1);
        item.ext_info.push((
            fid,
            PropertyValue::Enum(Token::new(
                crate::spec::forms::controls::form_group::DESIGNER_AUTO,
            )),
        ));
        let mut obj = object_with(vec![item]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V217, V221, &mut missing);
        assert!(missing.is_empty(), "{missing:?}");
        let ext = ext_of(&obj, "Страница");
        assert_eq!(ext.len(), 1, "поле остаётся ОДНО, а не дублируется");
        assert_eq!(
            ext[0].1,
            PropertyValue::Enum(Token::new("Vertical")),
            "Page: платформа, поднимая старый дамп, кладёт Vertical"
        );
    }

    /// Заданное в источнике значение материализация НЕ переписывает.
    #[test]
    fn explicit_source_value_survives() {
        let mut item = FormItem::new(FormControlKind::new("UsualGroup"), "Группа", 1);
        let fid = crate::spec::forms::controls::form_group::F_EXT_GROUP;
        item.ext_info
            .push((fid, PropertyValue::Enum(Token::new("Vertical"))));
        let mut obj = object_with(vec![item]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V217, V221, &mut missing);
        assert_eq!(
            ext_of(&obj, "Группа")
                .iter()
                .find(|(k, _)| *k == fid)
                .map(|(_, v)| v),
            Some(&PropertyValue::Enum(Token::new("Vertical")))
        );
    }

    /// Свёрнутая пара: `editMode = EnterOnInput` из источника, НЕ знавшего компаньона
    /// `autoEditMode`, поднимается до `Auto` — ровно так его разрешает платформа.
    #[test]
    fn folded_literal_is_promoted_when_companion_is_new() {
        let mut item = FormItem::new(FormControlKind::new("InputField"), "Поле", 1);
        let fid = crate::spec::forms::controls::form_field::F_EDIT_MODE;
        item.properties
            .push((fid, PropertyValue::Enum(Token::new("EnterOnInput"))));
        let mut obj = object_with(vec![item]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V220, V221, &mut missing);
        let props = &obj.form_bodies[0].body.items[0].properties;
        assert_eq!(
            props.iter().find(|(k, _)| *k == fid).map(|(_, v)| v),
            Some(&PropertyValue::Enum(Token::new("Auto")))
        );
    }

    /// …и НЕ поднимается, когда источник компаньона уже знал: там `EnterOnInput` — данные.
    #[test]
    fn folded_literal_is_left_alone_within_one_version() {
        let mut item = FormItem::new(FormControlKind::new("InputField"), "Поле", 1);
        let fid = crate::spec::forms::controls::form_field::F_EDIT_MODE;
        item.properties
            .push((fid, PropertyValue::Enum(Token::new("EnterOnInput"))));
        let mut obj = object_with(vec![item]);
        let mut missing = Vec::new();
        upgrade_object(&mut obj, V221, V221, &mut missing);
        let props = &obj.form_bodies[0].body.items[0].properties;
        assert_eq!(
            props.iter().find(|(k, _)| *k == fid).map(|(_, v)| v),
            Some(&PropertyValue::Enum(Token::new("EnterOnInput")))
        );
    }

    /// ДВА ВИТНЕССА об одном: значение факта `materialized` (дифференциал дампов
    /// integration_subsystem) обязано совпадать с тем, что даёт шов
    /// [`field_upgrade_value`] по реестру (прогон конфигурации ПОКРЫТИЯ). Корпуса разные —
    /// значит совпадение не тавтология, а подтверждение.
    #[test]
    fn materialized_fact_agrees_with_the_registry_witness() {
        let mut checked = 0usize;
        for &(owner, name, _from, (major, minor), value) in crate::version::FORM_MATERIALIZED_RAW {
            let Some(spec) = control_spec_for(owner) else {
                continue;
            };
            let Some(fs) = spec
                .properties
                .fields()
                .iter()
                .chain(spec.ext_info.fields().iter())
                .find(|f| f.name == name)
            else {
                continue;
            };
            let target = FormatVersion::new(major, minor);
            let resolved = field_upgrade_value(PropertyScope::Form { owner }, fs, target)
                .unwrap_or_else(|| panic!("{owner}.{name}: значение апгрейда не разрешилось"));
            assert_eq!(
                resolved,
                &PropertyValue::Enum(Token::new(value)),
                "{owner}.{name} @ {target}: факт и реестр расходятся"
            );
            checked += 1;
        }
        assert!(checked >= 3, "подтверждений слишком мало: {checked}");
    }

    /// Свойство БЕЗ засвидетельствованного значения даёт ВНЯТНУЮ ошибку, а не тихую
    /// подстановку. Проверяем сам шов правила 2 на поле, у которого нет ни витнесса
    /// реестра, ни умолчания спека.
    #[test]
    fn missing_value_is_a_loud_error() {
        let spec = control_spec_for("RadioButtonField").expect("вид зарегистрирован");
        let fs = spec
            .ext_info
            .fields()
            .iter()
            .find(|f| f.name == "choiceList")
            .expect("поле объявлено");
        let scope = PropertyScope::Form {
            owner: "RadioButtonField",
        };
        assert!(
            field_upgrade_value(scope, fs, V221).is_none(),
            "у choiceList умолчания нет ни у реестра, ни у спека"
        );
        let err = upgrade_result(
            V221,
            vec![
                MissingUpgradeValue {
                    owner: "RadioButtonField".into(),
                    at: "Тест.ФормаЭлемента.Переключатель".into(),
                    field: "choiceList",
                    target: V221,
                },
                MissingUpgradeValue {
                    owner: "UsualGroup".into(),
                    at: "Тест.ФормаЭлемента.Группа".into(),
                    field: "group",
                    target: V221,
                },
            ],
        )
        .expect_err("список непуст ⇒ отказ");
        let text = err.to_string();
        // Сообщение — часть контракта правила 2, поэтому его печатаем (`-- --nocapture`).
        println!("{text}");
        assert!(text.contains("choiceList"), "{text}");
        assert!(text.contains("group"), "ВСЕ, а не первое: {text}");
    }
}
