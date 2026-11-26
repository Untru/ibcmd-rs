//! Канонические спеки ВСПОМОГАТЕЛЬНЫХ частей (ARCHITECTURE.md §5).
//!
//! Модуль-на-часть: `rights.rs`, `predefined.rs`, `command_interface.rs`, … —
//! каждый отдаёт [`crate::spec::common::EntitySpec`]. Зеркалится в
//! `formats/*/src/auxiliary/<часть>.rs`.
//!
//! NB: каталог называется `auxiliary`, а НЕ `aux` (как в §5 черновике): `aux` —
//! зарезервированное DOS-имя устройства в Windows, и git (`core.protectNTFS`)
//! отказывается индексировать такие пути. Имя `auxiliary` — канон проекта на всех
//! платформах; инвентарный «aux» (`fields_aux.csv`, `--mode aux`) не затронут.
//!
//! ПУСТО в F2a намеренно: реальные aux-спеки — это F2b/grind по инвентарю
//! (`inventory/fields_aux.csv`, §6). Спекулятивных спеков тут НЕ вводим (§5).
