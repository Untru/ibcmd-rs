//! `core` (`morph1c-core`) — формат-нейтральный семантический IR, канонический
//! спек-фреймворк, spec-driven движок read/write, реестр версий и стабильная
//! сериализация IR. Это «внутренний формат» конвертера (ARCHITECTURE.md §1).
//!
//! # Архитектурный контракт, который реализует крейт
//! * [`ir`] — типы IR (`L1`/`L1f`, §1.2): один логический объект → ОДИН тип IR
//!   (§1.1), формат-нейтральный, с каноническими id и сохранённым порядком.
//! * [`spec`] — КАНОНИЧЕСКИЙ спек, модуль-на-сущность (§1.4, §5): канонический id,
//!   тип значения, нормализация, дефолт и порядок эмиссии заданы ОДИН раз (§1.6).
//! * [`engine`] — spec-driven движок (§1.4): ЕДИНСТВЕННЫЙ путь read/write; формат
//!   привносит лишь ПРОЕКЦИЮ. Структурно исключает дивергенцию форматов (§1.6).
//! * [`version`] — реестр платформа↔формат (§1.5; зеркало `docs/FORMATS.md`):
//!   `Since`-гейтинг и downgrade-guard.
//! * [`resolve`] — разрешение значения свойства из IR: явно заданное либо умолчание
//!   версии (умолчания РАЗРЕШИМЫ из IR, а не продублированы в нём).
//! * [`serialize`] — стабильная детерминированная сериализация IR (§1.5): golden,
//!   diff, обмен, человекочитаемый «декомпилят».
//!
//! # Бинарная приёмка (§1.0)
//! В IR НЕТ `Raw`. Всё структурное типизируется; опаковый платформенный бинарь —
//! только через [`ir::PropertyValue::Blob`]; неразобранное → типизированная ошибка
//! движка, НИКОГДА не silent-skip. Это касается всего крейта by construction.
//!
//! # Объём F2a
//! Здесь — ФРЕЙМВОРК + синтетическая валидация (см. `tests/`). Реальные коннекторы
//! EDT/Designer/`.cf` и реальные спеки сущностей — это F2b и далее; в `spec/*`
//! лежат лишь модуль-скелеты (§5), спекулятивных спеков нет.

pub mod engine;
pub mod ir;
pub mod resolve;
pub mod serialize;
pub mod spec;
pub mod version;

// Удобные ре-экспорты ключевых типов контракта.
pub use engine::{Decoded, EngineError, Projection};
pub use ir::{
    BlobRef, Configuration, ControlKind, DateFractions, FieldId, FormBody, FormControlKind,
    FormItem, GeneratedType, InternalInfo, Lang, MetadataObject, Module, ModuleBody, ObjectKind,
    PropertyValue,
    Template, Token, TypeQualifier, TypeRef, TypeSpec, Uuid, ValueKind, ValueScalarKind, ValueSpec,
};
pub use spec::common::{EntitySpec, FieldSpec, Normalize};
pub use version::{FormatVersion, PlatformVersion, Since};
