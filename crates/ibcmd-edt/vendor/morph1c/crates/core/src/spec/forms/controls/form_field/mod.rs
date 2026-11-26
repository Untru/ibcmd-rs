//! Канонический спек семейства контролов `FormField` (данные-привязанные поля формы) —
//! ARCHITECTURE.md §1.4/§1.6, под-IR L1f, срез LANE-F-2.
//!
//! EDT кодирует ЛЮБОЕ поле как `<items xsi:type="form:FormField">` с дискриминатором
//! `<type>InputField|CheckBoxField|LabelField|…</type>` и типо-специфичным
//! `<extInfo xsi:type="form:<Тип>ExtInfo">`; Designer — как элемент по имени типа
//! (`<InputField>`, `<CheckBoxField>`, `<LabelField>`). Канонический вид = EDT-дискриминатор.
//!
//! ОБЩИЕ свойства поля (тело) — ОДИН спек на всё семейство (549/569 полей корпуса SSL
//! CommonForms покрываются тремя самыми частыми типами: InputField 288, CheckBoxField 123,
//! LabelField 81; остальные типы — громкая §1.0-ошибка до следующего среза). Канонический
//! порядок полей = порядок EDT-тела (сверен топологически по всем 569 полям, конфликтов 0).
//!
//! # Ключевые корпусные факты (cross-omission witness, 569 полей)
//! * `dataPath` — DUAL-ENCODING: EDT `<dataPath xsi:type="form:DataPath"><segments>Путь`
//!   (ровно ОДИН `<segments>` во всех 569) ⟺ Designer `<DataPath>Путь</DataPath>` текстом.
//!   Канон — `Ref(Путь)`; оба формата дают одно значение (X by construction).
//! * `visible`/`showInHeader`/`showInFooter` — противоположные bool-дефолты (EDT эмитит
//!   true / Designer эмитит false), как у LabelDecoration.
//! * `enabled` — EDT эмитит ВСЕГДА (286 true + 2 false); Designer опускает true.
//! * `userVisible` — ТРИ-кодировка: EDT всегда несёт элемент (`<userVisible><common>true`
//!   = true; `<userVisible/>` = false); Designer опускает true / эмитит
//!   `<UserVisible><xr:Common>false</xr:Common></UserVisible>` (сверено 569+4).
//! * `editMode` — 4-литеральный канон {Auto, Enter, EnterOnInput, Directly}: EDT опускает
//!   `Directly`; Designer кодирует `Auto` ПАРОЙ `<EditMode>EnterOnInput</EditMode>` +
//!   `<AutoEditMode>true</AutoEditMode>`, опускает `Enter`, прочее — литералом
//!   (280 Auto + 6 Enter + 1 EnterOnInput + 1 Directly).
//! * `headerHorizontalAlign` — EDT эмитит ВСЕГДА (569/569); Designer опускает `Left`.
//! * `displayImportance` — EDT элемент ⟺ Designer АТРИБУТ `DisplayImportance` на элементе.
//!
//! extInfo InputField: `typeDomainEnabled`/`textSize` EDT эмитит всегда, Designer НЕ несёт
//! никогда (der-fill константы; дивергенция ловится X, не маскируется). Не смоделированы
//! (громкая §1.0-ошибка): `choiceList` (12), `choiceParameters` (1), `availableTypes` (1).


use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::spec::forms::controls::ControlSpec;

mod consts;
mod specs;

pub use consts::*;
pub use specs::*;
