//! Codec для `content`/`Content` корня `CommonAttribute` (состав общего реквизита) —
//! child-objects substrate, CommonAttribute-срез. Список «элементов состава»
//! (CommonAttributeContentItem), связывающих общий реквизит с объектами метаданных.
//!
//! Структура РАСХОДИТСЯ между форматами (X by construction — оба дают ОДИН IR):
//! * EDT: КАЖДЫЙ элемент — сиблинг `<content>` под КОРНЕМ с детьми в фикс-порядке:
//!   `<metadata>Путь</metadata>` (required) и `<use>Режим</use>` (required — сверено:
//!   ВСЕ элементы SSL-корпуса несут оба). Пустой список → нет тегов вовсе.
//! * Designer: ОДИН контейнер `<Content>` с детьми `<xr:Item>`; каждый item несёт
//!   `<xr:Metadata>Путь</xr:Metadata>`, `<xr:Use>Режим</xr:Use>` и
//!   `<xr:ConditionalSeparation/>` (в корпусе ВСЕГДА пуст — self-closing). Пустой список
//!   → самозакрывающийся `<Content/>`.
//!
//! Канонический IR: `List` элементов, каждый — `List([Str(metadata), Enum(use)])` в
//! порядке authored. `conditionalSeparation` per-item ВСЕГДА пуст в корпусе (Designer),
//! у EDT аналога нет → НЕ моделируется в IR (Designer эмитит фикс-пустой лист, EDT — ничего;
//! X by construction). §1.0: непустой `<xr:ConditionalSeparation>`/иная структура/чужой ns/
//! лишний лист → ОШИБКА, никогда silent-drop.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{PropertyValue, Token};

/// Диалект состава общего реквизита.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonAttributeContentDialect {
    /// EDT: сиблинги `<content><metadata>Path</metadata><use>Mode</use></content>` от КОРНЯ.
    Edt,
    /// Designer: контейнер `<Content>` с `<xr:Item>`'ами.
    Designer,
}

const I_METADATA: usize = 0;
const I_USE: usize = 1;
const N_FIELDS: usize = 2;

fn pack(records: Vec<Vec<PropertyValue>>) -> PropertyValue {
    PropertyValue::List(records.into_iter().map(PropertyValue::List).collect())
}

fn unpack(v: &PropertyValue) -> Result<Vec<&Vec<PropertyValue>>, String> {
    let outer = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "CommonAttribute content must be List, got {:?}",
                other.kind()
            ))
        }
    };
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            PropertyValue::List(rec) if rec.len() == N_FIELDS => out.push(rec),
            other => {
                return Err(format!(
                    "CommonAttribute content item must be List of {N_FIELDS}, got {:?}",
                    other.kind()
                ))
            }
        }
    }
    Ok(out)
}

fn rec_metadata(rec: &[PropertyValue]) -> Result<String, String> {
    match &rec[I_METADATA] {
        PropertyValue::Str(s) => Ok(s.clone()),
        other => Err(format!(
            "content metadata must be Str, got {:?}",
            other.kind()
        )),
    }
}

fn rec_use(rec: &[PropertyValue]) -> Result<String, String> {
    match &rec[I_USE] {
        PropertyValue::Enum(t) => Ok(t.as_str().to_string()),
        other => Err(format!("content use must be Enum, got {:?}", other.kind())),
    }
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

// --- EDT (multi-sibling from root) ------------------------------------------

/// Декодировать EDT: все сиблинги `<content>` под `root` (по порядку). Пустой → `List([])`.
pub fn decode_edt(root: &Element) -> Decoded {
    let mut records = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == "content" && c.prefix.is_empty())
    {
        match decode_edt_one(ch) {
            Ok(rec) => records.push(rec),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(records))
}

/// Claim EDT-список (для leftover) — то же, что читает [`decode_edt`].
pub fn claim_edt(root: &Element) {
    let _ = decode_edt(root);
}

fn decode_edt_one(el: &Element) -> Result<Vec<PropertyValue>, String> {
    if !el.attrs.is_empty() || !el.text.is_empty() {
        return Err("<content> must be attribute-less, no text (§1.0)".into());
    }
    el.claim();
    if el.children.len() != N_FIELDS {
        return Err(format!(
            "<content>: expected <metadata>+<use>, got {} children (§1.0)",
            el.children.len()
        ));
    }
    // metadata — required, первый.
    let md = &el.children[0];
    if md.local != "metadata"
        || !md.prefix.is_empty()
        || !md.attrs.is_empty()
        || !md.children.is_empty()
    {
        return Err(format!(
            "<content>: expected <metadata>Path</metadata>, got <{}>",
            qname(md)
        ));
    }
    md.claim_with_text();
    // use — required, второй.
    let us = &el.children[1];
    if us.local != "use" || !us.prefix.is_empty() || !us.attrs.is_empty() || !us.children.is_empty()
    {
        return Err(format!(
            "<content>: expected <use>Mode</use> after <metadata>, got <{}>",
            qname(us)
        ));
    }
    us.claim_with_text();
    Ok(vec![
        PropertyValue::Str(md.text.clone()),
        PropertyValue::Enum(Token::new(us.text.clone())),
    ])
}

/// Эмитировать EDT-список: один `<content>` на элемент (порядок IR). Пустой → ничего.
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let records = unpack(value)?;
    let mut out = Vec::with_capacity(records.len());
    for rec in records {
        let md = rec_metadata(rec)?;
        let us = rec_use(rec)?;
        let mut item = OutElement::branch("", "content");
        item.push(OutElement::leaf("", "metadata", md));
        item.push(OutElement::leaf("", "use", us));
        out.push(item);
    }
    Ok(out)
}

// --- Designer (single container) --------------------------------------------

/// Декодировать Designer-контейнер `<Content>` (уже claimed `locate`'ом). Пустой
/// (self-closing) → `List([])`.
pub fn decode_designer(host: &Element) -> Decoded {
    match decode_designer_inner(host) {
        Ok(records) => Decoded::Present(pack(records)),
        Err(e) => Decoded::Error(e),
    }
}

fn decode_designer_inner(host: &Element) -> Result<Vec<Vec<PropertyValue>>, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("Content container must be attribute-less, no text (§1.0)".into());
    }
    let mut records = Vec::with_capacity(host.children.len());
    for item in &host.children {
        if item.local != "Item" || item.prefix != "xr" {
            return Err(format!(
                "Content: expected <xr:Item>, got <{}>",
                qname(item)
            ));
        }
        if !item.attrs.is_empty() || !item.text.is_empty() {
            return Err("<xr:Item> must be attribute-less, no text (§1.0)".into());
        }
        item.claim();
        if item.children.len() != 3 {
            return Err(format!(
                "<xr:Item>: expected <xr:Metadata>+<xr:Use>+<xr:ConditionalSeparation>, got {} children (§1.0)",
                item.children.len()
            ));
        }
        // metadata — первый.
        let md = &item.children[0];
        if md.local != "Metadata"
            || md.prefix != "xr"
            || !md.attrs.is_empty()
            || !md.children.is_empty()
        {
            return Err(format!(
                "<xr:Item>: expected <xr:Metadata>Path</xr:Metadata>, got <{}>",
                qname(md)
            ));
        }
        md.claim_with_text();
        // use — второй.
        let us = &item.children[1];
        if us.local != "Use" || us.prefix != "xr" || !us.attrs.is_empty() || !us.children.is_empty()
        {
            return Err(format!(
                "<xr:Item>: expected <xr:Use>Mode</xr:Use>, got <{}>",
                qname(us)
            ));
        }
        us.claim_with_text();
        // conditionalSeparation — третий, ВСЕГДА пуст (self-closing) в корпусе. §1.0:
        // непустой → ошибка (нет IR-слота; непустое значение потерялось бы молча).
        let cs = &item.children[2];
        if cs.local != "ConditionalSeparation"
            || cs.prefix != "xr"
            || !cs.attrs.is_empty()
            || !cs.children.is_empty()
        {
            return Err(format!(
                "<xr:Item>: expected <xr:ConditionalSeparation/>, got <{}>",
                qname(cs)
            ));
        }
        if !cs.text.is_empty() {
            return Err(format!(
                "<xr:ConditionalSeparation> must be empty (no IR slot for non-empty value, §1.0), got {:?}",
                cs.text
            ));
        }
        cs.claim_with_text();
        records.push(vec![
            PropertyValue::Str(md.text.clone()),
            PropertyValue::Enum(Token::new(us.text.clone())),
        ]);
    }
    Ok(records)
}

/// Claim Designer-контейнер (host claimed выше) — то же, что читает [`decode_designer`].
pub fn claim_designer(host: &Element) {
    let _ = decode_designer_inner(host);
}

/// Эмитировать Designer-контейнер `<Content>` (имя/ns — из локуса). Пустой → self-closing.
/// Каждый item: `<xr:Metadata>`, `<xr:Use>`, фикс-пустой `<xr:ConditionalSeparation/>`.
pub fn emit_designer(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let records = unpack(value)?;
    if records.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut container = OutElement::branch(ns, tag);
    for rec in records {
        let md = rec_metadata(rec)?;
        let us = rec_use(rec)?;
        let mut item = OutElement::branch("xr", "Item");
        item.push(OutElement::leaf("xr", "Metadata", md));
        item.push(OutElement::leaf("xr", "Use", us));
        item.push(OutElement::self_closing("xr", "ConditionalSeparation"));
        container.push(item);
    }
    Ok(container)
}
