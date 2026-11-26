//! Канонический спек — модуль-на-сущность (ARCHITECTURE.md §1.4, §1.6, §5).
//!
//! Это ИСТОЧНИК инварианта §1.6: для каждого вида объекта / контрола / вспом.
//! части — ОДИН [`common::EntitySpec`], задающий канонический `id`, `value_kind`,
//! нормализацию, дефолт и порядок эмиссии КАЖДОГО свойства. Формат хранит лишь
//! ПРОЕКЦИЮ этого спека (имя тега / slot / диалект) в зеркальном модуле; движок
//! ([`crate::engine`]) выводит read/write обоих форматов из ОДНОГО спека —
//! рукописных пер-форматных ридеров (источник дивергенции) тут нет by construction.
//!
//! Раскладка одинакова здесь и в каждом формате (§5):
//! * [`metadata`] — спеки видов объектов (catalog, document, …);
//! * [`forms::controls`] — спеки контролов форм (input_field, table, …);
//! * [`auxiliary`] — спеки вспомогательных частей (rights, predefined, …);
//!   (каталог `auxiliary`, не `aux` — `aux` зарезервирован в Windows, см. модуль);
//! * [`common`] — общие примитивы свойств и сами типы `FieldSpec`/`EntitySpec`.
//!
//! F2a даёт ФРЕЙМВОРК и пустые модуль-скелеты; реальные спеки сущностей наполняются
//! в F2b/grind по инвентарю (§6). Спекулятивные спеки тут НЕ вводим (§5).

pub mod auxiliary;
pub mod catalog_child;
pub mod common;
pub mod forms;
pub mod ir_child;
pub mod metadata;
pub mod registry;

pub use common::{Align, EntitySpec, FieldSpec, Normalize, ProducedCategory, TriState};
pub use registry::{produced_categories_for, spec_for, SpecRegistration};
