//! Codec для `characteristics`/`Characteristics` корня `Catalog` (child-objects
//! substrate, Catalog-срез §3.x). Список «связок характеристик» (12 полей каждая),
//! связывающих план видов характеристик с табличной частью значений.
//!
//! Структура расходится между форматами (сверено 12/12 связок обоих форматов — ЕДИНАЯ
//! форма):
//! * EDT: КАЖДАЯ связка — сиблинг `<characteristics>` под корнем с 12 ПЛОСКИМИ листами
//!   в фиксированном порядке: `characteristicTypes, keyField, typesFilterField,
//!   typesFilterValue, dataPathField, multipleValuesUseField, characteristicValues,
//!   objectField, typeField, valueField, multipleValuesKeyField, multipleValuesOrderField`.
//!   Пустой список → нет тегов (дефолт-омиссия).
//! * Designer: один контейнер `<Characteristics>` с `<xr:Characteristic>`'ами; каждый —
//!   ДВЕ группы: `<xr:CharacteristicTypes from="<characteristicTypes>">` (несёт KeyField,
//!   TypesFilterField, TypesFilterValue, DataPathField, MultipleValuesUseField) и
//!   `<xr:CharacteristicValues from="<characteristicValues>">` (ObjectField, TypeField,
//!   ValueField, MultipleValuesKeyField, MultipleValuesOrderField). `from`-атрибуты несут
//!   characteristicTypes/characteristicValues-пути. Пустой → `<Characteristics/>`.
//!
//! Канонический IR (X by construction): `List` связок, каждая — `List` из 12 значений в
//! EDT-порядке: 11×`Str` (пути; dataPathField/multipleValues*Field несут либо путь, либо
//! unset-литерал `0`/`-1` — обе орфографии witnessed (SSL 16×"0"+104×"-1", ERP
//! 201×"0"+4521×"-1", пути — ERP Документ.ВыбытиеРНПТМатериаловИзПроизводства), хранятся
//! БУКВАЛЬНО), 1×`Value` (typesFilterValue, xsi). §1.0: иная структура/набор/чужой ns →
//! ОШИБКА.

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::value_codec::{self, ValueDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacteristicsDialect {
    /// EDT: сиблинги `<characteristics>` под корнем (12 плоских листов каждый).
    Edt,
    /// Designer: контейнер `<Characteristics>` с `<xr:Characteristic>`'ами (две группы).
    Designer,
}

/// Имена 12 полей связки В КАНОНИЧЕСКОМ (EDT) порядке + их вид-кодировка.
/// 0 char.types(Str) 1 keyField(Str) 2 typesFilterField(Str) 3 typesFilterValue(Value)
/// 4 dataPathField(Str) 5 multipleValuesUseField(Str) 6 char.values(Str) 7 objectField(Str)
/// 8 typeField(Str) 9 valueField(Str) 10 multipleValuesKeyField(Str) 11 mvOrderField(Str).
const N_FIELDS: usize = 12;

// EDT flat leaf tag names (порядок = канон IR).
const EDT_TAGS: [&str; N_FIELDS] = [
    "characteristicTypes",
    "keyField",
    "typesFilterField",
    "typesFilterValue",
    "dataPathField",
    "multipleValuesUseField",
    "characteristicValues",
    "objectField",
    "typeField",
    "valueField",
    "multipleValuesKeyField",
    "multipleValuesOrderField",
];

// Какие индексы — Value (остальное — Str-листы; в т.ч. dataPathField/multipleValues*Field,
// несущие путь ЛИБО unset-литерал `0`/`-1` — текст хранится буквально).
fn is_value(i: usize) -> bool {
    i == 3
}

// --- IR pack/unpack ---------------------------------------------------------

fn pack(records: Vec<Vec<PropertyValue>>) -> PropertyValue {
    PropertyValue::List(records.into_iter().map(PropertyValue::List).collect())
}

fn unpack(v: &PropertyValue) -> Result<Vec<&Vec<PropertyValue>>, String> {
    let outer = match v {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "characteristics must be List, got {:?}",
                other.kind()
            ))
        }
    };
    let mut out = Vec::with_capacity(outer.len());
    for item in outer {
        match item {
            PropertyValue::List(rec) if rec.len() == N_FIELDS => out.push(rec),
            PropertyValue::List(rec) => {
                return Err(format!(
                    "characteristic record must have {N_FIELDS} fields, got {}",
                    rec.len()
                ))
            }
            other => {
                return Err(format!(
                    "characteristic record must be List, got {:?}",
                    other.kind()
                ))
            }
        }
    }
    Ok(out)
}

// --- EDT --------------------------------------------------------------------

/// Декодировать EDT: все сиблинги `<characteristics>` под `root` (по порядку).
pub fn decode_edt(root: &Element) -> Decoded {
    let mut records = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == "characteristics" && c.prefix.is_empty())
    {
        match decode_edt_one(ch) {
            Ok(rec) => records.push(rec),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(records))
}

fn decode_edt_one(el: &Element) -> Result<Vec<PropertyValue>, String> {
    if !el.attrs.is_empty() {
        return Err("<characteristics> must have no attributes (§1.0)".into());
    }
    el.claim();
    if el.children.len() != N_FIELDS {
        return Err(format!(
            "<characteristics>: expected {N_FIELDS} leaves, found {}",
            el.children.len()
        ));
    }
    let mut rec = Vec::with_capacity(N_FIELDS);
    for (i, child) in el.children.iter().enumerate() {
        if child.local != EDT_TAGS[i] || !child.prefix.is_empty() {
            return Err(format!(
                "<characteristics>: expected <{}> at #{i}, got <{}>",
                EDT_TAGS[i],
                qname(child)
            ));
        }
        rec.push(decode_field_edt(child, i)?);
    }
    Ok(rec)
}

fn decode_field_edt(child: &Element, i: usize) -> Result<PropertyValue, String> {
    if is_value(i) {
        child.claim();
        value_codec::decode(ValueDialect::Edt, child)
    } else {
        if !child.attrs.is_empty() || !child.children.is_empty() {
            return Err(format!("<{}> must be a text leaf", child.local));
        }
        child.claim_with_text();
        Ok(PropertyValue::Str(child.text.clone()))
    }
}

/// Claim EDT (то же, что decode) — для leftover.
pub fn claim_edt(root: &Element) {
    let _ = decode_edt(root);
}

/// Эмитировать EDT: один `<characteristics>` с 12 листами на связку.
pub fn emit_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let records = unpack(value)?;
    let mut out = Vec::with_capacity(records.len());
    for rec in records {
        let mut el = OutElement::branch("", "characteristics");
        for (i, v) in rec.iter().enumerate() {
            el.push(emit_field_edt(i, v)?);
        }
        out.push(el);
    }
    Ok(out)
}

fn emit_field_edt(i: usize, v: &PropertyValue) -> Result<OutElement, String> {
    if is_value(i) {
        match v {
            PropertyValue::Value(spec) => {
                value_codec::encode(ValueDialect::Edt, "", EDT_TAGS[i], spec)
            }
            other => Err(format!(
                "characteristic field #{i} must be Value, got {:?}",
                other.kind()
            )),
        }
    } else {
        match v {
            PropertyValue::Str(s) => Ok(OutElement::leaf("", EDT_TAGS[i], s.clone())),
            other => Err(format!(
                "characteristic field #{i} must be Str, got {:?}",
                other.kind()
            )),
        }
    }
}

// --- Designer ---------------------------------------------------------------

// Designer nested-field names within each group (xr-prefixed), and which IR index they map.
// CharacteristicTypes group: from=#0 (char.types); children: KeyField(#1) TypesFilterField(#2)
//   TypesFilterValue(#3) DataPathField(#4) MultipleValuesUseField(#5).
// CharacteristicValues group: from=#6 (char.values); children: ObjectField(#7) TypeField(#8)
//   ValueField(#9) MultipleValuesKeyField(#10) MultipleValuesOrderField(#11).
const TYPES_CHILDREN: [(&str, usize); 5] = [
    ("KeyField", 1),
    ("TypesFilterField", 2),
    ("TypesFilterValue", 3),
    ("DataPathField", 4),
    ("MultipleValuesUseField", 5),
];
const VALUES_CHILDREN: [(&str, usize); 5] = [
    ("ObjectField", 7),
    ("TypeField", 8),
    ("ValueField", 9),
    ("MultipleValuesKeyField", 10),
    ("MultipleValuesOrderField", 11),
];

/// Декодировать Designer-контейнер `<Characteristics>` (уже claimed `locate`'ом).
pub fn decode_designer(host: &Element) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "<Characteristics> container must be attribute-less, no text".into(),
        );
    }
    let mut records = Vec::new();
    for ce in &host.children {
        if ce.local != "Characteristic" || ce.prefix != "xr" {
            return Decoded::Error(format!("expected <xr:Characteristic>, got <{}>", qname(ce)));
        }
        match decode_designer_one(ce) {
            Ok(rec) => records.push(rec),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(pack(records))
}

fn decode_designer_one(ce: &Element) -> Result<Vec<PropertyValue>, String> {
    if !ce.attrs.is_empty() {
        return Err("<xr:Characteristic> must have no attributes".into());
    }
    ce.claim();
    // Slots in IR order; fill as we parse the two groups.
    let mut rec: Vec<Option<PropertyValue>> = (0..N_FIELDS).map(|_| None).collect();
    if ce.children.len() != 2 {
        return Err(format!(
            "<xr:Characteristic>: expected 2 groups (Types,Values), got {}",
            ce.children.len()
        ));
    }
    decode_group(
        &ce.children[0],
        "CharacteristicTypes",
        0,
        &TYPES_CHILDREN,
        &mut rec,
    )?;
    decode_group(
        &ce.children[1],
        "CharacteristicValues",
        6,
        &VALUES_CHILDREN,
        &mut rec,
    )?;
    rec.into_iter()
        .enumerate()
        .map(|(i, o)| o.ok_or_else(|| format!("characteristic field #{i} missing")))
        .collect()
}

fn decode_group(
    grp: &Element,
    local: &str,
    from_idx: usize,
    children: &[(&str, usize)],
    rec: &mut [Option<PropertyValue>],
) -> Result<(), String> {
    if grp.local != local || grp.prefix != "xr" {
        return Err(format!("expected <xr:{local}>, got <{}>", qname(grp)));
    }
    grp.claim();
    // `from` attribute carries the path (#from_idx).
    let from = grp
        .attr("from")
        .ok_or_else(|| format!("<xr:{local}> missing @from"))?;
    rec[from_idx] = Some(PropertyValue::Str(from.value.clone()));
    from.claimed.set(true);
    if grp.attrs.iter().any(|a| !a.claimed.get()) {
        return Err(format!(
            "<xr:{local}> has unexpected extra attribute (§1.0)"
        ));
    }
    if grp.children.len() != children.len() {
        return Err(format!(
            "<xr:{local}>: expected {} children, got {}",
            children.len(),
            grp.children.len()
        ));
    }
    for (child, (name, idx)) in grp.children.iter().zip(children) {
        if child.local != *name || child.prefix != "xr" {
            return Err(format!(
                "<xr:{local}>: expected <xr:{name}>, got <{}>",
                qname(child)
            ));
        }
        rec[*idx] = Some(decode_field_designer(child, *idx)?);
    }
    Ok(())
}

fn decode_field_designer(child: &Element, i: usize) -> Result<PropertyValue, String> {
    if is_value(i) {
        child.claim();
        value_codec::decode(ValueDialect::Designer, child)
    } else {
        if !child.attrs.is_empty() || !child.children.is_empty() {
            return Err(format!("<xr:{}> must be a text leaf", child.local));
        }
        child.claim_with_text();
        Ok(PropertyValue::Str(child.text.clone()))
    }
}

/// Claim Designer (host claimed выше) — то же, что decode.
pub fn claim_designer(host: &Element) {
    let _ = decode_designer(host);
}

/// Эмитировать Designer-контейнер `<Characteristics>` (имя/ns — из локуса). Пустой → self-closing.
pub fn emit_designer(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let records = unpack(value)?;
    if records.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut container = OutElement::branch(ns, tag);
    for rec in records {
        let mut ce = OutElement::branch("xr", "Characteristic");
        ce.push(emit_group_designer(
            rec,
            "CharacteristicTypes",
            0,
            &TYPES_CHILDREN,
        )?);
        ce.push(emit_group_designer(
            rec,
            "CharacteristicValues",
            6,
            &VALUES_CHILDREN,
        )?);
        container.push(ce);
    }
    Ok(container)
}

fn emit_group_designer(
    rec: &[PropertyValue],
    local: &str,
    from_idx: usize,
    children: &[(&str, usize)],
) -> Result<OutElement, String> {
    let from = match &rec[from_idx] {
        PropertyValue::Str(s) => s.clone(),
        other => {
            return Err(format!(
                "characteristic field #{from_idx} must be Str, got {:?}",
                other.kind()
            ))
        }
    };
    let mut grp = OutElement::branch("xr", local).attr("from", from);
    for (name, idx) in children {
        grp.push(emit_field_designer(name, *idx, &rec[*idx])?);
    }
    Ok(grp)
}

fn emit_field_designer(name: &str, i: usize, v: &PropertyValue) -> Result<OutElement, String> {
    if is_value(i) {
        match v {
            PropertyValue::Value(spec) => {
                value_codec::encode(ValueDialect::Designer, "xr", name, spec)
            }
            other => Err(format!(
                "characteristic field #{i} must be Value, got {:?}",
                other.kind()
            )),
        }
    } else {
        match v {
            PropertyValue::Str(s) => Ok(OutElement::leaf("xr", name, s.clone())),
            other => Err(format!(
                "characteristic field #{i} must be Str, got {:?}",
                other.kind()
            )),
        }
    }
}

// --- helpers ----------------------------------------------------------------

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}
