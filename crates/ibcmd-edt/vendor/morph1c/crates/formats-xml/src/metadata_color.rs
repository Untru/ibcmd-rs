//! Metadata color projection over the existing typed form color codec.
use crate::{Element, OutElement};
use morph1c_core::ir::value::{PropertyValue, Token};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Edt,
    Designer,
}
pub fn decode(dialect: Dialect, el: &Element) -> Result<PropertyValue, String> {
    let canon = match dialect {
        Dialect::Edt => {
            if !el.text.is_empty() {
                return Err("metadata color contains mixed text".into());
            }
            match crate::form::decode_edt_color(el, &el.local).map_err(|e| e.to_string())? {
                PropertyValue::Ref(value) => {
                    // Check reference domain symmetrically with the Designer codec.
                    crate::form::color_to_designer(&value, &el.local).map_err(|e| e.to_string())?;
                    value
                }
                _ => return Err("unexpected color codec result".into()),
            }
        }
        Dialect::Designer => {
            if !el.attrs.is_empty() || !el.children.is_empty() {
                return Err("metadata color must be a scalar".into());
            }
            el.claim_with_text();
            if el.text == "auto" {
                "auto".into()
            } else {
                crate::form::color_from_designer(&el.text, &el.local).map_err(|e| e.to_string())?
            }
        }
    };
    if el.unclaimed_count() != 0 {
        return Err("metadata color contains unmapped attributes or children".into());
    }
    Ok(PropertyValue::Enum(Token::new(canon)))
}
pub fn encode(
    dialect: Dialect,
    ns: &str,
    tag: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let PropertyValue::Enum(value) = value else {
        return Err("metadata color requires a typed token".into());
    };
    match dialect {
        Dialect::Edt => {
            if !ns.is_empty() {
                return Err("EDT color element prefix must be empty".into());
            }
            crate::form::color_to_designer(value.as_str(), tag).map_err(|e| e.to_string())?;
            crate::form::render_edt_color(tag, value.as_str()).map_err(|e| e.to_string())
        }
        Dialect::Designer => {
            let text = if value.as_str() == "auto" {
                "auto".into()
            } else {
                crate::form::color_to_designer(value.as_str(), tag).map_err(|e| e.to_string())?
            };
            Ok(OutElement::leaf(ns, tag, text))
        }
    }
}
/// A QName value uses the root's declared bindings; absent/wrong bindings are
/// never repaired on write. Nested declarations are refused by the color codec.
pub fn validate_edt_bindings(root: &Element) -> Result<(), String> {
    fn uses(e: &Element) -> bool {
        e.local == "color"
            && e.attr("xsi:type")
                .is_some_and(|a| matches!(a.value.as_str(), "core:ColorRef" | "core:ColorDef"))
            || e.children.iter().any(uses)
    }
    if uses(root) {
        for (key, value) in [
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
        ] {
            if root.attr(key).is_none_or(|a| a.value != value) {
                return Err(format!("metadata color requires exact {key} binding"));
            }
        }
    }
    Ok(())
}
