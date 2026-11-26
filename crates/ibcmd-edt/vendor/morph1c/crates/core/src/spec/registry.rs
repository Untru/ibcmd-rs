//! Реестр канонических спеков ВИДОВ — модуль-на-вид, собран из файловой раскладки БЕЗ
//! правки центрального файла (ARCHITECTURE.md §5; ORCHESTRATION «два агента НЕ
//! редактируют одни файлы»).
//!
//! # Как собран (и почему так, а не inventory)
//! `build.rs` сканирует `src/spec/metadata/*.rs` и генерирует (в `OUT_DIR`) `pub mod`
//! для каждого файла + ссылаемый массив [`SPEC_CTORS`] их спек-конструкторов. Реестр
//! ([`all`]) строится из этого массива. Распределённая линкер-секция (inventory) была
//! отвергнута: в нашем графе `rlib`-ов модуль вида, на символы которого никто не
//! ссылается, ВЫРЕЗАЕТСЯ DCE вместе с саморегистрацией — вид молча исчезает (проверено).
//! Codegen-массив — обычный ссылаемый `const`: он ИМЕНУЕТ символ каждого вида, держит
//! модуль живым и НАДЁЖЕН by construction.
//!
//! # Что регистрирует вид
//! Канонический код вида берётся из самого спека ([`EntitySpec::entity`]) — отдельной
//! строки-ключа не нужно, поэтому источник истины один (§1.6). [`all`] перечисляет все
//! виды (детерминированно по коду), [`spec_for`]/[`registration_for`] ищут конкретный.
//!
//! # Контракт grind-агента
//! Файл `src/spec/metadata/<вид>.rs` определяет `pub fn <вид>() -> &'static EntitySpec`
//! (имя функции = stem файла). Добавление файла подхватывается пересборкой — без правки
//! `mod.rs`/`registry.rs` или любого общего match-арма.

use crate::spec::common::{EntitySpec, ProducedCategory};

// Сгенерированное build.rs: ссылаемый массив `SPEC_CTORS` конструкторов спеков всех
// видов (`crate::spec::metadata::<вид>::<вид>`). Сами `pub mod` — в `spec/metadata/mod.rs`.
include!(concat!(env!("OUT_DIR"), "/metadata_ctors.rs"));

/// Одна строка реестра: канонический код вида + конструктор его спека.
#[derive(Debug, Clone, Copy)]
pub struct SpecRegistration {
    /// Канонический код вида (`"CommonModule"`, …) — из [`EntitySpec::entity`].
    pub kind: &'static str,
    /// Конструктор канонического спека вида (кэш на процесс внутри модуля вида).
    pub spec: fn() -> &'static EntitySpec,
}

// NB: отдельного `register_spec!` нет — регистрация даётся codegen'ом build.rs, который
// ИМЕНУЕТ `<вид>::<вид>` (см. модульный док). Это и есть «нулевая правка общих файлов».

/// Конструктор регистрации из спек-конструктора (код вида = `EntitySpec::entity`).
fn registration(ctor: fn() -> &'static EntitySpec) -> SpecRegistration {
    SpecRegistration {
        kind: ctor().entity,
        spec: ctor,
    }
}

/// Все зарегистрированные виды, детерминированно по коду вида (стабильно для тестов).
///
/// Дубль кода вида среди файлов — паника (ошибка раскладки: два файла на один вид).
pub fn all() -> Vec<SpecRegistration> {
    let mut v: Vec<SpecRegistration> = SPEC_CTORS.iter().map(|&ctor| registration(ctor)).collect();
    v.sort_by_key(|r| r.kind);
    for w in v.windows(2) {
        assert_ne!(
            w[0].kind, w[1].kind,
            "duplicate kind {:?} registered by two files",
            w[0].kind
        );
    }
    v
}

/// Регистрация вида по каноническому коду, если зарегистрирован.
pub fn registration_for(kind: &str) -> Option<SpecRegistration> {
    SPEC_CTORS
        .iter()
        .map(|&ctor| registration(ctor))
        .find(|r| r.kind == kind)
}

/// Канонический спек вида по коду (через реестр), если зарегистрирован.
pub fn spec_for(kind: &str) -> Option<&'static EntitySpec> {
    registration_for(kind).map(|r| (r.spec)())
}

/// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`) вида
/// В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок), либо `None` если вид не объявил блок
/// (§1.6/§3.5). Источник — per-kind `pub const PRODUCED_CATEGORIES` в спек-файле вида,
/// собранный build.rs в [`PRODUCED_CTORS`] (ключ вида = `(ctor)().entity`).
///
/// Data-driven зеркало R1 std-attrs: обобщённый кодек `formats_xml::produced_types` зовёт
/// это вместо закрытого match-арма — вид с `producedTypes` объявляет свои категории В
/// СВОЁМ файле, добавление НЕ требует правки общих файлов (parallel-safe fan-out).
///
/// `None` (а не `Some(&[])`) для вида без блока — сохраняет ТОЧНУЮ прежнюю семантику
/// формата («блока нет» vs «блок есть»): kind, не попавший в `PRODUCED_CTORS`, либо с
/// пустым массивом, даёт `None` (== исторический `_ => None`).
pub fn produced_categories_for(kind: &str) -> Option<&'static [ProducedCategory]> {
    PRODUCED_CTORS
        .iter()
        .find(|(ctor, _)| ctor().entity == kind)
        .map(|(_, cats)| *cats)
        .filter(|cats| !cats.is_empty())
}

#[cfg(any())]
mod tests {
    use super::*;

    // Реестр НЕ пуст и содержит первый вид — анти-вакуумность механизма раскладки.
    #[test]
    fn registry_contains_common_module() {
        let kinds: Vec<&str> = all().iter().map(|r| r.kind).collect();
        assert!(
            kinds.contains(&"CommonModule"),
            "registry must contain CommonModule, got {kinds:?}"
        );
        assert_eq!(spec_for("CommonModule").unwrap().entity, "CommonModule");
        assert!(spec_for("NoSuchKind").is_none());
    }

    // `produced_categories_for` собран build.rs из per-kind `PRODUCED_CATEGORIES`: вид с
    // блоком (Catalog) даёт таблицу в EDT-порядке; вид без блока (CommonModule) и
    // незарегистрированный — `None` (== исторический `_ => None`).
    #[test]
    fn produced_categories_from_spec_registry() {
        let cats =
            produced_categories_for("Catalog").expect("Catalog declares PRODUCED_CATEGORIES");
        assert_eq!(
            cats.first().map(|c| c.category),
            Some("Object"),
            "EDT-порядок: Object первым"
        );
        assert_eq!(cats.len(), 5, "Catalog: 5 producedTypes-категорий");
        assert!(
            produced_categories_for("CommonModule").is_none(),
            "CommonModule без блока"
        );
        assert!(
            produced_categories_for("NoSuchKind").is_none(),
            "незарегистрированный → None"
        );
    }
}
