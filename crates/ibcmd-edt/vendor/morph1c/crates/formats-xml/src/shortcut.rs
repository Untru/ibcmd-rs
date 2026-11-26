//! Codec для datatype-поля `Shortcut`/`shortcut` (комбинация клавиш команды —
//! `BasicCommand.shortcut`, metamodel `value_kind=datatype`, type `Shortcut`,
//! guid `7f647b00-8821-4452-b29c-0e953d757fde`).
//!
//! Значение — строка-комбинация (`Ctrl+S`, `F3`, `Alt+2`, …). Оба XML-формата несут
//! её КАК ТЕКСТ листа, различаясь лишь именем тега (локус):
//! * EDT: `<shortcut>Ctrl+S</shortcut>`; пусто → тег ОТСУТСТВУЕТ (sparse-дефолт).
//! * Designer: `<Shortcut>Ctrl+S</Shortcut>`; пусто → `<Shortcut/>` (DENSE self-closing).
//!
//! Канонический IR — [`PropertyValue::Str`] (`""` = пусто, дефолт). Это ОТДЕЛЬНЫЙ
//! кодек (а не `PlainText`), т.к. `Shortcut` — самостоятельный datatype метамодели:
//! именованный value-kind даёт ему собственный дом (§1.6), и ЛЮБОЕ поле любого вида
//! может объявить `Codec::Shortcut` в своей проекции без пер-родительского хардкода
//! (data-driven — новый `.Command` получает его бесплатно). §1.0: не-строковый IR →
//! ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Декодировать лист-элемент `<shortcut>`/`<Shortcut>` (уже locate'нут). Текст листа —
/// комбинация клавиш; пустой/самозакрывающийся → `Str("")`.
pub fn decode(host: &Element) -> Decoded {
    host.claim_with_text();
    Decoded::Present(PropertyValue::Str(host.text.clone()))
}

/// Claim (host claimed выше) — то же, что decode.
pub fn claim(host: &Element) {
    host.claim_with_text();
}

/// Эмитировать `<tag>` (имя/ns — из локуса). Пустой → self-closing `<Tag/>` (Designer
/// DENSE-конвенция; EDT sparse дефолт до сюда не доходит — опущен по `is_default`).
pub fn encode(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    match value {
        PropertyValue::Str(s) => Ok(if s.is_empty() {
            OutElement::self_closing(ns, tag)
        } else {
            OutElement::leaf(ns, tag, s.clone())
        }),
        other => Err(format!("Shortcut expects Str, got {:?}", other.kind())),
    }
}
