//! languages — inline-сущность языка (EDT-only в дескрипторе).

use super::*;

// ============================================================================
// languages — inline-сущность языка (EDT-only в дескрипторе; Designer держит тело в
// отдельном файле, в корне — лишь bare <Language>Имя</Language> в ChildObjects).
// EDT: <languages uuid="…"><name>Имя</name>[<synonym>…]<languageCode>код</languageCode></languages>
// IR (EDT-only, x_ignore): List([ List([Str(uuid),Str(name),Str(langCode), <synonym-pairs flattened>]) ]).
// Несколько языков → несколько записей. Хранится для byte-exact R EDT; X-исключён.
// ============================================================================

/// Декодировать EDT inline `<languages>` сущности. Designer НЕ проецирует это поле
/// (тело языка — отдельный файл) → эта функция только для EDT.
pub fn decode_languages_edt(root: &Element) -> Decoded {
    let mut rows: Vec<PropertyValue> = Vec::new();
    for el in root
        .children
        .iter()
        .filter(|c| c.local == "languages" && c.prefix.is_empty())
    {
        el.claim();
        let uuid = match el.attr("uuid") {
            Some(a) => {
                a.claimed.set(true);
                a.value.clone()
            }
            None => return Decoded::Error("<languages> missing @uuid (§1.0)".into()),
        };
        if let Some(extra) = el.attrs.iter().find(|a| !a.claimed.get()) {
            return Decoded::Error(format!(
                "<languages> unexpected attribute {:?} (§1.0)",
                extra.name
            ));
        }
        let mut name = String::new();
        let mut lang_code = String::new();
        let mut synonym: Vec<PropertyValue> = Vec::new();
        let mut seen_name = false;
        let mut seen_code = false;
        for child in &el.children {
            if !child.prefix.is_empty() {
                return Decoded::Error(format!(
                    "<languages> child <{}> must be unprefixed",
                    qname(child)
                ));
            }
            match child.local.as_str() {
                "name" => {
                    if seen_name { return Decoded::Error("duplicate languages name".into()); }
                    seen_name = true;
                    if !child.attrs.is_empty() || !child.children.is_empty() {
                        return Decoded::Error("<languages>/<name> must be a text leaf".into());
                    }
                    child.claim_with_text();
                    name = child.text.clone();
                }
                "languageCode" => {
                    if seen_code { return Decoded::Error("duplicate languages languageCode".into()); }
                    seen_code = true;
                    if !child.attrs.is_empty() || !child.children.is_empty() {
                        return Decoded::Error(
                            "<languages>/<languageCode> must be a text leaf".into(),
                        );
                    }
                    child.claim_with_text();
                    lang_code = child.text.clone();
                }
                "synonym" => {
                    // <synonym><key>lang</key><value>text</value>…</synonym> — claim + capture.
                    child.claim();
                    let mut it = child.children.iter();
                    while let Some(k) = it.next() {
                        if k.local != "key" || !k.prefix.is_empty() {
                            return Decoded::Error("<languages>/<synonym>: expected <key>".into());
                        }
                        k.claim_with_text();
                        let v = match it.next() {
                            Some(v) if v.local == "value" && v.prefix.is_empty() => {
                                v.claim_with_text();
                                v.text.clone()
                            }
                            _ => {
                                return Decoded::Error(
                                    "<languages>/<synonym>: expected <value> after <key>".into(),
                                )
                            }
                        };
                        synonym.push(PropertyValue::Str(k.text.clone()));
                        synonym.push(PropertyValue::Str(v));
                    }
                }
                other => {
                    return Decoded::Error(format!(
                        "<languages>: unexpected child <{other}> (§1.0)"
                    ));
                }
            }
        }
        rows.push(PropertyValue::List(vec![
            PropertyValue::Str(uuid),
            PropertyValue::Str(name),
            PropertyValue::Str(lang_code),
            PropertyValue::List(synonym),
        ]));
    }
    Decoded::Present(PropertyValue::List(rows))
}

/// Claim EDT inline `<languages>` (то же, что читает decode).
pub fn claim_languages_edt(root: &Element) {
    let _ = decode_languages_edt(root);
}

/// Эмитировать EDT inline `<languages>` сущности из IR (byte-exact).
pub fn emit_languages_edt(value: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let rows = match value {
        PropertyValue::List(l) => l,
        other => return Err(format!("languages must be List, got {:?}", other.kind())),
    };
    let mut out = Vec::new();
    for row in rows {
        let cells = match row {
            PropertyValue::List(c) => c,
            other => {
                return Err(format!(
                    "languages row must be List, got {:?}",
                    other.kind()
                ))
            }
        };
        if cells.len() != 4 {
            return Err(format!(
                "languages row must be [uuid,name,langCode,synonym], got {}",
                cells.len()
            ));
        }
        let s = |i: usize| -> Result<&str, String> {
            match &cells[i] {
                PropertyValue::Str(s) => Ok(s.as_str()),
                other => Err(format!(
                    "languages cell {i} must be Str, got {:?}",
                    other.kind()
                )),
            }
        };
        let uuid = s(0)?;
        let name = s(1)?;
        let lang_code = s(2)?;
        let synonym = match &cells[3] {
            PropertyValue::List(p) => p,
            other => {
                return Err(format!(
                    "languages synonym must be List, got {:?}",
                    other.kind()
                ))
            }
        };
        let mut el = OutElement::branch("", "languages").attr("uuid", uuid.to_string());
        el.push(OutElement::leaf("", "name", name.to_string()));
        if !synonym.is_empty() {
            if synonym.len() % 2 != 0 {
                return Err("languages synonym must be key/value pairs".into());
            }
            let mut it = synonym.iter();
            while let (Some(k), Some(v)) = (it.next(), it.next()) {
                let ks = match k {
                    PropertyValue::Str(s) => s,
                    o => return Err(format!("synonym key must be Str, got {:?}", o.kind())),
                };
                let vs = match v {
                    PropertyValue::Str(s) => s,
                    o => return Err(format!("synonym value must be Str, got {:?}", o.kind())),
                };
                let mut syn = OutElement::branch("", "synonym");
                syn.push(OutElement::leaf("", "key", ks.clone()));
                syn.push(OutElement::leaf("", "value", vs.clone()));
                el.push(syn);
            }
        }
        el.push(OutElement::leaf("", "languageCode", lang_code.to_string()));
        out.push(el);
    }
    Ok(out)
}
