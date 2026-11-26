//! Codec для `linkByType`/`LinkByType` реквизита (связь по типу). Структура расходится:
//! * EDT: пусто → тег ОТСУТСТВУЕТ; непусто → `<linkByType><field>Path</field></linkByType>`;
//!   при НЕНУЛЕВОМ link-item — с ведущим `<linkItem>N</linkItem>` (ERP-witnessed
//!   AccountingRegister std-attrs `ExtDimensionN`: `<linkByType><linkItem>1</linkItem>
//!   <field>…StandardAttribute.Account</field></linkByType>`, Хозрасчетный).
//! * Designer: пусто → `<LinkByType/>`; непусто → `<LinkByType><xr:DataPath>Path</xr:DataPath>
//!   <xr:LinkItem>N</xr:LinkItem></LinkByType>` (`N`=0 у реквизитного linkByType; 1..3 у
//!   std-attrs `ExtDimensionN` — ERP-witnessed, все 3 регистра бухгалтерии).
//!
//! Канонический IR (X by construction): `PropertyValue::Str` — путь (`""` = пусто, дефолт).
//! Link-item в IR НЕ хранится: он ОПРЕДЕЛЁН МЕСТОМ листа — 0 у реквизитов, N у std-attrs
//! `ExtDimensionN` (имя-функция; выводит вызывающий и передаёт в `*_with_item`). §1.0:
//! иная структура/чужой ns/link-item ≠ ожидаемого → ОШИБКА, не догадка.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkByTypeDialect {
    /// EDT: `<linkByType><field>Path</field></linkByType>`.
    Edt,
    /// Designer: `<LinkByType><xr:DataPath>Path</xr:DataPath><xr:LinkItem>0</…></LinkByType>`.
    Designer,
}

/// Суффикс-маркер НЕнулевого link-item в канонической Str-нагрузке linkByType
/// (`<path>#LinkItem#<N>`): item — ДАННОЕ (номер колонки/субконто связующего реквизита),
/// ERP-witnessed на реквизитах (LinkItem 1..3: Document/Catalog/AccountingRegister/IR).
/// Нулевой item — плоский путь (byte-compat всех прежних корпусов). NB: std-attrs
/// ExtDimensionN-вариант ([`decode_with_item`]) хранит ПЛОСКИЙ путь — там item выводится
/// из ИМЕНИ атрибута, не из данных.
pub const LINK_ITEM_SUFFIX: &str = "#LinkItem#";

/// Собрать каноническую нагрузку: `path` (item 0) | `path#LinkItem#N`.
pub fn pack_payload(path: &str, item: i64) -> String {
    if item == 0 {
        path.to_string()
    } else {
        format!("{path}{LINK_ITEM_SUFFIX}{item}")
    }
}

/// Разобрать каноническую нагрузку → (path, item).
pub fn split_payload(payload: &str) -> (&str, i64) {
    if let Some(pos) = payload.rfind(LINK_ITEM_SUFFIX) {
        if let Ok(n) = payload[pos + LINK_ITEM_SUFFIX.len()..].parse::<i64>() {
            return (&payload[..pos], n);
        }
    }
    (payload, 0)
}

/// Декодировать host (уже claimed `locate`'ом). Пусто → `Str("")`. Реквизитный путь:
/// link-item — ПЕРЕМЕННАЯ (ERP-witnessed 0..3) — кодируется суффиксом нагрузки.
pub fn decode(dialect: LinkByTypeDialect, host: &Element) -> Decoded {
    let res = match dialect {
        LinkByTypeDialect::Edt => decode_edt_any(host),
        LinkByTypeDialect::Designer => decode_designer_any(host),
    };
    match res {
        Ok((path, item)) => Decoded::Present(PropertyValue::Str(pack_payload(&path, item))),
        Err(e) => Decoded::Error(e),
    }
}

/// EDT attr-level: опциональный ведущий `<linkItem>N</linkItem>` (омитится при 0) + `<field>`.
fn decode_edt_any(host: &Element) -> Result<(String, i64), String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("linkByType must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        return Ok((String::new(), 0));
    }
    let mut it = host.children.iter().peekable();
    let mut item = 0i64;
    if let Some(c) = it.peek() {
        if c.local == "linkItem" && c.prefix.is_empty() {
            let li = it.next().unwrap();
            if !li.attrs.is_empty() || !li.children.is_empty() {
                return Err("linkByType <linkItem>: expected plain text leaf".into());
            }
            item = li
                .text
                .parse()
                .map_err(|_| format!("linkByType <linkItem> not integer: {:?}", li.text))?;
            li.claim_with_text();
        }
    }
    let field = it
        .next()
        .filter(|c| c.local == "field" && c.prefix.is_empty())
        .ok_or("linkByType: expected <field>Path</field> leaf")?;
    if !field.attrs.is_empty() || !field.children.is_empty() {
        return Err("linkByType: expected <field>Path</field> leaf".into());
    }
    field.claim_with_text();
    if it.next().is_some() {
        return Err("linkByType: unexpected extra child (§1.0)".into());
    }
    Ok((field.text.clone(), item))
}

/// Designer attr-level: `<xr:DataPath>` + `<xr:LinkItem>N</xr:LinkItem>` (N — переменная).
fn decode_designer_any(host: &Element) -> Result<(String, i64), String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("LinkByType must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        return Ok((String::new(), 0));
    }
    if host.children.len() != 2 {
        return Err(format!(
            "LinkByType: expected <xr:DataPath>+<xr:LinkItem>, got {} children",
            host.children.len()
        ));
    }
    let dp = &host.children[0];
    if dp.local != "DataPath"
        || dp.prefix != "xr"
        || !dp.attrs.is_empty()
        || !dp.children.is_empty()
    {
        return Err("LinkByType: expected <xr:DataPath>Path</xr:DataPath>".into());
    }
    dp.claim_with_text();
    let li = &host.children[1];
    if li.local != "LinkItem" || li.prefix != "xr" || !li.attrs.is_empty() || !li.children.is_empty()
    {
        return Err("LinkByType: expected <xr:LinkItem>N</xr:LinkItem>".into());
    }
    let item: i64 = li
        .text
        .parse()
        .map_err(|_| format!("LinkByType <xr:LinkItem> not integer: {:?}", li.text))?;
    li.claim_with_text();
    Ok((dp.text.clone(), item))
}

/// Декодировать host с ОЖИДАЕМЫМ link-item (0 — реквизит; N — std-attrs `ExtDimensionN`).
/// §1.0: наблюдаемый item ≠ ожидаемого → ошибка.
pub fn decode_with_item(dialect: LinkByTypeDialect, host: &Element, item: i64) -> Decoded {
    let res = match dialect {
        LinkByTypeDialect::Edt => decode_edt(host, item),
        LinkByTypeDialect::Designer => decode_designer(host, item),
    };
    match res {
        Ok(v) => Decoded::Present(PropertyValue::Str(v)),
        Err(e) => Decoded::Error(e),
    }
}

fn decode_edt(host: &Element, item: i64) -> Result<String, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("linkByType must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        return Ok(String::new());
    }
    // Ненулевой item — ведущий `<linkItem>N</linkItem>` (ERP-witnessed std-attrs
    // ExtDimensionN); нулевой — leaf ОТСУТСТВУЕТ (реквизитный корпус-инвариант).
    let mut it = host.children.iter().peekable();
    if item != 0 {
        let li = it
            .next()
            .filter(|c| c.local == "linkItem" && c.prefix.is_empty())
            .ok_or("linkByType: expected leading <linkItem> (non-zero link item)")?;
        if !li.attrs.is_empty() || !li.children.is_empty() || li.text != item.to_string() {
            return Err(format!(
                "linkByType: expected <linkItem>{item}</linkItem>, got {:?} (§1.0)",
                li.text
            ));
        }
        li.claim_with_text();
    }
    let field = it
        .next()
        .filter(|c| c.local == "field" && c.prefix.is_empty())
        .ok_or("linkByType: expected <field>Path</field> leaf")?;
    if !field.attrs.is_empty() || !field.children.is_empty() {
        return Err("linkByType: expected <field>Path</field> leaf".into());
    }
    field.claim_with_text();
    if it.next().is_some() {
        return Err("linkByType: unexpected extra child (§1.0)".into());
    }
    Ok(field.text.clone())
}

fn decode_designer(host: &Element, item: i64) -> Result<String, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("LinkByType must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        return Ok(String::new());
    }
    if host.children.len() != 2 {
        return Err(format!(
            "LinkByType: expected <xr:DataPath>+<xr:LinkItem>, got {} children",
            host.children.len()
        ));
    }
    let dp = &host.children[0];
    if dp.local != "DataPath"
        || dp.prefix != "xr"
        || !dp.attrs.is_empty()
        || !dp.children.is_empty()
    {
        return Err("LinkByType: expected <xr:DataPath>Path</xr:DataPath>".into());
    }
    dp.claim_with_text();
    let li = &host.children[1];
    if li.local != "LinkItem"
        || li.prefix != "xr"
        || !li.attrs.is_empty()
        || !li.children.is_empty()
        || li.text != item.to_string()
    {
        return Err(format!(
            "LinkByType: expected <xr:LinkItem>{item}</xr:LinkItem> (§1.0)"
        ));
    }
    li.claim_with_text();
    Ok(dp.text.clone())
}

/// Claim (host claimed выше) — то же, что decode.
pub fn claim(dialect: LinkByTypeDialect, host: &Element) {
    let _ = decode(dialect, host);
}

/// Эмитировать host `<tag>` (имя/ns — из локуса). Пустой → self-closing. Реквизитный
/// путь: link-item — константа 0.
pub fn encode(
    dialect: LinkByTypeDialect,
    ns: &str,
    tag: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let payload = match value {
        PropertyValue::Str(s) => s.as_str(),
        other => return Err(format!("linkByType expects Str, got {:?}", other.kind())),
    };
    let (path, item) = split_payload(payload);
    if item == 0 {
        return encode_with_item(dialect, ns, tag, &PropertyValue::Str(path.to_string()), 0);
    }
    if path.is_empty() {
        return Err("linkByType: non-zero linkItem with empty path (§1.0)".into());
    }
    Ok(match dialect {
        LinkByTypeDialect::Edt => {
            let mut host = OutElement::branch(ns, tag);
            host.push(OutElement::leaf("", "linkItem", item.to_string()));
            host.push(OutElement::leaf("", "field", path.to_string()));
            host
        }
        LinkByTypeDialect::Designer => {
            let mut host = OutElement::branch(ns, tag);
            host.push(OutElement::leaf("xr", "DataPath", path.to_string()));
            host.push(OutElement::leaf("xr", "LinkItem", item.to_string()));
            host
        }
    })
}

/// Эмитировать host с ЗАДАННЫМ link-item (0 — реквизит; N — std-attrs `ExtDimensionN`).
pub fn encode_with_item(
    dialect: LinkByTypeDialect,
    ns: &str,
    tag: &str,
    value: &PropertyValue,
    item: i64,
) -> Result<OutElement, String> {
    let s = match value {
        PropertyValue::Str(s) => s,
        other => return Err(format!("linkByType expects Str, got {:?}", other.kind())),
    };
    Ok(match dialect {
        LinkByTypeDialect::Edt => {
            if s.is_empty() {
                OutElement::self_closing(ns, tag)
            } else {
                let mut host = OutElement::branch(ns, tag);
                if item != 0 {
                    host.push(OutElement::leaf("", "linkItem", item.to_string()));
                }
                host.push(OutElement::leaf("", "field", s.clone()));
                host
            }
        }
        LinkByTypeDialect::Designer => {
            if s.is_empty() {
                OutElement::self_closing(ns, tag)
            } else {
                let mut host = OutElement::branch(ns, tag);
                host.push(OutElement::leaf("xr", "DataPath", s.clone()));
                host.push(OutElement::leaf("xr", "LinkItem", item.to_string()));
                host
            }
        }
    })
}
