//! Стабильная сериализация IR (ARCHITECTURE.md §1.5).
//!
//! Формат — **RON** (Rusty Object Notation) с фиксированным [`ron::ser::PrettyConfig`].
//! Выбор обоснован:
//! * RON кодирует Rust-енумы (варианты [`crate::ir::PropertyValue`]) НАТИВНО и
//!   round-trip'ит их точно — в отличие от JSON, где enum'ы пришлось бы тегировать;
//! * результат человекочитаем — это и golden-эталон, и diff, и «декомпилят» `.cf`
//!   без платформы (§4, §1.5);
//! * детерминизм — структурный: IR НЕ содержит `HashMap`/`HashSet` (везде
//!   упорядоченные `Vec`), поэтому порядок вывода фиксирован раскладкой типа, а не
//!   случайным порядком обхода. Двойная сериализация даёт идентичные байты
//!   (тест `determinism`).
//!
//! Контракт: `deserialize(serialize(x)) == x` для любого валидного IR
//! ([`crate::ir::Configuration`] и [`crate::ir::PropertyValue`]). Проверяется
//! property-тестом (§3.3).

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Ошибка (де)сериализации IR.
#[derive(Debug)]
pub enum SerError {
    /// Не удалось сериализовать в RON.
    Serialize(ron::Error),
    /// Не удалось разобрать RON обратно в IR.
    Deserialize(ron::error::SpannedError),
}

impl std::fmt::Display for SerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerError::Serialize(e) => write!(f, "IR serialize failed: {e}"),
            SerError::Deserialize(e) => write!(f, "IR deserialize failed: {e}"),
        }
    }
}

impl std::error::Error for SerError {}

/// Фиксированная конфигурация форматирования — гарант детерминизма и читаемости.
///
/// Все опции заданы явно, чтобы вывод не зависел от дефолтов версии `ron`.
fn pretty() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        .indentor("  ".to_string())
        .struct_names(true)
        .separate_tuple_members(false)
        .enumerate_arrays(false)
        .new_line("\n".to_string())
}

/// Сериализовать любой IR-тип в стабильный, детерминированный RON-текст.
pub fn to_string<T: Serialize>(value: &T) -> std::result::Result<String, SerError> {
    ron::ser::to_string_pretty(value, pretty()).map_err(SerError::Serialize)
}

/// Разобрать стабильный RON-текст обратно в IR-тип.
pub fn from_str<T: DeserializeOwned>(s: &str) -> std::result::Result<T, SerError> {
    ron::from_str(s).map_err(SerError::Deserialize)
}
