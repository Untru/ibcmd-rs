//! Codec для СПИСКОВ строковых ссылок-путей корня `Catalog` (child-objects substrate,
//! Catalog-срез): `inputByString`/`dataLockFields` (field-style) и `basedOn`/`owners`
//! (item-style). Аналог [`crate::choice_param_links`] по структуре «много сиблингов в
//! EDT ⇔ один контейнер в Designer», но нагрузка — плоский список путей (`Str`).
//!
//! Структура расходится между форматами:
//! * EDT: КАЖДЫЙ элемент списка — отдельный сиблинг `<inputByString>Path</inputByString>`
//!   под КОРНЕМ (как `choiceParameterLinks` навигирует от родителя). Пустой список → нет
//!   тегов вовсе (дефолт-омиссия).
//! * Designer: ОДИН контейнер `<InputByString>` с детьми; field-style — `<xr:Field>Path
//!   </xr:Field>`, item-style — `<xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>`.
//!   Пустой → самозакрывающийся `<InputByString/>`.
//!
//! Канонический IR (X by construction): `PropertyValue::List` из `Str`-путей (в порядке
//! authored). Пустой → `List([])` (дефолт). §1.0: иная структура/чужой ns/лишний атрибут
//! → ОШИБКА, никогда silent-drop.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект+стиль списка ссылок.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefListDialect {
    /// EDT: сиблинги `<tag>Path</tag>` под корнем (tag — из локуса). Навигирует от
    /// РОДИТЕЛЯ (несколько одноимённых сиблингов; single-element `locate` их не покрыл бы).
    Edt,
    /// Designer field-style: контейнер с `<xr:Field>Path</xr:Field>`.
    DesignerField,
    /// Designer item-style: контейнер с `<xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>`.
    DesignerItem,
    /// Designer object-style: контейнер с `<xr:Object>Path</xr:Object>` (БЕЗ xsi:type) —
    /// `FunctionalOption.Content` (состав функциональной опции). По структуре = field-style
    /// (плоский текстовый лист, без атрибутов), но тег ребёнка `xr:Object`. Пустой →
    /// самозакрывающийся `<Content/>`.
    DesignerObject,
}

const XSI_TYPE: &str = "xsi:type";
const MD_OBJECT_REF: &str = "xr:MDObjectRef";

fn pack(items: Vec<String>) -> PropertyValue {
    PropertyValue::List(items.into_iter().map(PropertyValue::Str).collect())
}

fn unpack(v: &PropertyValue) -> Result<Vec<&str>, String> {
    match v {
        PropertyValue::List(l) => l
            .iter()
            .map(|item| match item {
                PropertyValue::Str(s) => Ok(s.as_str()),
                other => Err(format!("ref-list item must be Str, got {:?}", other.kind())),
            })
            .collect(),
        other => Err(format!("ref-list must be List, got {:?}", other.kind())),
    }
}

// --- EDT (from-root, multi-sibling) -----------------------------------------

/// Декодировать EDT-список: все сиблинги `<tag>` под `root` (по порядку). Пустой →
/// `List([])`. Claim'ит каждый узел+текст. `tag` — local-name из локуса.
pub fn decode_edt(root: &Element, tag: &str) -> Decoded {
    let mut items = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        if !ch.attrs.is_empty() || !ch.children.is_empty() {
            return Decoded::Error(format!(
                "<{tag}> ref-list item must be a plain text leaf (§1.0)"
            ));
        }
        ch.claim_with_text();
        items.push(ch.text.clone());
    }
    Decoded::Present(pack(items))
}

/// Claim EDT-список (для leftover) — то же, что читает [`decode_edt`].
pub fn claim_edt(root: &Element, tag: &str) {
    let _ = decode_edt(root, tag);
}

/// Эмитировать EDT-список: один `<tag>Path</tag>` на элемент (порядок IR). Пустой → ничего.
pub fn emit_edt(tag: &str, value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    Ok(unpack(value)?
        .into_iter()
        .map(|p| OutElement::leaf("", tag, p.to_string()))
        .collect())
}

// --- Designer (single container) --------------------------------------------

/// Стиль элемента Designer-списка ссылок (local-name тега + наличие `xsi:type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignerStyle {
    /// `<xr:Field>Path</xr:Field>` (без xsi).
    Field,
    /// `<xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>`.
    Item,
    /// `<xr:Object>Path</xr:Object>` (без xsi).
    Object,
}

impl DesignerStyle {
    /// local-name тега ребёнка.
    fn local(self) -> &'static str {
        match self {
            DesignerStyle::Field => "Field",
            DesignerStyle::Item => "Item",
            DesignerStyle::Object => "Object",
        }
    }
    /// Несёт ли элемент `xsi:type="xr:MDObjectRef"`.
    fn with_xsi(self) -> bool {
        matches!(self, DesignerStyle::Item)
    }
}

/// Декодировать Designer-контейнер `<Tag>` (уже claimed `locate`'ом), стиль `item_style`
/// (`true`=item, `false`=field) — back-compat обёртка над [`decode_designer_styled`].
pub fn decode_designer(host: &Element, item_style: bool) -> Decoded {
    decode_designer_styled(
        host,
        if item_style {
            DesignerStyle::Item
        } else {
            DesignerStyle::Field
        },
    )
}

/// Декодировать Designer-контейнер `<Tag>` (уже claimed `locate`'ом) в заданном
/// [`DesignerStyle`]. Пустой (self-closing) → `List([])`.
pub fn decode_designer_styled(host: &Element, style: DesignerStyle) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "ref-list <…> container must be attribute-less, no text (§1.0)".into(),
        );
    }
    let want_local = style.local();
    let want_xsi = style.with_xsi();
    let mut items = Vec::new();
    for ch in &host.children {
        if ch.local != want_local || ch.prefix != "xr" {
            return Decoded::Error(format!(
                "ref-list: expected <xr:{want_local}>, got <{}>",
                qname(ch)
            ));
        }
        if !ch.children.is_empty() {
            return Decoded::Error(format!(
                "<xr:{want_local}> ref-list item must be a text leaf"
            ));
        }
        if want_xsi {
            // xsi:type="xr:MDObjectRef" — единственный допустимый атрибут.
            let t = match ch.attr(XSI_TYPE) {
                Some(a) => a,
                None => return Decoded::Error(format!("<xr:{want_local}> missing {XSI_TYPE}")),
            };
            if t.value != MD_OBJECT_REF {
                return Decoded::Error(format!(
                    "<xr:{want_local}> {XSI_TYPE} must be {MD_OBJECT_REF:?}, got {:?}",
                    t.value
                ));
            }
            t.claimed.set(true);
            if ch.attrs.iter().any(|a| !a.claimed.get()) {
                return Decoded::Error(format!(
                    "<xr:{want_local}> has unexpected extra attribute (§1.0)"
                ));
            }
        } else if !ch.attrs.is_empty() {
            return Decoded::Error(format!(
                "<xr:{want_local}> ref-list item must have no attributes"
            ));
        }
        ch.claim_with_text();
        items.push(ch.text.clone());
    }
    Decoded::Present(pack(items))
}

/// Claim Designer-контейнер (то же, что читает [`decode_designer`]). Host claimed выше.
pub fn claim_designer(host: &Element, item_style: bool) {
    let _ = decode_designer(host, item_style);
}

/// Claim Designer-контейнер в заданном стиле (то же, что читает [`decode_designer_styled`]).
pub fn claim_designer_styled(host: &Element, style: DesignerStyle) {
    let _ = decode_designer_styled(host, style);
}

/// Эмитировать Designer-контейнер `<Tag>` (имя/ns — из локуса), стиль `item_style`
/// (`true`=item, `false`=field) — back-compat обёртка над [`emit_designer_styled`].
pub fn emit_designer(
    ns: &str,
    tag: &str,
    item_style: bool,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    emit_designer_styled(
        ns,
        tag,
        if item_style {
            DesignerStyle::Item
        } else {
            DesignerStyle::Field
        },
        value,
    )
}

/// Эмитировать Designer-контейнер `<Tag>` (имя/ns — из локуса) в заданном [`DesignerStyle`].
/// Пустой → self-closing.
pub fn emit_designer_styled(
    ns: &str,
    tag: &str,
    style: DesignerStyle,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let items = unpack(value)?;
    if items.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut container = OutElement::branch(ns, tag);
    for p in items {
        let leaf = match style {
            DesignerStyle::Item => {
                OutElement::leaf("xr", "Item", p.to_string()).attr(XSI_TYPE, MD_OBJECT_REF)
            }
            DesignerStyle::Field => OutElement::leaf("xr", "Field", p.to_string()),
            DesignerStyle::Object => OutElement::leaf("xr", "Object", p.to_string()),
        };
        container.push(leaf);
    }
    Ok(container)
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}
