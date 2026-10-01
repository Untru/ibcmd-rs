//! Codec для `StyleValue` ([`PropertyValue::StyleValue`] / [`StyleValueSpec`]) —
//! двухуровневого xsi-типизированного значения шрифта/цвета вида `StyleItem`.
//!
//! Отличается от плоского [`crate::value_codec`] (6 скалярных xsi-видов): значение
//! стиля НЕ скаляр, а вложенная структура из ДВУХ xsi-типов. Оба формата кодируют одну
//! логическую структуру ([`StyleValueSpec`]), расходясь СИЛЬНО структурно:
//!
//! | вид | EDT | Designer |
//! |-----|-----|----------|
//! | FontRef  | `<value xsi:type="core:FontValue"><value xsi:type="core:FontRef"><font>Style.X</font>[<height>H.0</height>][<bold/>][<italic/>][<underline/>][<strikeout/>][<scale>S</scale>]</value></value>` | `<Value xsi:type="v8ui:Font" ref="style:X" [height="H"] [bold=…][italic=…][underline=…][strikeout=…] kind="StyleItem" [scale="S"]/>` |
//! | FontDef  | `…<value xsi:type="core:FontDef"><faceName>Arial</faceName><height>H.0</height>[<bold>true</bold>]…</value>…` | `<Value xsi:type="v8ui:Font" faceName="Arial" height="H" bold=… italic=… underline=… strikeout=… kind="Absolute" scale="100"/>` |
//! | ColorRef | `…<value xsi:type="core:ColorRef"><color>Palette.Blue</color></value>…` | `<Value xsi:type="v8ui:Color">pal:Blue</Value>` |
//! | ColorDef | `…<value xsi:type="core:ColorDef">[<red>R</red>][<green>G</green>][<blue>B</blue>]</value>…` | `<Value xsi:type="v8ui:Color">#RRGGBB</Value>` |
//!
//! READ обоих диалектов даёт ИДЕНТИЧНЫЙ [`StyleValueSpec`] (X by construction). WRITE
//! детерминированно восстанавливает байты конвенции формата (R byte-exact). Разрежённость
//! (EDT опускает нулевые RGB-компоненты; у `FontRef` КАЖДЫЙ флаг начертания — независимый
//! тристейт, оба диалекта несут ровно переопределённые; у `FontDef` EDT опускает
//! false-флаги, Designer эмитит все четыре денсово) канонизируется в IR — оба сходятся
//! к одному значению. Witnessed ERP: `ЖирныйПодчеркнутыйШрифт` (bold+underline без
//! italic/strikeout), `ЖирныйШрифтEDI` (FontDef bold=true).
//!
//! FontRef несёт ДВА семейства ссылки (различает канон-префикс + Designer-@kind):
//! `Style.Имя` ↔ `ref="style:Имя" kind="StyleItem"` (стиль-шрифт) и `System.Имя` ↔
//! `ref="sys:Имя" kind="WindowsFont"` (системный шрифт; witnessed ERP StyleItems 9/9) —
//! см. [`FONT_REF_FAMILIES`].
//!
//! §1.0-total: незнакомый xsi-тип / лишний sub-элемент / лишний атрибут / чужой ns →
//! типизированная ОШИБКА, никогда silent-drop/guess/Raw. Домен — ровно засвидетельствованный
//! корпусами SSL (97 StyleItem, оба диалекта) + ERP (512 StyleItem, оба диалекта).

use morph1c_core::ir::value::{
    BorderStyle, ColorStyle, FontFace, FontFlags, FontStyle, PropertyValue, StyleValueSpec,
};

use crate::descriptor::Element;
use crate::emit::OutElement;

mod decode_designer;
mod decode_edt;
mod encode_designer;
mod encode_edt;
mod nodes;
mod refs;
#[cfg(any())]
mod tests;

pub(crate) use decode_designer::*;
pub(crate) use decode_edt::*;
pub(crate) use encode_designer::*;
pub(crate) use encode_edt::*;
pub(crate) use nodes::*;
pub(crate) use refs::*;

/// Диалект проекции стиль-значения (EDT nested `<value>` ↔ Designer flat `<Value>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleValueDialect {
    /// EDT: `<value xsi:type="core:FontValue"><value xsi:type="core:…Ref|…Def">…`.
    Edt,
    /// Designer: `<Value xsi:type="v8ui:Font" …/>` / `<Value xsi:type="v8ui:Color">…</Value>`.
    Designer,
}

const XSI_TYPE: &str = "xsi:type";

// EDT xsi-типы (`core:`-prefixed) внешнего и внутреннего уровня.
const EDT_FONT_VALUE: &str = "core:FontValue";
const EDT_COLOR_VALUE: &str = "core:ColorValue";
const EDT_BORDER_VALUE: &str = "core:BorderValue";
const EDT_FONT_REF: &str = "core:FontRef";
const EDT_FONT_DEF: &str = "core:FontDef";
const EDT_COLOR_REF: &str = "core:ColorRef";
const EDT_COLOR_DEF: &str = "core:ColorDef";
const EDT_BORDER_DEF: &str = "core:BorderDef";

// Designer xsi-типы (плоский `<Value>`).
const DES_FONT: &str = "v8ui:Font";
const DES_COLOR: &str = "v8ui:Color";
const DES_BORDER: &str = "v8ui:Border";
// xsi-тип листа `<v8ui:style>` внутри Designer-рамки.
const DES_BORDER_STYLE_XSI: &str = "v8ui:ControlBorderType";
// Designer `kind`-атрибут различает Ref (`StyleItem` — стиль-ссылка / `WindowsFont` —
// системный шрифт) и Def (`Absolute`).
const DES_KIND_REF: &str = "StyleItem";
const DES_KIND_DEF: &str = "Absolute";
const DES_KIND_WINDOWS: &str = "WindowsFont";

// Дефолты BorderDef (EDT опускает их → пустой `<value xsi:type="core:BorderDef"/>`).
const BORDER_STYLE_DEFAULT: &str = "WithoutBorder";

// Соответствие префиксов ColorRef: EDT `Prefix.Имя` ↔ Designer `prefix:Имя`.
const COLOR_PREFIXES: &[(&str, &str)] = &[("Palette", "pal"), ("Web", "web"), ("Style", "style")];
// Семейства FontRef: `(edt-префикс, designer-префикс, designer-@kind)`. Канон IR — EDT-форма
// `Prefix.Имя` (различимость «системный шрифт vs Style-ссылка» несёт сам префикс канона):
// * `Style.Имя` ↔ `ref="style:Имя" kind="StyleItem"` — ссылка на стиль-шрифт (SSL 97/97);
// * `System.Имя` ↔ `ref="sys:Имя" kind="WindowsFont"` — СИСТЕМНЫЙ шрифт платформы.
//   Witnessed ERP StyleItems 9/9 попарно (единственное имя корпуса — `DefaultGUIFont`;
//   напр. ИнформационныйЦентрПолужирныйШрифт10: EDT `<font>System.DefaultGUIFont</font>` +
//   height/флаги ↔ Designer `ref="sys:DefaultGUIFont" height="10" bold="true" …
//   kind="WindowsFont"` — та же опциональная height/флаги/scale-структура, что у StyleItem;
//   в designer-ERP `sys:` встречается ТОЛЬКО при kind="WindowsFont" и наоборот).
//   Маппинг механический (имя не участвует) — как у COLOR_PREFIXES, имена не allow-list'ятся.
// cf-сторона: `System.*` кодируется variant-байтом 1 + системным font-id (witnessed erp.cf
// `ВыделенноеОформление` 722b663f-… — `System.DefaultGUIFont` → id 0, см. formats-cf
// `style_item.rs` SYSTEM_FONT_INDEX); имена вне witnessed-таблицы id — типизированный отказ.
const FONT_REF_FAMILIES: &[(&str, &str, &str)] = &[
    ("Style", "style", DES_KIND_REF),
    ("System", "sys", DES_KIND_WINDOWS),
];

// ============================================================================
// DECODE
// ============================================================================

/// Разобрать host-элемент (`<value>`/`<Value>`) в канонический [`StyleValueSpec`].
/// Хост уже claimed вызывающим; здесь клеймится xsi-атрибут + подструктура (§1.0 B1).
pub fn decode(dialect: StyleValueDialect, host: &Element) -> Result<PropertyValue, String> {
    let spec = match dialect {
        StyleValueDialect::Edt => decode_edt(host)?,
        StyleValueDialect::Designer => decode_designer(host)?,
    };
    Ok(PropertyValue::StyleValue(spec))
}

// ============================================================================
// CLAIM (согласован с decode, §1.0 B1)
// ============================================================================

/// Claim РОВНО те узлы, что читает [`decode`] (host уже claimed вызывающим).
pub fn claim(dialect: StyleValueDialect, host: &Element) {
    match dialect {
        StyleValueDialect::Edt => {
            // Внешний xsi + внутренний `<value>` + его xsi + все его дети (лист+текст).
            if let Some(a) = host.attr(XSI_TYPE) {
                a.claimed.set(true);
            }
            if let Some(inner) = host.children.iter().find(|c| c.local == "value") {
                inner.claim();
                if let Some(a) = inner.attr(XSI_TYPE) {
                    a.claimed.set(true);
                }
                for leaf in &inner.children {
                    leaf.claim_with_text();
                }
            }
        }
        StyleValueDialect::Designer => {
            // Все атрибуты (v8ui:Font — атрибуты; v8ui:Color — только xsi:type) + текст (Color).
            for a in &host.attrs {
                a.claimed.set(true);
            }
            host.claim_text();
            // v8ui:Border несёт лист `<v8ui:style>` (у Font/Color детей нет) — клеймим его
            // поддерево (элемент + xsi-атрибут + текст) РОВНО как читает decode_des_border.
            for child in &host.children {
                child.claim_subtree();
            }
        }
    }
}

// ============================================================================
// ENCODE
// ============================================================================

/// Восстановить host-элемент из [`StyleValueSpec`] (byte-exact). Имя/ns хоста — из локуса.
pub fn encode(
    dialect: StyleValueDialect,
    host_prefix: &str,
    host_local: &str,
    spec: &StyleValueSpec,
) -> Result<OutElement, String> {
    match dialect {
        StyleValueDialect::Edt => Ok(encode_edt(host_prefix, host_local, spec)),
        StyleValueDialect::Designer => Ok(encode_designer(host_prefix, host_local, spec)),
    }
}
