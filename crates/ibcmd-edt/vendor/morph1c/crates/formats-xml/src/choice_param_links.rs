//! Codec для `choiceParameterLinks`/`ChoiceParameterLinks` — список связей параметров
//! выбора (child-objects substrate, InformationRegister-срез).
//!
//! В корпусе SSL почти всегда ПУСТ (self-closing `<…/>` → дефолт-омиссия), но ≥1
//! ресурс несёт непустую связь. Структура расходится между форматами:
//! * EDT: `<choiceParameterLinks><name>Отбор.X</name><field>Path</field></…>` —
//!   ОДИН элемент коллекции на связь, два листа `name`/`field`.
//! * Designer: `<ChoiceParameterLinks><xr:Link><xr:Name>Отбор.X</xr:Name>
//!   <xr:DataPath xsi:type="xs:string">Path</xr:DataPath><xr:ValueChange>Clear
//!   </xr:ValueChange></xr:Link></ChoiceParameterLinks>` — обёртка с `<xr:Link>`'ами.
//!
//! Канонический IR (X by construction): `PropertyValue::List` из `List([name, field])`
//! (обе — `Str`). Пустой → `List([])` (дефолт). `ValueChange=Clear` — константа
//! Designer (не в IR; регенерируется). §1.0: иная структура/`ValueChange` → ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinksDialect {
    /// EDT: элементы коллекции — прямые `<choiceParameterLinks>` под хостом? Нет: хост
    /// САМ есть один элемент связи. Несколько связей = несколько `<choiceParameterLinks>`
    /// СИБЛИНГОВ. Поэтому EDT навигирует от РОДИТЕЛЯ (см. `decode_edt`).
    Edt,
    /// Designer: один контейнер `<ChoiceParameterLinks>` с `<xr:Link>`'ами.
    Designer,
}

/// Упаковать связи в IR. Каноническая форма связи — `List([name, field])`. Если у связи
/// задан НЕ-дефолтный `changeMode` (EDT `<changeMode>`; редко — 1 связь в ERP-корпусе), она
/// упаковывается КАК `List([name, field, changeMode])`. 2-элементная форма (без changeMode)
/// сохраняется byte-identical для всех прежних видов (Catalog/IR/Designer её не несут).
fn pack(links: Vec<(String, String, Option<String>)>) -> PropertyValue {
    PropertyValue::List(
        links
            .into_iter()
            .map(|(n, f, cm)| {
                let mut rec = vec![PropertyValue::Str(n), PropertyValue::Str(f)];
                if let Some(cm) = cm {
                    rec.push(PropertyValue::Str(cm));
                }
                PropertyValue::List(rec)
            })
            .collect(),
    )
}

/// Распаковать связи из IR: `List([name, field])` либо `List([name, field, changeMode])`.
#[allow(clippy::type_complexity)]
fn unpack(v: &PropertyValue) -> Result<Vec<(&str, &str, Option<&str>)>, String> {
    let outer = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "choiceParameterLinks must be List, got {:?}",
                other.kind()
            ))
        }
    };
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            PropertyValue::List(p) if p.len() == 2 => {
                out.push((as_str(&p[0])?, as_str(&p[1])?, None));
            }
            PropertyValue::List(p) if p.len() == 3 => {
                out.push((as_str(&p[0])?, as_str(&p[1])?, Some(as_str(&p[2])?)));
            }
            _ => {
                return Err(
                    "choiceParameterLinks: each link must be List([name,field]) or \
                     List([name,field,changeMode])"
                        .into(),
                )
            }
        }
    }
    Ok(out)
}

fn as_str(v: &PropertyValue) -> Result<&str, String> {
    match v {
        PropertyValue::Str(s) => Ok(s),
        other => Err(format!(
            "choiceParameterLinks link member must be Str, got {:?}",
            other.kind()
        )),
    }
}

// --- DECODE -----------------------------------------------------------------

/// Designer: host = `<ChoiceParameterLinks>` контейнер (уже claimed `locate`'ом). Пуст
/// (self-closing) → `List([])`. Иначе — `<xr:Link>`'и.
pub fn decode_designer(host: &Element) -> Decoded {
    match decode_designer_inner(host) {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

fn decode_designer_inner(host: &Element) -> Result<PropertyValue, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("ChoiceParameterLinks must be unprefixed attribute-less container".into());
    }
    let mut links = Vec::new();
    for link in &host.children {
        if link.local != "Link" || link.prefix != "xr" {
            return Err(format!(
                "ChoiceParameterLinks: expected <xr:Link>, got <{}>",
                link.local
            ));
        }
        link.claim();
        // Строго: <xr:Name>, <xr:DataPath xsi:type="xs:string">, <xr:ValueChange>Clear.
        let mut it = link.children.iter();
        let name = req_xr_text(it.next(), "Name")?;
        let path_el = it
            .next()
            .ok_or("ChoiceParameterLinks <xr:Link> missing <xr:DataPath>")?;
        if path_el.local != "DataPath" || path_el.prefix != "xr" {
            return Err(format!(
                "ChoiceParameterLinks: expected <xr:DataPath>, got <{}>",
                path_el.local
            ));
        }
        // xsi:type="xs:string" обязателен.
        let xa = path_el
            .attr("xsi:type")
            .ok_or("xr:DataPath missing xsi:type")?;
        if xa.value != "xs:string" {
            return Err(format!(
                "xr:DataPath xsi:type must be xs:string, got {:?}",
                xa.value
            ));
        }
        xa.claimed.set(true);
        path_el.claim_with_text();
        let path = path_el.text.clone();
        let vc = it
            .next()
            .ok_or("ChoiceParameterLinks <xr:Link> missing <xr:ValueChange>")?;
        if vc.local != "ValueChange" || vc.prefix != "xr" {
            return Err(format!(
                "ChoiceParameterLinks: expected <xr:ValueChange>, got <{}>",
                vc.local
            ));
        }
        vc.claim_with_text();
        // `Clear` — дефолт (2-элементная связь); иное значение (ERP: `DontChange`) —
        // канонический changeMode (3-элементная связь; зеркало EDT `<changeMode>`).
        let change_mode = if vc.text == "Clear" {
            None
        } else {
            Some(vc.text.clone())
        };
        if it.next().is_some() {
            return Err("ChoiceParameterLinks <xr:Link> has unexpected extra child (§1.0)".into());
        }
        // claim <xr:Name> node+text.
        for c in &link.children {
            if c.local == "Name" && c.prefix == "xr" {
                c.claim_with_text();
            }
        }
        links.push((name, path, change_mode));
    }
    Ok(pack(links))
}

fn req_xr_text(el: Option<&Element>, local: &str) -> Result<String, String> {
    let e = el.ok_or_else(|| format!("ChoiceParameterLinks <xr:Link> missing <xr:{local}>"))?;
    if e.local != local || e.prefix != "xr" {
        return Err(format!(
            "ChoiceParameterLinks: expected <xr:{local}>, got <{}>",
            e.local
        ));
    }
    // Claim обязателен: без него листья <xr:Name>/<xr:ValueChange> падали в leftover —
    // ветка непустого designer-CPL была мёртвой на SSL (все CPL пустые) и вскрылась ERP.
    e.claim_with_text();
    Ok(e.text.clone())
}

/// EDT (from-root, multi-sibling): КАЖДАЯ связь — отдельный сиблинг
/// `<choiceParameterLinks><name>Отбор.X</name><field>Path</field></…>` под `root` (=
/// корень объекта / child-элемент). Читает ВСЕ сиблинги по порядку → `List` связей.
/// Ноль сиблингов (или все self-closing) → `List([])` (дефолт). Мирроринг
/// `choice_parameters::decode_edt` — заменяет прежний single-host путь, который терял 2-ю+
/// связь (AccumulationRegister-измерения несут по 2 связи; сверено ERP-корпусом).
pub fn decode_edt_from_root(root: &Element) -> Decoded {
    let mut links = Vec::new();
    for el in root
        .children
        .iter()
        .filter(|c| c.local == "choiceParameterLinks" && c.prefix.is_empty())
    {
        match decode_edt_one(el) {
            Ok(Some(link)) => links.push(link),
            Ok(None) => {} // self-closing/пустой сиблинг — пропускаем (не встречается непустым)
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(links))
}

/// Разобрать ОДИН `<choiceParameterLinks>` сиблинг → `(name, field, changeMode?)`. `None` —
/// пустой self-closing. Порядок листьев: `<name>`, [опц. `<changeMode>`], `<field>`
/// (сверено: 1 связь в ERP несёт `<changeMode>DontChange</changeMode>` между name и field).
#[allow(clippy::type_complexity)]
fn decode_edt_one(host: &Element) -> Result<Option<(String, String, Option<String>)>, String> {
    if !host.attrs.is_empty() {
        return Err("choiceParameterLinks must have no attributes".into());
    }
    host.claim();
    if host.children.is_empty() {
        return Ok(None);
    }
    let mut it = host.children.iter().peekable();
    let name = it
        .next()
        .filter(|e| e.local == "name" && e.prefix.is_empty())
        .ok_or_else(|| "choiceParameterLinks: expected <name>".to_string())?;
    name.claim_with_text();
    // Опциональный `<changeMode>` между <name> и <field> (дефолт — омитится).
    let change_mode = match it.peek() {
        Some(e) if e.local == "changeMode" && e.prefix.is_empty() => {
            let cm = it.next().unwrap();
            if !cm.attrs.is_empty() || !cm.children.is_empty() {
                return Err("choiceParameterLinks <changeMode>: expected plain text leaf".into());
            }
            cm.claim_with_text();
            Some(cm.text.clone())
        }
        _ => None,
    };
    let field = it
        .next()
        .filter(|e| e.local == "field" && e.prefix.is_empty())
        .ok_or_else(|| "choiceParameterLinks: expected <field>".to_string())?;
    field.claim_with_text();
    if it.next().is_some() {
        return Err(
            "choiceParameterLinks: unexpected extra child after <name>/<field> (§1.0)".into(),
        );
    }
    Ok(Some((name.text.clone(), field.text.clone(), change_mode)))
}

/// Декодировать НАБОР уже собранных сиблингов `<choiceParameterLinks>` (позиционный
/// std-attrs парсер собирает подряд идущие сиблинги сам; ERP-witnessed: Parent-атрибут
/// СкладскиеЯчейки несёт ДВЕ связи двумя сиблингами) — тот же IR, что from-root.
pub fn decode_edt_siblings(els: &[&Element]) -> Decoded {
    let mut links = Vec::with_capacity(els.len());
    for el in els {
        match decode_edt_one(el) {
            Ok(Some(link)) => links.push(link),
            Ok(None) => {}
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(links))
}

/// Claim EDT from-root — то же, что decode.
pub fn claim_edt_from_root(root: &Element) {
    let _ = decode_edt_from_root(root);
}

/// EDT (SINGLE-host): host = ОДИН `<choiceParameterLinks>` элемент связи (уже claimed
/// `locate`'ом). Используется ВНУТРИ std-attrs-блоков (`OptCpl`, Catalog), где связь —
/// одиночный позиционный лист (не сиблинг-коллекция). Для field-level коллекций
/// используется [`decode_edt_from_root`] (multi-sibling).
pub fn decode_edt(host: &Element) -> Decoded {
    match decode_edt_inner(host) {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

fn decode_edt_inner(host: &Element) -> Result<PropertyValue, String> {
    match decode_edt_one(host)? {
        Some(link) => Ok(pack(vec![link])),
        None => Ok(pack(Vec::new())),
    }
}

/// Собрать один EDT `<choiceParameterLinks>` из связи `(name, field, changeMode?)`.
/// Порядок листьев byte-exact: `<name>`, [опц. `<changeMode>`], `<field>`.
fn build_edt_link(
    prefix: &str,
    local: &str,
    name: &str,
    field: &str,
    cm: Option<&str>,
) -> OutElement {
    let mut el = OutElement::branch(prefix, local);
    el.push(OutElement::leaf("", "name", name.to_string()));
    if let Some(cm) = cm {
        el.push(OutElement::leaf("", "changeMode", cm.to_string()));
    }
    el.push(OutElement::leaf("", "field", field.to_string()));
    el
}

/// EDT (from-value, multi-sibling): по `<choiceParameterLinks>` на связь. Пусто → НОЛЬ
/// узлов (разрежённый EDT: дефолт-омиссия делается выше по `is_default`).
pub fn emit_edt_from_root(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let links = unpack(value)?;
    let mut out = Vec::with_capacity(links.len());
    for (name, field, cm) in links {
        out.push(build_edt_link("", "choiceParameterLinks", name, field, cm));
    }
    Ok(out)
}

// --- CLAIM ------------------------------------------------------------------

pub fn claim(dialect: LinksDialect, host: &Element) {
    match dialect {
        LinksDialect::Edt => {
            let _ = decode_edt_inner(host);
        }
        LinksDialect::Designer => {
            let _ = decode_designer_inner(host);
        }
    }
}

// --- ENCODE -----------------------------------------------------------------

/// EDT (SINGLE-host, внутри std-attrs OptCpl): пустой → self-closing
/// `<choiceParameterLinks/>`; одна связь → элемент с `<name>`/[`<changeMode>`]/`<field>`.
/// (>1 связь не встречается ВНУТРИ std-attrs; field-level коллекции — `emit_edt_from_root`.)
pub fn encode_edt(prefix: &str, local: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let links = unpack(value)?;
    if links.is_empty() {
        return Ok(OutElement::self_closing(prefix, local));
    }
    if links.len() > 1 {
        return Err(
            "choiceParameterLinks EDT (single-host): >1 link unsupported here (§1.0)".into(),
        );
    }
    let (name, field, cm) = links[0];
    Ok(build_edt_link(prefix, local, name, field, cm))
}

/// Designer: пустой → self-closing `<ChoiceParameterLinks/>`; иначе обёртка с `<xr:Link>`.
/// `ValueChange` = канонический changeMode связи (3-й элемент IR), дефолт `Clear`
/// (2-элементная связь) — зеркало EDT `<changeMode>` (ERP-witnessed `DontChange` на
/// корне Constant).
pub fn encode_designer(
    prefix: &str,
    local: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let links = unpack(value)?;
    if links.is_empty() {
        return Ok(OutElement::self_closing(prefix, local));
    }
    let mut wrapper = OutElement::branch(prefix, local);
    for (name, field, cm) in links {
        let mut link = OutElement::branch("xr", "Link");
        link.push(OutElement::leaf("xr", "Name", name.to_string()));
        link.push(
            OutElement::leaf("xr", "DataPath", field.to_string()).attr("xsi:type", "xs:string"),
        );
        link.push(OutElement::leaf("xr", "ValueChange", cm.unwrap_or("Clear").to_string()));
        wrapper.push(link);
    }
    Ok(wrapper)
}
