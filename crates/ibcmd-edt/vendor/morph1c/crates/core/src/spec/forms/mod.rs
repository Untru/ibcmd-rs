//! Канонические спеки форм (ARCHITECTURE.md §1.2 L1f, §5).
//!
//! Дерево формы — единое [`crate::ir::FormItem`] под обёрткой [`crate::ir::FormBody`];
//! спеки КОНТРОЛОВ живут в [`controls`] (модуль-на-контрол), форм-уровневые свойства — в
//! [`form_root`], команды формы — в [`command`]. Это foundation-слой L1f: каркас рекурсии
//! контрол-дерева + спек первого контрола; каталог контролов растёт срезами (как
//! metadata-виды).

pub mod command;
pub mod controls;
pub mod form_root;

pub use controls::{ControlSpec, FormControlRegistration};
