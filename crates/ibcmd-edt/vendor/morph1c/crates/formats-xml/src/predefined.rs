//! Codec для EDT-only блока `<predefined>` корня `Catalog` (предопределённые данные).
//! Designer тело предопределённых НЕ несёт inline (живёт отдельным файлом, как формы) —
//! поэтому поле X-ИСКЛЮЧЕНО (`x_ignore` в спеке), но УЧАСТВУЕТ в R EDT (byte-exact).
//!
//! Структура (сверено по 13 SSL-объектам + ERP-перепись: 130 каталогов, 5445 узлов):
//! * `<predefined>` (без атрибутов) → сиблинги `<items id="uuid">`.
//! * Каждый item/content: `<name>` (required), `[<description>]` (opt), `<code>` (Value-
//!   xsi блок, required), `[<isFolder>bool]` (opt; witnessed только `true`, 85 узлов),
//!   `[<content id="uuid">…]` (РЕКУРСИВНО, opt — та же структура; глубина до 3).
//! * `<code>` — ДВЕ witnessed-формы, детерминированные `codeType` дескриптора:
//!   `xsi:type="core:StringValue"` (5156 узлов ERP; codeType=String) и
//!   `xsi:type="core:NumberValue"` (289 узлов ERP, 10 каталогов; codeType отсутствует =
//!   дефолт Number). NumberValue-`<value>` ВСЕГДА непустые цифры (289/289; пустым бывает
//!   только StringValue-`<value>` — 3292 узла).
//!
//! Канонический IR (round-trip-маркер; X не сравнивает): `PropertyValue::List` из узлов;
//! каждый узел — `List([id:Str, name:Str, has_desc:Bool, description:Str, code:Str,
//! has_folder:Bool, isFolder:Bool, children:List, code_is_number:Bool])`. Достаточно для
//! byte-exact восстановления (presence-флаги отличают отсутствие от пустого;
//! `code_is_number` различает Value-xsi формы кода). §1.0: иная структура → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

// IR node layout indices — pub: их РЕЭКСПОРТИРУЮТ `morph1c_pipeline::predefined_read`
// (Designer-сайдкар) и `formats_cf::predefined_body` (cf-тело) — единый канон узла.
/// `@id` узла.
pub const I_ID: usize = 0;
/// `<name>`.
pub const I_NAME: usize = 1;
/// presence-флаг `<description>`.
pub const I_HAS_DESC: usize = 2;
/// Текст `<description>` (`""` при отсутствии).
pub const I_DESC: usize = 3;
/// Текст `<value>` внутри `<code>` (код required; пустой бывает только у StringValue).
pub const I_CODE: usize = 4;
/// presence-флаг `<isFolder>`.
pub const I_HAS_FOLDER: usize = 5;
/// Значение `<isFolder>` (bool; witnessed только `true`).
pub const I_FOLDER: usize = 6;
/// Рекурсивные `<content>`-дети (List; пуст у элементов).
pub const I_CHILDREN: usize = 7;
/// `<code xsi:type>`-форма: `false` = `core:StringValue`, `true` = `core:NumberValue`
/// (ERP-witnessed; на Designer-стороне зеркалится `<Code xsi:type="xs:decimal">`).
pub const I_CODE_IS_NUMBER: usize = 8;
/// Арность узла.
pub const N: usize = 9;

const XSI_TYPE: &str = "xsi:type";
const STRING_VALUE: &str = "core:StringValue";
const NUMBER_VALUE: &str = "core:NumberValue";

// --- DECODE (EDT, from root) ------------------------------------------------

/// Декодировать `<predefined>` от КОРНЯ источника. Отсутствие → Absent (поле опционально).
pub fn decode_edt(root: &Element) -> Decoded {
    let host = match root.child("predefined") {
        Some(h) if h.prefix.is_empty() => h,
        Some(_) => return Decoded::Error("predefined: must be unprefixed".into()),
        None => return Decoded::Absent,
    };
    host.claim();
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "predefined: container must be attribute-less, no text (§1.0)".into(),
        );
    }
    let mut items = Vec::new();
    for child in &host.children {
        if child.local != "items" || !child.prefix.is_empty() {
            return Decoded::Error(format!(
                "predefined: expected <items>, got <{}>",
                child.local
            ));
        }
        match decode_node(child) {
            Ok(n) => items.push(n),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(PropertyValue::List(items))
}

fn decode_node(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    // @id (required on items/content).
    let id = el
        .attr("id")
        .ok_or_else(|| format!("<{}> predefined node missing @id", el.local))?;
    let id_val = id.value.clone();
    id.claimed.set(true);
    if el.attrs.iter().any(|a| !a.claimed.get()) {
        return Err(format!(
            "<{}> predefined node has unexpected extra attribute",
            el.local
        ));
    }

    let mut node: Vec<PropertyValue> = vec![
        PropertyValue::Str(id_val),
        PropertyValue::Str(String::new()), // name
        PropertyValue::Bool(false),        // has_desc
        PropertyValue::Str(String::new()), // description
        PropertyValue::Str(String::new()), // code (текст внутри <value>)
        PropertyValue::Bool(false),        // has_folder
        PropertyValue::Bool(false),        // isFolder
        PropertyValue::List(Vec::new()),   // children
        PropertyValue::Bool(false),        // code_is_number
    ];
    debug_assert_eq!(node.len(), N);

    let mut it = el.children.iter().peekable();
    // name (required, first).
    let name = it
        .next()
        .filter(|c| c.local == "name" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <name>", el.local))?;
    name.claim_with_text();
    node[I_NAME] = PropertyValue::Str(name.text.clone());
    // [description].
    if let Some(c) = it.peek() {
        if c.local == "description" && c.prefix.is_empty() {
            let d = it.next().unwrap();
            d.claim_with_text();
            node[I_HAS_DESC] = PropertyValue::Bool(true);
            node[I_DESC] = PropertyValue::Str(d.text.clone());
        }
    }
    // code (required): ВСЕГДА `<code xsi:type="core:StringValue|core:NumberValue">
    // <value>TEXT</value></code>` (даже пустой StringValue — <value></value>).
    let code = it
        .next()
        .filter(|c| c.local == "code" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <code>", el.local))?;
    let (code_text, code_is_number) = decode_code(code)?;
    node[I_CODE] = PropertyValue::Str(code_text);
    node[I_CODE_IS_NUMBER] = PropertyValue::Bool(code_is_number);
    // [isFolder].
    if let Some(c) = it.peek() {
        if c.local == "isFolder" && c.prefix.is_empty() {
            let f = it.next().unwrap();
            f.claim_with_text();
            node[I_HAS_FOLDER] = PropertyValue::Bool(true);
            node[I_FOLDER] = match f.text.as_str() {
                "true" => PropertyValue::Bool(true),
                "false" => PropertyValue::Bool(false),
                o => return Err(format!("<isFolder> bool literal expected, got {o:?}")),
            };
        }
    }
    // [content…] recursive.
    let mut children = Vec::new();
    for c in it {
        if c.local != "content" || !c.prefix.is_empty() {
            return Err(format!(
                "<{}> predefined node: unexpected <{}> (§1.0)",
                el.local, c.local
            ));
        }
        children.push(decode_node(c)?);
    }
    node[I_CHILDREN] = PropertyValue::List(children);
    Ok(PropertyValue::List(node))
}

/// Claim (для leftover) — то же, что decode.
pub fn claim_edt(root: &Element) {
    let _ = decode_edt(root);
}

// --- EMIT (EDT) --------------------------------------------------------------

/// Эмитировать `<predefined>` из IR. Bool(false) presence-маркер сюда не доходит
/// (отсутствие поля = Absent в IR → не эмитится re-sparsify). Здесь value — `List`.
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let items = as_list(value)?;
    let mut host = OutElement::branch("", "predefined");
    for item in items {
        host.push(emit_node("items", item)?);
    }
    Ok(vec![host])
}

fn emit_node(tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let node = match value {
        PropertyValue::List(l) if l.len() == N => l,
        _ => return Err(format!("predefined node must be List of {N}")),
    };
    let id = as_str(&node[I_ID])?;
    let mut el = OutElement::branch("", tag).attr("id", id.to_string());
    el.push(OutElement::leaf(
        "",
        "name",
        as_str(&node[I_NAME])?.to_string(),
    ));
    if as_bool(&node[I_HAS_DESC])? {
        el.push(OutElement::leaf(
            "",
            "description",
            as_str(&node[I_DESC])?.to_string(),
        ));
    }
    let code_text = as_str(&node[I_CODE])?;
    let code_is_number = as_bool(&node[I_CODE_IS_NUMBER])?;
    let mut code = OutElement::branch("", "code").attr(
        XSI_TYPE,
        if code_is_number { NUMBER_VALUE } else { STRING_VALUE },
    );
    code.push(OutElement::leaf("", "value", code_text.to_string()));
    el.push(code);
    if as_bool(&node[I_HAS_FOLDER])? {
        let b = as_bool(&node[I_FOLDER])?;
        el.push(OutElement::leaf(
            "",
            "isFolder",
            if b { "true" } else { "false" },
        ));
    }
    for child in as_list(&node[I_CHILDREN])? {
        el.push(emit_node("content", child)?);
    }
    Ok(el)
}

// --- helpers ----------------------------------------------------------------

/// Разобрать `<code xsi:type="core:StringValue|core:NumberValue"><value>TEXT</value></code>`
/// → `(текст, is_number)`. Блок ВСЕГДА несёт `<value>`-ребёнок (даже пустой
/// `<value></value>` — только у StringValue), в отличие от общего value_codec (который
/// пустую строку самозакрывает). NumberValue-текст — непустые цифры (witnessed 289/289 ERP;
/// пустой/нецифровой НЕ witnessed → §1.0-отказ, не догадка). Claim'ит host+value.
fn decode_code(code: &Element) -> Result<(String, bool), String> {
    let xsi = code
        .attr(XSI_TYPE)
        .ok_or("predefined <code> missing xsi:type")?;
    let is_number = match xsi.value.as_str() {
        STRING_VALUE => false,
        NUMBER_VALUE => true,
        other => {
            return Err(format!(
                "predefined <code> xsi:type must be {STRING_VALUE:?} or {NUMBER_VALUE:?}, \
                 got {other:?}"
            ))
        }
    };
    xsi.claimed.set(true);
    if code.attrs.iter().any(|a| !a.claimed.get()) {
        return Err("predefined <code> has unexpected extra attribute (§1.0)".into());
    }
    code.claim();
    if code.children.len() != 1 {
        return Err("predefined <code>: expected single <value> child".into());
    }
    let v = &code.children[0];
    if v.local != "value" || !v.prefix.is_empty() || !v.attrs.is_empty() || !v.children.is_empty() {
        return Err("predefined <code>: expected <value>text</value> leaf".into());
    }
    v.claim_with_text();
    if is_number && (v.text.is_empty() || !v.text.bytes().all(|b| b.is_ascii_digit())) {
        return Err(format!(
            "predefined <code> NumberValue must be non-empty digits (witnessed 289/289), \
             got {:?} (§1.0)",
            v.text
        ));
    }
    Ok((v.text.clone(), is_number))
}

fn as_list(v: &PropertyValue) -> Result<&Vec<PropertyValue>, String> {
    match v {
        PropertyValue::List(l) => Ok(l),
        other => Err(format!("predefined: expected List, got {:?}", other.kind())),
    }
}
fn as_str(v: &PropertyValue) -> Result<&str, String> {
    match v {
        PropertyValue::Str(s) => Ok(s),
        other => Err(format!("predefined: expected Str, got {:?}", other.kind())),
    }
}
fn as_bool(v: &PropertyValue) -> Result<bool, String> {
    match v {
        PropertyValue::Bool(b) => Ok(*b),
        other => Err(format!("predefined: expected Bool, got {:?}", other.kind())),
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    fn decode_first(xml: &str) -> Result<PropertyValue, String> {
        let doc = crate::parse(xml.as_bytes()).expect("xml");
        match decode_edt(&doc.root) {
            Decoded::Present(PropertyValue::List(items)) => Ok(items[0].clone()),
            Decoded::Error(e) => Err(e),
            _ => panic!("unexpected decode outcome (absent/non-list)"),
        }
    }

    /// Witnessed SSL-шейп: StringValue-код (byte-exact round-trip формы узла).
    #[test]
    fn string_code_item_roundtrips() {
        let xml = "<r><predefined><items id=\"35dccbf2-dcce-4160-91a0-9697dd21aa79\">\
                   <name>Телефон</name><code xsi:type=\"core:StringValue\">\
                   <value>000000001</value></code></items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node.len(), N);
        assert_eq!(node[I_CODE], PropertyValue::Str("000000001".into()));
        assert_eq!(node[I_CODE_IS_NUMBER], PropertyValue::Bool(false));
        let out = emit_node("items", &node_v).unwrap();
        let code = &out.children[1];
        assert_eq!(code.attrs[0].1, STRING_VALUE);
    }

    /// Witnessed ERP-шейп (`ВидыВложенийДляАктированияЕИС`, codeType=Number):
    /// `<code xsi:type="core:NumberValue"><value>1</value></code>` — round-trip формы.
    #[test]
    fn number_code_item_roundtrips() {
        let xml = "<r><predefined><items id=\"cf932053-44bc-4a97-b8c6-66f4cfe0ea88\">\
                   <name>ДокументОПриемке</name><description>Документ о приемке</description>\
                   <code xsi:type=\"core:NumberValue\"><value>1</value></code>\
                   </items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_CODE], PropertyValue::Str("1".into()));
        assert_eq!(node[I_CODE_IS_NUMBER], PropertyValue::Bool(true));
        let out = emit_node("items", &node_v).unwrap();
        let code = &out.children[2];
        assert_eq!(code.attrs[0].1, NUMBER_VALUE);
        assert_eq!(code.children[0].text.as_deref(), Some("1"));
    }

    /// §1.0: пустой/нецифровой NumberValue не witnessed (289/289 непустые цифры) — отказ.
    #[test]
    fn empty_or_nondigit_number_code_is_refused() {
        let xml = "<r><predefined><items id=\"a\"><name>Х</name>\
                   <code xsi:type=\"core:NumberValue\"><value></value></code>\
                   </items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("non-empty digits"), "{err}");
        let xml = "<r><predefined><items id=\"a\"><name>Х</name>\
                   <code xsi:type=\"core:NumberValue\"><value>1.5</value></code>\
                   </items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("non-empty digits"), "{err}");
    }

    /// §1.0: посторонний xsi:type кода — отказ.
    #[test]
    fn foreign_code_xsi_type_is_refused() {
        let xml = "<r><predefined><items id=\"a\"><name>Х</name>\
                   <code xsi:type=\"core:DateValue\"><value>x</value></code>\
                   </items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("xsi:type"), "{err}");
    }
}
