//! usePurposes — Designer `<UsePurposes>` список назначений применения.

use super::*;

// ============================================================================
// usePurposes — Designer: <UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">
//   X</v8:Value>…</UsePurposes>. IR: List([Str]). (EDT — plain <usePurposes>X
//   </usePurposes> single text → EnumText; X-расхождение значений → поле x_ignore.)
// ============================================================================

const USE_PURPOSE_XSI: &str = "app:ApplicationUsePurpose";

/// Map a Designer `ApplicationUsePurpose` literal to the CANONICAL (EDT) spelling the IR carries.
///
/// The two formats spell the SAME purposes differently — Designer
/// `PlatformApplication`/`MobilePlatformApplication` ↔ EDT `PersonalComputer`/`MobileDevice`.
/// The IR is canonicalised to the EDT spelling so a cross-format convert emits each format's OWN
/// literal: without this, designer→edt carried the Designer spelling verbatim into the edt
/// `<usePurposes>`, which the platform compiles to a DIFFERENT config (the field is otherwise
/// `x_ignore`d, so the X-gate never caught it). §1.0: an unmapped literal is a hard error, never a
/// silent pass-through (that pass-through was exactly the latent bug).
fn use_purpose_designer_to_canonical(lit: &str) -> Result<&'static str, String> {
    match lit {
        "PlatformApplication" => Ok("PersonalComputer"),
        "MobilePlatformApplication" => Ok("MobileDevice"),
        other => Err(format!(
            "unknown Designer ApplicationUsePurpose {other:?} (only PlatformApplication / \
             MobilePlatformApplication witnessed, §1.0)"
        )),
    }
}

/// Inverse of [`use_purpose_designer_to_canonical`] — canonical (EDT) literal → Designer literal.
fn use_purpose_canonical_to_designer(lit: &str) -> Result<&'static str, String> {
    match lit {
        "PersonalComputer" => Ok("PlatformApplication"),
        "MobileDevice" => Ok("MobilePlatformApplication"),
        other => Err(format!(
            "unknown ApplicationUsePurpose {other:?} for Designer emit (only PersonalComputer / \
             MobileDevice witnessed, §1.0)"
        )),
    }
}

/// Декодировать Designer `<UsePurposes>` (host уже claimed locate'ом).
pub fn decode_use_purposes_designer(host: &Element) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error("<UsePurposes> must be attribute-less, no text (§1.0)".into());
    }
    let mut items = Vec::new();
    for ch in &host.children {
        if ch.local != "Value" || ch.prefix != "v8" {
            return Decoded::Error(format!(
                "<UsePurposes> child must be <v8:Value>, got <{}>",
                qname(ch)
            ));
        }
        let xsi = match ch.attr("xsi:type") {
            Some(a) => a,
            None => return Decoded::Error("<v8:Value> missing xsi:type (§1.0)".into()),
        };
        if xsi.value != USE_PURPOSE_XSI {
            return Decoded::Error(format!(
                "<v8:Value> xsi:type must be {USE_PURPOSE_XSI:?}, got {:?}",
                xsi.value
            ));
        }
        xsi.claimed.set(true);
        if let Some(extra) = ch.attrs.iter().find(|a| !a.claimed.get()) {
            return Decoded::Error(format!(
                "<v8:Value> unexpected attribute {:?} (§1.0)",
                extra.name
            ));
        }
        if !ch.children.is_empty() {
            return Decoded::Error("<v8:Value> must be a text leaf".into());
        }
        ch.claim_with_text();
        // Canonicalise the Designer literal to the EDT spelling the IR carries (see
        // `use_purpose_designer_to_canonical`), so a cross-format convert emits each format's own
        // literal instead of leaking the Designer spelling into edt.
        match use_purpose_designer_to_canonical(&ch.text) {
            Ok(canon) => items.push(PropertyValue::Str(canon.to_string())),
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(PropertyValue::List(items))
}

/// Claim Designer `<UsePurposes>` (host claimed выше; claim внутренности).
pub fn claim_use_purposes_designer(host: &Element) {
    let _ = decode_use_purposes_designer(host);
}

/// Эмитировать Designer `<UsePurposes>` (host-тег/ns — из локуса). Пустой → self-closing.
pub fn emit_use_purposes_designer(
    ns: &str,
    tag: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let items = match value {
        PropertyValue::List(l) => l,
        other => return Err(format!("UsePurposes must be List, got {:?}", other.kind())),
    };
    if items.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut host = OutElement::branch(ns, tag);
    for it in items {
        let s = match it {
            PropertyValue::Str(s) => s,
            other => {
                return Err(format!(
                    "UsePurposes item must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        // Map the canonical (EDT) literal back to the Designer spelling for emission.
        let designer_lit = use_purpose_canonical_to_designer(s)?;
        host.push(
            OutElement::leaf("v8", "Value", designer_lit.to_string())
                .attr("xsi:type", USE_PURPOSE_XSI),
        );
    }
    Ok(host)
}
