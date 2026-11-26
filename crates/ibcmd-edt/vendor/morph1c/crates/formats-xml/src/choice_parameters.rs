//! Codec для `choiceParameters`/`ChoiceParameters` реквизита (параметры выбора). Список
//! связок `(name, value)`, где value — скаляр (`Value`-xsi) ЛИБО фиксированный массив
//! скаляров. Структура расходится между форматами (сверено 6 непустых связок в 3 объектах):
//! * EDT: КАЖДАЯ связка — сиблинг `<choiceParameters><name>Path</name><value xsi:type=…>
//!   …</value></choiceParameters>`; массив — `xsi:type="core:FixedArrayValue"` с
//!   `<values xsi:type=…>`-элементами.
//! * Designer: контейнер `<ChoiceParameters>` с `<app:item name="Path"><app:value xsi:type=…>
//!   …</app:value></app:item>`; массив — `xsi:type="v8:FixedArray"` с `<v8:Value xsi:type=…>`.
//!
//! Канонический IR (X by construction): `List` связок; каждая — `List([name:Str, kind:Enum,
//! values:List])`, где `kind` = `"Scalar"` (1 элемент) или `"Array"` (N), `values` — список
//! `Value`. §1.0: иная структура/чужой ns/неизвестный xsi → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::value_codec::{self, ValueDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{PropertyValue, Token};

/// Диалект.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceParametersDialect {
    /// EDT: сиблинги `<choiceParameters>` от КОРНЯ.
    Edt,
    /// Designer: контейнер `<ChoiceParameters>` с `<app:item>`'ами.
    Designer,
}

const XSI_TYPE: &str = "xsi:type";
const EDT_FIXED_ARRAY: &str = "core:FixedArrayValue";
const DES_FIXED_ARRAY: &str = "v8:FixedArray";

fn pack(params: Vec<(String, bool, Vec<PropertyValue>)>) -> PropertyValue {
    PropertyValue::List(
        params
            .into_iter()
            .map(|(name, is_array, values)| {
                PropertyValue::List(vec![
                    PropertyValue::Str(name),
                    PropertyValue::Enum(Token::new(if is_array { "Array" } else { "Scalar" })),
                    PropertyValue::List(values),
                ])
            })
            .collect(),
    )
}

#[allow(clippy::type_complexity)]
fn unpack(v: &PropertyValue) -> Result<Vec<(&str, bool, &Vec<PropertyValue>)>, String> {
    let outer = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "choiceParameters must be List, got {:?}",
                other.kind()
            ))
        }
    };
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        let rec = match item {
            PropertyValue::List(r) if r.len() == 3 => r,
            _ => return Err("choiceParameters param must be List([name,kind,values])".into()),
        };
        let name = match &rec[0] {
            PropertyValue::Str(s) => s.as_str(),
            _ => return Err("choiceParameters name must be Str".into()),
        };
        let is_array = match &rec[1] {
            PropertyValue::Enum(t) if t.as_str() == "Array" => true,
            PropertyValue::Enum(t) if t.as_str() == "Scalar" => false,
            _ => return Err("choiceParameters kind must be Enum(Array|Scalar)".into()),
        };
        let values = match &rec[2] {
            PropertyValue::List(l) => l,
            _ => return Err("choiceParameters values must be List".into()),
        };
        out.push((name, is_array, values));
    }
    Ok(out)
}

// --- EDT --------------------------------------------------------------------

/// Декодировать EDT: все сиблинги `<choiceParameters>` под `root` (по порядку).
pub fn decode_edt(root: &Element) -> Decoded {
    let mut params = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == "choiceParameters" && c.prefix.is_empty())
    {
        // Пустой `<choiceParameters/>` НЕ встречается как сиблинг (пустой = дефолт-омиссия).
        match decode_edt_one(ch) {
            Ok(p) => params.push(p),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(params))
}

#[allow(clippy::type_complexity)]
fn decode_edt_one(el: &Element) -> Result<(String, bool, Vec<PropertyValue>), String> {
    if !el.attrs.is_empty() {
        return Err("<choiceParameters> must have no attributes".into());
    }
    el.claim();
    let mut it = el.children.iter();
    let name = it
        .next()
        .filter(|e| e.local == "name" && e.prefix.is_empty())
        .ok_or("choiceParameters: expected <name>")?;
    name.claim_with_text();
    let value = it
        .next()
        .filter(|e| e.local == "value" && e.prefix.is_empty())
        .ok_or("choiceParameters: expected <value>")?;
    if it.next().is_some() {
        return Err("choiceParameters: unexpected extra child after <name>/<value> (§1.0)".into());
    }
    let (is_array, values) = decode_edt_value(value)?;
    Ok((name.text.clone(), is_array, values))
}

fn decode_edt_value(value: &Element) -> Result<(bool, Vec<PropertyValue>), String> {
    let xsi = value
        .attr(XSI_TYPE)
        .ok_or("choiceParameters <value> missing xsi:type")?;
    if xsi.value == EDT_FIXED_ARRAY {
        xsi.claimed.set(true);
        value.claim();
        let mut values = Vec::new();
        for item in &value.children {
            if item.local != "values" || !item.prefix.is_empty() {
                return Err(format!(
                    "choiceParameters FixedArray: expected <values>, got <{}>",
                    item.local
                ));
            }
            item.claim();
            values.push(value_codec::decode(ValueDialect::Edt, item)?);
        }
        Ok((true, values))
    } else {
        // Скаляр: общий value_codec на самом host'е <value>.
        value.claim();
        Ok((false, vec![value_codec::decode(ValueDialect::Edt, value)?]))
    }
}

/// Claim EDT — то же, что decode.
pub fn claim_edt(root: &Element) {
    let _ = decode_edt(root);
}

/// Декодировать НАБОР уже собранных сиблингов `<choiceParameters>` (позиционный парсер
/// std-attrs собирает подряд идущие сиблинги сам) — тот же канонический IR, что
/// [`decode_edt`].
pub fn decode_edt_siblings(els: &[&Element]) -> Decoded {
    let mut params = Vec::with_capacity(els.len());
    for el in els {
        match decode_edt_one(el) {
            Ok(p) => params.push(p),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(params))
}

/// Эмитировать EDT: по `<choiceParameters>` на связку.
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let params = unpack(value)?;
    let mut out = Vec::with_capacity(params.len());
    for (name, is_array, values) in params {
        let mut el = OutElement::branch("", "choiceParameters");
        el.push(OutElement::leaf("", "name", name.to_string()));
        el.push(emit_edt_value(is_array, values)?);
        out.push(el);
    }
    Ok(out)
}

fn emit_edt_value(is_array: bool, values: &[PropertyValue]) -> Result<OutElement, String> {
    if is_array {
        let mut host = OutElement::branch("", "value").attr(XSI_TYPE, EDT_FIXED_ARRAY);
        for v in values {
            let spec = as_value(v)?;
            host.push(value_codec::encode(ValueDialect::Edt, "", "values", spec)?);
        }
        Ok(host)
    } else {
        let spec = as_value(&values[0])?;
        value_codec::encode(ValueDialect::Edt, "", "value", spec)
    }
}

// --- Designer ---------------------------------------------------------------

/// Декодировать Designer-контейнер `<ChoiceParameters>` (уже claimed `locate`'ом).
pub fn decode_designer(host: &Element) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error("ChoiceParameters container must be attribute-less, no text".into());
    }
    let mut params = Vec::new();
    for item in &host.children {
        if item.local != "item" || item.prefix != "app" {
            return Decoded::Error(format!(
                "ChoiceParameters: expected <app:item>, got <{}>",
                item.local
            ));
        }
        match decode_designer_one(item) {
            Ok(p) => params.push(p),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(params))
}

#[allow(clippy::type_complexity)]
fn decode_designer_one(item: &Element) -> Result<(String, bool, Vec<PropertyValue>), String> {
    let name = item
        .attr("name")
        .ok_or("ChoiceParameters <app:item> missing @name")?;
    let name_val = name.value.clone();
    name.claimed.set(true);
    if item.attrs.iter().any(|a| !a.claimed.get()) {
        return Err("ChoiceParameters <app:item> has unexpected extra attribute".into());
    }
    item.claim();
    if item.children.len() != 1 {
        return Err("ChoiceParameters <app:item>: expected single <app:value>".into());
    }
    let value = &item.children[0];
    if value.local != "value" || value.prefix != "app" {
        return Err(format!(
            "ChoiceParameters: expected <app:value>, got <{}>",
            value.local
        ));
    }
    let (is_array, values) = decode_designer_value(value)?;
    Ok((name_val, is_array, values))
}

fn decode_designer_value(value: &Element) -> Result<(bool, Vec<PropertyValue>), String> {
    // ERP несёт и `<app:value xsi:nil="true"/>` (Undefined) — скаляр без xsi:type; поэтому
    // FixedArray распознаём по НАЛИЧИЮ xsi:type, а скаляры целиком отдаём value_codec
    // (он умеет typed/nil и сам клеймит).
    let xsi = value.attr(XSI_TYPE);
    if let Some(xsi) = xsi.filter(|a| a.value == DES_FIXED_ARRAY) {
        xsi.claimed.set(true);
        value.claim();
        let mut values = Vec::new();
        for item in &value.children {
            if item.local != "Value" || item.prefix != "v8" {
                return Err(format!(
                    "ChoiceParameters FixedArray: expected <v8:Value>, got <{}>",
                    item.local
                ));
            }
            item.claim();
            values.push(value_codec::decode(ValueDialect::Designer, item)?);
        }
        Ok((true, values))
    } else {
        value.claim();
        Ok((
            false,
            vec![value_codec::decode(ValueDialect::Designer, value)?],
        ))
    }
}

/// Claim Designer (host claimed выше).
pub fn claim_designer(host: &Element) {
    let _ = decode_designer(host);
}

/// Эмитировать Designer-контейнер `<ChoiceParameters>` (имя/ns из локуса). Пустой → self-closing.
pub fn emit_designer(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let params = unpack(value)?;
    if params.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut container = OutElement::branch(ns, tag);
    for (name, is_array, values) in params {
        let mut item = OutElement::branch("app", "item").attr("name", name);
        item.push(emit_designer_value(is_array, values)?);
        container.push(item);
    }
    Ok(container)
}

fn emit_designer_value(is_array: bool, values: &[PropertyValue]) -> Result<OutElement, String> {
    if is_array {
        let mut host = OutElement::branch("app", "value").attr(XSI_TYPE, DES_FIXED_ARRAY);
        for v in values {
            let spec = as_value(v)?;
            host.push(value_codec::encode(
                ValueDialect::Designer,
                "v8",
                "Value",
                spec,
            )?);
        }
        Ok(host)
    } else {
        let spec = as_value(&values[0])?;
        value_codec::encode(ValueDialect::Designer, "app", "value", spec)
    }
}

fn as_value(v: &PropertyValue) -> Result<&morph1c_core::ir::value::ValueSpec, String> {
    match v {
        PropertyValue::Value(s) => Ok(s),
        other => Err(format!(
            "choiceParameters value must be Value, got {:?}",
            other.kind()
        )),
    }
}
