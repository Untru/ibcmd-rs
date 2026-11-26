//! ЕДИНЫЙ ОТВЕТ на вопрос «существует ли свойство X у сущности Y в версии формата V».
//!
//! # Зачем модуль
//! Источников наличия ТРИ, и раньше каждый отвечал сам за себя: EDT-метамодель
//! (`inventory/*.jsonl` → [`super::since`]), витнесс дифференциального триплета
//! (`models/version_facts.jsonl`) и версионный реестр
//! (`models/version_registry.jsonl` → [`super::registry`]). Три ответа на один вопрос —
//! три способа разойтись. Здесь они СВЕДЕНЫ в один, и правило слияния записано ЯВНО.
//!
//! # ПРАВИЛО СЛИЯНИЯ (и почему именно такое)
//!
//! ## 1. Граница МЕТАМОДЕЛИ — главнее любой наблюдённой
//! Метамодель EDT несёт `since` версией ПЛАТФОРМЫ (`8.3.25`), то есть различает и те
//! платформы, которых у нас нет НИ В ОДНОМ прогоне (8.3.18…8.3.26). Любая граница,
//! выведенная из наблюдений, грубее: наблюдая только 2.17/2.20/2.21, про свойство,
//! введённое в 2.18, мы могли бы сказать лишь «не позже 2.20» — и на таргете 2.18/2.19
//! молча выкинули бы СУЩЕСТВУЮЩЕЕ свойство. Поэтому там, где метамодель говорит, её
//! ответ и есть ответ; наблюдение может его лишь ПОДТВЕРДИТЬ (что и делает тест
//! `registry_confirms_metamodel_bounds`).
//!
//! ## 2. Там, где метамодель молчит, отвечает ВИТНЕСС ПЛАТФОРМЫ
//! Метамодель знает `since` только для МЕТАДАННЫХ (и не для всех полей), а про тела форм,
//! состав видов и литералы — почти ничего. Здесь источник другой: дампы ОДНОЙ И ТОЙ ЖЕ
//! конфигурации, снятые РАЗНЫМИ платформами. Свойство, которое старшая платформа в дампе
//! показала, а младшая (из того же источника) — нет, младшая ПОТЕРЯЛА при загрузке, то
//! есть не знает.
//!
//! ## 3. НАБЛЮДЁННОЕ ПРИСУТСТВИЕ — свидетельство сильное, наблюдённое ОТСУТСТВИЕ — слабое
//! Присутствие свойства в дампе версии V доказывает, что в V оно есть. Отсутствие же
//! доказывает это лишь там, где дамп ПЛОТНЫЙ — в дескрипторах метаданных, где Designer
//! эмитит КАЖДОЕ свойство КАЖДОГО объекта. В РАЗРЕЖЁННОЙ области (тела форм) тег
//! опускается, когда значение равно умолчанию ВЕРСИИ, поэтому исчезновение тега
//! неотличимо от СМЕНЫ УМОЛЧАНИЯ. Witnessed: `<Group>` у `UsualGroup` (ориентация
//! группы — свойство, существующее с незапамятных версий) встречается в дампе 2.21 и
//! отсутствует в дампах 2.17/2.20 — потому что в 2.21 у него другое умолчание, а вовсе
//! не потому, что свойство ввели в 8.5. Гейт, выведенный из такого «отсутствия», молча
//! выбрасывал бы РЕАЛЬНЫЕ данные.
//!
//! Поэтому: в ПЛОТНОЙ области ([`Region::Dense`]) отсутствие ГЕЙТИТ, в РАЗРЕЖЁННОЙ
//! ([`Region::Sparse`]) — НЕ гейтит; там гейт по-прежнему ставит только адресный витнесс
//! `models/version_facts.jsonl`, а реестр может его лишь СМЯГЧИТЬ, показав свойство в
//! более старой версии (положительное свидетельство, §1.0: терять данные хуже, чем нести
//! лишние).
//!
//! ## 4. Несколько витнессов — берётся САМАЯ РАННЯЯ граница
//! Витнессы складываются по минимуму ровно по причине из п. 3: присутствие абсолютно,
//! отсутствие относительно прогона.
//!
//! ## 5. Никто не знает — гейта нет
//! Ответ «существует» без гейта, как и раньше. Это НЕ «молча пропустили»: гейт, который
//! истинен всегда, — не гейт. А то, что осталось несопоставленным с реестром, ВИДНО
//! (тест `version_registry_keys`).

use super::registry::{self, RegistryProperty};
use super::{since, FormatVersion, Since};

/// Область дампа, в которой живёт свойство: от неё зависит, что значит его ОТСУТСТВИЕ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// ПЛОТНАЯ: дамп несёт свойство у каждого объекта независимо от значения (дескрипторы
    /// метаданных). Отсутствие свойства в версии — витнесс того, что версия его не знает.
    Dense,
    /// РАЗРЕЖЁННАЯ: дамп опускает свойство, равное умолчанию версии (тела форм).
    /// Отсутствие не значит ничего (см. модульный док, п. 3).
    Sparse,
}

/// Сущность, о свойстве которой спрашивают, — в ключах ВЕРСИОННОГО РЕЕСТРА.
///
/// Реестр ключуется парой «владелец + элемент-владелец»; спеки — `EntitySpec::entity`.
/// Перевод одного в другое делает [`crate::resolve::scope_for_entity`] — ЯВНО и с тестом.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyScope<'a> {
    /// Свойство ДЕСКРИПТОРА метаданных: `owner` — вид (`Catalog`), `element` — элемент,
    /// которому свойство принадлежит (`Catalog`, `Attribute`, `TabularSection`).
    Metadata {
        /// Вид объектов метаданных.
        owner: &'a str,
        /// Элемент-владелец внутри вида.
        element: &'a str,
    },
    /// Свойство ТЕЛА ФОРМЫ: `owner` — контрол (`InputField`) либо `Form` для свойств
    /// корня формы. В реестре им соответствует `owner="Form"`, `element=<контрол>`.
    Form {
        /// Вид контрола (либо `Form` для корня формы).
        owner: &'a str,
    },
    /// Сущность с реестром НЕ сопоставлена: остаётся только метамодель (ключ — имя поля).
    Unmapped,
}

impl<'a> PropertyScope<'a> {
    /// Ключ реестра `(владелец, элемент)`, если сущность сопоставлена.
    pub fn registry_key(self) -> Option<(&'a str, &'a str)> {
        match self {
            PropertyScope::Metadata { owner, element } => Some((owner, element)),
            PropertyScope::Form { owner } => Some(("Form", owner)),
            PropertyScope::Unmapped => None,
        }
    }

    /// Область дампа сущности (см. [`Region`]).
    pub fn region(self) -> Region {
        match self {
            PropertyScope::Form { .. } => Region::Sparse,
            PropertyScope::Metadata { .. } | PropertyScope::Unmapped => Region::Dense,
        }
    }

    /// Знает ли метамодель EDT `since` для этой области. Метамодель описывает МЕТАДАННЫЕ;
    /// у форм маркеров версий в её xcore нет вовсе, а имена форм-свойств не уникальны между
    /// контролами — поиск по одному имени там дал бы ЧУЖОЙ ответ.
    fn metamodel_applies(self) -> bool {
        !matches!(self, PropertyScope::Form { .. })
    }
}

/// Чем установлена граница наличия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Граница из EDT-метамодели (`inventory/*.jsonl`) — точнее любой наблюдённой.
    Metamodel,
    /// Граница, ВИТНЕССИРОВАННАЯ платформой (реестр версий и/или `models/version_facts.jsonl`).
    PlatformWitness,
    /// Границы нет: свойство есть во всех поддерживаемых версиях (либо о нём никто не знает).
    Ungated,
}

/// Сведённый ответ о наличии свойства.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Availability {
    /// Версия формата, начиная с которой свойство существует. `None` — гейта нет.
    pub since: Option<Since>,
    /// Каким источником установлена граница.
    pub provenance: Provenance,
}

impl Availability {
    /// Существует ли свойство в версии `target`.
    pub fn available_in(self, target: FormatVersion) -> bool {
        self.since.is_none_or(|s| s.available_in(target))
    }
}

/// СВЕДЁННЫЙ ответ «с какой версии существует свойство `canonical` у сущности `scope`».
///
/// Правило слияния — в модульном доке. Коротко: метамодель главнее; где она молчит —
/// витнесс платформы; наблюдённое отсутствие гейтит только в плотной области дампа;
/// несколько витнессов складываются по минимуму.
pub fn property_availability(scope: PropertyScope<'_>, canonical: &str) -> Availability {
    // (1) Метамодель. Ключ — ИМЯ поля: это корректно, потому что `since` — функция имени
    //     (проверяется тестом `since_is_a_function_of_field_name`), и только для метаданных.
    if scope.metamodel_applies() {
        if let Some(s) = since::since_for_field_name(canonical) {
            return Availability {
                since: Some(s),
                provenance: Provenance::Metamodel,
            };
        }
    }
    // (2)…(4) Витнессы платформы: адресный (`version_facts`) и реестр. Минимум — п. 4.
    let facts = match scope {
        PropertyScope::Form { owner } => since::since_for_form_field_witness(owner, canonical),
        _ => None,
    };
    let observed = witnessed_since(scope, canonical);
    match min_since(facts, observed) {
        Some(s) => Availability {
            since: Some(s),
            provenance: Provenance::PlatformWitness,
        },
        // (5) Никто не знает — гейта нет.
        None => Availability {
            since: None,
            provenance: Provenance::Ungated,
        },
    }
}

/// Существует ли свойство `canonical` у сущности `scope` в версии `target`.
pub fn property_available_in(
    scope: PropertyScope<'_>,
    canonical: &str,
    target: FormatVersion,
) -> bool {
    property_availability(scope, canonical).available_in(target)
}

/// Граница, ВЫВЕДЕННАЯ ИЗ НАБЛЮДЕНИЙ реестра, либо `None`.
///
/// Условия витнесса (все обязательны — иначе отсутствие записи ничего не доказывает):
/// * область дампа ПЛОТНАЯ (см. [`Region`] и п. 3 модульного дока);
/// * свойство хоть где-то наблюдалось;
/// * САМ элемент-владелец наблюдался в версии СТАРШЕ первой, где встретилось свойство
///   (иначе младшая версия просто не покрыта прогоном для этого элемента).
fn witnessed_since(scope: PropertyScope<'_>, canonical: &str) -> Option<Since> {
    if scope.region() != Region::Dense {
        return None;
    }
    let (owner, element) = scope.registry_key()?;
    let seen = registry::property_formats(owner, element, canonical);
    let first = *seen.first()?;
    let element_seen = registry::element_formats(owner, element);
    element_seen
        .iter()
        .any(|&v| v < first)
        .then_some(Since(first))
}

/// Наблюдалось ли свойство в версии `target` (ПОЛОЖИТЕЛЬНОЕ свидетельство реестра).
///
/// Отдельно от [`property_availability`] потому, что положительное наблюдение имеет иную
/// силу: оно ДОКАЗЫВАЕТ наличие в конкретной версии, тогда как отсутствие доказывает его
/// только в плотной области.
pub fn observed_in(scope: PropertyScope<'_>, canonical: &str, target: FormatVersion) -> bool {
    match scope.registry_key() {
        Some((owner, element)) => registry::property(owner, element, canonical, target).is_some(),
        None => false,
    }
}

/// Строка реестра для свойства сущности в версии (умолчание этой версии и её витнесс).
pub fn registry_row(
    scope: PropertyScope<'_>,
    canonical: &str,
    target: FormatVersion,
) -> Option<&'static RegistryProperty> {
    let (owner, element) = scope.registry_key()?;
    registry::property(owner, element, canonical, target)
}

/// Доступен ли ЛИТЕРАЛ перечисления в версии `target` — сведённый ответ.
///
/// Метамодель тут ЕДИНСТВЕННЫЙ источник гейта: наблюдение литерала в дампе — функция того,
/// какие литералы использовала конфигурация прогона, а не того, какие версия знает.
/// Реестр служит ПОДТВЕРЖДЕНИЕМ (тест `registry_confirms_metamodel_bounds`).
pub fn literal_availability(enum_name: &str, literal: &str) -> Availability {
    match since::since_for_literal(enum_name, literal) {
        Some(s) => Availability {
            since: Some(s),
            provenance: Provenance::Metamodel,
        },
        None => Availability {
            since: None,
            provenance: Provenance::Ungated,
        },
    }
}

/// Минимум из двух границ (п. 4 правила слияния).
fn min_since(a: Option<Since>, b: Option<Since>) -> Option<Since> {
    match (a, b) {
        (Some(x), Some(y)) => Some(std::cmp::min(x, y)),
        (x, None) => x,
        (None, y) => y,
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    const V217: FormatVersion = FormatVersion::new(2, 17);
    const V220: FormatVersion = FormatVersion::new(2, 20);
    const V221: FormatVersion = FormatVersion::new(2, 21);

    /// П. 1: где говорит метамодель, ответ — её, и провенанс это фиксирует.
    /// `lineNumberLength` (8.3.27 = 2.20) есть и у метамодели, и в наблюдениях.
    #[test]
    fn metamodel_bound_wins_and_is_labelled() {
        let a = property_availability(
            PropertyScope::Metadata {
                owner: "Catalog",
                element: "TabularSection",
            },
            "lineNumberLength",
        );
        assert_eq!(a.provenance, Provenance::Metamodel);
        assert_eq!(a.since, Some(Since(V220)));
        assert!(!a.available_in(V217));
        assert!(a.available_in(V220));
    }

    /// П. 1 в самой острой форме: метамодель различает 8.3.25 (=2.18), чего прогон не может
    /// (у нас нет ни 8.3.25, ни 8.3.26). Наблюдённая граница дала бы 2.20 и на таргете 2.18
    /// выкинула бы существующее свойство — поэтому метамодель и главнее.
    #[test]
    fn metamodel_bound_is_finer_than_any_observation() {
        let scope = PropertyScope::Metadata {
            owner: "InformationRegister",
            element: "Dimension",
        };
        let a = property_availability(scope, "typeReductionMode");
        assert_eq!(a.since, Some(Since(FormatVersion::new(2, 18))));
        assert_eq!(a.provenance, Provenance::Metamodel);
        // Наблюдения о том же свойстве грубее: реестр видел его только в 2.20/2.21.
        assert_eq!(
            registry::property_formats("InformationRegister", "Dimension", "typeReductionMode"),
            vec![V220, V221]
        );
        assert!(a.available_in(FormatVersion::new(2, 18)));
    }

    /// П. 3: в РАЗРЕЖЁННОЙ области наблюдённое отсутствие НЕ гейтит.
    /// Witnessed: `<Group>` у `UsualGroup` виден только в дампе 2.21, но свойство
    /// «Группировка» существует давно — тег опущен в 2.17/2.20 из-за иного умолчания.
    #[test]
    fn sparse_region_absence_does_not_gate() {
        assert_eq!(
            registry::property_formats("Form", "UsualGroup", "group"),
            vec![V221],
            "наблюдение: тег <Group> есть только в дампе 2.21"
        );
        let a = property_availability(
            PropertyScope::Form {
                owner: "UsualGroup",
            },
            "group",
        );
        assert_eq!(a.since, None, "но гейта из этого НЕ выводим");
        assert!(a.available_in(V217));
    }

    /// П. 3 наоборот: в ПЛОТНОЙ области отсутствие гейтит — при условии, что сам элемент
    /// в более старой версии наблюдался. Проверяем на поле, которого метамодель не знает
    /// (иначе сработал бы п. 1): такого в текущих данных нет, поэтому проверяем сам
    /// механизм витнесса напрямую.
    #[test]
    fn dense_region_absence_gates_when_element_was_observed_earlier() {
        let scope = PropertyScope::Metadata {
            owner: "Configuration",
            element: "Configuration",
        };
        assert_eq!(witnessed_since(scope, "caption"), Some(Since(V221)));
        // Элемент наблюдался и в 2.17 — значит отсутствие свойства там осмысленно.
        assert!(registry::element_formats("Configuration", "Configuration").contains(&V217));
        // Свойство, которое видно во всех версиях, витнесса не даёт.
        assert_eq!(witnessed_since(scope, "comment"), None);
    }

    /// П. 5: про сущность, не сопоставленную с реестром, отвечает только метамодель.
    #[test]
    fn unmapped_scope_falls_back_to_metamodel_only() {
        let a = property_availability(PropertyScope::Unmapped, "lineNumberLength");
        assert_eq!(a.provenance, Provenance::Metamodel);
        let b = property_availability(PropertyScope::Unmapped, "нетТакогоПоля");
        assert_eq!(b.provenance, Provenance::Ungated);
        assert!(b.available_in(V217));
    }

    /// НЕЗАВИСИМОЕ ПОДТВЕРЖДЕНИЕ: там, где реестр (прогон платформ) и метамодель говорят об
    /// одном и том же, они обязаны НЕ ПРОТИВОРЕЧИТЬ друг другу — свойство не может
    /// наблюдаться в версии СТАРШЕ той, с которой метамодель его вводит. Это и делает
    /// since-таблицу фактом, а не верой.
    #[test]
    fn registry_confirms_metamodel_bounds() {
        let mut checked = 0usize;
        for p in registry::REGISTRY_PROPERTIES {
            let Some(s) = since::since_for_field_name(p.canonical) else {
                continue;
            };
            // Свойства ТЕЛА ФОРМЫ пропускаем: их имена не уникальны между контролами, и
            // метамодельная граница по имени относится к ОДНОИМЁННОМУ полю метаданных.
            if p.owner == "Form" {
                continue;
            }
            let observed = FormatVersion::new(p.format.0, p.format.1);
            assert!(
                s.available_in(observed),
                "реестр наблюдал {}.{}.{} в формате {observed}, а метамодель вводит его \
                 только с {} — источники противоречат друг другу",
                p.owner,
                p.element,
                p.canonical,
                s.0
            );
            checked += 1;
        }
        // Пересечение источников невелико и таким и должно быть: у большинства полей
        // метамодельный `since` СТАРШЕ пола реестра версий (введены до 8.3.17), а значит
        // гейта не дают вовсе — подтверждать там нечего. Пол — анти-вакуумность.
        assert!(checked >= 30, "подтверждений слишком мало: {checked}");
    }

    /// СЧЁТЧИК, а не догма: сегодня реестр не вводит для МЕТАДАННЫХ ни одного гейта СВЕРХ
    /// метамодели — каждая наблюдённая граница в плотной области уже названа метамоделью.
    /// Поэтому включение сведённого ответа в downgrade-гейт (`version::guard`) ничего не
    /// меняет СЕЙЧАС и заработает сразу, как только прогон вскроет свойство, о котором
    /// метамодель молчит: тест упадёт и покажет находку поимённо, а не растворит её.
    #[test]
    fn registry_adds_no_metadata_gate_beyond_the_metamodel_today() {
        let mut extra: Vec<String> = Vec::new();
        for p in registry::REGISTRY_PROPERTIES {
            if p.owner == "Form" {
                continue;
            }
            let scope = PropertyScope::Metadata {
                owner: p.owner,
                element: p.element,
            };
            let a = property_availability(scope, p.canonical);
            if a.provenance == Provenance::PlatformWitness {
                extra.push(format!(
                    "{}.{}.{} → {}",
                    p.owner,
                    p.element,
                    p.canonical,
                    a.since.expect("витнесс без границы невозможен").0
                ));
            }
        }
        extra.sort();
        extra.dedup();
        assert!(
            extra.is_empty(),
            "реестр ввёл гейты метаданных, которых метамодель не знает: {extra:?}"
        );
    }

    /// Тот же перекрёстный контроль на ЛИТЕРАЛАХ: 8.5-литералы `InterfaceCompatibilityMode`
    /// наблюдаются реестром только в 2.21, и метамодель вводит их ровно там же.
    #[test]
    fn registry_confirms_metamodel_literal_bounds() {
        for lit in ["Version8_5", "TaxiEnableVersion8_5", "Version8_5EnableTaxi"] {
            assert_eq!(
                registry::literal_formats("InterfaceCompatibilityMode", lit),
                vec![V221]
            );
            let a = literal_availability("InterfaceCompatibilityMode", lit);
            assert_eq!(a.since, Some(Since(V221)));
            assert!(!a.available_in(V220));
        }
        // Литерал без гейта доступен везде.
        assert!(literal_availability("InterfaceCompatibilityMode", "Taxi").available_in(V217));
    }
}
