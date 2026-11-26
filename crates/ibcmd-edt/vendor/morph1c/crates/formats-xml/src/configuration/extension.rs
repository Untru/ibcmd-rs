//! extension — EDT-only структурный узел корня РАСШИРЕНИЯ (.cfe).

use super::*;

// ============================================================================
// extension — EDT-only структурный узел корня РАСШИРЕНИЯ (.cfe):
//   <extension xsi:type="mdclassExtension:ConfigurationExtension">
//     <defaultRunMode>Checked</defaultRunMode>…</extension>
// Список Checked-флагов заимствованных свойств. Designer аналога в дескрипторе не
// несёт → поле x_ignore. IR: List([Str(имя-флага)…]) в порядке источника.
// ============================================================================

/// xsi:type узла `<extension>` (единственный witnessed, §1.0).
const EXTENSION_XSI_TYPE: &str = "mdclassExtension:ConfigurationExtension";
/// Текст каждого листа-флага (единственный witnessed, §1.0).
const EXTENSION_FLAG_TEXT: &str = "Checked";

/// Декодировать EDT `<extension>` (host уже claimed locate'ом). §1.0: witnessed-only
/// форма — иной xsi:type, лишние атрибуты, текст флага ≠ `Checked` → ошибка.
pub fn decode_extension_flags_edt(host: &Element) -> Decoded {
    let xsi = match host.attr("xsi:type") {
        Some(a) => a,
        None => return Decoded::Error("<extension> missing xsi:type (§1.0)".into()),
    };
    if xsi.value != EXTENSION_XSI_TYPE {
        return Decoded::Error(format!(
            "<extension> xsi:type must be {EXTENSION_XSI_TYPE:?}, got {:?} (§1.0)",
            xsi.value
        ));
    }
    xsi.claimed.set(true);
    if let Some(extra) = host.attrs.iter().find(|a| !a.claimed.get()) {
        return Decoded::Error(format!(
            "<extension> unexpected attribute {:?} (§1.0)",
            extra.name
        ));
    }
    if !host.text.is_empty() {
        return Decoded::Error("<extension> must carry no text (§1.0)".into());
    }
    let mut flags: Vec<PropertyValue> = Vec::new();
    for ch in &host.children {
        if !ch.prefix.is_empty() {
            return Decoded::Error(format!(
                "<extension> child <{}> must be unprefixed (§1.0)",
                qname(ch)
            ));
        }
        if !ch.attrs.is_empty() || !ch.children.is_empty() {
            return Decoded::Error(format!(
                "<extension>/<{}> must be a plain text leaf (§1.0)",
                ch.local
            ));
        }
        if ch.text != EXTENSION_FLAG_TEXT {
            return Decoded::Error(format!(
                "<extension>/<{}> text must be {EXTENSION_FLAG_TEXT:?}, got {:?} (§1.0)",
                ch.local, ch.text
            ));
        }
        ch.claim_with_text();
        flags.push(PropertyValue::Str(ch.local.clone()));
    }
    Decoded::Present(PropertyValue::List(flags))
}

/// Claim EDT `<extension>` (то же, что читает decode; host claimed выше).
pub fn claim_extension_flags_edt(host: &Element) {
    let _ = decode_extension_flags_edt(host);
}

/// Эмитировать EDT `<extension>` из IR (`List([Str(имя-флага)…])`). Пустой список —
/// дефолт поля, разрежённый EDT его не эмитит; сюда дошёл — программная ошибка (§1.0).
pub fn emit_extension_flags_edt(
    ns: &str,
    tag: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let flags = match value {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "extension flags must be List, got {:?}",
                other.kind()
            ))
        }
    };
    if flags.is_empty() {
        return Err("extension flags list is empty (the default is never emitted, §1.0)".into());
    }
    let mut host = OutElement::branch(ns, tag).attr("xsi:type", EXTENSION_XSI_TYPE);
    for f in flags {
        let name = match f {
            PropertyValue::Str(s) => s,
            other => {
                return Err(format!(
                    "extension flag must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        host.push(OutElement::leaf("", name, EXTENSION_FLAG_TEXT));
    }
    Ok(host)
}
