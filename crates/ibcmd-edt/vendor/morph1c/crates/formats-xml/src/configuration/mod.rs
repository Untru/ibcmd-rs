//! Кодеки корня `Configuration` (§1.0).
//! Корень — единственный объект-вид с раскладкой, расходящейся с обычными metadata-
//! видами: `containedObjects`-блок (платформенные пары GUID), список `ChildObjects`
//! (ИМЕНА всех объектов конфигурации, НЕ рекурсия), плюс несколько структурно-
//! расходящихся свойств (`usePurposes`, `usedMobileApplicationFunctionalities`,
//! inline-сущность `languages`).
//!
//! Все кодеки навигируют от КОРНЯ источника (EDT — от `<mdclass:Configuration>`;
//! Designer — от `<MetaDataObject>`), как `RefList(Edt)`/standardAttributes, потому что
//! несут НЕСКОЛЬКО узлов либо лежат под фиксированной обёрткой (`InternalInfo`/
//! `ChildObjects`). Тотальность (§1.0): каждый узел claim'ится РОВНО как читается;
//! неизвестный тег/атрибут → ОШИБКА (никогда silent-drop).

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::version::{literal_available_in, FormatVersion};

mod child_objects;
mod contained_objects;
mod extension;
mod languages;
mod mobile_functionalities;
mod use_purposes;

pub use child_objects::*;
pub use contained_objects::*;
pub use extension::*;
pub use languages::*;
pub use mobile_functionalities::*;
pub use use_purposes::*;

/// Диалект формата для Configuration-кодеков.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDialect {
    /// EDT (`.mdo`): плоские узлы под корнем, kind-префиксные имена.
    Edt,
    /// Designer (`.xml`): обёртки `<InternalInfo>`/`<ChildObjects>`, bare-имена.
    Designer,
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

fn str_list(items: Vec<Vec<String>>) -> PropertyValue {
    PropertyValue::List(
        items
            .into_iter()
            .map(|row| PropertyValue::List(row.into_iter().map(PropertyValue::Str).collect()))
            .collect(),
    )
}

/// Развернуть `List([List([Str…])])` в `Vec<Vec<&str>>` (для эмиссии).
fn unpack_rows(v: &PropertyValue) -> Result<Vec<Vec<&str>>, String> {
    let rows = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "configuration list must be List, got {:?}",
                other.kind()
            ))
        }
    };
    rows.iter()
        .map(|row| match row {
            PropertyValue::List(cells) => cells
                .iter()
                .map(|c| match c {
                    PropertyValue::Str(s) => Ok(s.as_str()),
                    other => Err(format!(
                        "configuration list cell must be Str, got {:?}",
                        other.kind()
                    )),
                })
                .collect::<Result<Vec<_>, _>>(),
            other => Err(format!(
                "configuration list row must be List, got {:?}",
                other.kind()
            )),
        })
        .collect()
}

/// Спуститься к обёртке `<Configuration>` под Designer-корнем `<MetaDataObject>`.
/// (Configuration-кодеки навигируют от `source.root` = `<MetaDataObject>`.)
fn designer_config_wrapper(root: &Element) -> Option<&Element> {
    root.child("Configuration")
}
