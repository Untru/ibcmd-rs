//! ВЕРСИОННЫЙ РЕЕСТР сериализационной поверхности как ДАННЫЕ (FORMATS.md §7).
//!
//! # Что это
//! `models/version_registry.jsonl` отвечает на вопросы «что вообще есть в версии формата»:
//! какие ВИДЫ объектов знает платформа, какие у них СВОЙСТВА, каково УМОЛЧАНИЕ свойства
//! именно в этой версии, какие ЛИТЕРАЛЫ перечислений наблюдались. Собран он прогоном
//! конфигурации ПОКРЫТИЯ через все доступные платформы (`scripts/version_surface_probe.sh`
//! → `tools/build_version_registry.py`), то есть это НАБЛЮДЕНИЕ ЗА ПЛАТФОРМОЙ, а не наша
//! модель.
//!
//! Здесь — только ДОСТУП к этим данным: таблицы генерирует `build.rs` (рукописных списков
//! свойств/версий в коде нет), а правила, как реестр СОЧЕТАЕТСЯ с метамоделью, живут в
//! [`super::availability`] (наличие) и в [`crate::resolve`] (умолчания).
//!
//! # Ключ соединения со спеками
//! Реестр ключуется тройкой «владелец + элемент-владелец + КАНОНИЧЕСКОЕ имя»:
//! * `owner` — вид дескриптора (`Catalog`) либо `Form` для всего, что лежит в теле формы;
//! * `element` — элемент, которому свойство принадлежит: сам вид (`Catalog`), его часть
//!   (`Attribute`, `TabularSection`) либо контрол формы (`InputField`);
//! * `canonical` — имя EDT (`lineNumberLength`), а где реестр его не разрешил — Designer-имя,
//!   канонизированное конвенцией платформы (различие — регистр первой буквы).
//!
//! Спеки ключуются `EntitySpec::entity` + `FieldSpec::name`; перевод одного в другое —
//! [`crate::resolve::scope_for_entity`], и он ПРОВЕРЯЕМ (тест `version_registry_keys`
//! показывает, сколько полей спеков нашли себя в реестре и сколько нет).
//!
//! # Чего реестр НЕ знает
//! Версии, которых не было в прогоне (у нас нет платформ 8.3.18…8.3.26). Поэтому «свойства
//! нет в записях версии V» — осмысленно ТОЛЬКО когда V сама покрыта прогоном
//! ([`covers_version`]), и даже тогда лишь при выполненных условиях витнесса (см.
//! [`super::availability`]).

use std::collections::HashMap;
use std::sync::OnceLock;

use super::FormatVersion;

/// Одна строка реестра СВОЙСТВ: свойство, наблюдённое в конкретной версии формата.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryProperty {
    /// Версия формата `(major, minor)`, в дампе которой свойство наблюдалось.
    pub format: (u16, u16),
    /// Вид дескриптора (`Catalog`) либо `Form` для свойств тела формы.
    pub owner: &'static str,
    /// Элемент-владелец: сам вид, его часть (`Attribute`) или контрол формы (`InputField`).
    pub element: &'static str,
    /// Каноническое (EDT) имя свойства — ключ соединения со спеками.
    pub canonical: &'static str,
    /// Имя тега в Designer-XML (`LineNumberLength`) — для диагностики и плотных регионов.
    pub designer: &'static str,
    /// Умолчание, ВИТНЕССИРОВАННОЕ для ЭТОЙ версии (текст, как в дампе), либо `None` —
    /// прогон умолчания не установил (значение не было единогласным либо источник задавал
    /// свойство везде).
    pub default: Option<&'static str>,
}

// Сгенерированное build.rs из `models/version_registry.jsonl`: `REGISTRY_VERSIONS`,
// `REGISTRY_KINDS`, `REGISTRY_PROPERTIES`, `REGISTRY_LITERALS`.
include!(concat!(env!("OUT_DIR"), "/version_registry_tables.rs"));

/// Версии формата, покрытые прогоном реестра (по возрастанию).
pub fn versions() -> Vec<FormatVersion> {
    REGISTRY_VERSIONS
        .iter()
        .map(|&(major, minor)| FormatVersion::new(major, minor))
        .collect()
}

/// Покрыта ли версия прогоном реестра. Только для покрытой версии ОТСУТСТВИЕ записи
/// вообще что-то значит.
pub fn covers_version(v: FormatVersion) -> bool {
    REGISTRY_VERSIONS.contains(&(v.major, v.minor))
}

/// Существует ли ВИД объектов в версии — по росписи корня `.cf`, собранного платформой
/// (она несёт группу на каждый известный ей вид, даже с нулём объектов).
///
/// `None` — версия прогоном не покрыта, ответа НЕТ (а не «не существует»).
pub fn kind_available_in(kind: &str, target: FormatVersion) -> Option<bool> {
    if !covers_version(target) {
        return None;
    }
    Some(
        REGISTRY_KINDS
            .iter()
            .any(|&(fmt, k)| fmt == (target.major, target.minor) && k == kind),
    )
}

/// Есть ли у ГРУППЫ с этим class-guid'ом место в росписи корня версии `target`.
///
/// Роспись корня `.cf` — это и есть состав версии: платформа кладёт группу на каждый
/// известный ей вид (даже с нулём объектов) плюс несколько зарезервированных БЕЗЫМЯННЫХ
/// групп. Поэтому вопрос «появилась ли группа этого вида к версии V» — вопрос К ДАННЫМ, а не
/// таблица «guid → версия введения» в коде.
///
/// * `Some(true)`/`Some(false)` — версия покрыта прогоном И guid реестру известен (хотя бы в
///   одной версии): ответ содержателен;
/// * `None` — версия прогоном не покрыта ЛИБО guid реестру неизвестен вовсе. В обоих случаях
///   «группы нет» НЕ следует: вызывающий трактует `None` как «версионного ограничения нет».
pub fn root_group_available_in(guid: &str, target: FormatVersion) -> Option<bool> {
    if !covers_version(target) {
        return None;
    }
    let known = REGISTRY_KIND_GUIDS.iter().any(|&(_, g, _)| g == guid)
        || REGISTRY_UNNAMED_ROOT_GROUPS.iter().any(|&(_, g)| g == guid);
    if !known {
        return None;
    }
    let t = (target.major, target.minor);
    Some(
        REGISTRY_KIND_GUIDS.iter().any(|&(f, g, _)| f == t && g == guid)
            || REGISTRY_UNNAMED_ROOT_GROUPS.iter().any(|&(f, g)| f == t && g == guid),
    )
}

/// Class-guid'ы БЕЗЫМЯННЫХ (зарезервированных платформой, count-0) групп росписи корня в
/// версии `target` — в порядке данных (лексикографическом). Пусто, если версия не покрыта.
pub fn unnamed_root_groups(target: FormatVersion) -> Vec<&'static str> {
    let t = (target.major, target.minor);
    REGISTRY_UNNAMED_ROOT_GROUPS
        .iter()
        .filter(|&&(f, _)| f == t)
        .map(|&(_, g)| g)
        .collect()
}

/// Версии, в которых вид наблюдался (по возрастанию).
pub fn kind_formats(kind: &str) -> Vec<FormatVersion> {
    let mut v: Vec<FormatVersion> = REGISTRY_KINDS
        .iter()
        .filter(|&&(_, k)| k == kind)
        .map(|&((major, minor), _)| FormatVersion::new(major, minor))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Индекс `каноническое имя → строки реестра` (владелец/элемент отсеиваются потом).
///
/// Индексируем ОДНИМ уровнем — именем: составной ключ `(владелец, элемент, имя)` в
/// `HashMap` пришлось бы искать по `&('static str, …)`, то есть строить временный кортеж
/// со `'static`-строками, чего у вызывающего нет. Одноимённых строк в реестре десятки
/// (свойство × владелец × версия), поэтому досев по паре после поиска дешевле любой
/// перестройки ключей.
///
/// Строится один раз (реестр — статические данные): без индекса каждый вопрос об умолчании
/// сканировал бы весь массив свойств, а зовут его в том числе с пути записи.
fn property_index() -> &'static HashMap<&'static str, Vec<usize>> {
    static INDEX: OnceLock<HashMap<&'static str, Vec<usize>>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut map: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for (i, p) in REGISTRY_PROPERTIES.iter().enumerate() {
            map.entry(p.canonical).or_default().push(i);
        }
        map
    })
}

/// Индекс `владелец → элемент → версии, в которых элемент вообще наблюдался`.
///
/// Нужен витнессу отсутствия: «свойства нет в версии V» значит хоть что-то лишь тогда,
/// когда САМ элемент в версии V наблюдался (иначе этап прогона мог не загрузиться).
#[allow(clippy::type_complexity)]
fn element_index() -> &'static HashMap<&'static str, HashMap<&'static str, Vec<FormatVersion>>> {
    static INDEX: OnceLock<HashMap<&'static str, HashMap<&'static str, Vec<FormatVersion>>>> =
        OnceLock::new();
    INDEX.get_or_init(|| {
        let mut map: HashMap<&'static str, HashMap<&'static str, Vec<FormatVersion>>> =
            HashMap::new();
        for p in REGISTRY_PROPERTIES {
            let v = map
                .entry(p.owner)
                .or_default()
                .entry(p.element)
                .or_default();
            let fv = FormatVersion::new(p.format.0, p.format.1);
            if !v.contains(&fv) {
                v.push(fv);
            }
        }
        for by_element in map.values_mut() {
            for v in by_element.values_mut() {
                v.sort();
            }
        }
        map
    })
}

/// Строка реестра для свойства в КОНКРЕТНОЙ версии, если она там наблюдалась.
pub fn property(
    owner: &str,
    element: &str,
    canonical: &str,
    format: FormatVersion,
) -> Option<&'static RegistryProperty> {
    property_rows(owner, element, canonical).find(|p| p.format == (format.major, format.minor))
}

/// Версии, в которых свойство наблюдалось (по возрастанию).
pub fn property_formats(owner: &str, element: &str, canonical: &str) -> Vec<FormatVersion> {
    let mut v: Vec<FormatVersion> = property_rows(owner, element, canonical)
        .map(|p| FormatVersion::new(p.format.0, p.format.1))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Версии, в которых наблюдался САМ элемент-владелец (по возрастанию).
pub fn element_formats(owner: &str, element: &str) -> &'static [FormatVersion] {
    element_index()
        .get(owner)
        .and_then(|by_element| by_element.get(element))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Версии, в которых наблюдался литерал перечисления (по возрастанию).
pub fn literal_formats(enum_name: &str, literal: &str) -> Vec<FormatVersion> {
    let mut v: Vec<FormatVersion> = REGISTRY_LITERALS
        .iter()
        .filter(|&&(_, e, l)| e == enum_name && l == literal)
        .map(|&((major, minor), _, _)| FormatVersion::new(major, minor))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Строки реестра для свойства во всех версиях (досев по владельцу/элементу — см.
/// [`property_index`]).
fn property_rows<'a>(
    owner: &'a str,
    element: &'a str,
    canonical: &str,
) -> impl Iterator<Item = &'static RegistryProperty> + 'a {
    property_index()
        .get(canonical)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|&i| &REGISTRY_PROPERTIES[i])
        .filter(move |p| p.owner == owner && p.element == element)
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Анти-вакуумность: таблицы непусты и несут ось версий, ради которой реестр и заведён.
    #[test]
    fn registry_tables_are_populated() {
        assert!(versions().len() >= 3, "версий в реестре: {:?}", versions());
        assert!(REGISTRY_PROPERTIES.len() > 1000);
        assert!(covers_version(FormatVersion::new(2, 17)));
        // Версия, которой в прогоне не было: реестр честно не знает (а не «пусто»).
        assert!(!covers_version(FormatVersion::new(2, 18)));
    }

    /// Реестр знает СОСТАВ ВИДОВ версии — знание, которого у метамодели нет вовсе.
    /// Witnessed: `WebSocketClient` есть в росписи корня 2.20/2.21 и отсутствует в 2.17.
    #[test]
    fn kind_composition_is_a_function_of_version() {
        assert_eq!(
            kind_available_in("WebSocketClient", FormatVersion::new(2, 17)),
            Some(false)
        );
        assert_eq!(
            kind_available_in("WebSocketClient", FormatVersion::new(2, 20)),
            Some(true)
        );
        assert_eq!(
            kind_available_in("Catalog", FormatVersion::new(2, 17)),
            Some(true)
        );
        // Непокрытая версия — не «нет вида», а «ответа нет».
        assert_eq!(
            kind_available_in("Catalog", FormatVersion::new(2, 18)),
            None
        );
    }

    /// Умолчание достаётся ПО ВЕРСИИ, а не одно на все.
    #[test]
    fn property_default_is_looked_up_per_version() {
        let p = property(
            "Catalog",
            "Catalog",
            "hierarchical",
            FormatVersion::new(2, 17),
        )
        .expect("Catalog.hierarchical наблюдался в 2.17");
        assert_eq!(p.default, Some("false"));
        assert_eq!(p.designer, "Hierarchical");
        // Версия вне прогона — записи нет (а не чужое умолчание).
        assert!(property(
            "Catalog",
            "Catalog",
            "hierarchical",
            FormatVersion::new(2, 18)
        )
        .is_none());
    }

    /// Состав ГРУПП корня — тоже данные версии, и по guid'у тоже.
    /// Witnessed: `a7641777-…` (WebSocketClient) появляется в 2.20; безымянная
    /// зарезервированная группа `102f6202-…` — только в 2.21.
    #[test]
    fn root_group_composition_is_a_function_of_version() {
        const WSC: &str = "a7641777-7813-45c6-96ef-9d51587a6ac6";
        const RESERVED_221: &str = "102f6202-43fa-40b0-8898-acd3876daacb";
        const RESERVED_ALL: &str = "39bddf6a-0c3c-452b-921c-d99cfa1c2f1b";
        assert_eq!(root_group_available_in(WSC, FormatVersion::new(2, 17)), Some(false));
        assert_eq!(root_group_available_in(WSC, FormatVersion::new(2, 20)), Some(true));
        assert_eq!(
            root_group_available_in(RESERVED_221, FormatVersion::new(2, 20)),
            Some(false)
        );
        assert_eq!(
            root_group_available_in(RESERVED_221, FormatVersion::new(2, 21)),
            Some(true)
        );
        // Безымянная группа, которая есть во всех версиях, версионного ограничения не несёт.
        for v in [17, 20, 21] {
            assert_eq!(
                root_group_available_in(RESERVED_ALL, FormatVersion::new(2, v)),
                Some(true)
            );
        }
        // Неизвестный guid и непокрытая версия — «ответа нет», а не «группы нет».
        assert_eq!(root_group_available_in("00000000-0000-0000-0000-000000000000", FormatVersion::new(2, 21)), None);
        assert_eq!(root_group_available_in(WSC, FormatVersion::new(2, 18)), None);
        assert_eq!(
            unnamed_root_groups(FormatVersion::new(2, 21)),
            vec![RESERVED_221, RESERVED_ALL]
        );
        assert_eq!(
            unnamed_root_groups(FormatVersion::new(2, 20)),
            vec![RESERVED_ALL]
        );
    }

    /// `lineNumberLength` табличной части: наблюдался в 2.20/2.21 и НЕ наблюдался в 2.17,
    /// хотя сам элемент `TabularSection` в 2.17 наблюдался — это и есть витнесс отсутствия.
    #[test]
    fn presence_delta_is_observable() {
        let fmts = property_formats("Catalog", "TabularSection", "lineNumberLength");
        assert_eq!(
            fmts,
            vec![FormatVersion::new(2, 20), FormatVersion::new(2, 21)]
        );
        assert!(element_formats("Catalog", "TabularSection").contains(&FormatVersion::new(2, 17)));
    }
}
