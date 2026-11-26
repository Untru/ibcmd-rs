//! Codec для `Type` (`ОписаниеТипов`) — контейнерный кодек с зависящей-от-диалекта
//! канонизацией id и плотностью квалификаторов.
//!
//! `Type` 1С — упорядоченный НАБОР компонент-типов; у каждой примитивной компоненты
//! опциональный квалификатор (string/number/date/binary). Оба формата кодируют ОДНУ
//! логическую структуру, расходясь в трёх осях, которые нейтрализует канонизация:
//! 1. спеллинг type-id (EDT bare ↔ Designer QName `xs:`/`v8:`/`cfg:`, + алиасы);
//! 2. плотность квалификаторов (EDT SPARSE: дефолт-листы опущены; Designer DENSE: все);
//! 3. имена тегов/ns квалификаторов (чистая проекция).
//!
//! READ обоих диалектов даёт ИДЕНТИЧНЫЙ канонический [`TypeSpec`] (X by construction).
//! WRITE детерминированно восстанавливает байты конвенции формата (R byte-exact).
//!
//! §1.0: незнакомый QName/алиас, значение вне домена, orphan/dup-квалификатор, лишний
//! дочерний элемент → типизированная ОШИБКА, никогда silent-drop/guess.

use morph1c_core::ir::value::{DateFractions, PropertyValue, TypeQualifier, TypeRef, TypeSpec};

use crate::descriptor::Element;
use crate::emit::OutElement;

/// Диалект Type-проекции: спеллинг id + плотность квалификаторов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeDialect {
    /// EDT: bare id; lower-case теги; SPARSE (дефолт-листы опущены, пустой
    /// `<dateQualifiers/>`); элементы без префикса.
    Edt,
    /// Designer: QName id (`xs:`/`v8:`/`cfg:`); UpperCamel теги в ns `v8`; DENSE
    /// (все поля квалификатора всегда); canon↔QName таблица + xsd/ValueList алиасы.
    Designer,
}

mod tables;
mod decode;
mod encode;

pub use decode::*;
pub use encode::*;
pub use tables::*;

#[cfg(any())]
mod tests;
#[cfg(any())]
mod alias_tests;
#[cfg(any())]
mod config_alias_tests;
