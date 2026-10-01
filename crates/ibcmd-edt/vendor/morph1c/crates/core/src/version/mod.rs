//! Реестр платформа↔формат (ARCHITECTURE.md §1.5; источник правды — `docs/FORMATS.md`).
//!
//! Эти ДАННЫЕ держатся ЦЕНТРАЛЬНО здесь, а НЕ разбросаны по ридерам (§1.5: «реестр
//! в `crates/core/version` сверяется с FORMATS.md, не хардкодит»). Версия формата —
//! функция версии платформы, а НЕ свойство конфигурации (FORMATS.md §1): формат
//! входа детектится, формат выхода — параметр; downgrade — только с guard'ом.
//!
//! Детектированная версия ВХОДА при этом ЗАПОМИНАЕТСЯ В IR
//! ([`crate::ir::Configuration::source_version`]) — прежнее правило «версия в семантический
//! IR не течёт» отменено: без неё мешок свойств не отличает «не было в источнике» от
//! «есть, но равно умолчанию», и потребителям (downgrade-гейт, разрешение умолчаний)
//! приходилось получать версию мимо IR отдельным параметром.
//!
//! Таблица версий и namespace'ы зеркалят `docs/FORMATS.md`; при выходе новой
//! платформы правится FORMATS.md И эта таблица (тест сверки — TODO).

use serde::{Deserialize, Serialize};

pub mod availability;
mod guard;
pub mod layout;
pub mod registry;
mod since;
pub mod upgrade;
pub use availability::{
    literal_availability, property_availability, property_available_in, Availability, Provenance,
    PropertyScope, Region,
};
pub use guard::{
    check_downgrade, collect_config_downgrade_violations, collect_downgrade_violations,
    downgrade_result, retain_present_in_source, DowngradeError, Violation,
};
pub use layout::Layout;
pub use registry::{kind_available_in, root_group_available_in, unnamed_root_groups, RegistryProperty};
pub use upgrade::{
    apply_upgrade, upgrade_object, upgrade_result, MissingUpgradeValue, UpgradeError,
};
pub use since::{
    designer_field_available_in, form_field_available_in, form_materialized_on_upgrade,
    form_renamed_on_upgrade, literal_available_in, since_for_field, since_for_field_name,
    since_for_form_field, since_for_literal, FORM_MATERIALIZED_RAW, FORM_RENAMED_RAW,
    FORM_SINCE_RAW, SINCE_FIELDS_RAW, SINCE_LITERALS_RAW,
};

/// Разобрать версию ПЛАТФОРМЫ (`"8.3.27"`) и перевести в версию ФОРМАТА через
/// [`VERSION_TABLE`] (FORMATS.md §2). Это вход `--v8version`.
///
/// §1.0: неизвестная/битая версия — ГРОМКАЯ ошибка, НЕ дефолт. «Похожая» платформа не
/// подставляется: неизвестной версии соответствует неизвестный формат, а угаданный
/// ns-блок/набор свойств дал бы дамп, который платформа молча не поймёт.
pub fn parse_v8version(s: &str) -> Result<FormatVersion, String> {
    let parts: Vec<&str> = s.split('.').collect();
    let nums: Option<Vec<u16>> = parts.iter().map(|p| p.parse::<u16>().ok()).collect();
    let platform = match nums.as_deref() {
        Some([a, b, c]) => PlatformVersion::new(*a, *b, *c),
        _ => {
            return Err(format!(
                "malformed --v8version {s:?}: expected `major.minor.patch` (e.g. `8.3.27`)"
            ))
        }
    };
    format_for_platform(platform).ok_or_else(|| {
        let known: Vec<String> = VERSION_TABLE
            .iter()
            .map(|vp| vp.platform.to_string())
            .collect();
        format!(
            "unknown platform version {platform} — not in the FORMATS.md §2 registry \
             (known: {})",
            known.join(", ")
        )
    })
}

/// Версия формата выгрузки/компиляции (`2.20`, `2.21`, …) — то, чем штампуется
/// Designer-XML/EDT/`.cf` (FORMATS.md §1). Сравнима по порядку: новее = больше.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FormatVersion {
    /// Старший компонент (всегда `2` в известном диапазоне).
    pub major: u16,
    /// Младший компонент (`20`, `21`, …).
    pub minor: u16,
}

impl FormatVersion {
    /// Сконструировать версию формата.
    pub const fn new(major: u16, minor: u16) -> Self {
        FormatVersion { major, minor }
    }
}

impl std::fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Версия платформы 1С (`8.3.27`, `8.5.1`, …) — то, ЧЕМ выгрузили/скомпилировали.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlatformVersion {
    /// `8`
    pub major: u16,
    /// `3` / `5`
    pub minor: u16,
    /// `27` / `1`
    pub patch: u16,
}

impl PlatformVersion {
    /// Сконструировать версию платформы.
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        PlatformVersion {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for PlatformVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Одна строка таблицы FORMATS.md §2 «Платформа ↔ версия формата выгрузки».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPair {
    /// Версия платформы.
    pub platform: PlatformVersion,
    /// Соответствующая версия формата.
    pub format: FormatVersion,
}

const fn pair(pmaj: u16, pmin: u16, ppatch: u16, fmaj: u16, fmin: u16) -> VersionPair {
    VersionPair {
        platform: PlatformVersion::new(pmaj, pmin, ppatch),
        format: FormatVersion::new(fmaj, fmin),
    }
}

/// Полный официальный список «платформа → формат» (FORMATS.md §2, источник: доки
/// 1С, 2.17.2 «Версии формата выгрузки»). Зеркало таблицы — единственная копия в
/// коде. Отсортирован по возрастанию.
pub const VERSION_TABLE: &[VersionPair] = &[
    pair(8, 3, 17, 2, 10),
    pair(8, 3, 18, 2, 11),
    pair(8, 3, 19, 2, 12),
    pair(8, 3, 20, 2, 13),
    pair(8, 3, 21, 2, 14),
    pair(8, 3, 22, 2, 15),
    pair(8, 3, 23, 2, 16),
    pair(8, 3, 24, 2, 17),
    pair(8, 3, 25, 2, 18),
    pair(8, 3, 26, 2, 19),
    pair(8, 3, 27, 2, 20),
    pair(8, 5, 1, 2, 21),
    pair(8, 5, 4, 2, 22),
];

/// Формат корпуса ERP — выгружен 8.3.27 → формат 2.20 (FORMATS.md §1).
pub const ERP: FormatVersion = FormatVersion::new(2, 20);
/// Формат корпуса SSL — выгружен 8.5.1 → формат 2.21 (FORMATS.md §1).
pub const SSL: FormatVersion = FormatVersion::new(2, 21);

/// Версия формата для заданной версии платформы (если есть в реестре).
pub fn format_for_platform(p: PlatformVersion) -> Option<FormatVersion> {
    VERSION_TABLE
        .iter()
        .find(|vp| vp.platform == p)
        .map(|vp| vp.format)
}

/// Версия платформы для заданной версии формата (если есть в реестре).
pub fn platform_for_format(fv: FormatVersion) -> Option<PlatformVersion> {
    VERSION_TABLE
        .iter()
        .find(|vp| vp.format == fv)
        .map(|vp| vp.platform)
}

// ---------------------------------------------------------------------------
// Амбьентная ТАРГЕТ-версия записи (per-thread). Это версия ВЫХОДА — параметр конверсии, а
// не свойство читаемых данных (версию ВХОДА несёт IR: `Configuration::source_version`).
// Скоупом она передаётся потому, что исторические per-kind `write()`-обёртки version-блайнд
// (в сигнатуре версии нет). Живёт в core (кросс-форматная забота: designer-конверты, cf
// configuration-root); formats-designer делегирует сюда.
// ---------------------------------------------------------------------------

use std::cell::Cell;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static ROUNDTRIP_TARGET: Cell<Option<FormatVersion>> = const { Cell::new(None) };
}

/// RAII-guard амбьентного таргета (восстанавливает прежнее значение при `Drop`).
struct RoundtripTargetGuard(Option<FormatVersion>);

impl Drop for RoundtripTargetGuard {
    fn drop(&mut self) {
        ROUNDTRIP_TARGET.with(|c| c.set(self.0));
    }
}

/// Выполнить `f` с установленной амбьентной таргет-версией (сбрасывается по выходу).
pub fn with_roundtrip_target<T>(target: FormatVersion, f: impl FnOnce() -> T) -> T {
    let prev = ROUNDTRIP_TARGET.with(|c| c.replace(Some(target)));
    let _guard = RoundtripTargetGuard(prev);
    f()
}

/// Выполнить `f` с ЗАХВАЧЕННЫМ амбьентным таргетом (значение [`current_roundtrip_target`]
/// вызывающей нити, ВКЛЮЧАЯ `None`). Назначение — пронос амбьента через границу
/// rayon-воркера: thread_local НЕ наследуется пулом, и воркер без переустановки молча
/// пишет дефолтным (SSL) шейпом под чужим таргетом — класс «тихая порча». Каждый
/// fan-out (`par_iter`/`par_chunks`) на пути записи ОБЯЗАН захватить таргет до
/// распараллеливания и переустановить его этим скоупом в воркере.
pub fn with_captured_roundtrip_target<T>(
    target: Option<FormatVersion>,
    f: impl FnOnce() -> T,
) -> T {
    let prev = ROUNDTRIP_TARGET.with(|c| c.replace(target));
    let _guard = RoundtripTargetGuard(prev);
    f()
}

/// Текущая амбьентная таргет-версия (`None` вне [`with_roundtrip_target`]-scope).
pub fn current_roundtrip_target() -> Option<FormatVersion> {
    ROUNDTRIP_TARGET.with(|c| c.get())
}

// ---------------------------------------------------------------------------
// Амбьентная ВЕРСИЯ ИСТОЧНИКА на время ЧТЕНИЯ (per-thread).
//
// Зачем: ридер обязан отличать «тега нет, потому что свойство не задано» (омиссия — это
// ДАННЫЕ диалекта, и значение восстанавливается по его конвенции) от «тега нет, потому что
// В ЭТОЙ ВЕРСИИ ФОРМАТА свойства не существует» (омиссия не значит НИЧЕГО, и восстанавливать
// нечего — §1.0). Второе различимо только по версии источника.
//
// Файл-дескриптор Designer версию несёт сам (корневой `version=`), и там она берётся ИЗ
// ФАЙЛА; EDT-проект штампован версией ПЛАТФОРМЫ на уровне проекта (`DT-INF/PROJECT.PMF`), до
// отдельного сайдкара она не доходит — для него и нужен этот амбьент, который ставит
// whole-config ридер (`pipeline`). Вне скоупа — `None`: гейта нет, поведение прежнее.
// ---------------------------------------------------------------------------

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static SOURCE_VERSION: Cell<Option<FormatVersion>> = const { Cell::new(None) };
}

/// RAII-guard амбьентной версии источника (восстанавливает прежнее значение при `Drop`).
struct SourceVersionGuard(Option<FormatVersion>);

impl Drop for SourceVersionGuard {
    fn drop(&mut self) {
        SOURCE_VERSION.with(|c| c.set(self.0));
    }
}

/// Выполнить `f` с установленной амбьентной версией ИСТОЧНИКА (сбрасывается по выходу).
///
/// `None` означает «версия источника неизвестна» и снимает версионный гейт реконструкции —
/// ровно то же, что и отсутствие скоупа.
pub fn with_source_version<T>(source: Option<FormatVersion>, f: impl FnOnce() -> T) -> T {
    let prev = SOURCE_VERSION.with(|c| c.replace(source));
    let _guard = SourceVersionGuard(prev);
    f()
}

/// Текущая амбьентная версия источника (`None` вне [`with_source_version`]-scope).
///
/// Тот же готча, что у [`current_roundtrip_target`]: thread_local НЕ наследуется rayon-пулом,
/// поэтому каждый fan-out на пути ЧТЕНИЯ обязан захватить значение до распараллеливания и
/// переустановить его в воркере этим же скоупом.
pub fn current_source_version() -> Option<FormatVersion> {
    SOURCE_VERSION.with(|c| c.get())
}

/// Гейт `Since(feature)` (§1.5): свойство/фича, появившаяся в конкретном формате.
///
/// Свойство, введённое версией, при конвертации в более СТАРЫЙ таргет НЕ эмитится
/// (а [`downgrade_guard`] поднимает ошибку, если таргет старее входа). Список
/// введённых свойств — FORMATS.md §4 (наполняется при разборе разноверсионного
/// корпуса). Здесь — механизм гейтинга, данные приходят из спека (`FieldSpec.since`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Since(pub FormatVersion);

impl Since {
    /// Доступна ли фича в целевом формате `target` (target не старее введения).
    pub fn available_in(self, target: FormatVersion) -> bool {
        target >= self.0
    }
}

/// Ошибка несовместимости версий формата (§1.5 «downgrade-guard»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// Таргет старее входа — принудительный downgrade без явного разрешения.
    Downgrade {
        /// Формат входа.
        input: FormatVersion,
        /// Запрошенный (более старый) формат выхода.
        target: FormatVersion,
    },
}

impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionError::Downgrade { input, target } => write!(
                f,
                "refusing downgrade: input format {input} is newer than target {target} \
                 (force-downgrade not permitted without guard)"
            ),
        }
    }
}

impl std::error::Error for VersionError {}

/// Downgrade-guard hook (§1.5): отказывает в принудительном таргете старее входа.
///
/// Возвращает `Ok(())`, если `target >= input` (upgrade/равно — допустимо), иначе
/// типизированную [`VersionError::Downgrade`]. Это ЯВНАЯ ошибка, не тихая
/// деградация (§2.2). Реальная политика «allow-downgrade с потерей Since-полей»
/// подключается на этом hook'е выше по пайплайну.
pub fn downgrade_guard(input: FormatVersion, target: FormatVersion) -> Result<(), VersionError> {
    if target >= input {
        Ok(())
    } else {
        Err(VersionError::Downgrade { input, target })
    }
}

/// Designer-XML namespace'ы (FORMATS.md §3), `(префикс, URI)`. Зеркало таблицы.
/// Префикс по умолчанию представлен пустой строкой.
pub const DESIGNER_NAMESPACES: &[(&str, &str)] = &[
    ("", "http://v8.1c.ru/8.3/MDClasses"),
    ("v8", "http://v8.1c.ru/8.1/data/core"),
    ("v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xen", "http://v8.1c.ru/8.3/xcf/enums"),
    ("xpr", "http://v8.1c.ru/8.3/xcf/predef"),
    ("lf", "http://v8.1c.ru/8.2/managed-application/logform"),
    ("app", "http://v8.1c.ru/8.2/managed-application/core"),
    ("cmi", "http://v8.1c.ru/8.2/managed-application/cmi"),
    ("style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// EDT namespace'ы (FORMATS.md §3), `(префикс, URI)`.
pub const EDT_NAMESPACES: &[(&str, &str)] = &[
    ("mdclass", "http://g5.1c.ru/v8/dt/metadata/mdclass"),
    ("core", "http://g5.1c.ru/v8/dt/mcore"),
];

#[cfg(any())]
mod tests {
    use super::*;

    #[test]
    fn version_table_is_sorted_and_bijective_in_known_range() {
        // Зеркало FORMATS.md §2: монотонно по платформе И по формату, без дублей.
        for w in VERSION_TABLE.windows(2) {
            assert!(
                w[0].platform < w[1].platform,
                "platform not strictly increasing"
            );
            assert!(w[0].format < w[1].format, "format not strictly increasing");
        }
    }

    #[test]
    fn corpus_versions_match_formats_md() {
        assert_eq!(
            format_for_platform(PlatformVersion::new(8, 3, 27)),
            Some(ERP)
        );
        assert_eq!(
            format_for_platform(PlatformVersion::new(8, 5, 1)),
            Some(SSL)
        );
        assert_eq!(
            platform_for_format(SSL),
            Some(PlatformVersion::new(8, 5, 1))
        );
    }

    #[test]
    fn since_gate_and_downgrade_guard() {
        let s = Since(SSL); // фича 2.21
        assert!(s.available_in(SSL));
        assert!(!s.available_in(ERP)); // 2.20 старее → не эмитится
        assert!(downgrade_guard(ERP, SSL).is_ok()); // upgrade ok
        assert!(downgrade_guard(SSL, ERP).is_err()); // downgrade guarded
    }
}
