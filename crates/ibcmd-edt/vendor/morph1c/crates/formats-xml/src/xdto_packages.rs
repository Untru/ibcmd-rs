//! Codec для `xdtoPackages`/`XDTOPackages` вида `WebService` — СПИСОК ссылок на
//! XDTO-пакеты. Каждый элемент — типизированное (`String`|`Reference`) значение:
//! * `String` — URI пространства имён XDTO (напр. `http://v8.1c.ru/8.1/data/core`);
//! * `Reference` — ссылка на объект `XDTOPackage.<Имя>` (пакет конфигурации).
//!
//! Структура расходится между форматами (как [`crate::ref_list`], но нагрузка —
//! типизированная пара `(variant, value)`, а не плоская строка):
//! * **EDT**: КАЖДЫЙ элемент — отдельный сиблинг под КОРНЕМ:
//!   `<xdtoPackages xsi:type="core:StringValue"><value>URI</value></xdtoPackages>` или
//!   `<xdtoPackages xsi:type="core:ReferenceValue"><value>XDTOPackage.X</value></xdtoPackages>`.
//!   Пустой список → нет тегов вовсе (дефолт-омиссия).
//! * **Designer**: ОДИН контейнер `<XDTOPackages>` с `<xr:Item>`'ами; каждый item —
//!   `<xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type="xs:string|xr:MDObjectRef">
//!   value</xr:Value>`. Пустой → самозакрывающийся `<XDTOPackages/>`.
//!
//! Канонический IR (X by construction): `List` из `List([Str(variant), Str(value)])`,
//! `variant ∈ {"String","Reference"}`, в порядке authored. Пустой → `List([])` (дефолт).
//! §1.0: незнакомый xsi-тип / лишний атрибут / чужой ns / иная структура → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект `xdtoPackages`-проекции: EDT (multi-sibling) / Designer (single container).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdtoPackagesDialect {
    /// EDT: сиблинги `<xdtoPackages xsi:type>` под корнем.
    Edt,
    /// Designer: контейнер `<XDTOPackages>` с `<xr:Item>`'ами.
    Designer,
}

const XSI_TYPE: &str = "xsi:type";

// Варианты значения (канонический маркер в IR).
const VAR_STRING: &str = "String";
const VAR_REFERENCE: &str = "Reference";

// EDT xsi-типы.
const EDT_STRING: &str = "core:StringValue";
const EDT_REFERENCE: &str = "core:ReferenceValue";
// Designer xsi-типы.
const DES_STRING: &str = "xs:string";
const DES_REFERENCE: &str = "xr:MDObjectRef";

/// Упаковать список пар `(variant, value)` в канонический IR.
fn pack(items: Vec<(&'static str, String)>) -> PropertyValue {
    PropertyValue::List(
        items
            .into_iter()
            .map(|(variant, value)| {
                PropertyValue::List(vec![
                    PropertyValue::Str(variant.to_string()),
                    PropertyValue::Str(value),
                ])
            })
            .collect(),
    )
}

/// Распаковать канонический IR обратно в список `(variant, value)`.
fn unpack(value: &PropertyValue) -> Result<Vec<(&str, &str)>, String> {
    let items = match value {
        PropertyValue::List(l) => l,
        other => return Err(format!("xdtoPackages must be List, got {:?}", other.kind())),
    };
    items
        .iter()
        .map(|item| {
            let pair = match item {
                PropertyValue::List(p) => p,
                other => {
                    return Err(format!(
                        "xdtoPackages item must be List, got {:?}",
                        other.kind()
                    ))
                }
            };
            if pair.len() != 2 {
                return Err(format!(
                    "xdtoPackages item must be List of 2, got {}",
                    pair.len()
                ));
            }
            let variant = match &pair[0] {
                PropertyValue::Str(s) => s.as_str(),
                other => {
                    return Err(format!(
                        "xdtoPackages variant must be Str, got {:?}",
                        other.kind()
                    ))
                }
            };
            let value = match &pair[1] {
                PropertyValue::Str(s) => s.as_str(),
                other => {
                    return Err(format!(
                        "xdtoPackages value must be Str, got {:?}",
                        other.kind()
                    ))
                }
            };
            if variant != VAR_STRING && variant != VAR_REFERENCE {
                return Err(format!(
                    "xdtoPackages variant must be String/Reference, got {variant:?}"
                ));
            }
            Ok((variant, value))
        })
        .collect()
}

// --- EDT (from-root, multi-sibling) -----------------------------------------

/// Декодировать EDT-список: все сиблинги `<xdtoPackages>` под `root` (в порядке).
/// Каждый — `xsi:type="core:StringValue|core:ReferenceValue"` c `<value>`-ребёнком.
pub fn decode_edt(root: &Element, tag: &str) -> Decoded {
    let mut items = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        match decode_edt_item(ch) {
            Ok(pair) => items.push(pair),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(items))
}

fn decode_edt_item(el: &Element) -> Result<(&'static str, String), String> {
    let xsi = el
        .attr(XSI_TYPE)
        .ok_or_else(|| format!("xdtoPackages(edt) <{}> missing {XSI_TYPE} (§1.0)", el.local))?;
    xsi.claimed.set(true);
    if let Some(extra) = el.attrs.iter().find(|a| !a.claimed.get()) {
        return Err(format!(
            "xdtoPackages(edt) <{}> unexpected attribute {:?} (§1.0)",
            el.local, extra.name
        ));
    }
    let variant = match xsi.value.as_str() {
        EDT_STRING => VAR_STRING,
        EDT_REFERENCE => VAR_REFERENCE,
        other => {
            return Err(format!(
                "xdtoPackages(edt) unknown {XSI_TYPE} {other:?} (witnessed: core:StringValue/\
                 core:ReferenceValue — §1.0)"
            ))
        }
    };
    // Ровно один `<value>text</value>` текстовый лист.
    let mut it = el.children.iter();
    let value_el = it
        .next()
        .filter(|e| e.local == "value" && e.prefix.is_empty())
        .ok_or_else(|| "xdtoPackages(edt) expects <value> child (§1.0)".to_string())?;
    if let Some(extra) = it.next() {
        return Err(format!(
            "xdtoPackages(edt) unexpected extra child <{}> (§1.0)",
            qname(extra)
        ));
    }
    if !value_el.attrs.is_empty() || !value_el.children.is_empty() {
        return Err("xdtoPackages(edt) <value> must be a plain text leaf (§1.0)".into());
    }
    el.claim();
    value_el.claim_with_text();
    Ok((variant, value_el.text.clone()))
}

/// Claim EDT-список (то же, что читает [`decode_edt`]).
pub fn claim_edt(root: &Element, tag: &str) {
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
    {
        let _ = decode_edt_item(ch);
    }
}

/// Эмитировать EDT-список: один `<tag xsi:type><value>v</value></tag>` на элемент.
pub fn emit_edt(tag: &str, value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    unpack(value)?
        .into_iter()
        .map(|(variant, v)| {
            let xsi = if variant == VAR_STRING {
                EDT_STRING
            } else {
                EDT_REFERENCE
            };
            let mut host = OutElement::branch("", tag).attr(XSI_TYPE, xsi);
            host.push(OutElement::leaf("", "value", v.to_string()));
            Ok(host)
        })
        .collect()
}

// --- Designer (single container) --------------------------------------------

/// Декодировать Designer-контейнер `<XDTOPackages>` (уже claimed `locate`'ом). Пустой
/// (self-closing) → `List([])`.
pub fn decode_designer(host: &Element) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "xdtoPackages(designer) container must be attribute-less, no text (§1.0)".into(),
        );
    }
    let mut items = Vec::new();
    for item in &host.children {
        if item.local != "Item" || item.prefix != "xr" {
            return Decoded::Error(format!(
                "xdtoPackages(designer): expected <xr:Item>, got <{}>",
                qname(item)
            ));
        }
        match decode_designer_item(item) {
            Ok(pair) => items.push(pair),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(items))
}

fn decode_designer_item(item: &Element) -> Result<(&'static str, String), String> {
    if !item.attrs.is_empty() || !item.text.is_empty() {
        return Err(
            "xdtoPackages(designer) <xr:Item> must be attribute-less, no text (§1.0)".into(),
        );
    }
    // Три листа фиксированного порядка: Presentation, CheckState, Value.
    let mut it = item.children.iter();
    let pres = it
        .next()
        .filter(|e| e.local == "Presentation" && e.prefix == "xr")
        .ok_or_else(|| {
            "xdtoPackages(designer) expects <xr:Presentation> first (§1.0)".to_string()
        })?;
    if !pres.attrs.is_empty() || !pres.children.is_empty() || !pres.text.is_empty() {
        return Err("xdtoPackages(designer) <xr:Presentation> must be empty (§1.0)".into());
    }
    let check = it
        .next()
        .filter(|e| e.local == "CheckState" && e.prefix == "xr")
        .ok_or_else(|| "xdtoPackages(designer) expects <xr:CheckState> (§1.0)".to_string())?;
    if !check.attrs.is_empty() || !check.children.is_empty() {
        return Err("xdtoPackages(designer) <xr:CheckState> must be a text leaf (§1.0)".into());
    }
    if check.text != "0" {
        return Err(format!(
            "xdtoPackages(designer) <xr:CheckState> must be \"0\", got {:?} (§1.0)",
            check.text
        ));
    }
    let value_el = it
        .next()
        .filter(|e| e.local == "Value" && e.prefix == "xr")
        .ok_or_else(|| "xdtoPackages(designer) expects <xr:Value> (§1.0)".to_string())?;
    if let Some(extra) = it.next() {
        return Err(format!(
            "xdtoPackages(designer) unexpected extra child <{}> (§1.0)",
            qname(extra)
        ));
    }
    if !value_el.children.is_empty() {
        return Err("xdtoPackages(designer) <xr:Value> must be a text leaf (§1.0)".into());
    }
    let xsi = value_el
        .attr(XSI_TYPE)
        .ok_or_else(|| format!("xdtoPackages(designer) <xr:Value> missing {XSI_TYPE} (§1.0)"))?;
    xsi.claimed.set(true);
    if let Some(extra) = value_el.attrs.iter().find(|a| !a.claimed.get()) {
        return Err(format!(
            "xdtoPackages(designer) <xr:Value> unexpected attribute {:?} (§1.0)",
            extra.name
        ));
    }
    let variant = match xsi.value.as_str() {
        DES_STRING => VAR_STRING,
        DES_REFERENCE => VAR_REFERENCE,
        other => {
            return Err(format!(
                "xdtoPackages(designer) unknown {XSI_TYPE} {other:?} (witnessed: xs:string/\
                 xr:MDObjectRef — §1.0)"
            ))
        }
    };
    pres.claim();
    check.claim_with_text();
    value_el.claim_with_text();
    Ok((variant, value_el.text.clone()))
}

/// Claim Designer-контейнер (host уже claimed `locate`'ом).
pub fn claim_designer(host: &Element) {
    for item in &host.children {
        if item.local == "Item" && item.prefix == "xr" {
            item.claim();
            let _ = decode_designer_item(item);
        }
    }
}

/// Эмитировать Designer-контейнер `<XDTOPackages>`. Пустой → self-closing.
pub fn emit_designer(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let items = unpack(value)?;
    if items.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut container = OutElement::branch(ns, tag);
    for (variant, v) in items {
        let xsi = if variant == VAR_STRING {
            DES_STRING
        } else {
            DES_REFERENCE
        };
        let mut item = OutElement::branch("xr", "Item");
        item.push(OutElement::self_closing("xr", "Presentation"));
        item.push(OutElement::leaf("xr", "CheckState", "0"));
        item.push(OutElement::leaf("xr", "Value", v.to_string()).attr(XSI_TYPE, xsi));
        container.push(item);
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

#[cfg(any())]
mod tests {
    use super::*;
    use crate::read::parse;

    /// `parse(xml).root` — это ВЕРХНИЙ элемент (напр. `<r>`), а не документ-обёртка.
    fn root_of(xml: &str) -> Element {
        parse(xml.as_bytes()).expect("parse").root
    }
    /// Первый ребёнок верхнего элемента (напр. `<XDTOPackages>` из `<r><XDTOPackages/></r>`).
    fn host_of(xml: &str) -> Element {
        root_of(xml).children.into_iter().next().expect("host")
    }
    fn as_list(d: Decoded) -> PropertyValue {
        match d {
            Decoded::Present(v) => v,
            Decoded::Error(e) => panic!("not present: error {e}"),
            Decoded::Absent => panic!("not present: absent"),
        }
    }

    #[test]
    fn x_equal_string_variant() {
        let e = as_list(decode_edt(
            &root_of(
                "<r><xdtoPackages xsi:type=\"core:StringValue\"><value>http://v8.1c.ru/8.1/data/core</value></xdtoPackages></r>",
            ),
            "xdtoPackages",
        ));
        let d = as_list(decode_designer(&host_of(
            "<r><XDTOPackages><xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"xs:string\">http://v8.1c.ru/8.1/data/core</xr:Value></xr:Item></XDTOPackages></r>",
        )));
        assert_eq!(e, d);
    }

    #[test]
    fn x_equal_reference_variant() {
        let e = as_list(decode_edt(
            &root_of(
                "<r><xdtoPackages xsi:type=\"core:ReferenceValue\"><value>XDTOPackage.X</value></xdtoPackages></r>",
            ),
            "xdtoPackages",
        ));
        let d = as_list(decode_designer(&host_of(
            "<r><XDTOPackages><xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"xr:MDObjectRef\">XDTOPackage.X</xr:Value></xr:Item></XDTOPackages></r>",
        )));
        assert_eq!(e, d);
    }

    #[test]
    fn empty_lists_equal() {
        let e = as_list(decode_edt(&root_of("<r></r>"), "xdtoPackages"));
        let d = as_list(decode_designer(&host_of("<r><XDTOPackages/></r>")));
        assert_eq!(e, d);
        assert_eq!(e, PropertyValue::List(vec![]));
    }

    #[test]
    fn edt_round_trip_variant() {
        let v = as_list(decode_edt(
            &root_of(
                "<r><xdtoPackages xsi:type=\"core:StringValue\"><value>u</value></xdtoPackages></r>",
            ),
            "xdtoPackages",
        ));
        let out = emit_edt("xdtoPackages", &v).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].attrs[0],
            (XSI_TYPE.to_string(), EDT_STRING.to_string())
        );
    }

    #[test]
    fn unknown_xsi_errors() {
        let err = decode_edt(
            &root_of(
                "<r><xdtoPackages xsi:type=\"core:FooValue\"><value>u</value></xdtoPackages></r>",
            ),
            "xdtoPackages",
        );
        assert!(matches!(err, Decoded::Error(e) if e.contains("unknown")));
    }
}
