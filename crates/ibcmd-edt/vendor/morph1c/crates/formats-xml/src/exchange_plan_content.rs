//! Codec для `content` корня `ExchangePlan` (состав плана обмена) — child-objects
//! substrate, ExchangePlan-срез. Список «элементов состава» (ExchangePlanContentItem),
//! связывающих план обмена с регистрируемыми объектами метаданных.
//!
//! Структура (EDT-only — Designer `.cf` состав НЕ несёт; X-исключён по построению, см.
//! `morph1c_testkit`):
//! * EDT: КАЖДЫЙ элемент — сиблинг `<content>` под корнем с детьми в фикс-порядке:
//!   `<mdObject>Путь</mdObject>` (required) и ОПЦИОНАЛЬНО `<autoRecord>Режим</autoRecord>`.
//!   Пустой список → нет тегов вовсе.
//!
//! PRESENCE-OF-DEFAULT (субстрат): `autoRecord` — ОПЦИОНАЛЬНЫЙ тег, чьё присутствие НЕ
//! биективно с не-дефолтностью. SSL опускает его всюду; ERP эмитит ЯВНЫЙ `<autoRecord>Allow
//! </autoRecord>` — дефолт `Allow`, выписанный явно. Прежняя модель (запись фикс-длины
//! `[mdObject, autoRecord]` + re-sparsify «опустить при `Allow`») теряла эту явную форму →
//! byte-exact R невозможен. Теперь ПРИСУТСТВИЕ моделируется явно ДЛИНОЙ записи: `autoRecord`
//! кладётся в запись ⇔ он присутствовал в источнике (даже если это дефолт `Allow`), и
//! эмитится verbatim; отсутствие ⇒ запись из одного элемента (омиссия сохраняется).
//!
//! Канонический IR: `List` элементов, каждый — `List([Str(mdObject)])` (autoRecord опущен)
//! ЛИБО `List([Str(mdObject), Enum(autoRecord)])` (autoRecord присутствовал), в порядке
//! authored. §1.0: иная структура/чужой ns/лишний лист → ОШИБКА, не silent-drop.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{PropertyValue, Token};

const I_MD_OBJECT: usize = 0;
const I_AUTO_RECORD: usize = 1;

fn pack(records: Vec<Vec<PropertyValue>>) -> PropertyValue {
    PropertyValue::List(records.into_iter().map(PropertyValue::List).collect())
}

fn unpack(v: &PropertyValue) -> Result<Vec<&Vec<PropertyValue>>, String> {
    let outer = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "ExchangePlan content must be List, got {:?}",
                other.kind()
            ))
        }
    };
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            // 1 элемент [mdObject] (autoRecord опущен) ИЛИ 2 [mdObject, autoRecord]
            // (autoRecord присутствовал — presence-of-default).
            PropertyValue::List(rec) if rec.len() == 1 || rec.len() == 2 => out.push(rec),
            other => {
                return Err(format!(
                    "ExchangePlan content item must be List of 1..=2, got {:?}",
                    other.kind()
                ))
            }
        }
    }
    Ok(out)
}

// --- EDT (multi-sibling from root) ------------------------------------------

/// Декодировать EDT: все сиблинги `<content>` под `root` (по порядку).
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
    if !el.attrs.is_empty() {
        return Err("<content> must have no attributes (§1.0)".into());
    }
    el.claim();
    let mut it = el.children.iter();
    // mdObject — required, первый.
    let md = it.next().ok_or("<content>: expected <mdObject>")?;
    if md.local != "mdObject" || !md.prefix.is_empty() {
        return Err(format!(
            "<content>: expected <mdObject>, got <{}>",
            qname(md)
        ));
    }
    if !md.attrs.is_empty() || !md.children.is_empty() {
        return Err("<content><mdObject> must be a plain text leaf (§1.0)".into());
    }
    md.claim_with_text();
    let mut rec = vec![PropertyValue::Str(md.text.clone())];
    // autoRecord — ОПЦИОНАЛЬНО, второй. Его ПРИСУТСТВИЕ (даже == дефолт `Allow`)
    // сохраняется как второй элемент записи (presence-of-default → byte-exact R).
    if let Some(ar) = it.next() {
        if ar.local != "autoRecord" || !ar.prefix.is_empty() {
            return Err(format!(
                "<content>: expected <autoRecord> after <mdObject>, got <{}>",
                qname(ar)
            ));
        }
        if !ar.attrs.is_empty() || !ar.children.is_empty() {
            return Err("<content><autoRecord> must be a plain text leaf (§1.0)".into());
        }
        ar.claim_with_text();
        rec.push(PropertyValue::Enum(Token::new(ar.text.clone())));
    }
    if let Some(extra) = it.next() {
        return Err(format!(
            "<content>: unexpected extra leaf <{}> (§1.0)",
            qname(extra)
        ));
    }
    Ok(rec)
}

/// Эмитировать EDT-список: один `<content>` на элемент. `autoRecord` эмитится ⇔ он
/// присутствовал в источнике (2-й элемент записи) — включая явный дефолт `Allow`
/// (presence-of-default); отсутствие 2-го элемента ⇒ тег не эмитим (омиссия сохранена).
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let records = unpack(value)?;
    let mut out = Vec::with_capacity(records.len());
    for rec in records {
        let md = match &rec[I_MD_OBJECT] {
            PropertyValue::Str(s) => s.clone(),
            other => {
                return Err(format!(
                    "content mdObject must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        let mut item = OutElement::branch("", "content");
        item.push(OutElement::leaf("", "mdObject", md));
        if let Some(auto) = rec.get(I_AUTO_RECORD) {
            match auto {
                PropertyValue::Enum(t) => {
                    item.push(OutElement::leaf("", "autoRecord", t.as_str().to_string()))
                }
                other => {
                    return Err(format!(
                        "content autoRecord must be Enum, got {:?}",
                        other.kind()
                    ))
                }
            }
        }
        out.push(item);
    }
    Ok(out)
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}
