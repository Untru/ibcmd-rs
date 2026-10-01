//! Под-IR формы (`L1f`, ARCHITECTURE.md §1.2) — единое дерево [`FormItem`] плюс
//! форм-уровневая обёртка [`FormBody`].
//!
//! Узел = тип контрола + типизированный property-bag + типизированный `extInfo` + дети +
//! события. Дерево ЕДИНО для всех форматов (§1.6): прошлый проект имел три независимых
//! форм-кодека (~31k строк) — здесь форма читается/пишется тем же spec-driven движком,
//! что и метаданные, по спекам контролов (`spec/forms/controls`).
//!
//! # Что несёт под-IR (foundation L1f)
//! * [`FormBody`] — КОРЕНЬ под-IR формы: форм-уровневые свойства (`AutoTitle`/
//!   `CommandBarLocation`/…), служебные фиксированные дети (`autoCommandBar`,
//!   `commandInterface`) и КОРНЕВОЙ список контролов (`items`/`ChildItems`).
//! * [`FormItem`] — узел рекурсивного дерева контролов: вид ([`FormControlKind`]), имя,
//!   `id`, property-bag, типизированный `extInfo`-bag (тип-специфичные свойства контрола),
//!   события и рекурсивные дети.
//!
//! Дерево контролов РЕКУРСИВНО (контейнеры FormGroup/Pages/Page/Popup/Table несут детей);
//! рекурсия read/write живёт в `formats-xml` (как child-objects), а ДАННЫЕ (порядок полей,
//! дискриминатор, какие extInfo) — в спеках контролов (`spec/forms/controls`, §1.6).
//!
//! Идентичность контрола внутри формы — `(name, id)`: оба формата несут их (EDT — как
//! `<name>`/`<id>`-дети, Designer — как `name=`/`id=`-атрибуты), значение одинаково (X by
//! construction). Порядок детей и свойств ЗНАЧИМ (§3.2 «сравнение детей позиционное»;
//! §1.3 «сохранение исходного порядка»).

use serde::{Deserialize, Serialize};

use crate::ir::value::{Lang, PropertyValue};
use crate::ir::{FieldId, HelpPage, HelpResource};

mod body;
mod data_attributes;
mod dcs;
mod decorations;
mod event_order;
mod items;

pub use body::*;
pub use data_attributes::*;
pub use dcs::*;
pub use decorations::*;
pub use event_order::*;
pub use items::*;
