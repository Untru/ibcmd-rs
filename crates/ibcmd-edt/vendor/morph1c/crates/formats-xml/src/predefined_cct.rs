//! Codec для EDT-only блока `<predefined>` корня `ChartOfCharacteristicTypes`
//! (предопределённые виды характеристик). Вариант [`crate::predefined`] (Catalog) с ИНОЙ
//! структурой узла: каждый предопределённый элемент несёт `<type>` (ОписаниеТипов значения
//! характеристики), а `<code>` — ПЛОСКИЙ текст-лист (у Catalog — Value-xsi блок). Designer
//! тело предопределённых inline НЕ несёт — поэтому поле X-ИСКЛЮЧЕНО (`x_ignore` в спеке),
//! но УЧАСТВУЕТ в R EDT (byte-exact); Designer-сайдкар `Ext/Predefined.xml` читает
//! `morph1c_pipeline::predefined_read` в ЭТО ЖЕ спек-поле.
//!
//! Структура узла (witnessed-скан ВСЕГО ERP-корпуса CCT — 898 узлов 8 объектов — плюс
//! 2 SSL-объекта; ровно четыре шейпа):
//! * `<items id="uuid">` (или рекурсивный `<content id="uuid">`), дети ПО ПОРЯДКУ:
//!   `<name>` (required) → `[<description>]` → `[<isFolder>true]` → `[<code>]` →
//!   `<type>` (required; у ПАПОК — ПУСТОЙ `<type/>`) → `[<content id>…]` (рекурсивно,
//!   только у папок).
//! * витнессированные шейпы: `name,desc,code,type` (750); `name,desc,type` (69 — код у
//!   вида отключён, `ОбъектыАдресацииЗадач`/`РеквизитыЭлементовФинансовыхОтчетов`);
//!   `name,desc,isFolder,code,type∅,content×N` (79 — папки, `СтатьиАктивовПассивов`/
//!   `мирПроизвольныеПараметры`, вложенность до 3); SSL: `name[,desc],type`.
//!
//! Канонический IR (round-trip-маркер; X не сравнивает): `PropertyValue::List` из узлов;
//! узел — `List([id:Str, name:Str, has_desc:Bool, description:Str, code:Str,
//! has_folder:Bool, isFolder:Bool, children:List, has_code:Bool, type:Type])` — префикс
//! [0..7] СОВПАДАЕТ с Catalog-узлом ([`crate::predefined`]; общий код cf-харвеста путей),
//! CCT-хвост — `has_code`/`type`. Presence-флаги (`has_*`) восстанавливают EDT-разрежённость
//! byte-exact. §1.0: иная структура → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::type_codec::{self, TypeDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

// IR node layout indices — the [0..7] prefix mirrors `crate::predefined` (Catalog).
/// `@id` узла.
pub const I_ID: usize = 0;
/// `<name>`.
pub const I_NAME: usize = 1;
/// presence-флаг `<description>`.
pub const I_HAS_DESC: usize = 2;
/// Текст `<description>` (`""` при отсутствии).
pub const I_DESC: usize = 3;
/// Текст `<code>` (`""` при отсутствии; ПЛОСКИЙ лист — не Value-xsi, в отличие от Catalog).
pub const I_CODE: usize = 4;
/// presence-флаг `<isFolder>`.
pub const I_HAS_FOLDER: usize = 5;
/// Значение `<isFolder>` (bool; witnessed только `true`).
pub const I_FOLDER: usize = 6;
/// Рекурсивные `<content>`-дети (List; пуст у элементов).
pub const I_CHILDREN: usize = 7;
/// presence-флаг `<code>` (EDT-разрежённость: 69 ERP-узлов кода не несут).
pub const I_HAS_CODE: usize = 8;
/// `<type>` (ОписаниеТипов; ПУСТОЙ у папок — `<type/>`).
pub const I_TYPE: usize = 9;
/// Арность узла.
pub const N: usize = 10;

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
        PropertyValue::Str(String::new()), // code
        PropertyValue::Bool(false),        // has_folder
        PropertyValue::Bool(false),        // isFolder
        PropertyValue::List(Vec::new()),   // children
        PropertyValue::Bool(false),        // has_code
        PropertyValue::Type(morph1c_core::ir::value::TypeSpec { parts: Vec::new() }), // type
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
    // [isFolder] — witnessed ПЕРЕД <code> (в отличие от Catalog, где code раньше;
    // ERP `мирПроизвольныеПараметры`/`СтатьиАктивовПассивов`).
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
    // [code] — плоский текст-лист (witnessed ERP `АналитикиСтатейБюджетов` `000000016` и
    // ещё 828 узлов; 69 узлов кода не несут — presence-флаг). §1.0: present-empty
    // неотличим от absent при designer-реконструкции → ошибка, не догадка.
    if let Some(c) = it.peek() {
        if c.local == "code" && c.prefix.is_empty() {
            let code = it.next().unwrap();
            code.claim_with_text();
            if code.text.is_empty() {
                return Err(format!(
                    "<{}> predefined node: present-empty <code> (unwitnessed; absent ⟺ empty \
                     on the Designer side — §1.0)",
                    el.local
                ));
            }
            node[I_HAS_CODE] = PropertyValue::Bool(true);
            node[I_CODE] = PropertyValue::Str(code.text.clone());
        }
    }
    // type (required): `<type>…</type>` (ОписаниеТипов; у папок — пустой `<type/>`).
    let type_el = it
        .next()
        .filter(|c| c.local == "type" && c.prefix.is_empty())
        .ok_or_else(|| format!("<{}> predefined node missing <type>", el.local))?;
    type_el.claim();
    node[I_TYPE] = type_codec::decode(TypeDialect::Edt, type_el)?;
    // [content…] recursive (только у папок в witnessed-корпусе; структурно — как Catalog).
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

/// Эмитировать `<predefined>` из IR.
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
    if as_bool(&node[I_HAS_FOLDER])? {
        let b = as_bool(&node[I_FOLDER])?;
        el.push(OutElement::leaf(
            "",
            "isFolder",
            if b { "true" } else { "false" },
        ));
    }
    if as_bool(&node[I_HAS_CODE])? {
        el.push(OutElement::leaf(
            "",
            "code",
            as_str(&node[I_CODE])?.to_string(),
        ));
    }
    // `<type>` всегда present (у папок — ПУСТОЙ спек → self-closing `<type/>`, witnessed).
    el.push(type_codec::encode(
        TypeDialect::Edt,
        "",
        "type",
        as_type(&node[I_TYPE])?,
    )?);
    for child in as_list(&node[I_CHILDREN])? {
        el.push(emit_node("content", child)?);
    }
    Ok(el)
}

// --- helpers ----------------------------------------------------------------

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
fn as_type(v: &PropertyValue) -> Result<&morph1c_core::ir::value::TypeSpec, String> {
    match v {
        PropertyValue::Type(t) => Ok(t),
        other => Err(format!("predefined: expected Type, got {:?}", other.kind())),
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

    /// Witnessed ERP `АналитикиСтатейБюджетов`-шейп: name,desc,code,type — код в узле.
    #[test]
    fn flat_item_with_code_roundtrips() {
        let xml = "<r><predefined><items id=\"549c5386-98fe-4b3b-a5f3-f7a3c1abbc3d\">\
                   <name>ВидыНоменклатуры</name><description>Виды номенклатуры</description>\
                   <code>000000016</code><type><types>String</types></type>\
                   </items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_HAS_CODE], PropertyValue::Bool(true));
        assert_eq!(node[I_CODE], PropertyValue::Str("000000016".into()));
        assert_eq!(node[I_HAS_FOLDER], PropertyValue::Bool(false));
        let out = emit_node("items", &node_v).unwrap();
        let tags: Vec<_> = out.children.iter().map(|c| c.local.as_str()).collect();
        assert_eq!(tags, ["name", "description", "code", "type"]);
    }

    /// Witnessed ERP папка (`мирПроизвольныеПараметры`): isFolder ПЕРЕД code, пустой
    /// `<type/>`, рекурсивный `<content>`.
    #[test]
    fn folder_with_content_roundtrips() {
        let xml = "<r><predefined><items id=\"7c7b6aca-57dc-4583-b0cc-c5238684f1fa\">\
                   <name>Гр</name><description>Гр</description><isFolder>true</isFolder>\
                   <code>000000017</code><type/>\
                   <content id=\"92dbcb5e-6c20-4667-9aed-8a240f42d525\"><name>Эл</name>\
                   <description>Эл</description><code>000000018</code>\
                   <type><types>String</types></type></content>\
                   </items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_HAS_FOLDER], PropertyValue::Bool(true));
        assert_eq!(node[I_FOLDER], PropertyValue::Bool(true));
        match &node[I_TYPE] {
            PropertyValue::Type(t) => assert!(t.parts.is_empty(), "folder type is empty"),
            other => panic!("type: {other:?}"),
        }
        let PropertyValue::List(kids) = &node[I_CHILDREN] else { panic!() };
        assert_eq!(kids.len(), 1);
        // Re-emit preserves the witnessed order: name,desc,isFolder,code,type,content.
        let out = emit_node("items", &node_v).unwrap();
        let tags: Vec<_> = out.children.iter().map(|c| c.local.as_str()).collect();
        assert_eq!(tags, ["name", "description", "isFolder", "code", "type", "content"]);
        assert!(out.children[4].self_closing, "folder <type/> self-closes");
    }

    /// SSL/`ОбъектыАдресацииЗадач`-шейп (кода нет) — has_code=false, re-emit без <code>.
    #[test]
    fn item_without_code_roundtrips() {
        let xml = "<r><predefined><items id=\"58db8b99-289e-4ef3-a2d8-c4ec1aa03f40\">\
                   <name>ВсеОбъектыАдресации</name><description>Все</description>\
                   <type><types>String</types></type></items></predefined></r>";
        let node_v = decode_first(xml).unwrap();
        let PropertyValue::List(node) = &node_v else { panic!() };
        assert_eq!(node[I_HAS_CODE], PropertyValue::Bool(false));
        let out = emit_node("items", &node_v).unwrap();
        let tags: Vec<_> = out.children.iter().map(|c| c.local.as_str()).collect();
        assert_eq!(tags, ["name", "description", "type"]);
    }

    /// §1.0: present-empty `<code>` неотличим от absent на designer-стороне — отказ.
    #[test]
    fn present_empty_code_is_refused() {
        let xml = "<r><predefined><items id=\"a\"><name>Х</name><code></code>\
                   <type><types>String</types></type></items></predefined></r>";
        let err = decode_first(xml).unwrap_err();
        assert!(err.contains("present-empty"), "{err}");
    }
}
